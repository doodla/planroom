//! What the fold keeps while it applies events in `seq` order, and the model and lists it derives
//! from them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use planroom_contract::{
    Actor, Cause, Client, Edge, EdgeId, Edit, Event, EventBody, EventId, Layer, Node, NodeId, Op, Provenance,
    Seq, Status, Target, Verdict,
};
use serde_json::{Map, Value, json};

use super::claims::ClaimRecord;
use super::cycle::find_cycle;
use super::prepass::Prepass;
use crate::model::Model;
use crate::resolve::Resolved;

/// The field key a `put` or a `retract` sets: the whole target.
pub const WHOLE: &str = "*";
/// The field an override sets to move a node.
pub const PARENT: &str = "parent";
/// The field an override sets to rename a node.
pub const NAME: &str = "name";
/// The field an override sets to hide a node or an edge.
pub const HIDDEN: &str = "hidden";
/// The prefix of a pinned position's field, `pos@VIEW`.
pub const POS_PREFIX: &str = "pos@";
/// The prefix of a prop override's field, `props.KEY`.
pub const PROPS_PREFIX: &str = "props.";

/// A writer instance: the actor plus the client it writes from. Compare-and-set compares writer
/// instances, not actors, so two tabs of one person conflict like two people (DR-5).
#[derive(Debug, Clone, PartialEq)]
pub struct Writer {
    pub actor: Actor,
    pub client: Client,
}

impl Writer {
    /// The writer instance that wrote `event`.
    pub fn of(event: &Event) -> Writer {
        Writer { actor: event.actor.clone(), client: event.client.clone() }
    }

    /// Whether two writers are one instance: the same actor writing from the same client. An
    /// actor's display name doesn't count.
    pub fn same_instance(&self, other: &Writer) -> bool {
        actor_key(&self.actor) == actor_key(&other.actor) && self.client == other.client
    }

    /// Whether the writer is a person.
    pub fn is_person(&self) -> bool {
        matches!(self.actor, Actor::Person { .. })
    }
}

/// An actor's identity, without its display name.
pub fn actor_key(actor: &Actor) -> String {
    match actor {
        Actor::Person { id, .. } => format!("person\0{id}"),
        Actor::Agent { session, role, .. } => format!("agent\0{session}\0{role}"),
        Actor::Model { id } => format!("model\0{id}"),
        Actor::System { part } => format!("system\0{part}"),
    }
}

/// An edit target, ordered, so the fold's indexes are deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetKey {
    Node(NodeId),
    Edge(EdgeId),
}

impl From<&Target> for TargetKey {
    fn from(target: &Target) -> TargetKey {
        match target {
            Target::Node(id) => TargetKey::Node(id.clone()),
            Target::Edge(id) => TargetKey::Edge(id.clone()),
        }
    }
}

impl TargetKey {
    pub fn target(&self) -> Target {
        match self {
            TargetKey::Node(id) => Target::Node(id.clone()),
            TargetKey::Edge(id) => Target::Edge(id.clone()),
        }
    }
}

/// One field of one target: the unit of authored revisions.
pub type FieldKey = (TargetKey, String);

/// The fields an edit sets: each key of a `set`, or the whole target for a `put` or a `retract`.
pub fn edit_keys(edit: &Edit) -> Vec<FieldKey> {
    let target = TargetKey::from(&edit.target);
    match edit.op {
        Op::Set => edit.fields.keys().map(|f| (target.clone(), f.clone())).collect(),
        Op::Put | Op::Retract => vec![(target, WHOLE.to_owned())],
    }
}

/// Whether two field keys touch one value: the same field, or the whole of the same target.
pub fn overlaps(a: &FieldKey, b: &FieldKey) -> bool {
    a.0 == b.0 && (a.1 == b.1 || a.1 == WHOLE || b.1 == WHOLE)
}

/// An authored revision: the `seq` of the last applied event that set a field, and its writer
/// instance (architecture.md, "Versions and revisions").
#[derive(Debug, Clone, PartialEq)]
pub struct Rev {
    pub seq: Seq,
    pub event: EventId,
    pub writer: Writer,
}

/// What one applied edit put in one field.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Setting {
    Value(Value),
    Retract,
}

impl Setting {
    pub(crate) fn of(edit: &Edit, field: &str) -> Setting {
        match edit.op {
            Op::Set => Setting::Value(edit.fields.get(field).cloned().unwrap_or(Value::Null)),
            Op::Put => Setting::Value(Value::Object(edit.fields.clone().into_iter().collect())),
            Op::Retract => Setting::Retract,
        }
    }
}

/// One effective applied edit's setting of one field.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Setter {
    pub seq: Seq,
    pub event: EventId,
    pub writer: Writer,
    pub op: Op,
    pub layer: Layer,
    /// The fact value the field was set over, from the edit's `over`.
    pub over: Option<Value>,
    /// Whether the fold set the field. One it skipped, such as a move that lost to a loop, a set
    /// on a node that doesn't exist or a value with no meaning, writes no revision and holds
    /// nothing, though scan drift still reads it.
    pub applied: bool,
}

/// What one applied edit does to a node's parent. The fold keeps it whether or not an undo
/// reverses the edit, so compare-and-set can tell where an undo would put each node (DR-61).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParentEntry {
    /// A `set parent`, or a `put` with its parent or with none.
    Under(Option<NodeId>),
    /// A `retract` of the node.
    Removed,
}

impl ParentEntry {
    /// The node and the parent entry `edit` writes, if it writes one.
    fn of(edit: &Edit) -> Option<(NodeId, ParentEntry)> {
        let Target::Node(id) = &edit.target else { return None };
        let entry = match (edit.op, edit.fields.get(PARENT)) {
            (Op::Retract, _) => ParentEntry::Removed,
            (Op::Set | Op::Put, Some(Value::String(p))) => ParentEntry::Under(Some(NodeId(p.clone()))),
            (Op::Set | Op::Put, Some(Value::Null)) | (Op::Put, None) => ParentEntry::Under(None),
            (Op::Set, None) | (Op::Set | Op::Put, Some(_)) => return None,
        };
        Some((id.clone(), entry))
    }
}

/// A ghost: an agent's or a model's edit, pending a person's review. It stays out of the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Ghost {
    pub event: EventId,
    pub seq: Seq,
    pub writer: Writer,
    /// A chat turn or a stage run batches ghosts into one card.
    pub cause: Option<Cause>,
    pub edit: Edit,
}

/// An edit stored with `status: conflict` (DR-7): never merged, never lost.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredConflict {
    pub event: EventId,
    pub seq: Seq,
    pub writer: Writer,
    pub edit: Edit,
    /// The latest review that picked it, if any.
    pub resolved: Option<Verdict>,
}

/// A `version` event: a tag on `seq`.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionTag {
    pub event: EventId,
    pub seq: Seq,
    pub project: Option<NodeId>,
    pub label: String,
}

/// What a `set` does with one of its fields in the state the fold has reached.
#[derive(Debug, Clone, PartialEq)]
enum FieldOutcome {
    /// The fold sets the field.
    Set,
    /// The target doesn't exist: the fold skips the field and lists nothing.
    Skipped,
    /// The field can't take the value, or has no meaning: skipped and listed.
    Invalid,
    /// The new parent would close this loop: the move loses and is listed (DR-61).
    Cycle(Vec<NodeId>),
}

/// What the fold met that Diffs lists.
#[derive(Debug, Clone, PartialEq)]
pub enum FoldDiff {
    /// The fact under an override changed since the override was made over it; the override
    /// stays applied, with "keep yours" pre-selected (R6).
    ScanChangedUnderEdit { event: EventId, target: Target, field: String, over: Value, fact: Value },
    /// An override's target is gone from the facts, and no edit put it.
    TargetVanished { event: EventId, target: Target },
    /// A node an edit named moved where git reported a rename, and the edit followed it: listed
    /// once per move, with the first edit that followed it (DR-74).
    Followed { event: EventId, from: NodeId, to: NodeId },
    /// A parent change that would have closed a loop lost to the edits before it (DR-61).
    CycleBroken { event: EventId, node: NodeId, parent: NodeId, cycle: Vec<NodeId> },
    /// A parent an edit set doesn't resolve, so the node falls back to its fact parent (DR-61).
    ParentFallback { event: EventId, node: NodeId, parent: NodeId, fallback: Option<NodeId> },
    /// A person's later edit set a field this ghost sets, so it left the overlay (DR-7).
    GhostSuperseded { ghost: EventId, by: EventId },
    /// A `retract`: a rejected node or relation, kept out of questions and reconciliation.
    Retracted { event: EventId, target: Target },
    /// An edit the fold couldn't apply, such as a `put` without a kind or a field with no meaning.
    InvalidEdit { event: EventId, field: Option<String> },
}

/// The overlay that sits beside the model: what an override hides or pins, and the ghosts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overlay {
    pub hidden: BTreeSet<TargetKey>,
    /// Pinned positions by view and node, as the edits wrote them.
    pub positions: BTreeMap<(String, NodeId), Value>,
    /// Pending ghosts, in `seq` order.
    pub ghosts: Vec<Ghost>,
}

/// Everything one event index entry holds that compare-and-set and undo need.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Indexed {
    pub seq: Seq,
    pub idem: String,
    pub status: Status,
    pub keys: Vec<FieldKey>,
    pub undo_of: Option<EventId>,
}

/// The idempotency key a command's events share: `idem` without its `#N` suffix (DR-81).
pub(crate) fn idem_stem(idem: &str) -> &str {
    match idem.rsplit_once('#') {
        Some((stem, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => stem,
        _ => idem,
    }
}

/// The nodes the fold holds: the resolved facts, shared and never written, under the nodes the
/// edits changed, each copied from its fact on its first write. A fold copies only what its
/// edits touch, and a copy of the fold shares the facts.
#[derive(Debug, Clone)]
pub(crate) struct Nodes {
    /// The resolved facts, before any override, one per ID and sorted by ID: what drift and
    /// fallbacks compare with.
    facts: Arc<Vec<Node>>,
    /// Each node an edit changed, put or removed, by ID: `None` for one removed.
    changed: BTreeMap<NodeId, Option<Node>>,
}

impl Nodes {
    /// The nodes `facts` give, before any edit. Resolution leaves them one per ID and sorted;
    /// any others are sorted here, and of two with one ID the later stands.
    fn new(mut facts: Vec<Node>) -> Nodes {
        if !facts.is_sorted_by(|a, b| a.id < b.id) {
            facts.reverse();
            facts.sort_by(|a, b| a.id.cmp(&b.id));
            facts.dedup_by(|later, earlier| later.id == earlier.id);
        }
        Nodes { facts: Arc::new(facts), changed: BTreeMap::new() }
    }

    /// The fact node with this ID, before any edit.
    pub(crate) fn fact(&self, id: &NodeId) -> Option<&Node> {
        self.facts.binary_search_by(|n| n.id.cmp(id)).ok().map(|i| &self.facts[i])
    }

    /// The node with this ID as the edits so far leave it.
    pub(crate) fn get(&self, id: &NodeId) -> Option<&Node> {
        match self.changed.get(id) {
            Some(changed) => changed.as_ref(),
            None => self.fact(id),
        }
    }

    pub(crate) fn contains(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    /// The node to write, copied from its fact the first time an edit writes it.
    fn get_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        if !self.changed.contains_key(id) {
            let fact = self.fact(id)?.clone();
            self.changed.insert(id.clone(), Some(fact));
        }
        self.changed.get_mut(id).and_then(Option::as_mut)
    }

    fn insert(&mut self, node: Node) {
        self.changed.insert(node.id.clone(), Some(node));
    }

    fn remove(&mut self, id: &NodeId) {
        if self.fact(id).is_some() {
            self.changed.insert(id.clone(), None);
        } else {
            self.changed.remove(id);
        }
    }

    /// Every node, in ID order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Node> {
        let mut facts = self.facts.iter().peekable();
        let mut changed = self.changed.iter().peekable();
        std::iter::from_fn(move || {
            loop {
                let order = match (facts.peek(), changed.peek()) {
                    (None, None) => return None,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(fact), Some((id, _))) => fact.id.cmp(id),
                };
                match order {
                    std::cmp::Ordering::Less => return facts.next(),
                    std::cmp::Ordering::Equal => {
                        facts.next();
                    }
                    std::cmp::Ordering::Greater => {}
                }
                if let Some((_, Some(node))) = changed.next() {
                    return Some(node);
                }
            }
        })
    }
}

impl PartialEq for Nodes {
    /// Two holdings are equal when they hold the same facts and the same nodes.
    fn eq(&self, other: &Nodes) -> bool {
        (Arc::ptr_eq(&self.facts, &other.facts) || self.facts == other.facts) && self.iter().eq(other.iter())
    }
}

/// The state the fold builds in `seq` order. Applying one more event to it is the incremental
/// fold; everything else is derived from it in [`Core::finish`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Core {
    pub at: Seq,
    fact_edges: BTreeSet<EdgeId>,
    /// The nodes, the resolved facts among them.
    nodes: Nodes,
    /// The nodes whose current parent an edit set, with that edit.
    parent_from: BTreeMap<NodeId, (Seq, EventId)>,
    /// In fact order, then in the order edits put them; a retracted edge leaves a hole.
    edges: Vec<Option<Edge>>,
    edge_slots: BTreeMap<EdgeId, Vec<usize>>,
    /// The edges resolution set aside, kept for the caller to report.
    unresolved: Vec<Edge>,
    hidden: BTreeSet<TargetKey>,
    positions: BTreeMap<(String, NodeId), Value>,
    ghosts: BTreeMap<Seq, (Ghost, Vec<FieldKey>)>,
    conflicts: Vec<StoredConflict>,
    versions: Vec<VersionTag>,
    claims: Vec<ClaimRecord>,
    open_cards: Vec<EventId>,
    /// Every effective applied edit's settings, per field, in `seq` order.
    pub setters: BTreeMap<FieldKey, Vec<Setter>>,
    /// Every applied edit's parent entry, per node, in `seq` order, undone or not.
    pub parent_log: BTreeMap<NodeId, Vec<(Seq, EventId, ParentEntry)>>,
    pub index: BTreeMap<EventId, Indexed>,
    pub stems: BTreeMap<String, Vec<EventId>>,
    /// The ghosts each event caused, by that event, in `seq` order, undone or not: the ghosts a
    /// card holds, such as the Apply card's inbox edits.
    pub caused: BTreeMap<EventId, Vec<EventId>>,
    /// Every `card` event, in `seq` order, open or not.
    pub cards: Vec<EventId>,
    /// The fields each applied edit set, by event: what the fold set for an edit in effect, and
    /// for an undone one what it would set where it stands. An undo's revisions read it (DR-61).
    pub sets: BTreeMap<EventId, BTreeSet<String>>,
    diffs: Vec<FoldDiff>,
    /// The moves Diffs already lists.
    listed_moves: BTreeSet<(NodeId, NodeId)>,
}

impl Core {
    pub fn new(resolved: Resolved) -> Core {
        let fact_edges = resolved.edges.iter().map(|e| e.id.clone()).collect();
        let mut edge_slots: BTreeMap<EdgeId, Vec<usize>> = BTreeMap::new();
        for (i, edge) in resolved.edges.iter().enumerate() {
            edge_slots.entry(edge.id.clone()).or_default().push(i);
        }
        Core {
            at: 0,
            fact_edges,
            nodes: Nodes::new(resolved.nodes),
            parent_from: BTreeMap::new(),
            edges: resolved.edges.into_iter().map(Some).collect(),
            edge_slots,
            unresolved: resolved.unresolved,
            hidden: BTreeSet::new(),
            positions: BTreeMap::new(),
            ghosts: BTreeMap::new(),
            conflicts: Vec::new(),
            versions: Vec::new(),
            claims: Vec::new(),
            open_cards: Vec::new(),
            setters: BTreeMap::new(),
            parent_log: BTreeMap::new(),
            index: BTreeMap::new(),
            stems: BTreeMap::new(),
            caused: BTreeMap::new(),
            cards: Vec::new(),
            sets: BTreeMap::new(),
            diffs: Vec::new(),
            listed_moves: BTreeSet::new(),
        }
    }

    /// Apply one event, the next in `seq` order, as the pre-pass found it: undone events and
    /// reviewed ghosts are skipped, and a card's body and an unknown kind are never read.
    pub fn apply(&mut self, event: &Event, prepass: &Prepass) {
        self.at = self.at.max(event.seq);
        self.index_event(event);
        if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status)
            && let Some((node, entry)) = ParentEntry::of(edit)
        {
            self.parent_log.entry(node).or_default().push((event.seq, event.id.clone(), entry));
        }
        if !prepass.undone.contains(&event.id)
            && event.status == Status::Applied
            && let Some(moves) = prepass.followed.get(&event.id)
        {
            for (from, to) in moves {
                if self.listed_moves.insert((from.clone(), to.clone())) {
                    self.diffs.push(FoldDiff::Followed {
                        event: event.id.clone(),
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }
        if prepass.undone.contains(&event.id) {
            if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status) {
                let would = self.would_set(event, edit);
                self.sets.insert(event.id.clone(), would);
            }
            return;
        }
        match &event.body {
            EventBody::Edit(edit) => match event.status {
                Status::Applied => self.apply_edit(event, edit),
                Status::Ghost if !prepass.reviews.contains_key(&event.id) => {
                    let ghost = Ghost {
                        event: event.id.clone(),
                        seq: event.seq,
                        writer: Writer::of(event),
                        cause: event.cause.clone(),
                        edit: edit.clone(),
                    };
                    self.ghosts.insert(event.seq, (ghost, edit_keys(edit)));
                }
                Status::Conflict => self.conflicts.push(StoredConflict {
                    event: event.id.clone(),
                    seq: event.seq,
                    writer: Writer::of(event),
                    edit: edit.clone(),
                    resolved: prepass.reviews.get(&event.id).copied(),
                }),
                _ => {}
            },
            EventBody::Version { project, label } => self.versions.push(VersionTag {
                event: event.id.clone(),
                seq: event.seq,
                project: project.clone(),
                label: label.clone(),
            }),
            EventBody::Claim { target, until, release } => self.claims.push(ClaimRecord {
                event: event.id.clone(),
                seq: event.seq,
                writer: Writer::of(event),
                target: target.clone(),
                until: until.clone(),
                release: *release,
            }),
            EventBody::Card(_) => {
                if prepass.card_open(&event.id) {
                    self.open_cards.push(event.id.clone());
                }
            }
            EventBody::Review { .. }
            | EventBody::Undo { .. }
            | EventBody::Scan { .. }
            | EventBody::Decision(_)
            | EventBody::Handoff(_)
            | EventBody::Unknown(_) => {}
        }
    }

    fn index_event(&mut self, event: &Event) {
        let (keys, undo_of) = match &event.body {
            EventBody::Edit(edit) => (edit_keys(edit), None),
            EventBody::Undo { of } => (Vec::new(), Some(of.clone())),
            _ => (Vec::new(), None),
        };
        self.index.insert(
            event.id.clone(),
            Indexed { seq: event.seq, idem: event.idem.clone(), status: event.status, keys, undo_of },
        );
        self.stems.entry(idem_stem(&event.idem).to_owned()).or_default().push(event.id.clone());
        if let (EventBody::Edit(_), Status::Ghost, Some(Cause::Event(cause))) =
            (&event.body, event.status, &event.cause)
        {
            self.caused.entry(cause.clone()).or_default().push(event.id.clone());
        }
        if let EventBody::Card(_) = event.body {
            self.cards.push(event.id.clone());
        }
    }

    fn apply_edit(&mut self, event: &Event, edit: &Edit) {
        let writer = Writer::of(event);
        let keys = edit_keys(edit);
        // The fields the fold set: a `set`'s fields but those it skipped, and the whole target for
        // a `put` that builds and for a `retract`.
        let set: BTreeSet<String> = match (&edit.op, &edit.target) {
            (Op::Put, Target::Node(id)) => self.put_node(event, edit, id),
            (Op::Put, Target::Edge(id)) => self.put_edge(event, edit, id),
            (Op::Set, Target::Node(id)) => self.set_node(event, edit, id),
            (Op::Set, Target::Edge(id)) => self.set_edge(event, edit, id),
            (Op::Retract, Target::Node(id)) => {
                self.nodes.remove(id);
                self.parent_from.remove(id);
                self.diffs.push(FoldDiff::Retracted { event: event.id.clone(), target: edit.target.clone() });
                whole()
            }
            (Op::Retract, Target::Edge(id)) => {
                for slot in self.edge_slots.remove(id).unwrap_or_default() {
                    self.edges[slot] = None;
                }
                self.diffs.push(FoldDiff::Retracted { event: event.id.clone(), target: edit.target.clone() });
                whole()
            }
        };
        let applied: Vec<&FieldKey> = keys.iter().filter(|k| set.contains(&k.1)).collect();
        if writer.is_person() {
            let superseded: Vec<Seq> = self
                .ghosts
                .iter()
                .filter(|(seq, (_, ghost_keys))| {
                    **seq < event.seq && ghost_keys.iter().any(|g| applied.iter().any(|k| overlaps(g, k)))
                })
                .map(|(seq, _)| *seq)
                .collect();
            for seq in superseded {
                let (ghost, _) = self.ghosts.remove(&seq).expect("a pending ghost");
                self.diffs.push(FoldDiff::GhostSuperseded { ghost: ghost.event, by: event.id.clone() });
            }
        }
        for key in keys {
            let over = edit.over.get(&key.1).cloned();
            let applied = set.contains(&key.1);
            self.setters.entry(key).or_default().push(Setter {
                seq: event.seq,
                event: event.id.clone(),
                writer: writer.clone(),
                op: edit.op,
                layer: edit.layer,
                over,
                applied,
            });
        }
        self.sets.insert(event.id.clone(), set);
    }

    /// The parent a node's facts give it: `None` when it isn't a fact.
    pub fn fact_parent(&self, id: &NodeId) -> Option<Option<NodeId>> {
        self.nodes.fact(id).map(|n| n.parent.clone())
    }

    /// The last effective applied edit that set `key`: the one its revision and hold come from.
    pub fn last_set(&self, key: &FieldKey) -> Option<&Setter> {
        self.setters.get(key)?.iter().rev().find(|s| s.applied)
    }

    fn invalid(&mut self, event: &Event, field: Option<&str>) {
        self.diffs.push(FoldDiff::InvalidEdit { event: event.id.clone(), field: field.map(str::to_owned) });
    }

    /// Whether moving `node` under `parent` keeps the tree whole; a move that would close a loop
    /// loses and is listed.
    fn parent_allowed(&mut self, event: &Event, node: &NodeId, parent: &NodeId) -> bool {
        let nodes = &self.nodes;
        match find_cycle(node, parent, |n| nodes.get(n).and_then(|n| n.parent.clone())) {
            None => true,
            Some(cycle) => {
                self.diffs.push(FoldDiff::CycleBroken {
                    event: event.id.clone(),
                    node: node.clone(),
                    parent: parent.clone(),
                    cycle,
                });
                false
            }
        }
    }

    fn put_node(&mut self, event: &Event, edit: &Edit, id: &NodeId) -> BTreeSet<String> {
        let Some(mut node) = build_node(id, edit, authored(event)) else {
            self.invalid(event, None);
            return BTreeSet::new();
        };
        let earlier = self.nodes.get(id).and_then(|n| n.parent.clone());
        match node.parent.clone() {
            Some(parent) if !self.parent_allowed(event, id, &parent) => {
                node.parent = earlier;
            }
            Some(_) => {
                self.parent_from.insert(id.clone(), (event.seq, event.id.clone()));
            }
            None => {
                self.parent_from.remove(id);
            }
        }
        self.nodes.insert(node);
        whole()
    }

    fn put_edge(&mut self, event: &Event, edit: &Edit, id: &EdgeId) -> BTreeSet<String> {
        let Some(edge) = build_edge(id, edit, authored(event)) else {
            self.invalid(event, None);
            return BTreeSet::new();
        };
        match self.edge_slots.get_mut(id) {
            Some(slots) if !slots.is_empty() => {
                // A put replaces every edge with its ID by one, in the first one's place.
                for &slot in &slots[1..] {
                    self.edges[slot] = None;
                }
                slots.truncate(1);
                self.edges[slots[0]] = Some(edge);
            }
            _ => {
                self.edges.push(Some(edge));
                self.edge_slots.insert(id.clone(), vec![self.edges.len() - 1]);
            }
        }
        whole()
    }

    /// What a `set` of `field` to `value` on node `id` does in the state the fold has reached.
    /// Every field of a node that doesn't exist is skipped, a hide and a pin included, so it
    /// writes no revision and holds nothing (architecture.md, "Versions and revisions").
    fn node_field(&self, id: &NodeId, field: &str, value: &Value) -> FieldOutcome {
        if field == HIDDEN && !value.is_boolean() {
            return FieldOutcome::Invalid;
        }
        if !self.nodes.contains(id) {
            return FieldOutcome::Skipped;
        }
        if field.starts_with(POS_PREFIX) || field == HIDDEN || field.starts_with(PROPS_PREFIX) {
            FieldOutcome::Set
        } else if field == NAME {
            if value.is_string() { FieldOutcome::Set } else { FieldOutcome::Invalid }
        } else if field == PARENT {
            match value {
                Value::Null => FieldOutcome::Set,
                Value::String(p) => {
                    let nodes = &self.nodes;
                    match find_cycle(id, &NodeId(p.clone()), |n| nodes.get(n).and_then(|n| n.parent.clone()))
                    {
                        None => FieldOutcome::Set,
                        Some(cycle) => FieldOutcome::Cycle(cycle),
                    }
                }
                _ => FieldOutcome::Invalid,
            }
        } else {
            FieldOutcome::Invalid
        }
    }

    fn set_node(&mut self, event: &Event, edit: &Edit, id: &NodeId) -> BTreeSet<String> {
        let target = TargetKey::Node(id.clone());
        let mut set = BTreeSet::new();
        for (field, value) in &edit.fields {
            match self.node_field(id, field, value) {
                FieldOutcome::Set => {}
                FieldOutcome::Skipped => continue,
                FieldOutcome::Invalid => {
                    self.invalid(event, Some(field));
                    continue;
                }
                FieldOutcome::Cycle(cycle) => {
                    let parent = NodeId(value.as_str().expect("a parent ID").to_owned());
                    self.diffs.push(FoldDiff::CycleBroken {
                        event: event.id.clone(),
                        node: id.clone(),
                        parent,
                        cycle,
                    });
                    continue;
                }
            }
            if let Some(view) = field.strip_prefix(POS_PREFIX) {
                let key = (view.to_owned(), id.clone());
                if value.is_null() {
                    self.positions.remove(&key);
                } else {
                    self.positions.insert(key, value.clone());
                }
            } else if field == HIDDEN {
                if value.as_bool() == Some(true) {
                    self.hidden.insert(target.clone());
                } else {
                    self.hidden.remove(&target);
                }
            } else if field == NAME {
                self.nodes.get_mut(id).expect("a node").name = value.as_str().expect("a name").to_owned();
            } else if field == PARENT {
                self.nodes.get_mut(id).expect("a node").parent = value.as_str().map(|p| NodeId(p.to_owned()));
                self.parent_from.insert(id.clone(), (event.seq, event.id.clone()));
            } else if let Some(prop) = field.strip_prefix(PROPS_PREFIX) {
                let props = &mut self.nodes.get_mut(id).expect("a node").props;
                if value.is_null() {
                    props.remove(prop);
                } else {
                    props.insert(prop.to_owned(), value.clone());
                }
            }
            set.insert(field.clone());
        }
        set
    }

    /// What a `set` of `field` to `value` on edge `id` does in the state the fold has reached: a
    /// prop of an edge with no live copy is skipped.
    fn edge_field(&self, id: &EdgeId, field: &str, value: &Value) -> FieldOutcome {
        if field == HIDDEN {
            if value.is_boolean() { FieldOutcome::Set } else { FieldOutcome::Invalid }
        } else if field.starts_with(PROPS_PREFIX) {
            let live =
                self.edge_slots.get(id).is_some_and(|slots| slots.iter().any(|s| self.edges[*s].is_some()));
            if live { FieldOutcome::Set } else { FieldOutcome::Skipped }
        } else {
            FieldOutcome::Invalid
        }
    }

    fn set_edge(&mut self, event: &Event, edit: &Edit, id: &EdgeId) -> BTreeSet<String> {
        let target = TargetKey::Edge(id.clone());
        let slots = self.edge_slots.get(id).cloned().unwrap_or_default();
        let mut set = BTreeSet::new();
        for (field, value) in &edit.fields {
            match self.edge_field(id, field, value) {
                FieldOutcome::Set => {}
                FieldOutcome::Skipped | FieldOutcome::Cycle(_) => continue,
                FieldOutcome::Invalid => {
                    self.invalid(event, Some(field));
                    continue;
                }
            }
            if field == HIDDEN {
                if value.as_bool() == Some(true) {
                    self.hidden.insert(target.clone());
                } else {
                    self.hidden.remove(&target);
                }
            } else if let Some(prop) = field.strip_prefix(PROPS_PREFIX) {
                for slot in &slots {
                    if let Some(edge) = self.edges[*slot].as_mut() {
                        if value.is_null() {
                            edge.props.remove(prop);
                        } else {
                            edge.props.insert(prop.to_owned(), value.clone());
                        }
                    }
                }
            }
            set.insert(field.clone());
        }
        set
    }

    /// The fields `edit` would set if the fold applied it now, without applying it: what an
    /// undone edit set, read in the state the edits that stay in effect build up to its `seq`.
    /// An edit that only an edit undone with it made settable, such as a rename of a node that
    /// same command put, reads as setting nothing here; the undo's revision on the put's whole
    /// target still makes a write to that field from an older base conflict.
    fn would_set(&self, event: &Event, edit: &Edit) -> BTreeSet<String> {
        match (&edit.op, &edit.target) {
            (Op::Put, Target::Node(id)) => {
                if build_node(id, edit, authored(event)).is_some() {
                    whole()
                } else {
                    BTreeSet::new()
                }
            }
            (Op::Put, Target::Edge(id)) => {
                if build_edge(id, edit, authored(event)).is_some() {
                    whole()
                } else {
                    BTreeSet::new()
                }
            }
            (Op::Retract, _) => whole(),
            (Op::Set, Target::Node(id)) => edit
                .fields
                .iter()
                .filter(|(f, v)| self.node_field(id, f, v) == FieldOutcome::Set)
                .map(|(f, _)| f.clone())
                .collect(),
            (Op::Set, Target::Edge(id)) => edit
                .fields
                .iter()
                .filter(|(f, v)| self.edge_field(id, f, v) == FieldOutcome::Set)
                .map(|(f, _)| f.clone())
                .collect(),
        }
    }

    /// The fact value under a field, for scan drift: `None` when the field has no fact
    /// counterpart or the target isn't a fact.
    pub(crate) fn fact_value(&self, target: &TargetKey, field: &str) -> Option<Value> {
        match target {
            TargetKey::Node(id) => {
                let node = self.nodes.fact(id)?;
                if field == NAME {
                    Some(Value::String(node.name.clone()))
                } else if field == PARENT {
                    Some(node.parent.as_ref().map_or(Value::Null, |p| Value::String(p.0.clone())))
                } else {
                    let prop = field.strip_prefix(PROPS_PREFIX)?;
                    Some(node.props.get(prop).cloned().unwrap_or(Value::Null))
                }
            }
            TargetKey::Edge(_) => None,
        }
    }

    fn target_exists(&self, target: &TargetKey) -> bool {
        match target {
            TargetKey::Node(id) => self.nodes.contains(id),
            TargetKey::Edge(id) => self.edge_slots.get(id).is_some_and(|s| !s.is_empty()),
        }
    }

    fn is_fact(&self, target: &TargetKey) -> bool {
        match target {
            TargetKey::Node(id) => self.nodes.fact(id).is_some(),
            TargetKey::Edge(id) => self.fact_edges.contains(id),
        }
    }

    /// The model and lists at this state: parents that don't resolve fall back to their fact
    /// parent, any loop left is broken by `seq`, and drift on overrides is listed.
    pub fn finish(&self) -> Finished {
        let mut parents = Parents { nodes: &self.nodes, reset: BTreeMap::new() };
        let mut parent_from = self.parent_from.clone();
        let mut diffs = self.diffs.clone();
        for (id, (_, event)) in &self.parent_from {
            let Some(parent) = self.nodes.get(id).and_then(|n| n.parent.clone()) else { continue };
            if self.nodes.contains(&parent) {
                continue;
            }
            let fallback = self.nodes.fact(id).and_then(|n| n.parent.clone());
            parents.reset.insert(id.clone(), fallback.clone());
            parent_from.remove(id);
            diffs.push(FoldDiff::ParentFallback { event: event.clone(), node: id.clone(), parent, fallback });
        }
        break_cycles(&mut parents, &mut parent_from, &mut diffs);
        let mut vanished: BTreeMap<TargetKey, EventId> = BTreeMap::new();
        for ((target, field), setters) in &self.setters {
            let Some(last) = setters.last() else { continue };
            if last.op != Op::Set || last.layer != Layer::Actual {
                continue;
            }
            if !self.target_exists(target) && !self.is_fact(target) {
                let entry = vanished.entry(target.clone()).or_insert_with(|| last.event.clone());
                if self.index.get(entry).is_none_or(|e| e.seq < last.seq) {
                    *entry = last.event.clone();
                }
                continue;
            }
            let (Some(over), Some(fact)) = (&last.over, self.fact_value(target, field)) else { continue };
            if *over != fact {
                diffs.push(FoldDiff::ScanChangedUnderEdit {
                    event: last.event.clone(),
                    target: target.target(),
                    field: field.clone(),
                    over: over.clone(),
                    fact,
                });
            }
        }
        for (target, event) in vanished {
            diffs.push(FoldDiff::TargetVanished { event, target: target.target() });
        }
        // Each node is copied once, into the model, with the parent the end of the fold left it.
        let nodes = self
            .nodes
            .iter()
            .map(|n| {
                let mut node = n.clone();
                if let Some(parent) = parents.reset.get(&n.id) {
                    node.parent = parent.clone();
                }
                node
            })
            .collect();
        Finished {
            model: Model {
                at: self.at,
                nodes,
                edges: self.edges.iter().flatten().cloned().collect(),
                unresolved: self.unresolved.clone(),
            },
            overlay: Overlay {
                hidden: self.hidden.clone(),
                positions: self.positions.clone(),
                ghosts: self.ghosts.values().map(|(g, _)| g.clone()).collect(),
            },
            diffs,
            conflicts: self.conflicts.clone(),
            open_cards: self.open_cards.clone(),
            versions: self.versions.clone(),
            claims: self.claims.clone(),
        }
    }
}

/// What [`Core::finish`] derives.
pub(crate) struct Finished {
    pub model: Model,
    pub overlay: Overlay,
    pub diffs: Vec<FoldDiff>,
    pub conflicts: Vec<StoredConflict>,
    pub open_cards: Vec<EventId>,
    pub versions: Vec<VersionTag>,
    pub claims: Vec<ClaimRecord>,
}

/// The nodes' parents at the end of the fold: the parents the edits left, but those set back to
/// their fact parent there, kept beside the nodes rather than in a copy of them.
struct Parents<'a> {
    nodes: &'a Nodes,
    /// Each node whose parent the end of the fold set back to its fact parent, with that parent.
    reset: BTreeMap<NodeId, Option<NodeId>>,
}

impl Parents<'_> {
    /// The parent of node `id`.
    fn of(&self, id: &NodeId) -> Option<&NodeId> {
        match self.reset.get(id) {
            Some(parent) => parent.as_ref(),
            None => self.nodes.get(id).and_then(|n| n.parent.as_ref()),
        }
    }

    /// The parent of node `id` when it is a node: the next step up the tree.
    fn up(&self, id: &NodeId) -> Option<&NodeId> {
        self.of(id).and_then(|parent| self.nodes.get(parent)).map(|n| &n.id)
    }
}

/// Break every parent loop an edit holds: of the loop's nodes whose parent an edit set, the one
/// set at the highest `seq` goes back to its fact parent, until no loop holds an edited parent.
/// A loop of facts alone stays, and the tree counts it nowhere.
fn break_cycles(
    parents: &mut Parents<'_>,
    parent_from: &mut BTreeMap<NodeId, (Seq, EventId)>,
    diffs: &mut Vec<FoldDiff>,
) {
    loop {
        let Some(cycle) = first_edited_cycle(parents, parent_from) else { return };
        let loser = cycle
            .iter()
            .filter_map(|n| parent_from.get(n).map(|(seq, _)| (*seq, n.clone())))
            .max()
            .map(|(_, n)| n)
            .expect("an edited node in the loop");
        let (_, event) = parent_from.remove(&loser).expect("an edited parent");
        let parent = parents.of(&loser).cloned().expect("a parent in a loop");
        let fallback = parents.nodes.fact(&loser).and_then(|n| n.parent.clone());
        parents.reset.insert(loser.clone(), fallback);
        let mut cycle = cycle;
        cycle.push(cycle[0].clone());
        diffs.push(FoldDiff::CycleBroken { event, node: loser, parent, cycle });
    }
}

/// Whether a node whose parent an edit set sits on a parent loop: the walk up from it comes back
/// to it. A node found to reach the top is remembered, so no walk goes past it twice. Only then
/// can [`first_edited_cycle`] find a loop, so a fold with none walks only up from its edits.
fn edited_on_cycle(parents: &Parents<'_>, parent_from: &BTreeMap<NodeId, (Seq, EventId)>) -> bool {
    let mut topped: BTreeSet<&NodeId> = BTreeSet::new();
    for start in parent_from.keys() {
        let Some(start) = parents.nodes.get(start).map(|n| &n.id) else { continue };
        let mut path: Vec<&NodeId> = vec![start];
        let mut seen: BTreeSet<&NodeId> = BTreeSet::new();
        let mut at = start;
        loop {
            match parents.up(at) {
                Some(parent) if parent == start => return true,
                // A loop above that doesn't pass through `start`: the walk from one of its own
                // edited nodes finds it.
                Some(parent) if !seen.insert(parent) => break,
                Some(parent) if !topped.contains(parent) => {
                    path.push(parent);
                    at = parent;
                }
                _ => {
                    topped.extend(path);
                    break;
                }
            }
        }
    }
    false
}

/// The first parent loop, in ID order, that holds a node whose parent an edit set.
fn first_edited_cycle(
    parents: &Parents<'_>,
    parent_from: &BTreeMap<NodeId, (Seq, EventId)>,
) -> Option<Vec<NodeId>> {
    if !edited_on_cycle(parents, parent_from) {
        return None;
    }
    scan_edited_cycle(parents, parent_from)
}

/// [`first_edited_cycle`] by a walk up from every node, in ID order.
fn scan_edited_cycle(
    parents: &Parents<'_>,
    parent_from: &BTreeMap<NodeId, (Seq, EventId)>,
) -> Option<Vec<NodeId>> {
    let mut done: BTreeSet<&NodeId> = BTreeSet::new();
    for start in parents.nodes.iter().map(|n| &n.id) {
        let mut path: Vec<&NodeId> = Vec::new();
        let mut on_path: BTreeMap<&NodeId, usize> = BTreeMap::new();
        let mut at = start;
        loop {
            if done.contains(at) {
                break;
            }
            if let Some(&i) = on_path.get(at) {
                let cycle: Vec<NodeId> = path[i..].iter().map(|n| (*n).clone()).collect();
                if cycle.iter().any(|n| parent_from.contains_key(n)) {
                    return Some(cycle);
                }
                break;
            }
            on_path.insert(at, path.len());
            path.push(at);
            match parents.up(at) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        done.extend(path);
    }
    None
}

/// The fields set of a `put` or a `retract`: the whole target.
fn whole() -> BTreeSet<String> {
    BTreeSet::from([WHOLE.to_owned()])
}

fn authored(event: &Event) -> Provenance {
    Provenance::Authored { actor: event.actor.clone(), event: event.id.clone() }
}

/// The node a `put` writes: its fields, with its ID, layer and authored provenance, and empty
/// aliases, props and evidence where the fields leave them out. `None` when the fields don't make
/// a node, such as one without a kind.
pub(crate) fn build_node(id: &NodeId, edit: &Edit, provenance: Provenance) -> Option<Node> {
    let mut obj: Map<String, Value> = edit.fields.clone().into_iter().collect();
    obj.insert("id".into(), json!(id));
    obj.entry("parent").or_insert(Value::Null);
    obj.entry("aliases").or_insert(json!([]));
    obj.entry("props").or_insert(json!({}));
    obj.entry("evidence").or_insert(json!([]));
    obj.insert("layer".into(), serde_json::to_value(edit.layer).ok()?);
    obj.insert("provenance".into(), serde_json::to_value(provenance).ok()?);
    serde_json::from_value(Value::Object(obj)).ok()
}

/// The edge a `put` writes, completed as [`build_node`] completes a node.
pub(crate) fn build_edge(id: &EdgeId, edit: &Edit, provenance: Provenance) -> Option<Edge> {
    let mut obj: Map<String, Value> = edit.fields.clone().into_iter().collect();
    obj.insert("id".into(), json!(id));
    obj.entry("via").or_insert(Value::Null);
    obj.entry("via_ref").or_insert(Value::Null);
    obj.entry("props").or_insert(json!({}));
    obj.entry("evidence").or_insert(json!([]));
    obj.entry("confidence").or_insert(json!(1.0));
    obj.insert("layer".into(), serde_json::to_value(edit.layer).ok()?);
    obj.insert("provenance".into(), serde_json::to_value(provenance).ok()?);
    serde_json::from_value(Value::Object(obj)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commands_events_share_the_stem_of_their_key() {
        assert_eq!(idem_stem("k1#2"), "k1");
        assert_eq!(idem_stem("k1"), "k1");
        assert_eq!(idem_stem("scan:ab#after:12"), "scan:ab#after:12");
        assert_eq!(idem_stem("k#"), "k#");
    }

    use planroom_contract::{Kind, Level, Tier};
    use proptest::prelude::*;
    use proptest::test_runner::Config;

    fn id(key: u8) -> NodeId {
        NodeId(format!("r:x:{key}"))
    }

    fn node(key: u8, parent: Option<u8>, name: &str) -> Node {
        Node {
            id: id(key),
            kind: Kind("k.x".into()),
            level: Level::Code,
            parent: parent.map(id),
            name: name.into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    /// What one step does to the nodes: put a node, remove one, or rename one in place.
    #[derive(Debug, Clone)]
    enum NodeOp {
        Put(u8, Option<u8>),
        Remove(u8),
        Rename(u8, u8),
    }

    fn node_op() -> impl Strategy<Value = NodeOp> {
        prop_oneof![
            (0u8..12, prop::option::of(0u8..12)).prop_map(|(k, p)| NodeOp::Put(k, p)),
            (0u8..12).prop_map(NodeOp::Remove),
            (0u8..12, 0u8..9).prop_map(|(k, n)| NodeOp::Rename(k, n)),
        ]
    }

    #[test]
    fn facts_in_any_order_keep_the_later_of_one_id_as_a_map_would() {
        let nodes = Nodes::new(vec![node(3, None, "a"), node(1, None, "b"), node(3, Some(1), "c")]);
        let held: Vec<(&str, &str)> = nodes.iter().map(|n| (n.id.0.as_str(), n.name.as_str())).collect();
        assert_eq!(held, [("r:x:1", "b"), ("r:x:3", "c")]);
    }

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        /// The nodes hold what a map that every write copied would hold, and a write never
        /// reaches the facts, which a copy of the nodes still shares.
        #[test]
        fn a_write_copies_its_node_and_never_reaches_the_shared_facts(
            facts in prop::collection::btree_map(0u8..12, prop::option::of(0u8..12), 0..10),
            ops in prop::collection::vec(node_op(), 0..30),
        ) {
            let mut fact_nodes: Vec<Node> = facts.iter().map(|(&k, &p)| node(k, p, "fact")).collect();
            fact_nodes.sort_by(|a, b| a.id.cmp(&b.id));
            let mut nodes = Nodes::new(fact_nodes.clone());
            let before = nodes.clone();
            let mut want: BTreeMap<NodeId, Node> = fact_nodes.iter().map(|n| (n.id.clone(), n.clone())).collect();
            for op in ops {
                match op {
                    NodeOp::Put(k, p) => {
                        nodes.insert(node(k, p, "put"));
                        want.insert(id(k), node(k, p, "put"));
                    }
                    NodeOp::Remove(k) => {
                        nodes.remove(&id(k));
                        want.remove(&id(k));
                    }
                    NodeOp::Rename(k, n) => {
                        let name = format!("n{n}");
                        prop_assert_eq!(nodes.get_mut(&id(k)).is_some(), want.contains_key(&id(k)));
                        if let Some(held) = nodes.get_mut(&id(k)) {
                            held.name = name.clone();
                        }
                        if let Some(held) = want.get_mut(&id(k)) {
                            held.name = name;
                        }
                    }
                }
                let held: Vec<&Node> = nodes.iter().collect();
                let wanted: Vec<&Node> = want.values().collect();
                prop_assert_eq!(held, wanted);
                for k in 0..12 {
                    prop_assert_eq!(nodes.get(&id(k)), want.get(&id(k)));
                    prop_assert_eq!(nodes.contains(&id(k)), want.contains_key(&id(k)));
                    prop_assert_eq!(nodes.fact(&id(k)), fact_nodes.iter().find(|n| n.id == id(k)));
                }
            }
            prop_assert!(Arc::ptr_eq(&nodes.facts, &before.facts), "the facts are shared, never copied");
            prop_assert_eq!(&*before.facts, &fact_nodes);
            prop_assert_eq!(before.iter().cloned().collect::<Vec<_>>(), fact_nodes);
        }

        /// The walk up from the edited nodes finds a loop exactly when a walk up from every node
        /// finds one that holds an edited node.
        #[test]
        fn the_walk_from_the_edits_finds_a_loop_when_the_walk_from_every_node_does(
            parents in prop::collection::btree_map(0u8..14, prop::option::of(0u8..16), 0..14),
            edited in prop::collection::btree_set(0u8..16, 0..6),
            reset in prop::collection::btree_map(0u8..14, prop::option::of(0u8..16), 0..3),
        ) {
            let nodes = Nodes::new(parents.iter().map(|(&k, &p)| node(k, p, "n")).collect());
            let reset = reset.into_iter().map(|(k, p)| (id(k), p.map(id))).collect();
            let parents = Parents { nodes: &nodes, reset };
            let parent_from: BTreeMap<NodeId, (Seq, EventId)> =
                edited.iter().map(|&k| (id(k), (u64::from(k), EventId(format!("ev-{k}"))))).collect();
            let scanned = scan_edited_cycle(&parents, &parent_from);
            prop_assert_eq!(edited_on_cycle(&parents, &parent_from), scanned.is_some());
            prop_assert_eq!(first_edited_cycle(&parents, &parent_from), scanned);
        }
    }
}
