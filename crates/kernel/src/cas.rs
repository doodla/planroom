//! Compare-and-set, pure (architecture.md, "Writes: actors, base, conflicts and retries"): what
//! `store::commit` stores a write as, read from the fold of the store inside its transaction.
//!
//! In order, as the write path checks them:
//! 1. A parent change that would close a loop is refused, naming the loop, and so is an undo
//!    that would restore a parent that closes one (DR-61).
//! 2. An automatic write (an auto-enabled decision effect, a prefill or an `infer` output) that
//!    sets a field a person holds applies nothing, whatever its base, and its decision is
//!    `Superseded` (DR-73).
//! 3. A write conflicts when a field it sets has an authored revision above `base`, written by
//!    another writer instance; a `put` of an ID that holds other content conflicts too, keeping
//!    both. A person's conflict deals a card; an agent's or a model's doesn't (DR-7).
//! 4. Otherwise a person's, an automatic or a system write applies, and an agent's or a model's
//!    is a ghost. A ghost writes no revision, so a person's edit over it applies and supersedes it.
//!
//! Scans never write a revision, so a rescan never makes an edit stale (DR-5). An undo is a write
//! like any other: it sets every field its command set (DR-61).

use std::collections::BTreeMap;

use planroom_contract::{Actor, Edit, EventId, Node, NodeId, Op, Provenance, Seq, Status, Target};
use serde_json::Value;

use crate::fold::cycle::find_cycle;
use crate::fold::{FieldKey, Folded, Rev, WHOLE, Writer, build_edge, build_node, edit_keys};

/// Who a write speaks for, which decides its status when nothing conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    /// A person's write applies.
    Person,
    /// An agent's or a model's write is a ghost until a person accepts it (R5).
    Proposal,
    /// An auto-enabled decision effect, a `low`-stakes prefill or an `infer` output applies, but
    /// never over a field a person holds.
    Automatic,
    /// A system part, such as a duty, applies.
    System,
}

impl Authority {
    /// The authority an actor writes with when its write is none of the automatic kinds.
    pub fn of(actor: &Actor) -> Authority {
        match actor {
            Actor::Person { .. } => Authority::Person,
            Actor::Agent { .. } | Actor::Model { .. } => Authority::Proposal,
            Actor::System { .. } => Authority::System,
        }
    }
}

/// A write about to commit: who writes it, with what authority, from which view.
#[derive(Debug, Clone, PartialEq)]
pub struct Proposed {
    pub writer: Writer,
    pub authority: Authority,
    /// The `seq` of the view the writer acted on.
    pub base: Seq,
}

/// One field a write sets that another writer instance set after its base.
#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    pub target: Target,
    pub field: String,
    /// The revision that beat the base; `None` for a `put` over a fact that holds other content.
    pub theirs: Option<Rev>,
}

/// What a write is stored as, or why it isn't.
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    Applied,
    Ghost,
    /// Stored with `status: conflict`, never merged. `card` is true for a person's write.
    Conflict {
        clashes: Vec<Clash>,
        card: bool,
    },
    /// An automatic write over fields a person holds: it applies nothing.
    Superseded {
        held: Vec<(Target, String)>,
    },
    /// Refused with `invalid_argument`, naming the loop `[node, new parent, ..., node]`.
    Cycle {
        cycle: Vec<NodeId>,
    },
}

impl Check {
    /// The status the write's events are stored with; `None` when nothing is stored as an edit.
    pub fn status(&self) -> Option<Status> {
        match self {
            Check::Applied => Some(Status::Applied),
            Check::Ghost => Some(Status::Ghost),
            Check::Conflict { .. } => Some(Status::Conflict),
            Check::Superseded { .. } | Check::Cycle { .. } => None,
        }
    }
}

/// Check one edit against the fold.
pub fn check_edit(folded: &Folded, write: &Proposed, edit: &Edit) -> Check {
    check_edits(folded, write, std::slice::from_ref(edit))
}

/// Check every edit one command writes, together, as its one transaction commits them: a regroup's
/// `put` and its `set parent`s are one write, so a loop through the new group is refused too.
pub fn check_edits(folded: &Folded, write: &Proposed, edits: &[Edit]) -> Check {
    if let Some(cycle) = parent_cycle(folded, edits) {
        return Check::Cycle { cycle };
    }
    let keys: Vec<FieldKey> = edits.iter().flat_map(edit_keys).collect();
    if let Some(check) = held(folded, write, &keys) {
        return check;
    }
    let mut clashes = Vec::new();
    for edit in edits {
        clashes.extend(clashes_of(folded, write, edit));
    }
    settle(write, clashes)
}

/// Check an undo of the command that wrote `of`: it sets every field that command set, so it is
/// refused when a parent it restores closes a loop, and it conflicts like any write over a
/// revision above its base (DR-61).
pub fn check_undo(folded: &Folded, write: &Proposed, of: &EventId) -> Check {
    if let Some(cycle) = undo_cycle(folded, of) {
        return Check::Cycle { cycle };
    }
    let keys = folded.undo_keys(of);
    if let Some(check) = held(folded, write, &keys) {
        return check;
    }
    let mut clashes = Vec::new();
    for key in &keys {
        clashes.extend(stale(folded, write, key, &whole_target_keys(folded, key)));
    }
    settle(write, clashes)
}

/// The edit that accepting a ghost, or choosing "yours" on a conflict, writes: a copy of its
/// fields with the base of the view the person accepted from. The caller sets its `cause` to the
/// reviewed event, and it passes [`check_edit`] like any write, so an accept over a newer write is
/// a card, never a silent no-op (DR-5).
pub fn copy_for_review(edit: &Edit, base: Seq) -> Edit {
    Edit { base, ..edit.clone() }
}

fn settle(write: &Proposed, mut clashes: Vec<Clash>) -> Check {
    if !clashes.is_empty() {
        clashes.dedup();
        return Check::Conflict { clashes, card: write.authority == Authority::Person };
    }
    match write.authority {
        Authority::Proposal => Check::Ghost,
        Authority::Person | Authority::Automatic | Authority::System => Check::Applied,
    }
}

fn held(folded: &Folded, write: &Proposed, keys: &[FieldKey]) -> Option<Check> {
    if write.authority != Authority::Automatic {
        return None;
    }
    let mut held: Vec<(Target, String)> = keys
        .iter()
        .filter(|(t, f)| folded.held_by_person(&t.target(), f))
        .map(|(t, f)| (t.target(), f.clone()))
        .collect();
    held.dedup();
    (!held.is_empty()).then_some(Check::Superseded { held })
}

/// The fields whose revisions a write of `key` must not be stale against: the field and the whole
/// target for one field, every field of the target for the whole target.
fn whole_target_keys(folded: &Folded, key: &FieldKey) -> Vec<FieldKey> {
    let (target, field) = key;
    if field == WHOLE {
        folded.revisions(&target.target()).into_iter().map(|(f, _)| (target.clone(), f)).collect()
    } else {
        vec![key.clone(), (target.clone(), WHOLE.to_owned())]
    }
}

/// The clashes of `key`, read through the revisions of `against`.
fn stale(folded: &Folded, write: &Proposed, key: &FieldKey, against: &[FieldKey]) -> Vec<Clash> {
    against
        .iter()
        .filter_map(|k| folded.revision_of(k))
        .filter(|rev| rev.seq > write.base && !rev.writer.same_instance(&write.writer))
        .map(|rev| Clash { target: key.0.target(), field: key.1.clone(), theirs: Some(rev) })
        .collect()
}

fn clashes_of(folded: &Folded, write: &Proposed, edit: &Edit) -> Vec<Clash> {
    let mut out = Vec::new();
    for key in edit_keys(edit) {
        out.extend(stale(folded, write, &key, &whole_target_keys(folded, &key)));
    }
    if edit.op == Op::Put && out.is_empty() && !same_content(folded, write, edit) {
        let theirs = folded.revisions(&edit.target).into_iter().map(|(_, rev)| rev).max_by_key(|r| r.seq);
        if theirs.as_ref().is_none_or(|r| !r.writer.same_instance(&write.writer)) {
            out.push(Clash { target: edit.target.clone(), field: WHOLE.to_owned(), theirs });
        }
    }
    out
}

/// Whether a `put` writes what its target already holds, or a target that doesn't exist; the
/// provenance doesn't count.
fn same_content(folded: &Folded, write: &Proposed, edit: &Edit) -> bool {
    let provenance =
        Provenance::Authored { actor: write.writer.actor.clone(), event: EventId(String::new()) };
    match &edit.target {
        Target::Node(id) => {
            let Some(current) = folded.model.nodes.iter().find(|n| &n.id == id) else { return true };
            build_node(id, edit, provenance.clone())
                .is_some_and(|new| Node { provenance: provenance.clone(), ..current.clone() } == new)
        }
        Target::Edge(id) => {
            let Some(current) = folded.model.edges.iter().find(|e| &e.id == id) else { return true };
            build_edge(id, edit, provenance.clone()).is_some_and(|new| {
                planroom_contract::Edge { provenance: provenance.clone(), ..current.clone() } == new
            })
        }
    }
}

/// The loop the edits' parent changes would close, applied in order over the folded model.
fn parent_cycle(folded: &Folded, edits: &[Edit]) -> Option<Vec<NodeId>> {
    let mut parents: BTreeMap<NodeId, Option<NodeId>> =
        folded.model.nodes.iter().map(|n| (n.id.clone(), n.parent.clone())).collect();
    for edit in edits {
        let Target::Node(id) = &edit.target else { continue };
        let parent = match edit.op {
            Op::Set | Op::Put => match edit.fields.get(crate::fold::PARENT) {
                Some(Value::String(p)) => Some(NodeId(p.clone())),
                Some(Value::Null) => None,
                Some(_) => continue,
                None if edit.op == Op::Put => None,
                None => continue,
            },
            Op::Retract => {
                parents.remove(id);
                continue;
            }
        };
        if let Some(p) = &parent
            && let Some(cycle) = find_cycle(id, p, |n| parents.get(n).cloned().flatten())
        {
            return Some(cycle);
        }
        parents.insert(id.clone(), parent);
    }
    None
}

/// The loop an undo of `of`'s command would close, with every parent it changes back in place.
fn undo_cycle(folded: &Folded, of: &EventId) -> Option<Vec<NodeId>> {
    let mut parents: BTreeMap<NodeId, Option<NodeId>> =
        folded.model.nodes.iter().map(|n| (n.id.clone(), n.parent.clone())).collect();
    let changed = folded.parents_after_undo(of);
    for (node, parent) in &changed {
        match parent {
            Some(parent) => parents.insert(node.clone(), parent.clone()),
            None => parents.remove(node),
        };
    }
    changed.iter().find_map(|(node, parent)| {
        let Some(Some(parent)) = parent else { return None };
        find_cycle(node, parent, |n| parents.get(n).cloned().flatten())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Client, Surface};

    #[test]
    fn an_actor_writes_with_its_own_authority() {
        let person = Actor::Person { id: "person:ann".into(), name: None };
        let agent = Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None };
        assert_eq!(Authority::of(&person), Authority::Person);
        assert_eq!(Authority::of(&agent), Authority::Proposal);
        assert_eq!(Authority::of(&Actor::Model { id: "m".into() }), Authority::Proposal);
        let w = Writer { actor: person, client: Client { surface: Surface::Canvas, id: "tab_1".into() } };
        assert_eq!(
            settle(&Proposed { writer: w, authority: Authority::Person, base: 0 }, vec![]),
            Check::Applied
        );
    }
}
