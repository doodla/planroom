//! Renames git reported, carried through `scan` events (DR-74): an edit whose node is gone from
//! the facts follows it to where git says it moved, so an override keeps its file or directory.
//!
//! Before the fold applies anything, each edit naming a node the facts don't hold is resolved
//! through the renames of every later `scan` event, in `seq` order, one step per event: its
//! target, the parent it sets and the parent it was made over. An edit whose node the facts hold
//! is left as it is, and so is one no rename reaches, which the fold then lists as vanished.

use std::borrow::Cow;
use std::collections::BTreeMap;

use planroom_contract::{Event, EventBody, EventId, Node, NodeId, Seq, Target};
use serde_json::Value;

use super::state::PARENT;

/// Each edit the fold resolved through a rename, with every move it followed: old ID, new ID.
pub(crate) type Followed = BTreeMap<EventId, Vec<(NodeId, NodeId)>>;

/// The renames of each `scan` event in `events` that records any, in `seq` order.
struct Moves<'a>(Vec<(Seq, BTreeMap<&'a NodeId, &'a NodeId>)>);

impl<'a> Moves<'a> {
    fn of(events: &[&'a Event]) -> Moves<'a> {
        let moves = events
            .iter()
            .filter_map(|e| match &e.body {
                EventBody::Scan { renames, .. } if !renames.is_empty() => {
                    Some((e.seq, renames.iter().map(|r| (&r.from, &r.to)).collect()))
                }
                _ => None,
            })
            .collect();
        Moves(moves)
    }

    /// Where `id` is once every rename recorded after `seq` has applied: `None` when none moves it.
    fn follow(&self, id: &NodeId, seq: Seq) -> Option<NodeId> {
        let start = self.0.partition_point(|(s, _)| *s <= seq);
        let mut at = id;
        for (_, renames) in &self.0[start..] {
            if let Some(to) = renames.get(at) {
                at = to;
            }
        }
        (at != id).then(|| at.clone())
    }
}

/// `events`, sorted by `seq`, with each edit that names a node `nodes` doesn't hold resolved
/// through the renames recorded after it; and what each resolved edit followed. `nodes` is sorted
/// by ID, as resolving leaves it, so a fold with no recorded rename pays for nothing here.
pub(crate) fn follow<'a>(events: &[&'a Event], nodes: &[Node]) -> (Vec<Cow<'a, Event>>, Followed) {
    let moves = Moves::of(events);
    let mut followed = Followed::new();
    if moves.0.is_empty() {
        return (events.iter().map(|e| Cow::Borrowed(*e)).collect(), followed);
    }
    let out = events
        .iter()
        .map(|event| {
            let EventBody::Edit(edit) = &event.body else { return Cow::Borrowed(*event) };
            let resolve = |id: &NodeId| -> Option<NodeId> {
                if held(nodes, id) { None } else { moves.follow(id, event.seq) }
            };
            let mut moved: Vec<(NodeId, NodeId)> = Vec::new();
            let target = match &edit.target {
                Target::Node(id) => resolve(id).inspect(|to| moved.push((id.clone(), to.clone()))),
                Target::Edge(_) => None,
            };
            let parent = parent_of(edit.fields.get(PARENT))
                .and_then(|p| resolve(&p).inspect(|to| moved.push((p.clone(), to.clone()))));
            // The parent the edit was made over follows too, so a moved parent isn't drift; it
            // isn't listed, since the edit didn't name it.
            let over = parent_of(edit.over.get(PARENT)).and_then(|p| resolve(&p));
            if target.is_none() && parent.is_none() && over.is_none() {
                return Cow::Borrowed(*event);
            }
            let mut event = (*event).clone();
            let EventBody::Edit(edit) = &mut event.body else { unreachable!("an edit") };
            if let Some(target) = target {
                edit.target = Target::Node(target);
            }
            if let Some(parent) = parent {
                edit.fields.insert(PARENT.to_owned(), Value::String(parent.0));
            }
            if let Some(over) = over {
                edit.over.insert(PARENT.to_owned(), Value::String(over.0));
            }
            if !moved.is_empty() {
                moved.sort();
                moved.dedup();
                followed.insert(event.id.clone(), moved);
            }
            Cow::Owned(event)
        })
        .collect();
    (out, followed)
}

/// Whether `nodes`, sorted by ID, holds `id`.
fn held(nodes: &[Node], id: &NodeId) -> bool {
    nodes.binary_search_by(|n| n.id.cmp(id)).is_ok()
}

fn parent_of(value: Option<&Value>) -> Option<NodeId> {
    value.and_then(Value::as_str).map(|p| NodeId(p.to_owned()))
}
