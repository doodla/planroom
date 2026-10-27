//! The fold (architecture.md, "The fold"): the model at `at`, from the active snapshots' facts and
//! the events, pure and deterministic, so `fold(at = N)` is reproducible.
//!
//! It resolves the facts, resolves each edit whose node is gone through the renames later `scan`
//! events recorded ([`renames`], DR-74), runs the pre-pass (undone events and the latest review of
//! each event), then applies every event up to `at` in `seq` order:
//! - an applied `edit` goes into the model: `put` inserts, `set` overrides fields, `retract`
//!   removes, and overrides are re-applied over the facts on every fold, so a rescan can't drop
//!   them;
//! - a ghost, an agent's or a model's edit, goes into the overlay until a person reviews it or a
//!   person's edit to one of its fields supersedes it;
//! - a stored conflict, a version tag and a claim are kept as listed; a card is open while no
//!   review names it, and its body is never read, as an unknown kind's isn't.
//!
//! Then parents that don't resolve fall back to their fact parent, a parent loop an older store
//! or an import holds is broken by `seq`, and every override whose fact changed under it is
//! listed (DR-5, DR-7, DR-61, DR-73). Each applied undo writes its revision on every field it
//! changes back, among those the edits it reverses set. Claims stay out of the model: [`claims::active`] applies
//! the clock to them. A `put` or `set` names only fields of the four shapes and the override
//! fields `parent`, `name`, `hidden`, `pos@VIEW` and `props.KEY`.

pub mod claims;
pub mod cycle;
mod prepass;
mod renames;
mod state;

use std::collections::BTreeSet;

use planroom_contract::{Event, EventBody, EventId, NodeId, Seq, Target, Verdict, Vocabulary};

use crate::model::{Facts, Model};
use crate::resolve::{Resolved, resolve};
use prepass::Prepass;
use state::{Core, ParentEntry};

pub use claims::{ActiveClaim, ClaimRecord};
pub use state::{
    FieldKey, FoldDiff, Ghost, HIDDEN, NAME, Overlay, PARENT, POS_PREFIX, PROPS_PREFIX, Rev, StoredConflict,
    TargetKey, VersionTag, WHOLE, Writer, actor_key, edit_keys, overlaps,
};

pub(crate) use state::{build_edge, build_node};

/// The model at `at`, from `facts`, the `events` up to `at`, and the loaded packs' `vocabulary`.
pub fn fold(facts: &Facts, events: &[Event], vocabulary: &Vocabulary, at: Seq) -> Model {
    fold_full(facts, events, vocabulary, at).model
}

/// Everything a fold derives: the model, the overlay beside it, what Diffs lists, and the
/// authored revisions compare-and-set reads. Nothing stores it (DR-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Folded {
    pub model: Model,
    pub overlay: Overlay,
    /// In the order the fold met them: while applying, then at the end.
    pub diffs: Vec<FoldDiff>,
    pub conflicts: Vec<StoredConflict>,
    /// The `card` events no review names, in `seq` order (DR-89).
    pub open_cards: Vec<EventId>,
    pub versions: Vec<VersionTag>,
    /// Every applied claim, for [`claims::active`] to read with the clock.
    pub claims: Vec<ClaimRecord>,
    core: Core,
    prepass: Prepass,
}

/// The fold at `at`, with everything it derives. Events above `at` are left out, in any order.
pub fn fold_full(facts: &Facts, events: &[Event], vocabulary: &Vocabulary, at: Seq) -> Folded {
    fold_resolved(resolve(facts, vocabulary), events, at)
}

/// The fold at `at` over facts already resolved.
fn fold_resolved(resolved: Resolved, events: &[Event], at: Seq) -> Folded {
    let mut up_to: Vec<&Event> = events.iter().filter(|e| e.seq <= at).collect();
    up_to.sort_by_key(|e| e.seq);
    // An edit whose node is gone follows the renames later scans recorded (DR-74).
    let (followed_events, followed) = renames::follow(&up_to, &resolved.nodes);
    let up_to: Vec<&Event> = followed_events.iter().map(|e| e.as_ref()).collect();
    let undos = prepass::Undos::of(&up_to);
    let mut prepass = prepass::prepass(&up_to, &undos);
    prepass.followed = followed;
    let mut core = Core::new(resolved);
    for event in &up_to {
        core.apply(event, &prepass);
    }
    prepass.undo_revs = prepass::undo_revisions(&up_to, &undos, &core.sets);
    core.at = at;
    finished(core, prepass)
}

/// The open cards among `events`, in `seq` order: the `card` events that no review standing names
/// and no applied undo reverses (DR-89). It decides as the fold does, through the same pre-pass,
/// so a store that reads only the events that bear on cards (every `card`, `undo` and `review` of
/// a card, and each event an undo names, for the command it reverses) gets the fold's answer.
pub fn open_cards(events: &[Event]) -> Vec<EventId> {
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|e| e.seq);
    let undos = prepass::Undos::of(&sorted);
    let prepass = prepass::prepass(&sorted, &undos);
    sorted
        .into_iter()
        .filter(|e| matches!(e.body, EventBody::Card(_)) && prepass.card_open(&e.id))
        .map(|e| e.id.clone())
        .collect()
}

/// Whether applying `event` to a kept fold needs a fold from scratch: a scan changes the
/// snapshot set, and an undo or a review changes the status of edits already applied (DR-61).
pub fn refolds(event: &Event) -> bool {
    matches!(event.body, EventBody::Scan { .. } | EventBody::Undo { .. } | EventBody::Review { .. })
}

/// Apply one event to a kept fold, as `serve` and `watch` do (DR-61). `history` holds the events
/// the fold already saw, and `facts` the active snapshots' facts once `event` is applied. An event
/// that [`refolds`] folds from scratch; any other, the next in `seq` order, is applied to the kept
/// state. Either way the result equals [`fold_full`] at `event.seq`. An event the fold already
/// applied changes nothing.
pub fn fold_one(
    folded: &mut Folded,
    facts: &Facts,
    history: &[Event],
    vocabulary: &Vocabulary,
    event: &Event,
) {
    if folded.core.index.contains_key(&event.id) {
        return;
    }
    if refolds(event) || event.seq <= folded.core.at {
        let mut all: Vec<Event> = history.iter().filter(|e| e.id != event.id).cloned().collect();
        all.push(event.clone());
        *folded = fold_full(facts, &all, vocabulary, event.seq.max(folded.core.at));
        return;
    }
    let mut core = std::mem::replace(&mut folded.core, Core::new(Default::default()));
    core.apply(event, &folded.prepass);
    core.at = event.seq;
    let prepass = std::mem::take(&mut folded.prepass);
    *folded = finished(core, prepass);
}

/// The fold's reviews, cards and ghosts as they would stand once one more applied undo reverses
/// one command, read by the same rules as the fold's ([`Folded::after_undo`]).
#[derive(Debug, Clone)]
pub struct AfterUndo<'a> {
    folded: &'a Folded,
    undone: BTreeSet<EventId>,
}

impl AfterUndo<'_> {
    /// Whether an applied undo would reverse `event`.
    pub fn is_undone(&self, event: &EventId) -> bool {
        self.undone.contains(event)
    }

    /// The reviews of `event` that would stand, in `seq` order.
    pub fn reviews_of(&self, event: &EventId) -> Vec<EventId> {
        let log = self.folded.prepass.review_log.get(event).into_iter().flatten();
        log.filter(|(id, _)| !self.undone.contains(id)).map(|(id, _)| id.clone()).collect()
    }

    /// The latest review of `event` that would stand.
    pub fn verdict(&self, event: &EventId) -> Option<Verdict> {
        let log = self.folded.prepass.review_log.get(event)?;
        log.iter().rev().find(|(id, _)| !self.undone.contains(id)).map(|(_, verdict)| *verdict)
    }

    /// The ghosts `cause` caused that no undo would reverse, reviewed or not, in `seq` order.
    pub fn ghosts_of(&self, cause: &EventId) -> Vec<EventId> {
        let caused = self.folded.core.caused.get(cause).into_iter().flatten();
        caused.filter(|id| !self.undone.contains(*id)).cloned().collect()
    }

    /// Whether the card event `card` would be open: no undo reverses it and no review that stands
    /// names it, the rule the fold decides by (DR-89).
    pub fn card_open(&self, card: &EventId) -> bool {
        prepass::card_open(self.is_undone(card), !self.reviews_of(card).is_empty())
    }
}

fn finished(core: Core, prepass: Prepass) -> Folded {
    let done = core.finish();
    Folded {
        model: done.model,
        overlay: done.overlay,
        diffs: done.diffs,
        conflicts: done.conflicts,
        open_cards: done.open_cards,
        versions: done.versions,
        claims: done.claims,
        core,
        prepass,
    }
}

impl Folded {
    /// The `seq` the fold is at.
    pub fn at(&self) -> Seq {
        self.core.at
    }

    /// The authored revision of one field of `target` (`WHOLE` for the target itself): the
    /// later of the last effective applied edit that set it and the last undo that changed it
    /// back. Scans never write one (DR-5).
    pub fn revision(&self, target: &Target, field: &str) -> Option<Rev> {
        self.revision_of(&(TargetKey::from(target), field.to_owned()))
    }

    pub(crate) fn revision_of(&self, key: &FieldKey) -> Option<Rev> {
        let set = self.core.last_set(key).map(|s| Rev {
            seq: s.seq,
            event: s.event.clone(),
            writer: s.writer.clone(),
        });
        let undo = self.prepass.undo_revs.get(key).cloned();
        match (set, undo) {
            (Some(a), Some(b)) => Some(if b.seq > a.seq { b } else { a }),
            (a, b) => a.or(b),
        }
    }

    /// Every field of `target` that has an authored revision, with it, in field order.
    pub fn revisions(&self, target: &Target) -> Vec<(String, Rev)> {
        let key = TargetKey::from(target);
        let mut fields: Vec<&String> = self
            .core
            .setters
            .keys()
            .chain(self.prepass.undo_revs.keys())
            .filter(|(t, _)| *t == key)
            .map(|(_, f)| f)
            .collect();
        fields.sort();
        fields.dedup();
        fields
            .into_iter()
            .filter_map(|f| self.revision_of(&(key.clone(), f.clone())).map(|rev| (f.clone(), rev)))
            .collect()
    }

    /// Whether a person holds one field of `target`: its current value comes from a person's
    /// effective edit, or a person put or retracted the whole target since. For `WHOLE`, whether a
    /// person holds any of its fields. An undo of the person's edit releases it (DR-73).
    pub fn held_by_person(&self, target: &Target, field: &str) -> bool {
        let key = TargetKey::from(target);
        if field == WHOLE {
            return self.core.setters.keys().filter(|(t, _)| *t == key).any(|(_, f)| self.holds(&key, f));
        }
        self.holds(&key, field)
    }

    fn holds(&self, target: &TargetKey, field: &str) -> bool {
        let last = |f: &str| self.core.last_set(&(target.clone(), f.to_owned()));
        let latest = match (last(field), last(WHOLE)) {
            (Some(a), Some(b)) => Some(if b.seq > a.seq { b } else { a }),
            (a, b) => a.or(b),
        };
        latest.is_some_and(|s| s.writer.is_person())
    }

    /// The events of the command that wrote `event`: every event sharing its key stem, in `seq`
    /// order; empty when the fold never saw `event`.
    pub fn command_of(&self, event: &EventId) -> Vec<EventId> {
        let Some(found) = self.core.index.get(event) else { return Vec::new() };
        let mut members: Vec<(Seq, EventId)> = self
            .core
            .stems
            .get(state::idem_stem(&found.idem))
            .into_iter()
            .flatten()
            .filter_map(|id| self.core.index.get(id).map(|e| (e.seq, id.clone())))
            .collect();
        members.sort();
        members.into_iter().map(|(_, id)| id).collect()
    }

    /// The fields an undo of `event`'s command changes back: every field its edits set, and for
    /// an undo among them, the fields of the command that undo reversed.
    pub fn undo_keys(&self, event: &EventId) -> Vec<FieldKey> {
        let mut keys = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut todo = vec![event.clone()];
        while let Some(next) = todo.pop() {
            for member in self.command_of(&next) {
                if !seen.insert(member.clone()) {
                    continue;
                }
                let Some(found) = self.core.index.get(&member) else { continue };
                keys.extend(found.keys.iter().cloned());
                if let Some(of) = &found.undo_of {
                    todo.push(of.clone());
                }
            }
        }
        keys.sort();
        keys.dedup();
        keys
    }

    /// Where an undo of `event`'s command would put each node whose parent it changes: the
    /// parent the last edit left in effect gives, or the fact parent when none is; `None` for a
    /// node it would leave out of the model. In node order (DR-61).
    pub fn parents_after_undo(&self, event: &EventId) -> Vec<(NodeId, Option<Option<NodeId>>)> {
        let after = self.prepass.undone_after(&self.command_of(event));
        let toggled: BTreeSet<&EventId> = self.prepass.undone.symmetric_difference(&after).collect();
        let mut out = Vec::new();
        for (node, log) in &self.core.parent_log {
            if !log.iter().any(|(_, e, _)| toggled.contains(e)) {
                continue;
            }
            let parent = match log.iter().rev().find(|(_, e, _)| !after.contains(e)) {
                Some((_, _, ParentEntry::Under(parent))) => Some(parent.clone()),
                Some((_, _, ParentEntry::Removed)) => None,
                None => self.core.fact_parent(node),
            };
            out.push((node.clone(), parent));
        }
        out
    }

    /// The latest review of `event` that stands: no undo reversed it.
    pub fn verdict(&self, event: &EventId) -> Option<Verdict> {
        self.prepass.reviews.get(event).copied()
    }

    /// The ghosts `cause` caused, such as a card's, that no undo reversed, reviewed or not, in
    /// `seq` order.
    pub fn ghosts_of(&self, cause: &EventId) -> Vec<EventId> {
        self.core
            .caused
            .get(cause)
            .into_iter()
            .flatten()
            .filter(|id| !self.prepass.undone.contains(*id))
            .cloned()
            .collect()
    }

    /// Every `card` event the fold saw, open or not, in `seq` order.
    pub fn cards(&self) -> &[EventId] {
        &self.core.cards
    }

    /// The reviews, cards and ghosts as they would stand once one more applied undo reverses the
    /// command that wrote `event` (DR-61): what an undo has to close or reopen in its own write.
    pub fn after_undo(&self, event: &EventId) -> AfterUndo<'_> {
        AfterUndo { folded: self, undone: self.prepass.undone_after(&self.command_of(event)) }
    }

    /// Whether an applied undo reverses `event`.
    pub fn is_undone(&self, event: &EventId) -> bool {
        self.prepass.undone.contains(event)
    }

    /// The fact's own value under one field of `target`, before any override: what an edit
    /// records in its `over`, so the fold can tell when a later scan changed the fact under it.
    /// `None` when the target isn't a fact or the field has no fact counterpart, such as `hidden`
    /// or a pinned position.
    pub fn fact_value(&self, target: &Target, field: &str) -> Option<serde_json::Value> {
        self.core.fact_value(&TargetKey::from(target), field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Kind, Layer, Level, Node, NodeId, Provenance, Tier};
    use proptest::prelude::*;
    use proptest::test_runner::Config;

    const TIERS: [Tier; 7] = [
        Tier::Precise,
        Tier::Syntactic,
        Tier::Config,
        Tier::Pattern,
        Tier::Linked,
        Tier::Inference,
        Tier::Universal,
    ];

    fn node(key: u8, tier: usize) -> Node {
        Node {
            id: NodeId(format!("r:x:{key}")),
            kind: Kind("k.x".into()),
            level: Level::Code,
            parent: None,
            name: format!("n{key}"),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: TIERS[tier] },
            evidence: vec![],
        }
    }

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        #[test]
        fn a_folded_model_holds_each_fact_node_once_in_id_order(
            facts in prop::collection::vec((0u8..6, 0usize..TIERS.len()), 0..24),
            at in 0u64..1_000,
        ) {
            let nodes: Vec<Node> = facts.iter().map(|&(key, tier)| node(key, tier)).collect();
            let model = fold(&Facts { nodes: nodes.clone(), ..Facts::default() }, &[], &Vocabulary::default(), at);
            let ids: Vec<&str> = model.nodes.iter().map(|n| n.id.0.as_str()).collect();
            let mut want: Vec<&str> = nodes.iter().map(|n| n.id.0.as_str()).collect();
            want.sort_unstable();
            want.dedup();
            prop_assert_eq!(ids, want);
            prop_assert_eq!(model.at, at);
        }
    }

    #[test]
    fn a_fold_with_no_events_is_its_resolved_facts_at_its_seq() {
        let facts = Facts::default();
        let vocabulary = Vocabulary::default();
        let model = fold(&facts, &[], &vocabulary, 7);
        let resolved = resolve(&facts, &vocabulary);
        assert_eq!(
            model,
            Model { at: 7, nodes: resolved.nodes, edges: resolved.edges, unresolved: resolved.unresolved }
        );
    }

    #[test]
    fn edges_set_aside_as_unresolved_survive_the_fold() {
        use planroom_contract::{
            Actor, Client, Edge, EdgeId, Edit, Event, EventBody, EventId, Op, Ref, Status, Surface, Target,
            Timestamp,
        };
        let unresolved = Edge {
            id: EdgeId("e:000000000001".into()),
            kind: Kind("k.rel".into()),
            from: Ref::Id(NodeId("r:x:1".into())),
            to: Ref::Alias("host:nowhere".into()),
            via: None,
            via_ref: None,
            props: Default::default(),
            evidence: vec![],
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Pattern },
        };
        let resolved =
            Resolved { nodes: vec![node(1, 0)], edges: vec![], unresolved: vec![unresolved.clone()] };
        let mut folded = fold_resolved(resolved, &[], 0);
        assert_eq!(folded.model.unresolved, std::slice::from_ref(&unresolved));
        assert!(folded.model.edges.is_empty(), "an unresolved edge stays out of the model's edges");

        // An event applied to the kept fold keeps them too.
        let rename = Event {
            seq: 1,
            id: EventId("ev-1".into()),
            idem: "k1".into(),
            request_hash: "h".into(),
            at: Timestamp("2026-10-26T12:00:00.000000Z".into()),
            actor: Actor::Person { id: "person:ann".into(), name: None },
            client: Client { surface: Surface::Canvas, id: "tab_1".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            body: EventBody::Edit(Edit {
                op: Op::Set,
                target: Target::Node(NodeId("r:x:1".into())),
                layer: Layer::Actual,
                fields: [("name".to_owned(), serde_json::json!("one"))].into(),
                base: 0,
                over: Default::default(),
            }),
        };
        fold_one(&mut folded, &Facts::default(), &[], &Vocabulary::default(), &rename);
        assert_eq!(folded.model.nodes[0].name, "one");
        assert_eq!(folded.model.unresolved, std::slice::from_ref(&unresolved));
    }
}
