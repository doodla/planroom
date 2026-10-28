//! The fold's pre-pass (architecture.md, "The fold", step 3): which events an applied `undo`
//! reverses and the latest review of each event; and, once the fold has applied every event,
//! the authored revisions undos write.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use planroom_contract::{Event, EventBody, EventId, Seq, Status, Target, Verdict};

use super::lookup::{Lookup, LookupSet};
use super::state::{FieldKey, Indexed, Rev, Setting, Writer, edit_keys, idem_stem};

/// What the pre-pass found in the events up to `at`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Prepass {
    /// Every event an effective applied `undo` reverses.
    pub undone: LookupSet<EventId>,
    /// The latest review of each event, from reviews no undo reversed.
    pub reviews: Lookup<EventId, Verdict>,
    /// Every review of each event that wasn't stored as a conflict, undone or not, in `seq`
    /// order: what a review that stands would be once one more undo applies.
    pub review_log: Lookup<EventId, Vec<(EventId, Verdict)>>,
    /// The revision each applied undo wrote on every field it changed back (DR-61), from
    /// [`undo_revisions`] once the fold has applied every event.
    pub undo_revs: BTreeMap<FieldKey, Rev>,
    /// Each applied undo and the events it reverses, in `seq` order.
    pub steps: Vec<(EventId, Vec<EventId>)>,
    /// Each edit whose vanished node the fold followed through a rename, with the moves (DR-74).
    pub followed: super::renames::Followed,
}

impl Prepass {
    /// Whether the card event `id` is open: no applied undo reverses it and no review that stands
    /// names it (DR-89). The fold and [`super::open_cards`] both decide by this one rule, so a
    /// review an undo reversed reopens its card in both.
    pub(crate) fn card_open(&self, id: &EventId) -> bool {
        card_open(self.undone.contains(id), self.reviews.contains_key(id))
    }

    /// The events undone once one more undo, later than every event the fold saw, reverses
    /// `reverses`: they, plus what each earlier undo reverses unless it is undone itself.
    pub(crate) fn undone_after(&self, reverses: &[EventId]) -> BTreeSet<EventId> {
        let mut undone: BTreeSet<EventId> = reverses.iter().cloned().collect();
        for (undo, reversed) in self.steps.iter().rev() {
            if !undone.contains(undo) {
                undone.extend(reversed.iter().cloned());
            }
        }
        undone
    }
}

/// The one rule for whether a card is open, for the fold, the store's read and an undo's plan
/// alike: no applied undo reverses it and no review that stands names it (DR-89).
pub(crate) fn card_open(undone: bool, reviewed: bool) -> bool {
    !undone && !reviewed
}

/// One applied `undo`, and the events of the command it reverses (DR-61): every event that
/// shares the named event's key stem, `idem#1` to `idem#n`.
struct UndoStep<'a> {
    event: &'a Event,
    reverses: Vec<&'a Event>,
}

/// The applied undos among a fold's events, found once for both the pre-pass and the revisions
/// undos write.
pub(crate) struct Undos<'a> {
    /// Each applied `undo`, in `seq` order, with what it reverses.
    steps: Vec<UndoStep<'a>>,
    /// The events by ID, the later of two with one ID.
    by_id: HashMap<&'a EventId, &'a Event>,
}

impl<'a> Undos<'a> {
    /// The applied undos in `events`, which are sorted by `seq`.
    pub(crate) fn of(events: &[&'a Event]) -> Undos<'a> {
        let by_id: HashMap<&EventId, &Event> = events.iter().map(|e| (&e.id, *e)).collect();
        let steps = undo_steps(events, &by_id);
        Undos { steps, by_id }
    }
}

/// Every applied `undo` in `events`, which are sorted by `seq` and indexed by ID in `by_id`, with
/// what it reverses.
fn undo_steps<'a>(events: &[&'a Event], by_id: &HashMap<&'a EventId, &'a Event>) -> Vec<UndoStep<'a>> {
    fn applied_undo<'a>(e: &&'a Event) -> Option<&'a EventId> {
        match &e.body {
            EventBody::Undo { of } if e.status == Status::Applied => Some(of),
            _ => None,
        }
    }
    // Only the commands an undo names are gathered.
    let named: HashSet<&str> = events
        .iter()
        .filter_map(applied_undo)
        .filter_map(|of| by_id.get(of))
        .map(|e| idem_stem(&e.idem))
        .collect();
    let mut stems: HashMap<&str, Vec<&Event>> = HashMap::new();
    if !named.is_empty() {
        for event in events {
            let stem = idem_stem(&event.idem);
            if named.contains(stem) {
                stems.entry(stem).or_default().push(event);
            }
        }
    }
    events
        .iter()
        .filter(|e| e.status == Status::Applied)
        .filter_map(|e| match &e.body {
            EventBody::Undo { of } => {
                let reverses = by_id
                    .get(of)
                    .and_then(|named| stems.get(idem_stem(&named.idem)))
                    .map(|members| members.iter().copied().filter(|m| m.seq < e.seq).collect())
                    .unwrap_or_default();
                Some(UndoStep { event: e, reverses })
            }
            _ => None,
        })
        .collect()
}

/// The events undone at `at`: a later undo reverses its command unless an undo reverses it.
fn undone_as_of(steps: &[UndoStep<'_>], at: Seq) -> BTreeSet<EventId> {
    let mut undone = BTreeSet::new();
    for step in steps.iter().rev().filter(|u| u.event.seq <= at) {
        if undone.contains(&step.event.id) {
            continue;
        }
        undone.extend(step.reverses.iter().map(|e| e.id.clone()));
    }
    undone
}

/// The pre-pass over `events`, which are sorted by `seq` and all at or below the fold's `at`.
/// Its undo revisions stay empty until [`undo_revisions`] fills them once the fold has applied
/// every event.
pub(crate) fn prepass(events: &[&Event], undos: &Undos<'_>) -> Prepass {
    let last = events.last().map_or(0, |e| e.seq);
    let undone: LookupSet<EventId> = undone_as_of(&undos.steps, last).into_iter().collect();

    let mut reviews = Lookup::default();
    let mut review_log: Lookup<EventId, Vec<(EventId, Verdict)>> = Lookup::default();
    for event in events {
        if let EventBody::Review { of, verdict } = &event.body
            && event.status != Status::Conflict
        {
            review_log.entry(of.clone()).or_default().push((event.id.clone(), *verdict));
            if !undone.contains(&event.id) {
                reviews.insert(of.clone(), *verdict);
            }
        }
    }
    let steps = undos
        .steps
        .iter()
        .map(|u| (u.event.id.clone(), u.reverses.iter().map(|e| e.id.clone()).collect()))
        .collect();
    Prepass { undone, reviews, review_log, undo_revs: BTreeMap::new(), steps, followed: Default::default() }
}

/// The revision each applied undo in `events` writes on every field it changes back (DR-61).
/// `index` holds, for each applied edit, the mask of its keys the fold set (`Indexed::set`): a
/// field an edit names but the fold skipped, such as a move that lost to a loop, a set on a node
/// that doesn't exist or a value the field can't take, held nothing, so an undo changes nothing
/// back in it and writes no revision there.
///
/// The undos are taken in `seq` order, and [`Undone`] keeps which events are undone as each one
/// lands, so the cost grows with what each undo changes, not with every undo before it.
pub(crate) fn undo_revisions(
    events: &[&Event],
    undos: &Undos<'_>,
    index: &Lookup<Arc<str>, Indexed>,
) -> BTreeMap<FieldKey, Rev> {
    let set_keys = |event: &Event, edit: &planroom_contract::Edit| -> Vec<FieldKey> {
        let Some(set) = index.get(event.id.0.as_str()).and_then(|e| e.set.as_ref()) else {
            return Vec::new();
        };
        edit_keys(edit).into_iter().enumerate().filter(|(i, _)| set.has(*i)).map(|(_, k)| k).collect()
    };
    // An undo changes back only fields of the edits some undo reverses, so only the targets of
    // those edits are read below.
    let reversed: HashSet<&Target> = undos
        .steps
        .iter()
        .flat_map(|step| &step.reverses)
        .filter_map(|event| match &event.body {
            EventBody::Edit(edit) => Some(&edit.target),
            _ => None,
        })
        .collect();
    // Every applied edit's settings of the fields it set on those targets, for the value a field
    // held either side of an undo.
    let mut settings: BTreeMap<FieldKey, Vec<(Seq, &EventId, Setting)>> = BTreeMap::new();
    for event in events {
        if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status)
            && reversed.contains(&edit.target)
        {
            for key in set_keys(event, edit) {
                let setting = Setting::of(edit, &key.1);
                settings.entry(key).or_default().push((event.seq, &event.id, setting));
            }
        }
    }
    let mut undone = Undone::new(&undos.steps);
    let mut undo_revs: BTreeMap<FieldKey, Rev> = BTreeMap::new();
    for (k, step) in undos.steps.iter().enumerate() {
        // The events this undo's landing undoes or brings back.
        let toggled = undone.land(k);
        let mut keys: BTreeSet<FieldKey> = BTreeSet::new();
        for id in &toggled {
            if let Some(event @ Event { body: EventBody::Edit(edit), status: Status::Applied, .. }) =
                undos.by_id.get(id).copied()
            {
                keys.extend(set_keys(event, edit));
            }
        }
        for key in keys {
            // Whether an event was undone just before this undo, and just after it.
            let after = |id: &EventId| undone.contains(id);
            let before = |id: &EventId| undone.contains(id) != toggled.contains(id);
            let value = |is_undone: &dyn Fn(&EventId) -> bool| {
                settings.get(&key).and_then(|all| {
                    all.iter()
                        .rev()
                        .find(|(seq, id, _)| *seq < step.event.seq && !is_undone(id))
                        .map(|(_, _, setting)| setting)
                })
            };
            if value(&before) == value(&after) {
                continue;
            }
            let rev =
                Rev { seq: step.event.seq, event: step.event.id.clone(), writer: Writer::of(step.event) };
            match undo_revs.get(&key) {
                Some(held) if held.seq > rev.seq => {}
                _ => {
                    undo_revs.insert(key, rev);
                }
            }
        }
    }
    undo_revs
}

/// Which events are undone as the undos land one by one in `seq` order: what [`undone_as_of`]
/// gives at each undo's `seq`, kept up to date instead of recomputed.
///
/// An event is undone while at least one effective undo reverses it, and an undo is effective
/// while none reverses it. Every undo reverses only earlier events, so when one lands, the change
/// it makes runs backward through the events it reverses: each event's count of effective undos
/// that reverse it is settled once, latest first, after every later change to it.
struct Undone<'a> {
    steps: &'a [UndoStep<'a>],
    /// Each undo step's index, by its event.
    step_of: HashMap<&'a EventId, usize>,
    /// The effective undos that reverse each event; an event is undone while its count is above 0.
    count: HashMap<&'a EventId, usize>,
}

impl<'a> Undone<'a> {
    fn new(steps: &'a [UndoStep<'a>]) -> Undone<'a> {
        let step_of = steps.iter().enumerate().map(|(i, s)| (&s.event.id, i)).collect();
        Undone { steps, step_of, count: HashMap::new() }
    }

    fn contains(&self, id: &EventId) -> bool {
        self.count.get(id).is_some_and(|c| *c > 0)
    }

    /// Land step `k`, the latest so far, and return the events it undid or brought back.
    fn land(&mut self, k: usize) -> BTreeSet<EventId> {
        // Pending changes to each event's count, by its seq, settled latest first.
        let mut pending: BTreeMap<Seq, (&'a EventId, i64)> = BTreeMap::new();
        let add = |pending: &mut BTreeMap<Seq, (&'a EventId, i64)>, step: &UndoStep<'a>, delta: i64| {
            for event in &step.reverses {
                pending.entry(event.seq).or_insert((&event.id, 0)).1 += delta;
            }
        };
        // The newest undo is effective: nothing later reverses it.
        add(&mut pending, &self.steps[k], 1);
        let mut toggled = BTreeSet::new();
        while let Some((_, (id, delta))) = pending.pop_last() {
            let old = self.count.get(id).copied().unwrap_or(0);
            let new = (old as i64 + delta).max(0) as usize;
            self.count.insert(id, new);
            if (old > 0) == (new > 0) {
                continue;
            }
            toggled.insert(id.clone());
            // An undo that became undone stops reversing its events, and one brought back
            // reverses them again.
            if let Some(&j) = self.step_of.get(id) {
                add(&mut pending, &self.steps[j], if new > 0 { -1 } else { 1 });
            }
        }
        toggled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Actor, Client, Edit, Layer, NodeId, Op, Surface, Target, Timestamp};
    use proptest::prelude::*;
    use proptest::test_runner::Config;

    /// Events from `plan`: each entry is an edit (`None`) or an undo of an earlier event; every
    /// third event shares its command with the one before it.
    fn events(plan: &[Option<usize>]) -> Vec<Event> {
        let mut out: Vec<Event> = Vec::new();
        for (i, step) in plan.iter().enumerate() {
            let seq = i as Seq + 1;
            let body = match step {
                Some(of) if i > 0 => EventBody::Undo { of: out[of % i].id.clone() },
                _ => EventBody::Edit(Edit {
                    op: Op::Set,
                    target: Target::Node(NodeId(format!("r:x:{}", i % 3))),
                    layer: Layer::Actual,
                    fields: BTreeMap::from([("name".to_owned(), serde_json::json!(format!("n{i}")))]),
                    base: 0,
                    over: BTreeMap::new(),
                }),
            };
            let idem = if i % 3 == 2 { format!("k{}#2", i - 1) } else { format!("k{i}#1") };
            out.push(Event {
                seq,
                id: EventId(format!("ev-{seq}")),
                idem,
                request_hash: "h".into(),
                at: Timestamp("2026-10-26T12:00:00Z".into()),
                actor: Actor::Person { id: "person:ann".into(), name: None },
                client: Client { surface: Surface::Cli, id: "cli:1".into() },
                project: None,
                cause: None,
                status: Status::Applied,
                body,
            });
        }
        out
    }

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        /// Landing the undos one by one gives, at each, what recomputing from every undo up to
        /// it gives, and the events it toggles are the difference from just before it.
        #[test]
        fn undos_landed_one_by_one_match_a_recomputation(
            plan in prop::collection::vec(prop::option::of(0usize..64), 1..48),
        ) {
            let events = events(&plan);
            let refs: Vec<&Event> = events.iter().collect();
            let steps = Undos::of(&refs).steps;
            let mut undone = Undone::new(&steps);
            for (k, step) in steps.iter().enumerate() {
                let toggled = undone.land(k);
                let before = undone_as_of(&steps, step.event.seq - 1);
                let after = undone_as_of(&steps, step.event.seq);
                for event in &events {
                    prop_assert_eq!(undone.contains(&event.id), after.contains(&event.id), "{}", event.id.0);
                }
                let want: BTreeSet<EventId> = before.symmetric_difference(&after).cloned().collect();
                prop_assert_eq!(toggled, want);
            }
        }
    }
}
