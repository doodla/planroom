//! A scan of every repo of a workspace, committed in one transaction with its link snapshot.

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{Actor, Client, EventBody, EventId, Rename, ScanStats, Seq, Status, Timestamp};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{NewEvent, check_version, idem_exists, insert_event};
use crate::read::{self, FactRow};
use crate::{Store, StoreError};

/// The inputs of a scan event's key (DR-8).
#[derive(Debug, Clone, Copy)]
pub struct ScanKey<'a> {
    pub repo: &'a str,
    pub commit: &'a str,
    /// The repo's [`RepoScan::inputs`].
    pub inputs: &'a str,
    /// The repo's active snapshot before this scan; `None` on its first scan.
    pub previous: Option<u64>,
}

/// The key functions a scan commit uses, passed in by the composition root.
#[derive(Debug, Clone, Copy)]
pub struct ScanRules {
    /// The scan event's `(idem, request_hash)`.
    pub scan_key: fn(&ScanKey<'_>) -> (String, String),
    /// A link snapshot's `commit_id` from its sorted member snapshot IDs and the packs hash.
    pub link_key: fn(&[u64], &str) -> String,
}

/// One repo's extracted state at one commit.
#[derive(Debug, Clone)]
pub struct RepoScan {
    pub repo: String,
    /// Empty while the repo has no commit yet.
    pub commit: String,
    /// The hash of every extraction input besides the commit: the packs hash, the extractors'
    /// options and anything else the facts depend on. With the repo and commit it keys the
    /// snapshot (in its `packs_hash` column), so a snapshot's facts are a function of its key.
    pub inputs: String,
    /// The branch `HEAD` named, for the scan event only: it's no input of the facts.
    pub branch: String,
    /// The renames git reports between the repo's active snapshot and this one, for the scan
    /// event: files and wholly moved directories, as old and new IDs (DR-74).
    pub renames: Vec<Rename>,
    pub stats: ScanStats,
    /// Sorted by ID, one row per ID.
    pub facts: Vec<FactRow>,
    /// The ID the scan event gets if this repo's active snapshot moves.
    pub event_id: EventId,
}

/// A scan of every repo of a workspace, committed in one transaction.
#[derive(Debug, Clone)]
pub struct ScanWrite {
    pub at: Timestamp,
    pub actor: Actor,
    pub client: Client,
    /// Keys the link snapshot, with its member snapshots.
    pub packs_hash: String,
    pub repos: Vec<RepoScan>,
    /// The store's latest `seq` when this scan began reading, before any repo was read: a repo
    /// whose active scan event is newer was moved by another scan meanwhile.
    pub started: Seq,
    /// Each repo root's namespace, as this scan named its IDs. A repo whose root `meta.repos`
    /// fixed otherwise, or whose namespace it fixed for another root, is
    /// [`RepoOutcome::NamespaceTaken`]. The same transaction fixes the root of every repo it maps
    /// that `meta.repos` doesn't hold yet, and never changes one it holds, so a namespace is fixed
    /// at its root's first scan and no two roots ever hold one (DR-74).
    pub namespaces: BTreeMap<String, String>,
}

/// The `meta` key that holds each repo root's namespace (DR-74).
pub const META_REPOS: &str = "repos";

/// What a scan commit did for one repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoOutcome {
    /// The repo's active snapshot already was this one: nothing moved, so no event.
    Unchanged { snapshot: u64 },
    /// The active snapshot moved: a new `scan` event at `seq`.
    Recorded { snapshot: u64, seq: Seq },
    /// This scan's snapshot is no longer the repo's current one, so the active snapshot, `active`,
    /// stays: `HEAD` no longer names the commit this scan read, or another scan moved the repo
    /// after this one began and this scan's other inputs are out of date. A scan that read an
    /// older state never moves a repo back (R10). Nothing of this scan's is stored: no snapshot,
    /// no facts and no event.
    Superseded { active: u64 },
    /// The repo has no commit yet, so it maps nothing: no snapshot and no event (DR-86).
    Unborn,
    /// Another scan fixed this repo's root under another namespace, or this namespace for another
    /// root, after this scan named its repos: its IDs name a repo set that no longer holds, so
    /// nothing of this scan's is stored for it, and `active`, the repo's active snapshot if it has
    /// one, stays (DR-74).
    NamespaceTaken { active: Option<u64> },
}

/// What a scan commit did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanCommitted {
    /// In the order of `ScanWrite::repos`.
    pub repos: Vec<(String, RepoOutcome)>,
    /// The link snapshot built for the new active set, when any repo moved.
    pub link: Option<u64>,
    /// The store's latest `seq` after the commit.
    pub seq: Seq,
}

impl Store {
    /// Commit a scan of several repos in one IMMEDIATE transaction. For each repo whose active
    /// snapshot moves, it stores the snapshot and its facts (unless that snapshot exists) and
    /// appends a `scan` event keyed by [`ScanRules::scan_key`]; a repo whose active snapshot is
    /// already this one writes nothing, so two processes that rescan one head join on one event
    /// (DR-8). When any repo moved, the same transaction builds the workspace link snapshot of
    /// the new active set, and every new scan event names it (DR-12).
    ///
    /// `is_current` re-reads a repo's inputs under the write lock: a repo with an active snapshot
    /// moves only when this scan's snapshot is still the repo's current one. Its second argument
    /// says whether another scan moved the repo after [`ScanWrite::started`], to a snapshot other
    /// than this one; then the caller checks every input, not only `HEAD`, since that scan may
    /// have read a newer repo set. Otherwise a newer scan has landed, or will, so the repo is
    /// [`RepoOutcome::Superseded`] and nothing moves back.
    pub fn commit_scan(
        &mut self,
        scan: &ScanWrite,
        rules: ScanRules,
        is_current: &mut dyn FnMut(&RepoScan, bool) -> bool,
    ) -> Result<ScanCommitted, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_version(&tx)?;
        let mut active = read::active_snapshots(&tx)?;
        let mut moved = Vec::new();
        let mut outcomes = Vec::with_capacity(scan.repos.len());
        let mut fixed = read_namespaces(&tx)?;
        let taken = taken_namespaces(&fixed, &scan.namespaces);
        for repo in &scan.repos {
            // A repo with no commit yet maps nothing, so nothing of it is stored (DR-86).
            if repo.commit.is_empty() {
                outcomes.push((repo.repo.clone(), RepoOutcome::Unborn));
                continue;
            }
            let previous = active.get(&repo.repo).map(|a| a.snapshot);
            // A namespace another scan fixed otherwise names another repo set's IDs.
            if taken.contains(repo.repo.as_str()) {
                outcomes.push((repo.repo.clone(), RepoOutcome::NamespaceTaken { active: previous }));
                continue;
            }
            let existing = find_snapshot(&tx, &repo.repo, &repo.commit, "head", &repo.inputs)?;
            if let Some(snapshot) = existing.filter(|s| previous == Some(*s)) {
                outcomes.push((repo.repo.clone(), RepoOutcome::Unchanged { snapshot }));
                continue;
            }
            // Whether the repo moves is settled before anything of this scan is stored, so a
            // superseded scan leaves no snapshot and no facts behind.
            let newer_scan = active.get(&repo.repo).is_some_and(|a| a.seq > scan.started);
            if let Some(active) = previous.filter(|_| !is_current(repo, newer_scan)) {
                outcomes.push((repo.repo.clone(), RepoOutcome::Superseded { active }));
                continue;
            }
            // The branch isn't part of the key, so a head snapshot records none: it's on the
            // scan event that moved to it.
            let (snapshot, created) = put_snapshot(&tx, &repo.repo, &repo.commit, "", "head", &repo.inputs)?;
            if created {
                insert_facts(&tx, snapshot, &repo.facts)?;
            }
            let key = ScanKey { repo: &repo.repo, commit: &repo.commit, inputs: &repo.inputs, previous };
            let (mut idem, request_hash) = (rules.scan_key)(&key);
            // A repo that moves A -> B -> A -> B meets its own earlier key: that event is not the
            // active one, so this is a new move, keyed after the scan it moves away from.
            if idem_exists(&tx, &idem)? {
                let after = active.get(&repo.repo).map_or(0, |a| a.seq);
                idem = format!("{idem}#after:{after}");
            }
            moved.push((repo, snapshot, idem, request_hash));
            active.insert(repo.repo.clone(), read::Active { snapshot, seq: 0 });
            outcomes.push((repo.repo.clone(), RepoOutcome::Unchanged { snapshot }));
        }
        let mut link = None;
        if !moved.is_empty() {
            let mut members: Vec<u64> = active.values().map(|a| a.snapshot).collect();
            members.sort_unstable();
            let commit_id = (rules.link_key)(&members, &scan.packs_hash);
            let (link_id, _) = put_snapshot(&tx, "ws", &commit_id, "", "link", &scan.packs_hash)?;
            link = Some(link_id);
            for (repo, snapshot, idem, request_hash) in moved {
                let event = NewEvent {
                    id: repo.event_id.clone(),
                    at: scan.at.clone(),
                    actor: scan.actor.clone(),
                    client: scan.client.clone(),
                    project: None,
                    cause: None,
                    status: Status::Applied,
                    target: Some(repo.repo.clone()),
                    body: EventBody::Scan {
                        repo: repo.repo.clone(),
                        commit: repo.commit.clone(),
                        branch: repo.branch.clone(),
                        snapshot,
                        link: link_id,
                        renames: repo.renames.clone(),
                        stats: repo.stats,
                    },
                };
                let seq = insert_event(&tx, &event, &idem, &request_hash)?;
                let slot = outcomes
                    .iter_mut()
                    .find(|(name, _)| *name == repo.repo)
                    .expect("each moved repo has an outcome");
                slot.1 = RepoOutcome::Recorded { snapshot, seq };
            }
        }
        // Only a repo this scan maps fixes its root: one superseded or unborn leaves it to the scan
        // that maps it.
        let before = fixed.len();
        for (root, ns) in &scan.namespaces {
            let maps = outcomes.iter().any(|(name, outcome)| {
                name == ns && matches!(outcome, RepoOutcome::Recorded { .. } | RepoOutcome::Unchanged { .. })
            });
            if maps {
                fixed.entry(root.clone()).or_insert_with(|| ns.clone());
            }
        }
        if fixed.len() != before {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![META_REPOS, serde_json::to_string(&fixed)?],
            )?;
        }
        let seq = read::max_seq(&tx)?;
        tx.commit()?;
        if link.is_some() {
            self.notify(seq);
        }
        Ok(ScanCommitted { repos: outcomes, link, seq })
    }
}

/// Each repo root's namespace as `meta.repos` fixed it; none before a first scan fixed one.
fn read_namespaces(tx: &Transaction<'_>) -> Result<BTreeMap<String, String>, StoreError> {
    let held: Option<String> =
        tx.query_row("SELECT value FROM meta WHERE key = ?1", [META_REPOS], |r| r.get(0)).optional()?;
    Ok(match held {
        Some(text) => serde_json::from_str(&text)?,
        None => BTreeMap::new(),
    })
}

/// The namespaces of `namespaces` that `fixed` contradicts: one whose root is fixed under another
/// namespace, or that is fixed for another root.
fn taken_namespaces<'a>(
    fixed: &BTreeMap<String, String>,
    namespaces: &'a BTreeMap<String, String>,
) -> BTreeSet<&'a str> {
    let owner: BTreeMap<&str, &str> = fixed.iter().map(|(root, ns)| (ns.as_str(), root.as_str())).collect();
    namespaces
        .iter()
        .filter(|(root, ns)| {
            fixed.get(*root).is_some_and(|held| held != *ns)
                || owner.get(ns.as_str()).is_some_and(|holder| *holder != root.as_str())
        })
        .map(|(_, ns)| ns.as_str())
        .collect()
}

/// The ID of the snapshot with this key, if one is stored.
fn find_snapshot(
    tx: &Transaction<'_>,
    repo: &str,
    commit: &str,
    purpose: &str,
    packs_hash: &str,
) -> Result<Option<u64>, StoreError> {
    let id: Option<i64> = tx
        .query_row(
            "SELECT id FROM snapshots WHERE repo = ?1 AND commit_id = ?2 AND purpose = ?3 AND packs_hash = ?4",
            params![repo, commit, purpose, packs_hash],
            |r| r.get(0),
        )
        .optional()?;
    Ok(id.map(|id| id as u64))
}

/// Insert a snapshot unless it exists; returns its ID and whether this call created it.
fn put_snapshot(
    tx: &Transaction<'_>,
    repo: &str,
    commit: &str,
    branch: &str,
    purpose: &str,
    packs_hash: &str,
) -> Result<(u64, bool), StoreError> {
    let created = tx.execute(
        "INSERT INTO snapshots (repo, commit_id, branch, purpose, packs_hash) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (repo, commit_id, purpose, packs_hash) DO NOTHING",
        params![repo, commit, branch, purpose, packs_hash],
    )? == 1;
    let id: i64 = tx.query_row(
        "SELECT id FROM snapshots WHERE repo = ?1 AND commit_id = ?2 AND purpose = ?3 AND packs_hash = ?4",
        params![repo, commit, purpose, packs_hash],
        |r| r.get(0),
    )?;
    Ok((id as u64, created))
}

fn insert_facts(tx: &Transaction<'_>, snapshot: u64, facts: &[FactRow]) -> Result<(), StoreError> {
    let mut stmt = tx.prepare(
        "INSERT INTO facts (snapshot, id, shape, kind, hash, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for fact in facts {
        stmt.execute(params![
            snapshot as i64,
            fact.id,
            fact.shape.as_str(),
            fact.kind,
            fact.hash,
            fact.body
        ])?;
    }
    Ok(())
}
