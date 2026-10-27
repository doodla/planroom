//! The fold's pre-pass (architecture.md, "The fold", step 3): which events an applied `undo`
//! reverses and the latest review of each event; and, once the fold has applied every event,
//! the authored revisions undos write.

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{Event, EventBody, EventId, Seq, Status, Verdict};

use super::state::{FieldKey, Rev, Setting, Writer, edit_keys, idem_stem};

/// What the pre-pass found in the events up to `at`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Prepass {
    /// Every event an effective applied `undo` reverses.
    pub undone: BTreeSet<EventId>,
    /// The latest review of each event, from reviews no undo reversed.
    pub reviews: BTreeMap<EventId, Verdict>,
    /// The revision each applied undo wrote on every field it changed back (DR-61), from
    /// [`undo_revisions`] once the fold has applied every event.
    pub undo_revs: BTreeMap<FieldKey, Rev>,
    /// Each applied undo and the events it reverses, in `seq` order.
    pub steps: Vec<(EventId, Vec<EventId>)>,
}

impl Prepass {
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

/// One applied `undo`, and the events of the command it reverses (DR-61): every event that
/// shares the named event's key stem, `idem#1` to `idem#n`.
struct UndoStep<'a> {
    event: &'a Event,
    reverses: Vec<&'a Event>,
}

/// Every applied `undo` in `events`, which are sorted by `seq`, with what it reverses.
fn undo_steps<'a>(events: &[&'a Event]) -> Vec<UndoStep<'a>> {
    let by_id: BTreeMap<&EventId, &Event> = events.iter().map(|e| (&e.id, *e)).collect();
    let mut stems: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for event in events {
        stems.entry(idem_stem(&event.idem)).or_default().push(event);
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
pub(crate) fn prepass(events: &[&Event]) -> Prepass {
    let undos = undo_steps(events);
    let last = events.last().map_or(0, |e| e.seq);
    let undone = undone_as_of(&undos, last);

    let mut reviews = BTreeMap::new();
    for event in events {
        if let EventBody::Review { of, verdict } = &event.body
            && event.status != Status::Conflict
            && !undone.contains(&event.id)
        {
            reviews.insert(of.clone(), *verdict);
        }
    }
    let steps = undos
        .iter()
        .map(|u| (u.event.id.clone(), u.reverses.iter().map(|e| e.id.clone()).collect()))
        .collect();
    Prepass { undone, reviews, undo_revs: BTreeMap::new(), steps }
}

/// The revision each applied undo in `events` writes on every field it changes back (DR-61).
/// `sets` holds the fields each applied edit set, as the fold found them: a field an edit names
/// but the fold skipped, such as a move that lost to a loop, a set on a node that doesn't exist
/// or a value the field can't take, held nothing, so an undo changes nothing back in it and
/// writes no revision there.
pub(crate) fn undo_revisions(
    events: &[&Event],
    sets: &BTreeMap<EventId, BTreeSet<String>>,
) -> BTreeMap<FieldKey, Rev> {
    let undos = undo_steps(events);
    let set_keys = |event: &Event, edit: &planroom_contract::Edit| -> Vec<FieldKey> {
        let set = sets.get(&event.id);
        edit_keys(edit).into_iter().filter(|(_, f)| set.is_some_and(|s| s.contains(f))).collect()
    };
    let by_id: BTreeMap<&EventId, &Event> = events.iter().map(|e| (&e.id, *e)).collect();

    // Every applied edit's settings of the fields it set, for the value a field held either side
    // of an undo.
    let mut settings: BTreeMap<FieldKey, Vec<(Seq, &EventId, Setting)>> = BTreeMap::new();
    for event in events {
        if let (EventBody::Edit(edit), Status::Applied) = (&event.body, event.status) {
            for key in set_keys(event, edit) {
                let setting = Setting::of(edit, &key.1);
                settings.entry(key).or_default().push((event.seq, &event.id, setting));
            }
        }
    }
    let mut undo_revs: BTreeMap<FieldKey, Rev> = BTreeMap::new();
    for step in &undos {
        let before = undone_as_of(&undos, step.event.seq.saturating_sub(1));
        let after = undone_as_of(&undos, step.event.seq);
        let toggled: BTreeSet<&EventId> = before.symmetric_difference(&after).collect();
        let mut keys: BTreeSet<FieldKey> = BTreeSet::new();
        for id in toggled {
            if let Some(event @ Event { body: EventBody::Edit(edit), status: Status::Applied, .. }) =
                by_id.get(id).copied()
            {
                keys.extend(set_keys(event, edit));
            }
        }
        for key in keys {
            let value = |undone: &BTreeSet<EventId>| {
                settings.get(&key).and_then(|all| {
                    all.iter()
                        .rev()
                        .find(|(seq, id, _)| *seq < step.event.seq && !undone.contains(*id))
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
