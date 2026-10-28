//! Evidence verification at ingest (DR-12, DR-21): a `Line` must match the line index of the blob
//! at its commit, and a `Ref` must name a commit and a ref that exist. Evidence that fails is
//! dropped; a fact left with none is dropped too.

use std::collections::{BTreeSet, HashMap};

use planroom_contract::{Evidence, FactBatch};

use crate::git::{GitError, Objects};
use crate::lines;

/// Whether line `line` of `text` is what an `Evidence::Line` whose `contains` is `contains` cites,
/// as ingest checks it against a blob.
pub fn line_holds(text: &str, line: u32, contains: &str) -> bool {
    lines::line(text, line).map(lines::cut).as_deref() == Some(contains)
}

/// One fact or evidence item verification removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    /// The fact's ID.
    pub fact: String,
    /// Why, such as `line 4 of orders/go.mod differs`.
    pub reason: String,
    /// Whether the whole fact went, because no evidence was left.
    pub fact_dropped: bool,
}

/// Checks evidence against one repo's objects at one commit.
pub struct Verifier<'a> {
    repo: &'a str,
    commit: &'a str,
    refs: &'a BTreeSet<String>,
    objects: Objects,
    /// Each blob read so far, with its line index, so checking every line a file's facts cite
    /// reads the blob once.
    blobs: HashMap<String, Option<(String, lines::Index)>>,
    commits: HashMap<String, bool>,
}

impl<'a> Verifier<'a> {
    pub fn new(repo: &'a str, commit: &'a str, refs: &'a BTreeSet<String>, objects: Objects) -> Verifier<'a> {
        Verifier { repo, commit, refs, objects, blobs: HashMap::new(), commits: HashMap::new() }
    }

    /// Verify every evidence item in `batch`, removing what fails.
    pub fn verify(&mut self, batch: &mut FactBatch) -> Result<Vec<Dropped>, GitError> {
        let mut dropped = Vec::new();
        let mut keep_nodes = Vec::with_capacity(batch.nodes.len());
        for mut node in std::mem::take(&mut batch.nodes) {
            if self.retain(&node.id.0, &mut node.evidence, &mut dropped)? {
                keep_nodes.push(node);
            }
        }
        batch.nodes = keep_nodes;
        let mut keep_edges = Vec::with_capacity(batch.edges.len());
        for mut edge in std::mem::take(&mut batch.edges) {
            if self.retain(&edge.id.0, &mut edge.evidence, &mut dropped)? {
                keep_edges.push(edge);
            }
        }
        batch.edges = keep_edges;
        let mut keep_hints = Vec::with_capacity(batch.hints.len());
        for mut hint in std::mem::take(&mut batch.hints) {
            if self.retain(&hint.id, &mut hint.evidence, &mut dropped)? {
                keep_hints.push(hint);
            }
        }
        batch.hints = keep_hints;
        Ok(dropped)
    }

    /// Keep the evidence that verifies; false when none is left.
    fn retain(
        &mut self,
        fact: &str,
        evidence: &mut Vec<Evidence>,
        dropped: &mut Vec<Dropped>,
    ) -> Result<bool, GitError> {
        let before = std::mem::take(evidence);
        let mut reasons = Vec::new();
        for item in before {
            match self.check(&item)? {
                None => evidence.push(item),
                Some(reason) => reasons.push(reason),
            }
        }
        let fact_dropped = evidence.is_empty();
        dropped.extend(reasons.into_iter().map(|reason| Dropped {
            fact: fact.to_owned(),
            reason,
            fact_dropped,
        }));
        Ok(!fact_dropped)
    }

    /// `None` when `item` verifies, else why not.
    fn check(&mut self, item: &Evidence) -> Result<Option<String>, GitError> {
        match item {
            Evidence::Line { path, line, contains, commit, .. } => {
                let commit = commit.as_deref().unwrap_or(self.commit);
                let Some(rel) = path.strip_prefix(self.repo).and_then(|p| p.strip_prefix('/')) else {
                    return Ok(Some(format!("{path} is outside repo {}", self.repo)));
                };
                let key = format!("{commit}:{rel}");
                if !self.blobs.contains_key(&key) {
                    let text = self.objects.get(&key)?.and_then(|b| String::from_utf8(b).ok());
                    let indexed = text.map(|t| {
                        let index = lines::Index::new(&t);
                        (t, index)
                    });
                    self.blobs.insert(key.clone(), indexed);
                }
                let Some((text, index)) = &self.blobs[&key] else {
                    return Ok(Some(format!("{path} is not in commit {commit}")));
                };
                Ok(match (index.line(text, *line), contains) {
                    (None, _) => Some(format!("{path} has no line {line} at {commit}")),
                    (Some(actual), Some(expected)) if lines::cut(actual) != *expected => {
                        Some(format!("line {line} of {path} differs at {commit}"))
                    }
                    _ => None,
                })
            }
            Evidence::Ref { commit, ref_name } => {
                if !self.commits.contains_key(commit) {
                    let exists = self.objects.get(&format!("{commit}^{{commit}}"))?.is_some();
                    self.commits.insert(commit.clone(), exists);
                }
                Ok(if !self.commits[commit] {
                    Some(format!("commit {commit} doesn't exist"))
                } else if ref_name.as_ref().is_some_and(|r| !self.refs.contains(r)) {
                    Some(format!("ref {} doesn't exist", ref_name.as_deref().unwrap_or_default()))
                } else {
                    None
                })
            }
        }
    }
}
