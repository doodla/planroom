//! The user-level ledger (contracts.md, Store schema and migrations; DR-51, DR-78): it changes
//! only by additions, so an older engine keeps reading and appending to a newer ledger until a
//! migration raises `min_reader_version`; and a ledger migration keeps a checked `VACUUM INTO`
//! copy, the last three of them.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use planroom_store::migrate::Migration;
use planroom_store::{LEDGER_MIGRATIONS, Ledger, LedgerRow, StoreError};

const INIT: &str = include_str!("../ledger/0001_init.sql");

/// A newer engine's ledger: one added column.
const NEWER: &[Migration] = &[(1, INIT), (2, "ALTER TABLE ledger ADD COLUMN region TEXT;")];

/// A newer engine still, whose migration says older engines would misread the file.
const RAISED: &[Migration] = &[
    (1, INIT),
    (2, "ALTER TABLE ledger ADD COLUMN region TEXT;"),
    (3, "UPDATE meta SET value = '0.2.0' WHERE key = 'min_reader_version';"),
];

/// A migration that fails halfway: its first statement applies, its second can't.
const FAILING: &[Migration] = &[
    (1, INIT),
    (2, "ALTER TABLE ledger ADD COLUMN region TEXT;"),
    (3, "ALTER TABLE ledger ADD COLUMN zone TEXT; INSERT INTO no_such_table VALUES (1);"),
];

const V3: &[Migration] = &[(1, INIT), (2, "SELECT 1;"), (3, "SELECT 1;")];
const V4: &[Migration] = &[(1, INIT), (2, "SELECT 1;"), (3, "SELECT 1;"), (4, "SELECT 1;")];
const V5: &[Migration] = &[(1, INIT), (2, "SELECT 1;"), (3, "SELECT 1;"), (4, "SELECT 1;"), (5, "SELECT 1;")];

fn row(purpose: &str) -> LedgerRow {
    LedgerRow {
        at: "2026-10-28T12:00:00Z".into(),
        workspace_id: Some("ws-ledger".into()),
        provider: "gateway".into(),
        model: "anthropic/claude-haiku-5.5".into(),
        purpose: purpose.into(),
        scope: "day".into(),
        uncached_in: Some(120),
        cache_read: Some(4000),
        cache_write: Some(0),
        output: Some(80),
        cost_usd: Some(0.0021),
        prefix_hash: Some("ab12".into()),
        generation_id: Some("gen_1".into()),
        replayed: false,
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Read the file at `path` directly, as SQLite sees it.
fn raw(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn user_version(path: &Path) -> u32 {
    raw(path).pragma_query_value(None, "user_version", |r| r.get(0)).unwrap()
}

fn copies(data: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(data.join("backups")) else { return vec![] };
    let mut names: Vec<String> = entries
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with("ledger-v"))
        .collect();
    names.sort();
    names
}

#[test]
fn an_older_engine_reads_a_newer_ledger() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let file = data.join("ledger.db");

    // A newer engine creates the ledger at version 2 (from nothing, so no copy) and records a row.
    let mut newer = Ledger::open_with_migrations(&data, "0.2.0", NEWER).unwrap();
    let first = newer.record(&row("plan.stage.shape")).unwrap();
    drop(newer);

    // An older engine opens it as it is: no migration down, no copy, its own columns read.
    let mut older = Ledger::open_at(&data, "0.1.0").unwrap();
    assert_eq!(user_version(&file), 2);
    assert_eq!(copies(&data), Vec::<String>::new());
    assert_eq!(older.rows(0, 10).unwrap(), vec![(first, row("plan.stage.shape"))]);
    let second = older.record(&row("plan.stage.ask")).unwrap();
    assert!(second > first);
    let region: Option<String> =
        raw(&file).query_row("SELECT region FROM ledger WHERE id = ?1", [second], |r| r.get(0)).unwrap();
    assert_eq!(region, None, "an older engine's row leaves a newer column NULL");
    // Never a write-ahead log: its mode would be stored in the file and bind every engine.
    let journal: String = raw(&file).query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(journal, "delete");
    assert!(!data.join("ledger.db-wal").exists());
    drop(older);

    // A migration that raises `min_reader_version` past the older engine makes it refuse,
    // naming the file, and leave it as it was.
    Ledger::open_with_migrations(&data, "0.2.0", RAISED).unwrap();
    let before = fs::read(&file).unwrap();
    match Ledger::open_at(&data, "0.1.0") {
        Err(StoreError::NewerFile { file: named, found, supported }) => {
            assert_eq!((named, found.as_str(), supported.as_str()), (file.clone(), "0.2.0", "0.1.0"));
        }
        Err(other) => panic!("expected NewerFile, got {other}"),
        Ok(_) => panic!("expected NewerFile, got an open ledger"),
    }
    assert_eq!(fs::read(&file).unwrap(), before, "a refused ledger is left as it was");
    let refused = StoreError::NewerFile { file, found: "0.2.0".into(), supported: "0.1.0".into() };
    assert_eq!(refused.code(), planroom_contract::ErrorCode::SchemaMismatch);
}

#[test]
fn a_raised_min_reader_version_refuses_a_ledger_already_open() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    Ledger::open_with_migrations(&data, "0.2.0", NEWER).unwrap();
    let mut older = Ledger::open_at(&data, "0.1.0").unwrap();
    // Another engine raises it while this one holds the file open: its next row refuses.
    Ledger::open_with_migrations(&data, "0.2.0", RAISED).unwrap();
    assert!(matches!(older.record(&row("plan.stage.shape")), Err(StoreError::NewerFile { .. })));
    assert_eq!(Ledger::open_with_migrations(&data, "0.2.0", RAISED).unwrap().rows(0, 10).unwrap(), vec![]);
}

#[test]
fn a_ledger_migration_keeps_a_copy() {
    let tmp = tempfile::tempdir().unwrap();
    // A data root no command has created yet is created private, with its parents.
    let data = tmp.path().join("deep").join("data");
    let file = data.join("ledger.db");
    let mut v1 = Ledger::open_at(&data, "0.1.0").unwrap();
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&file), 0o600);
    assert_eq!(user_version(&file), LEDGER_MIGRATIONS.len() as u32);
    let id = v1.record(&row("plan.stage.shape")).unwrap();
    drop(v1);

    // The migration from 1 to 2 keeps a checked copy of version 1 in a private backups folder.
    let v2 = Ledger::open_with_migrations(&data, "0.2.0", NEWER).unwrap();
    let copy = data.join("backups").join("ledger-v1.db");
    assert_eq!(copies(&data), ["ledger-v1.db"]);
    assert_eq!(mode(&copy), 0o600);
    assert_eq!(mode(&data.join("backups")), 0o700);
    let check: String = raw(&copy).query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(check, "ok");
    assert_eq!(user_version(&copy), 1);
    let held: String =
        raw(&copy).query_row("SELECT purpose FROM ledger WHERE id = ?1", [id], |r| r.get(0)).unwrap();
    assert_eq!(held, "plan.stage.shape");
    assert_eq!(user_version(&file), 2);
    assert_eq!(v2.rows(0, 10).unwrap(), vec![(id, row("plan.stage.shape"))]);
    drop(v2);

    // A failing migration rolls back to the version it found, keeps its copy and says where.
    let err = match Ledger::open_with_migrations(&data, "0.3.0", FAILING) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("the planted migration fails"),
    };
    assert!(err.contains("ledger-v2.db"), "{err}");
    assert_eq!(user_version(&file), 2);
    let zone = raw(&file).prepare("SELECT zone FROM ledger").is_ok();
    assert!(!zone, "the failed migration's first statement was rolled back too");
    assert_eq!(copies(&data), ["ledger-v1.db", "ledger-v2.db"]);

    // Successive migrations keep the last three copies.
    for migrations in [V3, V4, V5] {
        Ledger::open_with_migrations(&data, "0.3.0", migrations).unwrap();
    }
    assert_eq!(copies(&data), ["ledger-v2.db", "ledger-v3.db", "ledger-v4.db"]);
    assert_eq!(user_version(&file), 5);
    // No temporary copy is left behind.
    let leftovers: Vec<_> = fs::read_dir(data.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[test]
fn an_existing_data_root_keeps_its_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    fs::create_dir(&data).unwrap();
    fs::set_permissions(&data, fs::Permissions::from_mode(0o750)).unwrap();
    Ledger::open_at(&data, "0.1.0").unwrap();
    assert_eq!(mode(&data), 0o750);
    assert_eq!(mode(&data.join("ledger.db")), 0o600);
}
