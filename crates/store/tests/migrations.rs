//! Migrations and the store-version check (contracts.md, Store schema and migrations; DR-73): a
//! process that waited on the write lock while a newer engine migrated the store refuses inside
//! its own transaction, so it never writes into a store it can't read.

use std::path::Path;
use std::time::Duration;

use planroom_contract::{
    Actor, Client, EventBody, EventId, PresenceRow, ScanStats, Status, Surface, Timestamp,
};
use planroom_store::{
    FactRow, Init, NewEvent, RepoScan, STORE_VERSION, ScanKey, ScanRules, ScanWrite, Shape, Store,
    StoreError, Tail, Write,
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

/// What `watch`'s live loop does with its tail: poll `data_version` every 200 ms and re-read the
/// store version whenever it moves (DR-78). Returns the error that ends the loop and when; a loop
/// still running after 5 s fails the test rather than hang it.
fn watch_until_refused(tail: &Tail) -> (StoreError, std::time::Instant) {
    let mut seen = tail.data_version().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        assert!(std::time::Instant::now() < deadline, "the watch never stopped after the migration");
        std::thread::sleep(Duration::from_millis(200));
        let now = tail.data_version().unwrap();
        if now != seen {
            seen = now;
            if let Err(e) = tail.check_version() {
                return (e, std::time::Instant::now());
            }
        }
    }
}

#[test]
fn an_older_watch_exits_after_a_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let mut older = Store::open(&path, &init()).unwrap();
    let tail = Tail::open(&path).unwrap().expect("the store exists");
    tail.check_version().expect("this build reads the store it created");

    let watching = std::thread::spawn(move || watch_until_refused(&tail));
    std::thread::sleep(Duration::from_millis(300));
    migrate_newer(&path, || {});
    let migrated = std::time::Instant::now();

    let (refused, at) = watching.join().unwrap();
    match refused {
        StoreError::SchemaMismatch { found, supported, writer_version } => {
            assert_eq!((found, supported), (STORE_VERSION + 1, STORE_VERSION));
            assert_eq!(writer_version.as_deref(), Some("9.9.0"), "it names the engine that wrote the store");
        }
        other => panic!("the watch must stop with schema_mismatch, got {other:?}"),
    }
    let took = at.saturating_duration_since(migrated);
    assert!(
        took < Duration::from_millis(400),
        "the watch noticed within one poll, took {} ms",
        took.as_millis()
    );
    // Its process's next write is refused too, so nothing lands in the migrated store.
    assert!(matches!(older.commit(write(), |_| json!({})), Err(StoreError::SchemaMismatch { .. })));
    assert_eq!(events(&path), 0);
}

#[test]
fn an_older_process_writes_no_meta_into_a_newer_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let mut older = Store::open(&path, &init()).unwrap();
    migrate_newer(&path, || {});

    // Every write transaction checks the store version, not only a commit's.
    let refused = [
        older.meta_insert_once("backup_ref", "refs/x").map(|_| ()),
        older.meta_swap("person_pubkey", None, "key", &[]).map(|_| ()),
        older.spend_token("digest", 1, 2).map(|_| ()),
        older.put_layout("map:system:runtime", "h", "{}"),
        older.touch_presence(
            &PresenceRow {
                actor: Actor::Person { id: "person:ann".into(), name: None },
                surface: Surface::Test,
                doing: None,
                focus: None,
                seen_at: Timestamp("2026-10-26T00:00:00Z".into()),
            },
            "tab_1",
        ),
    ];
    for result in refused {
        assert!(matches!(result, Err(StoreError::SchemaMismatch { .. })), "{result:?}");
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    let written: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM meta WHERE key IN ('backup_ref', 'person_pubkey', 'spent_tokens')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(written, 0, "the older process wrote no meta into the migrated store");
    let cached: i64 = conn
        .query_row("SELECT (SELECT COUNT(*) FROM layouts) + (SELECT COUNT(*) FROM presence)", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cached, 0, "nor a layout or a presence row");
}
