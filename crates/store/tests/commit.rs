//! `store::commit` and `store::commit_scan`: idempotency, scan keys and the link snapshot.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use planroom_contract::{Actor, Client, EventBody, EventId, ScanStats, Status, Surface, Timestamp};
use planroom_store::{
    FactRow, Init, NewEvent, RepoOutcome, RepoScan, STORE_VERSION, ScanKey, ScanRules, ScanWrite, Shape,
    Store, StoreError, Tail, Write,
};
use serde_json::json;

fn init() -> Init {
    Init {
        workspace_id: "ws-test".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

fn open(dir: &Path) -> Store {
    Store::open(&dir.join("store.db"), &init()).unwrap()
}

fn version_event(n: u32) -> NewEvent {
    NewEvent {
        id: EventId(format!("ev-{n}")),
        at: Timestamp("2026-10-26T00:00:00Z".into()),
        actor: Actor::System { part: "test".into() },
        client: Client { surface: Surface::Test, id: "test".into() },
        project: None,
        cause: None,
        status: Status::Applied,
        target: None,
        body: EventBody::Version { project: None, label: format!("v{n}") },
    }
}

const RULES: ScanRules = ScanRules {
    scan_key: |k: &ScanKey<'_>| {
        let key = format!("scan:{}:{}:{}:{:?}", k.repo, k.commit, k.packs_hash, k.previous);
        (key.clone(), format!("hash:{key}"))
    },
    link_key: |members: &[u64], packs: &str| format!("link:{members:?}:{packs}"),
};

fn repo(name: &str, commit: &str, event: &str) -> RepoScan {
    RepoScan {
        repo: name.into(),
        commit: commit.into(),
        branch: "main".into(),
        stats: ScanStats::default(),
        facts: vec![FactRow {
            id: format!("{name}:repo:{name}"),
            shape: Shape::Node,
            kind: "test.repo".into(),
            hash: "h".into(),
            body: "{}".into(),
        }],
        event_id: EventId(event.into()),
    }
}

fn scan(repos: Vec<RepoScan>) -> ScanWrite {
    ScanWrite {
        at: Timestamp("2026-10-26T00:00:00Z".into()),
        actor: Actor::System { part: "scan".into() },
        client: Client { surface: Surface::Cli, id: "cli:1".into() },
        packs_hash: "p".into(),
        repos,
    }
}

#[test]
fn a_new_store_is_migrated_with_wal_and_mode_0600() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    assert_eq!(store.meta("store_version").unwrap().as_deref(), Some(STORE_VERSION.to_string().as_str()));
    assert_eq!(store.meta("workspace_id").unwrap().as_deref(), Some("ws-test"));
    assert_eq!(store.seq().unwrap(), 0);
    let mode = std::fs::metadata(dir.path().join("store.db")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    // Reopening keeps the record and migrates nothing.
    drop(store);
    let again = Store::open_existing(&dir.path().join("store.db")).unwrap().unwrap();
    assert_eq!(again.meta("workspace_id").unwrap().as_deref(), Some("ws-test"));
}

#[test]
fn a_newer_store_is_a_schema_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    drop(open(dir.path()));
    let path = dir.path().join("store.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", STORE_VERSION + 1).unwrap();
    drop(conn);
    let err = Store::open_existing(&path).err().expect("refused");
    assert!(matches!(err, StoreError::SchemaMismatch { found, supported, ref writer_version }
        if found == STORE_VERSION + 1 && supported == STORE_VERSION && writer_version.as_deref() == Some("0.1.0")));
}

#[test]
fn a_retry_with_the_same_key_returns_the_stored_reply() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let write = Write { idem: "k1".into(), request_hash: "h1".into(), events: vec![version_event(1)] };
    let first = store.commit(write.clone(), |seqs| json!({"seq": seqs[0]})).unwrap();
    assert!(!first.replayed);
    let again = store.commit(write, |_| panic!("a replay builds no reply")).unwrap();
    assert!(again.replayed);
    assert_eq!(again.seqs, first.seqs);
    assert_eq!(again.reply, json!({"seq": first.seqs[0]}));
    assert_eq!(store.seq().unwrap(), first.seqs[0]);
}

#[test]
fn the_same_key_with_another_request_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let write = Write { idem: "k1".into(), request_hash: "h1".into(), events: vec![version_event(1)] };
    store.commit(write, |_| json!(null)).unwrap();
    let other = Write { idem: "k1".into(), request_hash: "h2".into(), events: vec![version_event(2)] };
    let err = store.commit(other, |_| json!(null)).unwrap_err();
    assert!(matches!(err, StoreError::IdempotencyMismatch { ref idem, .. } if idem == "k1"), "{err}");
}

#[test]
fn several_events_are_keyed_idem_n_and_replay_whole() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let write = Write {
        idem: "multi".into(),
        request_hash: "h".into(),
        events: vec![version_event(1), version_event(2)],
    };
    let first = store.commit(write.clone(), |seqs| json!(seqs)).unwrap();
    let events = store.events_after(0, 10).unwrap();
    assert_eq!(events.iter().map(|e| e.idem.as_str()).collect::<Vec<_>>(), ["multi#1", "multi#2"]);
    let again = store.commit(write, |_| json!(null)).unwrap();
    assert!(again.replayed);
    assert_eq!(again.seqs, first.seqs);
    assert_eq!(again.reply, json!(first.seqs));
}

#[test]
fn an_unchanged_rescan_writes_no_event_and_a_move_writes_one_with_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let first = store.commit_scan(&scan(vec![repo("a", "c1", "e1"), repo("b", "c1", "e2")]), RULES).unwrap();
    assert!(matches!(first.repos[0].1, RepoOutcome::Recorded { .. }));
    assert!(matches!(first.repos[1].1, RepoOutcome::Recorded { .. }));
    let link = first.link.expect("a link snapshot");
    assert_eq!(first.seq, 2);

    let same = store.commit_scan(&scan(vec![repo("a", "c1", "e3"), repo("b", "c1", "e4")]), RULES).unwrap();
    assert!(same.repos.iter().all(|(_, o)| matches!(o, RepoOutcome::Unchanged { .. })));
    assert_eq!((same.link, same.seq), (None, 2));

    let moved = store.commit_scan(&scan(vec![repo("a", "c2", "e5"), repo("b", "c1", "e6")]), RULES).unwrap();
    assert!(matches!(moved.repos[0].1, RepoOutcome::Recorded { seq: 3, .. }));
    assert!(matches!(moved.repos[1].1, RepoOutcome::Unchanged { .. }));
    assert_ne!(moved.link, Some(link));
    let active = store.active_set().unwrap();
    assert_eq!(active.link, moved.link);
    assert_eq!(active.repos.len(), 2);
    assert_eq!(store.facts(&active.snapshots()).unwrap().len(), 2);
}

#[test]
fn moving_back_and_forth_records_every_move() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    for (i, commit) in ["x", "a", "x", "a", "x"].iter().enumerate() {
        let done = store.commit_scan(&scan(vec![repo("r", commit, &format!("e{i}"))]), RULES).unwrap();
        assert!(matches!(done.repos[0].1, RepoOutcome::Recorded { .. }), "move {i} to {commit}");
    }
    assert_eq!(store.seq().unwrap(), 5);
}

#[test]
fn the_tail_sees_every_commit_through_data_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let tail = Tail::open(&dir.path().join("store.db")).unwrap().unwrap();
    let before = tail.data_version().unwrap();
    store
        .commit(Write { idem: "k".into(), request_hash: "h".into(), events: vec![version_event(1)] }, |_| {
            json!(null)
        })
        .unwrap();
    assert_ne!(tail.data_version().unwrap(), before);
    let events = tail.events_after(0, 10).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].body, EventBody::Version { ref label, .. } if label == "v1"));
}

#[test]
fn a_commit_hook_gets_each_committed_seq() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let (tx, rx) = std::sync::mpsc::channel();
    store.set_commit_hook(tx);
    let done = store
        .commit(Write { idem: "k".into(), request_hash: "h".into(), events: vec![version_event(1)] }, |_| {
            json!(null)
        })
        .unwrap();
    assert_eq!(rx.try_recv().unwrap(), done.seqs[0]);
}
