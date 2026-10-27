//! Migrations and the store-version check (contracts.md, Store schema and migrations; DR-73): a
//! process that waited on the write lock while a newer engine migrated the store refuses inside
//! its own transaction, so it never writes into a store it can't read.

use std::path::Path;
use std::time::Duration;

use planroom_contract::{Actor, Client, EventBody, EventId, ScanStats, Status, Surface, Timestamp};
use planroom_store::{
    FactRow, Init, NewEvent, RepoScan, STORE_VERSION, ScanKey, ScanRules, ScanWrite, Shape, Store,
    StoreError, Write,
};
use serde_json::json;

fn init() -> Init {
    Init {
        workspace_id: "ws-migrations".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

fn write() -> Write {
    Write {
        idem: "older-writer".into(),
        request_hash: "h".into(),
        events: vec![NewEvent {
            id: EventId("ev-older".into()),
            at: Timestamp("2026-10-26T00:00:00Z".into()),
            actor: Actor::Person { id: "person:ann".into(), name: None },
            client: Client { surface: Surface::Test, id: "tab_1".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: "v1".into() },
        }],
    }
}

fn scan() -> ScanWrite {
    ScanWrite {
        at: Timestamp("2026-10-26T00:00:00Z".into()),
        actor: Actor::System { part: "scan".into() },
        client: Client { surface: Surface::Test, id: "test".into() },
        packs_hash: "p".into(),
        repos: vec![RepoScan {
            repo: "orders".into(),
            commit: "c1".into(),
            inputs: "p".into(),
            branch: "main".into(),
            stats: ScanStats::default(),
            facts: vec![FactRow {
                id: "orders:repo:orders".into(),
                shape: Shape::Node,
                kind: "test.repo".into(),
                hash: "h".into(),
                body: "{}".into(),
            }],
            event_id: EventId("ev-scan".into()),
        }],
        started: 0,
    }
}

const RULES: ScanRules = ScanRules {
    scan_key: |k: &ScanKey<'_>| (format!("scan:{}:{}", k.repo, k.commit), "hash".into()),
    link_key: |members: &[u64], packs: &str| format!("link:{members:?}:{packs}"),
};

/// What a newer engine's migration does to the file: a higher `user_version` and its own engine
/// version in `meta`, in one IMMEDIATE transaction, which `ready` says it holds before it commits.
fn migrate_newer(path: &Path, ready: impl FnOnce()) {
    let mut conn = rusqlite::Connection::open(path).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
    ready();
    std::thread::sleep(Duration::from_millis(300));
    tx.pragma_update(None, "user_version", STORE_VERSION + 1).unwrap();
    tx.execute("UPDATE meta SET value = '9.9.0' WHERE key = 'engine_version'", []).unwrap();
    tx.commit().unwrap();
}

fn events(path: &Path) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0)).unwrap()
}

#[test]
fn an_older_writer_refuses_after_a_concurrent_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let mut older = Store::open(&path, &init()).unwrap();
    let mut older_scan = Store::open_existing(&path).unwrap().unwrap();

    // The newer engine takes the write lock first; the older process's commit and scan start
    // while it migrates, wait on the lock, and run only after the migration committed.
    let (held, holding) = std::sync::mpsc::channel();
    let newer = {
        let path = path.clone();
        std::thread::spawn(move || migrate_newer(&path, move || held.send(()).unwrap()))
    };
    holding.recv().unwrap();
    let committing = std::thread::spawn(move || older.commit(write(), |_| json!({})));
    let scanning = std::thread::spawn(move || older_scan.commit_scan(&scan(), RULES, &mut |_, _| true));
    newer.join().unwrap();

    for refused in [committing.join().unwrap().map(|_| ()), scanning.join().unwrap().map(|_| ())] {
        match refused {
            Err(StoreError::SchemaMismatch { found, supported, writer_version }) => {
                assert_eq!((found, supported), (STORE_VERSION + 1, STORE_VERSION));
                assert_eq!(
                    writer_version.as_deref(),
                    Some("9.9.0"),
                    "it names the engine that wrote the store"
                );
            }
            other => panic!("an older writer must refuse after the migration, got {other:?}"),
        }
    }
    assert_eq!(events(&path), 0, "the older process wrote nothing into the migrated store");
}
