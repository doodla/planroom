//! What a built-in's facts charge, measured by the runner's own costs ([`super::NODE_COST`],
//! [`super::HINT_COST`], [`super::EVIDENCE_COST`], [`super::MAP_COST`] and the rest), against the
//! same per-file capture budget ([`work::capture_budget`]) and the same repo cut ([`work::Held`],
//! [`super::REPO_CAPTURE_MAX`]) the declarative runner keeps (DR-100). So every extractor is
//! bounded the same way: a file whose facts would charge more than its budget is partial with
//! `work_limit` and gives no fact, and in sorted path order the first file whose charge would
//! take the extractor's total past the repo's bound, and every file after it, are partial with
//! `repo_limit`.
//!
//! A built-in has its facts in hand, so it charges each as it makes it: its structure, every text
//! it holds, its props' or attributes' map with their entries, and each line of its evidence. A
//! file's charge depends on the file alone, its text, its path and its repo's name, never on the
//! files beside it; a fact two files give is charged to each, so the run holds no more than the
//! files' charges together.

use std::collections::BTreeMap;

use planroom_contract::{Edge, Evidence, Hint, Node, PartialReason, Provenance, Ref};
use serde_json::Value;

use super::work::{self, Held};
use super::{ENTRY_COST, EVIDENCE_COST, HINT_COST, LIST_COST, NODE_COST, fixed_size, map_heap};

/// One built-in's run over a repo: the repo's cut, and the file being read's charge against its
/// own budget.
pub struct Charges {
    held: Held,
    spent: u64,
    budget: u64,
}

impl Charges {
    /// Nothing charged yet, under the repo bound `max`.
    pub fn new(max: u64) -> Charges {
        Charges { held: Held::new(max), spent: 0, budget: 0 }
    }

    /// Start a file of `len` bytes: `Err(RepoLimit)` when an earlier file reached the repo's
    /// bound, so this one isn't read at all.
    pub fn start(&mut self, len: usize) -> Result<(), PartialReason> {
        if self.held.cut() {
            return Err(PartialReason::RepoLimit);
        }
        self.spent = 0;
        self.budget = work::capture_budget(len);
        Ok(())
    }

    /// Charge `bytes` to the file being read: `Err(WorkLimit)` once its facts hold more than its
    /// budget.
    pub fn add(&mut self, bytes: u64) -> Result<(), PartialReason> {
        self.spent = self.spent.saturating_add(bytes);
        if self.spent > self.budget { Err(PartialReason::WorkLimit) } else { Ok(()) }
    }

    /// The file is read: add its charge to the repo's total, or `Err(RepoLimit)` when that would
    /// take the total past the bound, so the cut falls here.
    pub fn finish(&mut self) -> Result<(), PartialReason> {
        if self.held.add(self.spent) { Ok(()) } else { Err(PartialReason::RepoLimit) }
    }
}

fn size(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// The text one line of evidence holds.
fn evidence_text(evidence: &Evidence) -> usize {
    match evidence {
        Evidence::Line { path, contains, commit, branch, .. } => {
            path.len()
                + contains.as_ref().map_or(0, String::len)
                + commit.as_ref().map_or(0, String::len)
                + branch.as_ref().map_or(0, String::len)
        }
        Evidence::Ref { commit, ref_name } => commit.len() + ref_name.as_ref().map_or(0, String::len),
    }
}

/// What one further line of evidence on a fact that has its ID holds.
pub fn further(evidence: &Evidence) -> u64 {
    EVIDENCE_COST.saturating_add(size(evidence_text(evidence)))
}

/// What a fact's evidence holds beside its structure: every line's text, and each line after the
/// first its own [`EVIDENCE_COST`].
fn evidence(items: &[Evidence]) -> u64 {
    let texts: usize = items.iter().map(evidence_text).sum();
    size(texts).saturating_add(EVIDENCE_COST.saturating_mul(size(items.len().saturating_sub(1))))
}

/// What a props or attributes map holds: its nodes, and each entry's key and value.
fn map(entries: &BTreeMap<String, Value>) -> u64 {
    let texts: usize = entries.iter().map(|(k, v)| ENTRY_COST + k.len() + fixed_size(v)).sum();
    map_heap(entries.len()).saturating_add(size(texts))
}

fn provenance(p: &Provenance) -> usize {
    match p {
        Provenance::Extracted { extractor, .. } => extractor.len(),
        _ => 0,
    }
}

fn reference(r: &Ref) -> usize {
    match r {
        Ref::Id(id) => id.0.len(),
        Ref::Alias(alias) => alias.len(),
    }
}

/// What a node holds: [`NODE_COST`], its ID, kind, name and parent, its aliases' list, its props
/// and its evidence.
pub fn node(n: &Node) -> u64 {
    let aliases: usize = n.aliases.iter().map(|a| ENTRY_COST + a.len()).sum();
    let list = if n.aliases.is_empty() { 0 } else { LIST_COST };
    let texts = n.id.0.len()
        + n.kind.0.len()
        + n.name.len()
        + n.parent.as_ref().map_or(0, |p| ENTRY_COST + p.0.len())
        + aliases
        + provenance(&n.provenance);
    NODE_COST
        .saturating_add(size(texts))
        .saturating_add(list)
        .saturating_add(map(&n.props))
        .saturating_add(evidence(&n.evidence))
}

/// What an edge holds: [`NODE_COST`], its ID, kind, ends and via, its props and its evidence.
pub fn edge(e: &Edge) -> u64 {
    let texts = e.id.0.len()
        + e.kind.0.len()
        + reference(&e.from)
        + reference(&e.to)
        + e.via.as_ref().map_or(0, |v| ENTRY_COST + v.len())
        + provenance(&e.provenance);
    NODE_COST.saturating_add(size(texts)).saturating_add(map(&e.props)).saturating_add(evidence(&e.evidence))
}

/// What a hint holds: [`HINT_COST`], its ID, kind, scope, key and value, its attributes and its
/// evidence.
pub fn hint(h: &Hint) -> u64 {
    let texts = h.id.len()
        + h.kind.0.len()
        + h.scope.0.len()
        + h.key.len()
        + h.value.as_ref().map_or(0, |v| ENTRY_COST + v.len())
        + provenance(&h.provenance);
    HINT_COST.saturating_add(size(texts)).saturating_add(map(&h.attrs)).saturating_add(evidence(&h.evidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declarative::{CAPTURE_FLOOR, CAPTURE_PER_BYTE};

    #[test]
    fn a_file_is_refused_past_its_budget_and_the_repo_cut_stays() {
        let mut charges = Charges::new(CAPTURE_FLOOR * 2);
        charges.start(0).unwrap();
        assert_eq!(charges.add(CAPTURE_FLOOR), Ok(()), "up to the floor");
        assert_eq!(charges.add(1), Err(PartialReason::WorkLimit), "past it");
        charges.start(10).unwrap();
        charges.add(CAPTURE_FLOOR + 10 * CAPTURE_PER_BYTE).unwrap();
        assert_eq!(charges.finish(), Ok(()));
        charges.start(0).unwrap();
        charges.add(CAPTURE_FLOOR).unwrap();
        assert_eq!(charges.finish(), Err(PartialReason::RepoLimit), "past the repo's bound");
        assert_eq!(charges.start(0), Err(PartialReason::RepoLimit), "every file after the cut");
    }
}
