//! What the fold keeps while it applies events in `seq` order, and the model and lists it derives
//! from them.

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use planroom_contract::{
    Actor, Cause, Client, Edge, EdgeId, Edit, Event, EventBody, EventId, Layer, Node, NodeId, Op, Provenance,
    Seq, Status, Target, Verdict,
};
use serde_json::{Map, Value, json};

use super::claims::ClaimRecord;
use super::cycle::find_cycle_by;
use super::lookup::Lookup;
use super::prepass::Prepass;
use crate::model::Model;
use crate::resolve::{Places, Resolved};

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

/// Whether any of `edit`'s keys ([`edit_keys`]) [`overlaps`] `key`, read from the edit without
/// building its keys.
fn ghost_overlaps(edit: &Edit, key: &FieldKey) -> bool {
    let same_target = match (&edit.target, &key.0) {
        (Target::Node(a), TargetKey::Node(b)) => a == b,
        (Target::Edge(a), TargetKey::Edge(b)) => a == b,
        _ => false,
    };
    same_target
        && match edit.op {
            // A `set` has a key per field, any of which may be named as the whole target is.
            Op::Set => {
                !edit.fields.is_empty()
                    && (key.1 == WHOLE || edit.fields.contains_key(WHOLE) || edit.fields.contains_key(&key.1))
            }
            Op::Put | Op::Retract => true,
        }
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
    /// The event's ID, shared with the event index.
    pub event: Arc<str>,
    /// Shared by the settings of one writer's run of events, not copied into each.
    pub writer: Arc<Writer>,
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
    /// The command's key stem ([`idem_stem`]), shared with [`Core::stems`].
    pub stem: Arc<str>,
    pub status: Status,
    pub keys: Vec<FieldKey>,
    pub undo_of: Option<EventId>,
    /// For an applied edit, the fields it set as a [`Mask`] over `keys`: what the fold set for an
    /// edit in effect, and for an undone one what it would set where it stands. An undo's
    /// revisions read it (DR-61).
    pub set: Option<Mask>,
}

/// The events of one command, by ID, in the order the fold met them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Members {
    first: Arc<str>,
    more: Vec<Arc<str>>,
}

impl Members {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &str> {
        std::iter::once(&*self.first).chain(self.more.iter().map(|id| &**id))
    }
}

/// The idempotency key a command's events share: `idem` without its `#N` suffix (DR-81).
pub(crate) fn idem_stem(idem: &str) -> &str {
    match idem.rsplit_once('#') {
        Some((stem, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => stem,
        _ => idem,
    }
}

/// The nodes the fold holds: the resolved facts, shared and never written, under what the edits
/// changed. A node an edit puts is held whole; for a fact an edit renames, moves or gives a prop,
/// only the fields the edits set are held, over the fact. The fold copies no fact for an edit,
/// only once each into the model, and a copy of the fold shares the facts.
#[derive(Debug, Clone)]
pub(crate) struct Nodes {
    /// The resolved facts, before any override: what drift and fallbacks compare with.
    facts: Arc<FactNodes>,
    /// What the edits made of each fact, by its place among the facts; empty until an edit
    /// changes one.
    changed: Vec<Option<Changed>>,
    /// The nodes edits put that aren't facts, by ID.
    others: BTreeMap<NodeId, Node>,
}

/// The fact nodes, one per ID and sorted by ID, and where each ID sits among them.
struct FactNodes {
    nodes: Vec<Node>,
    places: Places,
}

impl FactNodes {
    /// The place of the fact with ID `id`.
    fn place(&self, id: &NodeId) -> Option<usize> {
        self.places.find(&self.nodes, id)
    }
}

impl PartialEq for FactNodes {
    fn eq(&self, other: &FactNodes) -> bool {
        self.nodes == other.nodes
    }
}

impl std::fmt::Debug for FactNodes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.nodes.fmt(f)
    }
}

/// What the edits so far made of one fact.
#[derive(Debug, Clone, PartialEq)]
enum Changed {
    /// Removed.
    Gone,
    /// Put whole over it.
    Put(Box<Node>),
    /// The fact, with the fields edits set on it.
    Edited(Box<Edited>),
}

/// The fields edits set on a fact node.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Edited {
    name: Option<String>,
    parent: Option<Option<NodeId>>,
    /// Each prop an edit set, `None` for one it removed.
    props: BTreeMap<String, Option<Value>>,
}

impl Edited {
    /// `fact` with these fields set on it.
    fn over(&self, fact: &Node) -> Node {
        // Each field is copied from the fact or from the edits, never from both.
        let mut node = Node {
            id: fact.id.clone(),
            kind: fact.kind.clone(),
            level: fact.level,
            parent: self.parent.as_ref().unwrap_or(&fact.parent).clone(),
            name: self.name.as_ref().unwrap_or(&fact.name).clone(),
            aliases: fact.aliases.clone(),
            props: fact.props.clone(),
            layer: fact.layer,
            provenance: fact.provenance.clone(),
            evidence: fact.evidence.clone(),
        };
        for (key, value) in &self.props {
            match value {
                Some(value) => {
                    node.props.insert(key.clone(), value.clone());
                }
                None => {
                    node.props.remove(key);
                }
            }
        }
        node
    }
}

/// A node an edit writes, as the nodes hold it.
pub(crate) enum NodeMut<'a> {
    Put(&'a mut Node),
    Edited(&'a mut Edited),
}

impl NodeMut<'_> {
    fn set_name(self, name: String) {
        match self {
            NodeMut::Put(node) => node.name = name,
            NodeMut::Edited(edited) => edited.name = Some(name),
        }
    }

    fn set_parent(self, parent: Option<NodeId>) {
        match self {
            NodeMut::Put(node) => node.parent = parent,
            NodeMut::Edited(edited) => edited.parent = Some(parent),
        }
    }

    /// Set prop `key`, or remove it for `None`.
    fn set_prop(self, key: &str, value: Option<Value>) {
        match (self, value) {
            (NodeMut::Put(node), Some(value)) => {
                node.props.insert(key.to_owned(), value);
            }
            (NodeMut::Put(node), None) => {
                node.props.remove(key);
            }
            (NodeMut::Edited(edited), value) => {
                edited.props.insert(key.to_owned(), value);
            }
        }
    }
}

impl Nodes {
    /// The nodes `facts` give, before any edit, with their `places` when resolution found them.
    /// Resolution leaves them one per ID and sorted; any others are sorted here, and of two with
    /// one ID the later stands.
    fn new(mut facts: Vec<Node>, places: Option<Places>) -> Nodes {
        let places = match places {
            Some(places) if facts.is_sorted_by(|a, b| a.id < b.id) => places,
            _ => {
                if !facts.is_sorted_by(|a, b| a.id < b.id) {
                    facts.reverse();
                    facts.sort_by(|a, b| a.id.cmp(&b.id));
                    facts.dedup_by(|later, earlier| later.id == earlier.id);
                }
                Places::of(&facts)
            }
        };
        Nodes {
            facts: Arc::new(FactNodes { nodes: facts, places }),
            changed: Vec::new(),
            others: BTreeMap::new(),
        }
    }

    /// The fact node with this ID, before any edit.
    pub(crate) fn fact(&self, id: &NodeId) -> Option<&Node> {
        self.facts.place(id).map(|i| &self.facts.nodes[i])
    }

    fn changed(&self, place: usize) -> Option<&Changed> {
        self.changed.get(place).and_then(Option::as_ref)
    }

    /// The node with this ID as the edits so far leave it, as its ID where the nodes hold it and
    /// its parent; `None` when there's no such node.
    fn find(&self, id: &NodeId) -> Option<(&NodeId, Option<&NodeId>)> {
        let Some(place) = self.facts.place(id) else {
            return self.others.get(id).map(|n| (&n.id, n.parent.as_ref()));
        };
        let fact = &self.facts.nodes[place];
        match self.changed(place) {
            None => Some((&fact.id, fact.parent.as_ref())),
            Some(Changed::Gone) => None,
            Some(Changed::Put(node)) => Some((&node.id, node.parent.as_ref())),
            Some(Changed::Edited(edited)) => {
                let parent = match &edited.parent {
                    Some(parent) => parent.as_ref(),
                    None => fact.parent.as_ref(),
                };
                Some((&fact.id, parent))
            }
        }
    }

    pub(crate) fn contains(&self, id: &NodeId) -> bool {
        self.find(id).is_some()
    }

    /// Node `id`'s ID where the nodes hold it, when there's such a node.
    pub(crate) fn held(&self, id: &NodeId) -> Option<&NodeId> {
        self.find(id).map(|(held, _)| held)
    }

    /// Node `id`'s parent: `None` when it sits at the top or there's no such node.
    pub(crate) fn parent(&self, id: &NodeId) -> Option<&NodeId> {
        self.find(id).and_then(|(_, parent)| parent)
    }

    /// What the edits made of the fact at `place`, to write.
    fn change(&mut self, place: usize) -> &mut Option<Changed> {
        if self.changed.is_empty() {
            self.changed.resize(self.facts.nodes.len(), None);
        }
        &mut self.changed[place]
    }

    /// The node to write. The first write to a fact starts the fields kept over it.
    fn get_mut(&mut self, id: &NodeId) -> Option<NodeMut<'_>> {
        let Some(place) = self.facts.place(id) else { return self.others.get_mut(id).map(NodeMut::Put) };
        let change = self.change(place);
        match change.get_or_insert_with(|| Changed::Edited(Box::default())) {
            Changed::Gone => None,
            Changed::Put(node) => Some(NodeMut::Put(node)),
            Changed::Edited(edited) => Some(NodeMut::Edited(edited)),
        }
    }

    fn insert(&mut self, node: Node) {
        match self.facts.place(&node.id) {
            Some(place) => *self.change(place) = Some(Changed::Put(Box::new(node))),
            None => {
                self.others.insert(node.id.clone(), node);
            }
        }
    }

    fn remove(&mut self, id: &NodeId) {
        match self.facts.place(id) {
            Some(place) => *self.change(place) = Some(Changed::Gone),
            None => {
                self.others.remove(id);
            }
        }
    }

    /// Every node, in ID order: a fact no edit changed and a node an edit put as held, any other
    /// fact with the fields edits set on it.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Cow<'_, Node>> {
        self.merged().filter_map(|held| match held {
            Held::Fact(fact, None) | Held::Other(fact) => Some(Cow::Borrowed(fact)),
            Held::Fact(_, Some(Changed::Gone)) => None,
            Held::Fact(_, Some(Changed::Put(node))) => Some(Cow::Borrowed(&**node)),
            Held::Fact(fact, Some(Changed::Edited(edited))) => Some(Cow::Owned(edited.over(fact))),
        })
    }

    /// How many nodes there are at most: the facts and the other nodes edits put.
    fn len(&self) -> usize {
        self.facts.nodes.len() + self.others.len()
    }

    /// Every node's ID, in ID order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = &NodeId> {
        self.merged().filter_map(|held| match held {
            Held::Fact(_, Some(Changed::Gone)) => None,
            Held::Fact(_, Some(Changed::Put(node))) => Some(&node.id),
            Held::Fact(fact, _) | Held::Other(fact) => Some(&fact.id),
        })
    }

    /// The facts, each with what edits made of it, and the other nodes, in ID order.
    fn merged(&self) -> impl Iterator<Item = Held<'_>> {
        let mut facts = self.facts.nodes.iter().enumerate().peekable();
        let mut others = self.others.values().peekable();
        std::iter::from_fn(move || {
            let fact_first = match (facts.peek(), others.peek()) {
                (None, None) => return None,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some((_, fact)), Some(other)) => fact.id < other.id,
            };
            Some(if fact_first {
                let (place, fact) = facts.next()?;
                Held::Fact(fact, self.changed(place))
            } else {
                Held::Other(others.next()?)
            })
        })
    }
}

/// One node as [`Nodes::merged`] meets it.
enum Held<'a> {
    Fact(&'a Node, Option<&'a Changed>),
    Other(&'a Node),
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
    /// The nodes, the resolved facts among them.
    nodes: Nodes,
    /// The nodes whose current parent an edit set, with that edit.
    parent_from: BTreeMap<NodeId, (Seq, EventId)>,
    /// In fact order, then in the order edits put them; a retracted edge leaves a hole.
    edges: Vec<Option<Edge>>,
    /// Where each edge sits in `edges`, and which are facts.
    edge_index: EdgeIndex,
    /// The edges resolution set aside, kept for the caller to report.
    unresolved: Vec<Edge>,
    hidden: BTreeSet<TargetKey>,
    positions: BTreeMap<(String, NodeId), Value>,
    ghosts: BTreeMap<Seq, Ghost>,
    conflicts: Vec<StoredConflict>,
    versions: Vec<VersionTag>,
    claims: Vec<ClaimRecord>,
    open_cards: Vec<EventId>,
    /// Every effective applied edit's settings, per field, in `seq` order.
    pub setters: BTreeMap<FieldKey, Vec<Setter>>,
    /// Every applied edit's parent entry, per node, in `seq` order, undone or not.
    pub parent_log: BTreeMap<NodeId, Vec<(Seq, EventId, ParentEntry)>>,
    /// Each event the fold saw, by ID.
    pub index: Lookup<Arc<str>, Indexed>,
    /// The events of each command, by key stem, in `seq` order.
    pub stems: Lookup<Arc<str>, Members>,
    /// The ghosts each event caused, by that event, in `seq` order, undone or not: the ghosts a
    /// card holds, such as the Apply card's inbox edits.
    pub caused: Lookup<EventId, Vec<Arc<str>>>,
    /// Every `card` event, in `seq` order, open or not.
    pub cards: Vec<EventId>,
    diffs: Vec<FoldDiff>,
    /// The moves Diffs already lists.
    listed_moves: BTreeSet<(NodeId, NodeId)>,
    /// The writer of the last edit applied, which the next one by the same writer shares.
    recent_writer: Recent,
}

/// Which of an edit's keys, in [`edit_keys`] order, the fold set: a `set`'s fields but those it
/// skipped, the whole target for a `put` that builds and for a `retract`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Mask {
    /// The first 64 keys, one bit each.
    low: u64,
    /// Any further keys, 64 to a word.
    high: Vec<u64>,
}

impl Mask {
    fn set(&mut self, i: usize) {
        if i < 64 {
            self.low |= 1 << i;
        } else {
            let word = i / 64 - 1;
            if self.high.len() <= word {
                self.high.resize(word + 1, 0);
            }
            self.high[word] |= 1 << (i % 64);
        }
    }

    /// Whether the fold set key `i`.
    pub(crate) fn has(&self, i: usize) -> bool {
        if i < 64 {
            self.low & (1 << i) != 0
        } else {
            self.high.get(i / 64 - 1).is_some_and(|w| w & (1 << (i % 64)) != 0)
        }
    }

    /// The mask of a `put` or a `retract` that set its whole target, its only key.
    fn whole() -> Mask {
        Mask { low: 1, high: Vec::new() }
    }
}

/// The writer of the last edit applied: a cache that lets a run of one writer's edits share one
/// copy, never part of what the fold holds, so it counts for nothing in equality or `Debug`.
#[derive(Clone, Default)]
struct Recent(Option<Arc<Writer>>);

impl Recent {
    /// The writer of `event`, shared with the last edit's when it is the same writer.
    fn writer(&mut self, event: &Event) -> Arc<Writer> {
        match &self.0 {
            Some(w) if w.actor == event.actor && w.client == event.client => w.clone(),
            _ => {
                let w = Arc::new(Writer::of(event));
                self.0 = Some(w.clone());
                w
            }
        }
    }
}

impl PartialEq for Recent {
    fn eq(&self, _: &Recent) -> bool {
        true
    }
}

impl std::fmt::Debug for Recent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recent")
    }
}

/// Where each edge ID sits among the fold's edges, and which IDs the facts hold. It is built from
/// the edges as resolution left them, since only an edit to an edge changes them, the first time
/// an applied edit needs an edge's place: a put, a set or a retract of an edge in effect, or a set
/// of one that an undo reversed. A fold with no such edit never builds it.
#[derive(Clone, Default)]
struct EdgeIndex {
    /// How many of the fold's edges are facts: the first ones.
    facts: usize,
    built: Option<BuiltEdges>,
    /// How many passes over every edge the index made, building itself or standing in for itself
    /// before it is built: what a test bounds the index's work by.
    #[cfg(test)]
    passes: Passes,
}

#[cfg(test)]
#[derive(Default)]
struct Passes(std::sync::atomic::AtomicUsize);

#[cfg(test)]
impl Clone for Passes {
    fn clone(&self) -> Passes {
        Passes(self.0.load(std::sync::atomic::Ordering::Relaxed).into())
    }
}

#[derive(Clone)]
struct BuiltEdges {
    slots: BTreeMap<EdgeId, Vec<usize>>,
    facts: BTreeSet<EdgeId>,
}

impl EdgeIndex {
    /// Count one pass over every edge.
    fn pass(&self) {
        #[cfg(test)]
        self.passes.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn build(&mut self, edges: &[Option<Edge>]) -> &mut BuiltEdges {
        if self.built.is_none() {
            self.pass();
        }
        let facts = self.facts;
        self.built.get_or_insert_with(|| {
            let mut slots: BTreeMap<EdgeId, Vec<usize>> = BTreeMap::new();
            for (i, edge) in edges.iter().enumerate() {
                if let Some(edge) = edge {
                    slots.entry(edge.id.clone()).or_default().push(i);
                }
            }
            let facts = edges[..facts].iter().flatten().map(|e| e.id.clone()).collect();
            BuiltEdges { slots, facts }
        })
    }

    /// The slots of edge `id`, from the index once built, else from the edges themselves.
    fn slots(&self, edges: &[Option<Edge>], id: &EdgeId) -> Vec<usize> {
        match &self.built {
            Some(built) => built.slots.get(id).cloned().unwrap_or_default(),
            None => {
                self.pass();
                edges
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.as_ref().is_some_and(|e| e.id == *id))
                    .map(|(i, _)| i)
                    .collect()
            }
        }
    }

    fn is_fact(&self, edges: &[Option<Edge>], id: &EdgeId) -> bool {
        match &self.built {
            Some(built) => built.facts.contains(id),
            None => {
                self.pass();
                edges[..self.facts].iter().flatten().any(|e| e.id == *id)
            }
        }
    }
}

impl PartialEq for EdgeIndex {
    /// Derived from the edges, which equality compares already.
    fn eq(&self, other: &EdgeIndex) -> bool {
        self.facts == other.facts
    }
}

impl std::fmt::Debug for EdgeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeIndex").field("facts", &self.facts).finish_non_exhaustive()
    }
}

impl Core {
    pub fn new(resolved: Resolved) -> Core {
        Core::placed(resolved, None)
    }

    /// [`Core::new`] with the places resolution found for its nodes.
    pub fn placed(resolved: Resolved, places: Option<Places>) -> Core {
        Core {
            at: 0,
            nodes: Nodes::new(resolved.nodes, places),
            parent_from: BTreeMap::new(),
            edge_index: EdgeIndex { facts: resolved.edges.len(), ..EdgeIndex::default() },
            edges: resolved.edges.into_iter().map(Some).collect(),
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
            index: Lookup::default(),
            stems: Lookup::default(),
            caused: Lookup::default(),
            cards: Vec::new(),
            diffs: Vec::new(),
            listed_moves: BTreeSet::new(),
            recent_writer: Recent::default(),
        }
    }

    /// Apply one event, the next in `seq` order, as the pre-pass found it: undone events and
    /// reviewed ghosts are skipped, and a card's body and an unknown kind are never read.
    pub fn apply(&mut self, event: &Event, prepass: &Prepass) {
        self.at = self.at.max(event.seq);
        let (keys, undo_of) = match &event.body {
            EventBody::Edit(edit) => (edit_keys(edit), None),
            EventBody::Undo { of } => (Vec::new(), Some(of.clone())),
            _ => (Vec::new(), None),
        };
        let id: Arc<str> = Arc::from(event.id.0.as_str());
        let set = self.apply_body(event, prepass, &id, &keys);
        self.index_event(event, id, keys, undo_of, set);
    }

    /// Apply `event`'s body, and return the fields it set when it is an applied edit, as a
    /// [`Mask`] over its `keys`.
    fn apply_body(
        &mut self,
        event: &Event,
        prepass: &Prepass,
        id: &Arc<str>,
        keys: &[FieldKey],
    ) -> Option<Mask> {
        if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status)
            && let Some((node, entry)) = ParentEntry::of(edit)
        {
            self.parent_log.entry(node).or_default().push((event.seq, event.id.clone(), entry));
        }
        let undone = prepass.undone.contains(&event.id);
        if !undone
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
        if undone {
            if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status) {
                return Some(self.would_set(event, edit));
            }
            return None;
        }
        match &event.body {
            EventBody::Edit(edit) => match event.status {
                Status::Applied => return Some(self.apply_edit(event, edit, id, keys)),
                Status::Ghost if !prepass.reviews.contains_key(&event.id) => {
                    let ghost = Ghost {
                        event: event.id.clone(),
                        seq: event.seq,
                        writer: Writer::of(event),
                        cause: event.cause.clone(),
                        edit: edit.clone(),
                    };
                    self.ghosts.insert(event.seq, ghost);
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
        None
    }

    /// Index `event` under its ID, with what its body set when it set anything. Of two events
    /// with one ID, the later is indexed, and keeps the earlier's mask when it set nothing. A
    /// mask counts positions in one edit's keys, so on another edit it names other fields: the
    /// fold computes what it did with field names only for histories the store can hold, whose
    /// `events.id` is unique, never two events with one ID.
    fn index_event(
        &mut self,
        event: &Event,
        id: Arc<str>,
        keys: Vec<FieldKey>,
        undo_of: Option<EventId>,
        set: Option<Mask>,
    ) {
        let stem: Arc<str> = Arc::from(idem_stem(&event.idem));
        match self.stems.entry(stem.clone()) {
            Entry::Occupied(mut members) => members.get_mut().more.push(id.clone()),
            Entry::Vacant(vacant) => {
                vacant.insert(Members { first: id.clone(), more: Vec::new() });
            }
        }
        let indexed = Indexed { seq: event.seq, stem, status: event.status, keys, undo_of, set };
        if let Some(earlier) = self.index.insert(id.clone(), indexed)
            && earlier.set.is_some()
            && let Some(later) = self.index.get_mut(&*id)
            && later.set.is_none()
        {
            later.set = earlier.set;
        }
        if let (EventBody::Edit(_), Status::Ghost, Some(Cause::Event(cause))) =
            (&event.body, event.status, &event.cause)
        {
            match self.caused.get_mut(cause) {
                Some(ghosts) => ghosts.push(id),
                None => {
                    self.caused.insert(cause.clone(), vec![id]);
                }
            }
        } else if let EventBody::Card(_) = event.body {
            self.cards.push(event.id.clone());
        }
    }

    /// Size the event tables for `events` more events.
    pub fn reserve(&mut self, events: usize) {
        self.index.reserve(events);
        self.stems.reserve(events);
    }

    fn apply_edit(&mut self, event: &Event, edit: &Edit, id: &Arc<str>, keys: &[FieldKey]) -> Mask {
        let writer = self.recent_writer.writer(event);
        // The fields the fold set: a `set`'s fields but those it skipped, and the whole target for
        // a `put` that builds and for a `retract`.
        let set: Mask = match (&edit.op, &edit.target) {
            (Op::Put, Target::Node(id)) => self.put_node(event, edit, id),
            (Op::Put, Target::Edge(id)) => self.put_edge(event, edit, id),
            (Op::Set, Target::Node(id)) => self.set_node(event, edit, id),
            (Op::Set, Target::Edge(id)) => self.set_edge(event, edit, id),
            (Op::Retract, Target::Node(id)) => {
                self.nodes.remove(id);
                self.parent_from.remove(id);
                self.diffs.push(FoldDiff::Retracted { event: event.id.clone(), target: edit.target.clone() });
                Mask::whole()
            }
            (Op::Retract, Target::Edge(id)) => {
                for slot in self.edge_index.build(&self.edges).slots.remove(id).unwrap_or_default() {
                    self.edges[slot] = None;
                }
                self.diffs.push(FoldDiff::Retracted { event: event.id.clone(), target: edit.target.clone() });
                Mask::whole()
            }
        };
        if writer.is_person() && !self.ghosts.is_empty() {
            let applied: Vec<&FieldKey> =
                keys.iter().enumerate().filter(|(i, _)| set.has(*i)).map(|(_, k)| k).collect();
            let superseded: Vec<Seq> = self
                .ghosts
                .iter()
                .filter(|(seq, ghost)| {
                    **seq < event.seq && applied.iter().any(|k| ghost_overlaps(&ghost.edit, k))
                })
                .map(|(seq, _)| *seq)
                .collect();
            for seq in superseded {
                let ghost = self.ghosts.remove(&seq).expect("a pending ghost");
                self.diffs.push(FoldDiff::GhostSuperseded { ghost: ghost.event, by: event.id.clone() });
            }
        }
        for (i, key) in keys.iter().enumerate() {
            let setter = Setter {
                seq: event.seq,
                event: id.clone(),
                writer: writer.clone(),
                op: edit.op,
                layer: edit.layer,
                over: edit.over.get(&key.1).cloned(),
                applied: set.has(i),
            };
            match self.setters.get_mut(key) {
                Some(setters) => setters.push(setter),
                None => {
                    self.setters.insert(key.clone(), vec![setter]);
                }
            }
        }
        set
    }

    /// The index entry of event `id`, if the fold saw it.
    pub fn indexed(&self, id: &EventId) -> Option<&Indexed> {
        self.index.get(id.0.as_str())
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
        match find_cycle_by(node, parent, |n| nodes.parent(n)) {
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

    fn put_node(&mut self, event: &Event, edit: &Edit, id: &NodeId) -> Mask {
        let Some(mut node) = build_node(id, edit, authored(event)) else {
            self.invalid(event, None);
            return Mask::default();
        };
        let earlier = self.nodes.parent(id).cloned();
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
        Mask::whole()
    }

    fn put_edge(&mut self, event: &Event, edit: &Edit, id: &EdgeId) -> Mask {
        let Some(edge) = build_edge(id, edit, authored(event)) else {
            self.invalid(event, None);
            return Mask::default();
        };
        let built = self.edge_index.build(&self.edges);
        match built.slots.get_mut(id) {
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
                built.slots.insert(id.clone(), vec![self.edges.len() - 1]);
            }
        }
        Mask::whole()
    }

    /// What a `set` of `field` to `value` on node `id` does in the state the fold has reached.
    /// Every field of a node that doesn't exist is skipped, a hide and a pin included, so it
    /// writes no revision and holds nothing (architecture.md, "Versions and revisions").
    /// `exists` is whether node `id` exists, which no field of one edit changes.
    fn node_field(&self, exists: bool, id: &NodeId, field: &str, value: &Value) -> FieldOutcome {
        if field == HIDDEN && !value.is_boolean() {
            return FieldOutcome::Invalid;
        }
        if !exists {
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
                    match find_cycle_by(id, &NodeId(p.clone()), |n| nodes.parent(n)) {
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

    fn set_node(&mut self, event: &Event, edit: &Edit, id: &NodeId) -> Mask {
        let exists = self.nodes.contains(id);
        let mut set = Mask::default();
        for (i, (field, value)) in edit.fields.iter().enumerate() {
            match self.node_field(exists, id, field, value) {
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
                let target = TargetKey::Node(id.clone());
                if value.as_bool() == Some(true) {
                    self.hidden.insert(target);
                } else {
                    self.hidden.remove(&target);
                }
            } else if field == NAME {
                self.nodes.get_mut(id).expect("a node").set_name(value.as_str().expect("a name").to_owned());
            } else if field == PARENT {
                self.nodes
                    .get_mut(id)
                    .expect("a node")
                    .set_parent(value.as_str().map(|p| NodeId(p.to_owned())));
                self.parent_from.insert(id.clone(), (event.seq, event.id.clone()));
            } else if let Some(prop) = field.strip_prefix(PROPS_PREFIX) {
                let value = (!value.is_null()).then(|| value.clone());
                self.nodes.get_mut(id).expect("a node").set_prop(prop, value);
            }
            set.set(i);
        }
        set
    }

    /// What a `set` of `field` to `value` on edge `id` does in the state the fold has reached: a
    /// prop of an edge with no live copy is skipped.
    fn edge_field(&self, id: &EdgeId, field: &str, value: &Value) -> FieldOutcome {
        if field == HIDDEN {
            if value.is_boolean() { FieldOutcome::Set } else { FieldOutcome::Invalid }
        } else if field.starts_with(PROPS_PREFIX) {
            let live = self.edge_index.slots(&self.edges, id).iter().any(|s| self.edges[*s].is_some());
            if live { FieldOutcome::Set } else { FieldOutcome::Skipped }
        } else {
            FieldOutcome::Invalid
        }
    }

    fn set_edge(&mut self, event: &Event, edit: &Edit, id: &EdgeId) -> Mask {
        let target = TargetKey::Edge(id.clone());
        self.edge_index.build(&self.edges);
        let slots = self.edge_index.slots(&self.edges, id);
        let mut set = Mask::default();
        for (i, (field, value)) in edit.fields.iter().enumerate() {
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
            set.set(i);
        }
        set
    }

    /// The fields `edit` would set if the fold applied it now, without applying it: what an
    /// undone edit set, read in the state the edits that stay in effect build up to its `seq`.
    /// An edit that only an edit undone with it made settable, such as a rename of a node that
    /// same command put, reads as setting nothing here; the undo's revision on the put's whole
    /// target still makes a write to that field from an older base conflict.
    fn would_set(&mut self, event: &Event, edit: &Edit) -> Mask {
        let mut set = Mask::default();
        match (&edit.op, &edit.target) {
            (Op::Put, Target::Node(id)) => {
                if build_node(id, edit, authored(event)).is_some() {
                    set = Mask::whole();
                }
            }
            (Op::Put, Target::Edge(id)) => {
                if build_edge(id, edit, authored(event)).is_some() {
                    set = Mask::whole();
                }
            }
            (Op::Retract, _) => set = Mask::whole(),
            (Op::Set, Target::Node(id)) => {
                let exists = self.nodes.contains(id);
                for (i, (f, v)) in edit.fields.iter().enumerate() {
                    if self.node_field(exists, id, f, v) == FieldOutcome::Set {
                        set.set(i);
                    }
                }
            }
            (Op::Set, Target::Edge(id)) => {
                // An undone edit reads the index as an applied one does, so many of them pass
                // over the edges once rather than once each.
                self.edge_index.build(&self.edges);
                for (i, (f, v)) in edit.fields.iter().enumerate() {
                    if self.edge_field(id, f, v) == FieldOutcome::Set {
                        set.set(i);
                    }
                }
            }
        }
        set
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
            TargetKey::Edge(id) => !self.edge_index.slots(&self.edges, id).is_empty(),
        }
    }

    fn is_fact(&self, target: &TargetKey) -> bool {
        match target {
            TargetKey::Node(id) => self.nodes.fact(id).is_some(),
            TargetKey::Edge(id) => self.edge_index.is_fact(&self.edges, id),
        }
    }

    /// The model and lists at this state: parents that don't resolve fall back to their fact
    /// parent, any loop left is broken by `seq`, and drift on overrides is listed.
    pub fn finish(&self) -> Finished {
        let mut parents = Parents { nodes: &self.nodes, reset: BTreeMap::new() };
        let mut parent_from = self.parent_from.clone();
        let mut diffs = self.diffs.clone();
        for (id, (_, event)) in &self.parent_from {
            let Some(parent) = self.nodes.parent(id) else { continue };
            if self.nodes.contains(parent) {
                continue;
            }
            let parent = parent.clone();
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
                let entry = vanished.entry(target.clone()).or_insert_with(|| EventId(last.event.to_string()));
                if self.indexed(entry).is_none_or(|e| e.seq < last.seq) {
                    *entry = EventId(last.event.to_string());
                }
                continue;
            }
            // A fact is read only for an override that recorded the value it was made over.
            let Some(over) = &last.over else { continue };
            let Some(fact) = self.fact_value(target, field) else { continue };
            if *over != fact {
                diffs.push(FoldDiff::ScanChangedUnderEdit {
                    event: EventId(last.event.to_string()),
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
        let mut nodes = Vec::with_capacity(self.nodes.len());
        nodes.extend(self.nodes.iter().map(Cow::into_owned));
        if !parents.reset.is_empty() {
            for node in &mut nodes {
                if let Some(parent) = parents.reset.get(&node.id) {
                    node.parent = parent.clone();
                }
            }
        }
        Finished {
            model: Model {
                at: self.at,
                nodes,
                edges: {
                    let mut edges = Vec::with_capacity(self.edges.len());
                    edges.extend(self.edges.iter().flatten().cloned());
                    edges
                },
                unresolved: self.unresolved.clone(),
            },
            overlay: Overlay {
                hidden: self.hidden.clone(),
                positions: self.positions.clone(),
                ghosts: self.ghosts.values().cloned().collect(),
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
            None => self.nodes.parent(id),
        }
    }

    /// The parent of node `id` when it is a node: the next step up the tree.
    fn up(&self, id: &NodeId) -> Option<&NodeId> {
        self.of(id).and_then(|parent| self.nodes.held(parent))
    }

    /// Node `id`, when it is one, as its ID where the nodes hold it and its parent: one lookup
    /// for both, so a walk up the tree finds each node once.
    fn node(&self, id: &NodeId) -> Option<(&NodeId, Option<&NodeId>)> {
        let (held, parent) = self.nodes.find(id)?;
        match self.reset.get(held) {
            Some(reset) => Some((held, reset.as_ref())),
            None => Some((held, parent)),
        }
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
        let Some((start, mut above)) = parents.node(start) else { continue };
        let mut path: Vec<&NodeId> = vec![start];
        let mut seen: BTreeSet<&NodeId> = BTreeSet::new();
        loop {
            // The next step up, as [`Parents::up`] gives it, with the parent above that.
            match above.and_then(|parent| parents.node(parent)) {
                Some((parent, _)) if parent == start => return true,
                // A loop above that doesn't pass through `start`: the walk from one of its own
                // edited nodes finds it.
                Some((parent, _)) if !seen.insert(parent) => break,
                Some((parent, next)) if !topped.contains(parent) => {
                    path.push(parent);
                    above = next;
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
    for start in parents.nodes.ids() {
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

    /// What one step does to the nodes: put a node, remove one, or rename, move or set a prop of
    /// one in place.
    #[derive(Debug, Clone)]
    enum NodeOp {
        Put(u8, Option<u8>),
        Remove(u8),
        Rename(u8, u8),
        Move(u8, Option<u8>),
        Prop(u8, u8, Option<u8>),
    }

    fn node_op() -> impl Strategy<Value = NodeOp> {
        prop_oneof![
            (0u8..12, prop::option::of(0u8..12)).prop_map(|(k, p)| NodeOp::Put(k, p)),
            (0u8..12).prop_map(NodeOp::Remove),
            (0u8..12, 0u8..9).prop_map(|(k, n)| NodeOp::Rename(k, n)),
            (0u8..12, prop::option::of(0u8..12)).prop_map(|(k, p)| NodeOp::Move(k, p)),
            (0u8..12, 0u8..3, prop::option::of(0u8..3)).prop_map(|(k, p, v)| NodeOp::Prop(k, p, v)),
        ]
    }

    /// An undone `set` of an edge's prop reads the edge index as an applied one does, so a history
    /// of many such sets passes over the edges once, to build the index, and not once for each:
    /// the fold's cost stays linear in the events and the edges.
    #[test]
    fn undone_edge_sets_pass_over_the_edges_once() {
        use planroom_contract::{Actor, Client, Edge, EdgeId, Ref, Surface, Timestamp, Vocabulary};
        const EDGES: usize = 2_000;
        const SETS: usize = 200;
        let edge = |i: usize| Edge {
            id: EdgeId(format!("e:{i:012x}")),
            kind: Kind("k.rel".into()),
            from: Ref::Id(id((i % 100) as u8)),
            to: Ref::Id(id(((i + 1) % 100) as u8)),
            via: None,
            via_ref: None,
            props: Default::default(),
            evidence: vec![],
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        };
        let facts = crate::model::Facts {
            nodes: (0..100).map(|k| node(k, None, "n")).collect(),
            edges: (0..EDGES).map(edge).collect(),
            ..Default::default()
        };
        let event = |seq: Seq, body: EventBody| Event {
            seq,
            id: EventId(format!("ev-{seq}")),
            idem: format!("k{seq}"),
            request_hash: "h".into(),
            at: Timestamp("2026-10-26T12:00:00Z".into()),
            actor: Actor::Person { id: "person:ann".into(), name: None },
            client: Client { surface: Surface::Cli, id: "cli:1".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            body,
        };
        let mut events = Vec::new();
        for i in 0..SETS {
            let seq = events.len() as Seq + 1;
            let set = Edit {
                op: Op::Set,
                target: Target::Edge(EdgeId(format!("e:{:012x}", i * 7 % EDGES))),
                layer: Layer::Actual,
                fields: BTreeMap::from([("props.x".to_owned(), json!(i))]),
                base: 0,
                over: BTreeMap::new(),
            };
            events.push(event(seq, EventBody::Edit(set)));
            events.push(event(seq + 1, EventBody::Undo { of: EventId(format!("ev-{seq}")) }));
        }
        let folded = super::super::fold_full(&facts, &events, &Vocabulary::default(), events.len() as Seq);
        assert!(folded.is_undone(&EventId("ev-1".into())));
        assert!(folded.model.edges.iter().all(|e| e.props.is_empty()), "every set is undone");
        let passes = folded.core.edge_index.passes.0.load(std::sync::atomic::Ordering::Relaxed);
        assert!(passes <= 1, "{passes} passes over {EDGES} edges for {SETS} undone edge sets");
    }

    #[test]
    fn facts_in_any_order_keep_the_later_of_one_id_as_a_map_would() {
        let nodes = Nodes::new(vec![node(3, None, "a"), node(1, None, "b"), node(3, Some(1), "c")], None);
        let held: Vec<(String, String)> = nodes.iter().map(|n| (n.id.0.clone(), n.name.clone())).collect();
        assert_eq!(held, [("r:x:1".into(), "b".into()), ("r:x:3".into(), "c".into())]);
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
            let mut fact_nodes: Vec<Node> = facts
                .iter()
                .map(|(&k, &p)| {
                    let mut n = node(k, p, "fact");
                    n.props.insert(format!("p{}", k % 3), serde_json::json!(k));
                    n
                })
                .collect();
            fact_nodes.sort_by(|a, b| a.id.cmp(&b.id));
            let mut nodes = Nodes::new(fact_nodes.clone(), None);
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
                            held.set_name(name.clone());
                        }
                        if let Some(held) = want.get_mut(&id(k)) {
                            held.name = name;
                        }
                    }
                    NodeOp::Move(k, p) => {
                        if let Some(held) = nodes.get_mut(&id(k)) {
                            held.set_parent(p.map(id));
                        }
                        if let Some(held) = want.get_mut(&id(k)) {
                            held.parent = p.map(id);
                        }
                    }
                    NodeOp::Prop(k, p, v) => {
                        let (key, value) = (format!("p{p}"), v.map(|v| serde_json::json!(v)));
                        if let Some(held) = nodes.get_mut(&id(k)) {
                            held.set_prop(&key, value.clone());
                        }
                        if let Some(held) = want.get_mut(&id(k)) {
                            match value {
                                Some(value) => held.props.insert(key, value),
                                None => held.props.remove(&key),
                            };
                        }
                    }
                }
                let held: Vec<Node> = nodes.iter().map(Cow::into_owned).collect();
                let wanted: Vec<Node> = want.values().cloned().collect();
                prop_assert_eq!(held, wanted);
                let ids: Vec<&NodeId> = nodes.ids().collect();
                prop_assert_eq!(ids, want.keys().collect::<Vec<_>>());
                for k in 0..12 {
                    let wanted = want.get(&id(k));
                    prop_assert_eq!(nodes.contains(&id(k)), wanted.is_some());
                    prop_assert_eq!(nodes.held(&id(k)), wanted.map(|n| &n.id));
                    prop_assert_eq!(nodes.parent(&id(k)), wanted.and_then(|n| n.parent.as_ref()));
                    prop_assert_eq!(nodes.fact(&id(k)), fact_nodes.iter().find(|n| n.id == id(k)));
                }
            }
            prop_assert!(Arc::ptr_eq(&nodes.facts, &before.facts), "the facts are shared, never copied");
            prop_assert_eq!(&before.facts.nodes, &fact_nodes);
            prop_assert_eq!(before.iter().map(Cow::into_owned).collect::<Vec<_>>(), fact_nodes);
        }

        /// The walk up from the edited nodes finds a loop exactly when a walk up from every node
        /// finds one that holds an edited node.
        #[test]
        fn the_walk_from_the_edits_finds_a_loop_when_the_walk_from_every_node_does(
            parents in prop::collection::btree_map(0u8..14, prop::option::of(0u8..16), 0..14),
            edited in prop::collection::btree_set(0u8..16, 0..6),
            reset in prop::collection::btree_map(0u8..14, prop::option::of(0u8..16), 0..3),
        ) {
            let nodes = Nodes::new(parents.iter().map(|(&k, &p)| node(k, p, "n")).collect(), None);
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
