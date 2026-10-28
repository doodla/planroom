//! Renames git reported, carried through `scan` events (DR-74): an edit whose node is gone from
//! the facts follows it to where git says it moved, so an override keeps its file or directory.
//!
//! Before the fold applies anything, each edit naming a node the facts don't hold is resolved
//! through the renames of every later `scan` event, in `seq` order, one step per event: its
//! target, the parent it sets and the parent it was made over. An edit whose node the facts hold
//! is left as it is, and so is one no rename reaches, which the fold then lists as vanished.

use std::collections::{BTreeMap, HashMap};

use planroom_contract::{Event, EventBody, EventId, Node, NodeId, Seq, Target};
use serde_json::Value;

use super::lookup::Lookup;
use super::state::PARENT;

/// Each edit the fold resolved through a rename, with every move it followed: old ID, new ID.
pub(crate) type Followed = Lookup<EventId, Vec<(NodeId, NodeId)>>;

/// The renames the `scan` events in `events` record, by the ID each moves from.
struct Moves<'a> {
    /// The `seq` of each scan that records a rename, in `seq` order.
    scans: Vec<Seq>,
    /// For each ID a rename moves, the scans that move it, by their place in `scans` and in that
    /// order, each with the ID it moves to. Of two renames of one ID in one scan, the later.
    from: HashMap<&'a str, Vec<(usize, &'a NodeId)>>,
}

impl<'a> Moves<'a> {
    fn of(events: &[&'a Event]) -> Moves<'a> {
        let mut scans = Vec::new();
        let mut from: HashMap<&str, Vec<(usize, &NodeId)>> = HashMap::new();
        for event in events {
            let EventBody::Scan { renames, .. } = &event.body else { continue };
            if renames.is_empty() {
                continue;
            }
            let moved: BTreeMap<&NodeId, &NodeId> = renames.iter().map(|r| (&r.from, &r.to)).collect();
            for (old, new) in moved {
                from.entry(old.0.as_str()).or_default().push((scans.len(), new));
            }
            scans.push(event.seq);
        }
        Moves { scans, from }
    }

    fn is_empty(&self) -> bool {
        self.scans.is_empty()
    }

    /// Whether any rename moves `id`.
    fn moves(&self, id: &str) -> bool {
        self.from.contains_key(id)
    }

    /// Where `id` is once every rename recorded after `seq` has applied, scan by scan in `seq`
    /// order: `None` when none moves it.
    fn follow(&self, id: &str, seq: Seq) -> Option<NodeId> {
        let mut next = self.scans.partition_point(|s| *s <= seq);
        let mut at = id;
        // Each step jumps to the next scan that moves the node where it is, so a walk costs the
        // moves it makes, not every scan after `seq`.
        while let Some(scans) = self.from.get(at) {
            let i = scans.partition_point(|(scan, _)| *scan < next);
            let Some(&(scan, to)) = scans.get(i) else { break };
            at = to.0.as_str();
            next = scan + 1;
        }
        (at != id).then(|| NodeId(at.to_owned()))
    }
}

/// Each edit of `events`, sorted by `seq`, that names a node `nodes` doesn't hold, resolved
/// through the renames recorded after it, by its place in `events`; and what each resolved edit
/// followed. Every other event stands as it is. `nodes` is sorted by ID, as resolving leaves it,
/// so a fold with no recorded rename pays for nothing here.
pub(crate) fn follow(events: &[&Event], nodes: &[Node]) -> (Vec<(usize, Event)>, Followed) {
    let moves = Moves::of(events);
    let mut followed = Followed::default();
    if moves.is_empty() {
        return (Vec::new(), followed);
    }
    let out = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| {
            let EventBody::Edit(edit) = &event.body else { return None };
            let resolve = |id: &str| -> Option<NodeId> {
                // Only an ID some rename moves can follow one, so most skip the search for it.
                if !moves.moves(id) || held(nodes, id) { None } else { moves.follow(id, event.seq) }
            };
            let mut moved: Vec<(NodeId, NodeId)> = Vec::new();
            let target = match &edit.target {
                Target::Node(id) => resolve(&id.0).inspect(|to| moved.push((id.clone(), to.clone()))),
                Target::Edge(_) => None,
            };
            let parent = parent_of(edit.fields.get(PARENT))
                .and_then(|p| resolve(p).inspect(|to| moved.push((NodeId(p.to_owned()), to.clone()))));
            // The parent the edit was made over follows too, so a moved parent isn't drift; it
            // isn't listed, since the edit didn't name it.
            let over = parent_of(edit.over.get(PARENT)).and_then(resolve);
            if target.is_none() && parent.is_none() && over.is_none() {
                return None;
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
            Some((at, event))
        })
        .collect();
    (out, followed)
}

/// Whether `nodes`, sorted by ID, holds `id`.
fn held(nodes: &[Node], id: &str) -> bool {
    nodes.binary_search_by(|n| n.id.0.as_str().cmp(id)).is_ok()
}

fn parent_of(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}
