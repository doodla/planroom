//! Reads interleaved with writes from other connections (DR-73): a reply folds what one read
//! transaction holds, and its `meta.seq` is that transaction's highest `seq`, so a write that
//! lands between two of its reads never shows in one read and not another, and the reply never
//! claims a `seq` its content doesn't reflect.

use std::path::Path;

use planroom_contract::{Actor, Client, EventBody, EventId, ScanStats, Seq, Status, Surface, Timestamp};
use planroom_store::{
    FactRow, Init, NewEvent, RepoScan, ScanKey, ScanRules, ScanWrite, Shape, Snapshot, Store, Tail, Write,
};
use serde_json::json;

fn init() -> Init {
    Init {
        workspace_id: "ws-interleave".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-27T00:00:00Z".into(),
    }
}

const RULES: ScanRules = ScanRules {
    scan_key: |k: &ScanKey<'_>| {
        let key = format!("scan:{}:{}:{}:{:?}", k.repo, k.commit, k.inputs, k.previous);
        (key.clone(), format!("hash:{key}"))
    },
    link_key: |members: &[u64], packs: &str| format!("link:{members:?}:{packs}"),
};

/// A scan of repo `r` at `commit`, whose one fact names the commit, so each snapshot's facts
/// say which scan made them.
fn scan_at(commit: &str, event: &str) -> ScanWrite {
    ScanWrite {
        at: Timestamp("2026-10-27T00:00:00Z".into()),
        actor: Actor::System { part: "scan".into() },
        client: Client { surface: Surface::Cli, id: "cli:1".into() },
        packs_hash: "p".into(),
        repos: vec![RepoScan {
            repo: "r".into(),
            commit: commit.into(),
            inputs: "p".into(),
            branch: "main".into(),
            stats: ScanStats::default(),
            facts: vec![FactRow {
                id: "r:repo:r".into(),
                shape: Shape::Node,
                kind: "test.repo".into(),
                hash: format!("h-{commit}"),
                body: json!({"commit": commit}).to_string(),
            }],
            event_id: EventId(event.into()),
        }],
        started: 0,
    }
}

fn version(n: usize) -> Write {
    let event = NewEvent {
        id: EventId(format!("v-{n}")),
        at: Timestamp("2026-10-27T00:00:00Z".into()),
        actor: Actor::System { part: "test".into() },
        client: Client { surface: Surface::Test, id: "test".into() },
        project: None,
        cause: None,
        status: Status::Applied,
        target: None,
        body: EventBody::Version { project: None, label: format!("v{n}") },
    };
    Write { idem: format!("v-{n}"), request_hash: "h".into(), events: vec![event] }
}

/// What one reply folds: the commit its active facts name, its events' highest `seq`, and the
/// `seq` it reports. `between` runs after each of the reads, as another writer's commit would.
fn read_reply(snapshot: &Snapshot<'_>, between: &mut dyn FnMut()) -> (String, Seq, Seq) {
    let seq = snapshot.seq();
    between();
    let active = snapshot.active_set().unwrap();
    between();
    let facts = snapshot.facts(&active.snapshots()).unwrap();
    between();
    let events = snapshot.events_after(0).unwrap();
    between();
    assert!(snapshot.event_at(seq + 1).unwrap().is_none(), "no event past the snapshot's seq");
    let fact = facts.iter().find(|f| f.id == "r:repo:r").expect("the repo's fact");
    let commit =
        serde_json::from_str::<serde_json::Value>(&fact.body).unwrap()["commit"].as_str().unwrap().to_owned();
    (commit, events.last().map_or(0, |e| e.seq), seq)
}

fn open(dir: &Path) -> (Store, Store) {
    let path = dir.join("store.db");
    let reader = Store::open(&path, &init()).unwrap();
    let writer = Store::open_existing(&path).unwrap().unwrap();
    (reader, writer)
}

#[test]
fn meta_seq_never_exceeds_the_folded_view() {
    let tmp = tempfile::tempdir().unwrap();
    let (reader, mut writer) = open(tmp.path());
    writer.commit_scan(&scan_at("c0", "s0"), RULES, &mut |_, _| true).unwrap();

    // A write lands after each read in turn: the scan moves the repo, then a version tags it.
    let mut n = 0;
    for _ in 0..4 {
        let before = reader.seq().unwrap();
        let snapshot = reader.snapshot().unwrap();
        let committed = std::cell::Cell::new(false);
        let mut write = || {
            if !committed.get() {
                n += 1;
                let commit = format!("c{n}");
                writer.commit_scan(&scan_at(&commit, &format!("s{n}")), RULES, &mut |_, _| true).unwrap();
                writer.commit(version(n), |seqs| json!({"seq": seqs[0]})).unwrap();
                committed.set(true);
            }
        };
        let (commit, last_event, seq) = read_reply(&snapshot, &mut write);
        assert!(committed.get());
        assert_eq!(seq, before, "the snapshot's seq is the store's when it began");
        assert_eq!(last_event, seq, "the reply's seq is the last event it folded, never one past it");
        assert_eq!(commit, format!("c{}", n - 1), "the facts are the active set's at that seq");
        drop(snapshot);
        // The writes landed: the next snapshot reads them, and only them.
        let after = reader.snapshot().unwrap();
        let (commit, last_event, seq) = read_reply(&after, &mut || {});
        assert_eq!((commit, last_event, seq), (format!("c{n}"), seq, reader.seq().unwrap()));
        assert_eq!(seq, before + 2);
    }

    // The live loop's read-only connection takes the same snapshots.
    let tail = Tail::open(&tmp.path().join("store.db")).unwrap().unwrap();
    let snapshot = tail.snapshot().unwrap();
    let seq = snapshot.seq();
    writer.commit(version(99), |seqs| json!({"seq": seqs[0]})).unwrap();
    assert_eq!(snapshot.events_after(0).unwrap().last().map(|e| e.seq), Some(seq));
    assert_eq!(snapshot.meta("workspace_id").unwrap().as_deref(), Some("ws-interleave"));
    drop(snapshot);
    assert_eq!(tail.seq().unwrap(), seq + 1);
}
