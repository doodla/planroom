//! `gc` (architecture.md, Duties and Store schema; DR-47): the duty that prunes derived rows after
//! a scan, under the `gc` lease.
//!
//! - **Snapshots.** Each repo's, the workspace link's (`ws`) included, keep their
//!   [`KEPT_SNAPSHOTS`] newest by `id`, the active set (each repo's active snapshot and the link
//!   the last `scan` event names), and every snapshot a `version` event refers to: those the last
//!   `scan` event of each repo at or before its `seq` names, with that event's link. Any other is
//!   a candidate, and its facts go with it (`ON DELETE CASCADE`).
//! - **Bound.** One run deletes at most [`PRUNED_PER_REPO`] candidates per repo, oldest first,
//!   each in its own transaction. That bounds the deletes, not the work: the read of `version`
//!   events grows with the event log. Until M2's duty runner moves `gc` off the scan's path, its
//!   time counts in the scan's `took_ms` and in `serve`'s request. A scan adds at most one
//!   snapshot per repo, so a backlog still shrinks with each.
//! - **Model cache.** `model_cache` rows whose model no config pins go.
//! - Events, `fact_rev`, presence, layouts, `meta` and `blob_facts` stay: `blob_facts` is pruned by
//!   the per-blob cache's writer, which keys it.
//!
//! Each delete runs in an IMMEDIATE transaction that checks the store version (DR-96), renews the
//! lease ([`lease::hold`], so a run whose lease was taken over stops with `store_busy`), and checks
//! again that the candidate is still one, since a scan may have made it active meanwhile. A reader
//! reads in one transaction ([`Snapshot`]), and WAL keeps that read's version of the store whole
//! whatever a delete commits after it began. `gc` deletes only derived rows and writes only its
//! lease, never anything a person or an agent authored.

use std::collections::BTreeSet;

use rusqlite::{Connection, TransactionBehavior, params, params_from_iter};

use planroom_contract::EventBody;

use crate::lease::{self, Lease};
use crate::read::Snapshot;
use crate::{Store, StoreError, commit};

/// How many of each repo's newest snapshots are always kept.
pub const KEPT_SNAPSHOTS: usize = 20;

/// The most snapshots of one repo a single run deletes.
pub const PRUNED_PER_REPO: usize = 2;

/// The lease a `gc` run holds.
pub const GC_LEASE: &str = "gc";

/// What one `gc` run deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Collected {
    pub snapshots: usize,
    pub model_cache: usize,
}

impl Store {
    /// Run `gc` as `holder`, keeping the `model_cache` rows of `pinned_models` (all of them when
    /// it's empty). `None` when another run holds the `gc` lease: that run collects, and this
    /// trigger joins it.
    pub fn collect(
        &mut self,
        holder: &str,
        pinned_models: &[String],
    ) -> Result<Option<Collected>, StoreError> {
        let Some(lease) = self.take_lease(GC_LEASE, holder)? else { return Ok(None) };
        let collected = self.collect_under(&lease, pinned_models);
        // Released whatever the run did, so a failed run doesn't hold the next one off for 30 s.
        let released = self.release_lease(&lease);
        let collected = collected?;
        released?;
        Ok(Some(collected))
    }

    fn collect_under(&mut self, lease: &Lease, pinned_models: &[String]) -> Result<Collected, StoreError> {
        let candidates = candidates(&self.snapshot()?)?;
        let mut collected = Collected::default();
        for (repo, id) in candidates {
            let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            commit::check_version(&tx)?;
            lease::hold(&tx, lease)?;
            // Read inside the delete's own transaction: the store as the delete will find it.
            let snapshot = Snapshot::over(tx)?;
            if kept(&snapshot, &repo)?.contains(&id) {
                continue;
            }
            collected.snapshots +=
                snapshot.tx().execute("DELETE FROM snapshots WHERE id = ?1", params![id as i64])?;
            snapshot.commit()?;
        }
        if !pinned_models.is_empty() {
            let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            commit::check_version(&tx)?;
            lease::hold(&tx, lease)?;
            let marks = vec!["?"; pinned_models.len()].join(", ");
            collected.model_cache += tx.execute(
                &format!("DELETE FROM model_cache WHERE model NOT IN ({marks})"),
                params_from_iter(pinned_models),
            )?;
            tx.commit()?;
        }
        Ok(collected)
    }
}

impl Snapshot<'_> {
    /// The IDs of `repo`'s snapshots in this version of the store, newest first.
    pub fn snapshot_ids(&self, repo: &str) -> Result<Vec<u64>, StoreError> {
        ids(self.tx(), repo)
    }
}

/// Each repo's oldest prunable snapshots, at most [`PRUNED_PER_REPO`] of them, as (repo, id).
fn candidates(snapshot: &Snapshot<'_>) -> Result<Vec<(String, u64)>, StoreError> {
    let conn = snapshot.tx();
    let mut stmt = conn.prepare("SELECT DISTINCT repo FROM snapshots ORDER BY repo")?;
    let repos: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let referred = referred(snapshot)?;
    let mut out = Vec::new();
    for repo in repos {
        let mut ids = ids(conn, &repo)?;
        ids.reverse();
        let old = ids.len().saturating_sub(KEPT_SNAPSHOTS);
        out.extend(
            ids.into_iter()
                .take(old)
                .filter(|id| !referred.contains(id))
                .take(PRUNED_PER_REPO)
                .map(|id| (repo.clone(), id)),
        );
    }
    Ok(out)
}

/// The snapshots of `repo` that stay: its newest, and every one the active set or a `version`
/// event refers to.
fn kept(snapshot: &Snapshot<'_>, repo: &str) -> Result<BTreeSet<u64>, StoreError> {
    let mut kept = referred(snapshot)?;
    kept.extend(ids(snapshot.tx(), repo)?.into_iter().take(KEPT_SNAPSHOTS));
    Ok(kept)
}

/// Every snapshot the active set or a `version` event refers to, whatever its repo.
fn referred(snapshot: &Snapshot<'_>) -> Result<BTreeSet<u64>, StoreError> {
    let mut out: BTreeSet<u64> = snapshot.active_set()?.snapshots().into_iter().collect();
    // What a fold at a version's `seq` reads: each repo's last scan at or before it, with its link.
    let mut stmt = snapshot.tx().prepare(
        "SELECT DISTINCT s.body FROM events s
         WHERE s.seq IN (
             SELECT MAX(e.seq) FROM events e JOIN events v ON e.seq <= v.seq
             WHERE e.kind = 'scan' AND v.kind = 'version'
             GROUP BY v.seq, e.target
         )",
    )?;
    let bodies = stmt.query_map([], |r| r.get::<_, String>(0))?;
    for body in bodies {
        let EventBody::Scan { snapshot, link, .. } = serde_json::from_str(&body?)? else {
            return Err(StoreError::Sqlite("a scan row holds another event body".into()));
        };
        out.insert(snapshot);
        out.insert(link);
    }
    Ok(out)
}

/// `repo`'s snapshot IDs, newest first.
fn ids(conn: &Connection, repo: &str) -> Result<Vec<u64>, StoreError> {
    let mut stmt = conn.prepare_cached("SELECT id FROM snapshots WHERE repo = ?1 ORDER BY id DESC")?;
    let ids = stmt.query_map([repo], |r| r.get::<_, i64>(0))?;
    Ok(ids.map(|id| id.map(|id| id as u64)).collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Init;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let init = Init {
            workspace_id: "ws-gc".into(),
            engine_version: "0.1.0".into(),
            created_at: "2026-10-28T00:00:00Z".into(),
        };
        let store = Store::open(&dir.path().join("store.db"), &init).unwrap();
        (dir, store)
    }

    /// Plant `n` snapshots of each of `repos`, interleaved, each with one fact.
    fn plant(store: &Store, repos: &[&str], n: usize) {
        for i in 0..n {
            for repo in repos {
                store
                    .conn
                    .execute(
                        "INSERT INTO snapshots (repo, commit_id, branch, purpose, packs_hash) VALUES (?1, ?2, '', 'head', 'p')",
                        params![repo, format!("c{i}")],
                    )
                    .unwrap();
                let id = store.conn.last_insert_rowid();
                store
                    .conn
                    .execute(
                        "INSERT INTO facts (snapshot, id, shape, kind, hash, body) VALUES (?1, 'n', 'node', 'k', 'h', '{}')",
                        [id],
                    )
                    .unwrap();
            }
        }
    }

    fn snapshot_ids(store: &Store, repo: &str) -> Vec<u64> {
        store.snapshot().unwrap().snapshot_ids(repo).unwrap()
    }

    fn fact_count(store: &Store) -> i64 {
        store.conn.query_row("SELECT COUNT(*) FROM facts", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn one_run_prunes_at_most_two_per_repo() {
        let (_dir, mut store) = store();
        plant(&store, &["a", "b"], 30);
        let (a, b) = (snapshot_ids(&store, "a"), snapshot_ids(&store, "b"));
        let collected = store.collect("{\"pid\":1}", &[]).unwrap().unwrap();
        assert_eq!(collected, Collected { snapshots: 4, model_cache: 0 });
        // The two oldest of each repo went, with their facts.
        assert_eq!(snapshot_ids(&store, "a"), a[..28]);
        assert_eq!(snapshot_ids(&store, "b"), b[..28]);
        assert_eq!(fact_count(&store), 56);
        for _ in 0..10 {
            store.collect("{\"pid\":1}", &[]).unwrap().unwrap();
        }
        assert_eq!(snapshot_ids(&store, "a"), a[..KEPT_SNAPSHOTS]);
        assert_eq!(snapshot_ids(&store, "b"), b[..KEPT_SNAPSHOTS]);
        assert_eq!(fact_count(&store), 40);
        // The run released its lease each time.
        let held: i64 = store.conn.query_row("SELECT COUNT(*) FROM leases", [], |r| r.get(0)).unwrap();
        assert_eq!(held, 0);
    }

    #[test]
    fn model_cache_rows_of_unpinned_models_go() {
        let (_dir, mut store) = store();
        for (key, model) in [("k1", "pinned/one"), ("k2", "dropped/two")] {
            store
                .conn
                .execute(
                    "INSERT INTO model_cache (key, provider, model, response, usage, at) VALUES (?1, 'gateway', ?2, '{}', '{}', 't')",
                    [key, model],
                )
                .unwrap();
        }
        let models = |store: &Store| -> Vec<String> {
            let mut stmt = store.conn.prepare("SELECT model FROM model_cache ORDER BY model").unwrap();
            stmt.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        };
        // No pins known: nothing is taken for unpinned.
        assert_eq!(store.collect("{\"pid\":1}", &[]).unwrap().unwrap().model_cache, 0);
        assert_eq!(models(&store), ["dropped/two", "pinned/one"]);
        let pinned = vec!["pinned/one".to_owned(), "pinned/other".to_owned()];
        assert_eq!(store.collect("{\"pid\":1}", &pinned).unwrap().unwrap().model_cache, 1);
        assert_eq!(models(&store), ["pinned/one"]);
    }
}
