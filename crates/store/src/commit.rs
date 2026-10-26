//! `store::commit`: every write is one `BEGIN IMMEDIATE` transaction with the idempotency check.

use planroom_contract::{
    Actor, Cause, Client, EventBody, EventId, NodeId, ScanStats, Seq, Status, Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;

use crate::read::{self, FactRow};
use crate::{Store, StoreError};

/// One event to append. `seq` is assigned at commit.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub id: EventId,
    pub at: Timestamp,
    pub actor: Actor,
    pub client: Client,
    pub project: Option<NodeId>,
    pub cause: Option<Cause>,
    pub status: Status,
    /// The node, edge or repo the event is about, for the `events_target` index.
    pub target: Option<String>,
    pub body: EventBody,
}

/// The events one command writes, under one idempotency key.
#[derive(Debug, Clone)]
pub struct Write {
    /// The caller's key, a derived one, or a fresh uuid v7 (DR-8). Several events are keyed
    /// `idem#1`, `idem#2` and so on.
    pub idem: String,
    /// sha256 of the canonical request.
    pub request_hash: String,
    pub events: Vec<NewEvent>,
}

/// What a commit wrote, or what a retry found.
#[derive(Debug, Clone, PartialEq)]
pub struct Committed {
    /// The events' `seq`s, in order.
    pub seqs: Vec<Seq>,
    /// The whole reply, stored on the first event.
    pub reply: Value,
    /// The same key and request were committed before; nothing was written.
    pub replayed: bool,
}

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
}

/// What a scan commit did for one repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoOutcome {
    /// The repo's active snapshot already was this one: nothing moved, so no event.
    Unchanged { snapshot: u64 },
    /// The active snapshot moved: a new `scan` event at `seq`.
    Recorded { snapshot: u64, seq: Seq },
    /// The repo's `HEAD` no longer names the commit this scan read, so the active snapshot,
    /// `active`, stays: a scan that read an older head never moves a repo back (R10).
    Superseded { active: u64 },
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
    /// Append one command's events in one IMMEDIATE transaction. A key seen before with the same
    /// request hash returns the stored reply and writes nothing; with another hash it is
    /// `idempotency_mismatch`. `reply` builds the reply from the assigned `seq`s, inside the
    /// transaction, and it is stored on the first event.
    pub fn commit(
        &mut self,
        write: Write,
        reply: impl FnOnce(&[Seq]) -> Value,
    ) -> Result<Committed, StoreError> {
        assert!(!write.events.is_empty(), "a commit writes at least one event");
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(found) = replay(&tx, &write)? {
            return Ok(found);
        }
        let many = write.events.len() > 1;
        let mut seqs = Vec::with_capacity(write.events.len());
        for (i, event) in write.events.iter().enumerate() {
            let idem = if many { format!("{}#{}", write.idem, i + 1) } else { write.idem.clone() };
            seqs.push(insert_event(&tx, event, &idem, &write.request_hash)?);
        }
        let reply = reply(&seqs);
        tx.execute(
            "UPDATE events SET reply = ?1 WHERE seq = ?2",
            params![reply.to_string(), seqs[0] as i64],
        )?;
        tx.commit()?;
        self.notify(*seqs.last().expect("one event at least"));
        Ok(Committed { seqs, reply, replayed: false })
    }

    /// Commit a scan of several repos in one IMMEDIATE transaction. For each repo whose active
    /// snapshot moves, it stores the snapshot and its facts (unless that snapshot exists) and
    /// appends a `scan` event keyed by [`ScanRules::scan_key`]; a repo whose active snapshot is
    /// already this one writes nothing, so two processes that rescan one head join on one event
    /// (DR-8). When any repo moved, the same transaction builds the workspace link snapshot of
    /// the new active set, and every new scan event names it (DR-12).
    ///
    /// `is_head` re-reads a repo's `HEAD` under the write lock: a repo with an active snapshot
    /// moves only when `HEAD` still names the commit this scan read. Otherwise a newer scan may
    /// have landed, or will, so the repo is [`RepoOutcome::Superseded`] and nothing moves back.
    pub fn commit_scan(
        &mut self,
        scan: &ScanWrite,
        rules: ScanRules,
        is_head: &mut dyn FnMut(&RepoScan) -> bool,
    ) -> Result<ScanCommitted, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut active = read::active_snapshots(&tx)?;
        let mut moved = Vec::new();
        let mut outcomes = Vec::with_capacity(scan.repos.len());
        for repo in &scan.repos {
            // The branch isn't part of the key, so a head snapshot records none: it's on the
            // scan event that moved to it.
            let (snapshot, created) = put_snapshot(&tx, &repo.repo, &repo.commit, "", "head", &repo.inputs)?;
            if created {
                insert_facts(&tx, snapshot, &repo.facts)?;
            }
            let previous = active.get(&repo.repo).map(|a| a.snapshot);
            if previous == Some(snapshot) {
                outcomes.push((repo.repo.clone(), RepoOutcome::Unchanged { snapshot }));
                continue;
            }
            if let Some(active) = previous.filter(|_| !is_head(repo)) {
                outcomes.push((repo.repo.clone(), RepoOutcome::Superseded { active }));
                continue;
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
        let seq = read::max_seq(&tx)?;
        tx.commit()?;
        if link.is_some() {
            self.notify(seq);
        }
        Ok(ScanCommitted { repos: outcomes, link, seq })
    }
}

/// The stored reply when `write`'s key was committed before with the same request hash.
fn replay(tx: &Transaction<'_>, write: &Write) -> Result<Option<Committed>, StoreError> {
    let found = tx
        .query_row(
            "SELECT seq, id, request_hash, reply FROM events WHERE idem = ?1 OR idem = ?1 || '#1'",
            [&write.idem],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((seq, id, hash, reply)) = found else { return Ok(None) };
    if hash != write.request_hash {
        return Err(StoreError::IdempotencyMismatch {
            idem: write.idem.clone(),
            stored_event: id,
            stored_seq: seq as Seq,
        });
    }
    let mut stmt =
        tx.prepare("SELECT seq FROM events WHERE idem = ?1 OR idem LIKE ?2 ESCAPE '\\' ORDER BY seq")?;
    let like = format!("{}#%", escape_like(&write.idem));
    let seqs =
        stmt.query_map(params![write.idem, like], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
    let reply = match reply {
        Some(text) => serde_json::from_str(&text)?,
        None => Value::Null,
    };
    Ok(Some(Committed { seqs: seqs.into_iter().map(|s| s as Seq).collect(), reply, replayed: true }))
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn idem_exists(tx: &Transaction<'_>, idem: &str) -> Result<bool, StoreError> {
    Ok(tx.query_row("SELECT 1 FROM events WHERE idem = ?1", [idem], |_| Ok(())).optional()?.is_some())
}

fn insert_event(
    tx: &Transaction<'_>,
    event: &NewEvent,
    idem: &str,
    request_hash: &str,
) -> Result<Seq, StoreError> {
    let status = serde_json::to_value(event.status)?;
    tx.execute(
        "INSERT INTO events (id, idem, request_hash, at, actor, client, project, cause, kind, target, status, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event.id.0,
            idem,
            request_hash,
            event.at.0,
            serde_json::to_string(&event.actor)?,
            serde_json::to_string(&event.client)?,
            event.project.as_ref().map(|p| p.0.as_str()),
            event.cause.as_ref().map(serde_json::to_string).transpose()?,
            event.body.kind().as_str(),
            event.target,
            status.as_str().unwrap_or_default(),
            serde_json::to_string(&event.body)?,
        ],
    )?;
    Ok(tx.last_insert_rowid() as Seq)
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
