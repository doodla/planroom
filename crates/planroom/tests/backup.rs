//! Safe migrations and the older engine's line (contracts.md, Store schema and migrations;
//! DR-53, DR-69): a migration copies the store with `VACUUM INTO` first, runs every step in one
//! transaction and keeps the copy when it fails; an engine older than the store refuses it and
//! names the fix.
//!
//! The migration tests plant their own migrations through the store's debug-only constructor and
//! register this process's data root as the engine does at start-up, with the real filesystem
//! type. They share that registration and the store's notices, so they run one at a time.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, OnceLock};

use planroom_contract::{Actor, Client, EventBody, EventId, Status, Surface, Timestamp};
use planroom_store::migrate::{MIGRATIONS, Migration};
use planroom_store::{Environment, Init, NewEvent, Notice, STORE_VERSION, Store, Write};
use serde_json::{Value, json};

/// This process's data root, registered once, and a guard that keeps the other tests out.
fn setup() -> (&'static Path, MutexGuard<'static, ()>) {
    static SERIAL: Mutex<()> = Mutex::new(());
    static DATA: OnceLock<tempfile::TempDir> = OnceLock::new();
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let data = DATA.get_or_init(|| {
        let data = tempfile::tempdir().unwrap();
        planroom_store::register(Environment {
            data_root: data.path().to_path_buf(),
            fs_type: planroom::app::fs_type::fs_type,
            engine_version: "0.2.0".into(),
        });
        data
    });
    planroom_store::take_notices();
    (data.path(), guard)
}

/// A directory-of-repos workspace's state directory, `.planroom/`, and its store path.
fn workspace(dir: &Path) -> (PathBuf, PathBuf) {
    let root = dir.join("shop");
    std::fs::create_dir_all(root.join(".planroom")).unwrap();
    let store = root.join(".planroom/store.db");
    (root, store)
}

fn init(id: &str) -> Init {
    Init {
        workspace_id: id.into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

fn commit(store: &mut Store, n: usize) {
    let write = Write {
        idem: format!("w{n}"),
        request_hash: "h".into(),
        events: vec![NewEvent {
            id: EventId(format!("e{n}")),
            at: Timestamp("2026-10-26T00:00:00Z".into()),
            actor: Actor::Person { id: "person:ann".into(), name: None },
            client: Client { surface: Surface::Test, id: "test".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: format!("v{n}") },
        }],
    };
    store.commit(write, |_| json!({})).unwrap();
}

/// One value a query returns on the database at `path`, read by a process of its own.
fn sql(path: &Path, query: &str) -> String {
    let script =
        "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); print(c.execute(sys.argv[2]).fetchone()[0])";
    let out = Command::new("python3").args(["-c", script]).arg(path).arg(query).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// This build's migrations, then a good one, then one that fails partway through its SQL.
static FAILING: &[Migration] = &[
    MIGRATIONS[0],
    (2, "CREATE TABLE planted_ok (x INTEGER); INSERT INTO planted_ok VALUES (1);"),
    (3, "CREATE TABLE planted_bad (x INTEGER); INSERT INTO no_such_table VALUES (1);"),
];

#[test]
fn a_failed_migration_rolls_back() {
    let (data, _serial) = setup();
    let dir = tempfile::tempdir().unwrap();
    let (_, path) = workspace(dir.path());
    let mut store = Store::open(&path, &init("ws-failed")).unwrap();
    commit(&mut store, 1);
    commit(&mut store, 2);
    drop(store);

    // A process killed while it wrote a copy left its temporary file behind, unwritten for an
    // hour; another store that shares this workspace ID is writing one now.
    let killed = data.join("backups/ws-failed/.store-v1.db.4242-0.tmp");
    let writing = data.join("backups/ws-failed/.store-v1.db.4243-0.tmp");
    std::fs::create_dir_all(killed.parent().unwrap()).unwrap();
    for tmp in [&killed, &writing] {
        std::fs::write(tmp, "half a copy").unwrap();
    }
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&killed).unwrap().set_modified(hour_ago).unwrap();

    let failed = Store::open_with_migrations(&path, &init("ws-failed"), FAILING).err().expect("it fails");
    let copy = data.join("backups/ws-failed/store-v1.db");
    assert!(failed.to_string().contains(&copy.display().to_string()), "the error names the copy: {failed}");

    // Every step rolled back, the good one too: the store is exactly as it was.
    assert_eq!(sql(&path, "PRAGMA user_version"), "1");
    assert_eq!(sql(&path, "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'planted%'"), "0");
    assert_eq!(sql(&path, "SELECT value FROM meta WHERE key = 'store_version'"), "1");
    assert_eq!(sql(&path, "SELECT COUNT(*) FROM events"), "2");
    let reopened = Store::open_existing(&path).unwrap().expect("this build still opens it");
    assert_eq!(reopened.seq().unwrap(), 2);

    // The copy stays, whole, private, with every event.
    assert_eq!(sql(&copy, "PRAGMA integrity_check"), "ok");
    assert_eq!(sql(&copy, "PRAGMA user_version"), "1");
    assert_eq!(sql(&copy, "SELECT COUNT(*) FROM events"), "2");
    assert_eq!(mode(&copy), 0o600);
    assert_eq!(mode(copy.parent().unwrap()), 0o700);
    assert_eq!(mode(&data.join("backups")), 0o700);
    let leftovers: Vec<_> = std::fs::read_dir(copy.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name != "store-v1.db")
        .collect();
    assert_eq!(
        leftovers,
        vec![".store-v1.db.4243-0.tmp".to_owned()],
        "no temporary file is left beside the copy but the one another store is writing"
    );
    let migrated =
        planroom_store::take_notices().into_iter().filter(|n| matches!(n, Notice::Migrated { .. }));
    assert_eq!(migrated.count(), 0, "a failed migration reports no store_migrated");
}

/// This build's migrations and `n` more good ones, one table each.
fn grown(n: usize) -> &'static [Migration] {
    let steps: Vec<Migration> = std::iter::once(MIGRATIONS[0])
        .chain((0..n).map(|i| {
            let sql: &'static str =
                Box::leak(format!("CREATE TABLE grown_{i} (x INTEGER);").into_boxed_str());
            ((i + 2) as u32, sql)
        }))
        .collect();
    Box::leak(steps.into_boxed_slice())
}

#[test]
fn a_successful_migration_reports_its_copy_and_keeps_the_last_three() {
    let (data, _serial) = setup();
    let dir = tempfile::tempdir().unwrap();
    let (root, path) = workspace(dir.path());
    let mut store = Store::open(&path, &init("ws-grown")).unwrap();
    commit(&mut store, 1);
    drop(store);
    let copies = data.join("backups/ws-grown");
    assert!(!copies.exists(), "a new store, from version 0, takes no copy");

    let store = Store::open_with_migrations(&path, &init("ws-grown"), grown(1)).unwrap();
    drop(store);
    assert_eq!(sql(&path, "PRAGMA user_version"), "2");
    assert_eq!(sql(&path, "SELECT value FROM meta WHERE key = 'engine_version'"), "0.2.0");
    let backup = copies.join("store-v1.db");
    assert_eq!(
        planroom_store::take_notices(),
        vec![Notice::Migrated { from: 1, to: 2, backup: backup.clone() }],
        "the migration reports store_migrated with its copy"
    );
    assert_eq!(sql(&backup, "PRAGMA user_version"), "1");
    assert_eq!(sql(&backup, "SELECT COUNT(*) FROM events"), "1");

    // The warning a command's reply carries, with the copy's path masked like any path.
    let warning =
        planroom::app::fs_type::warning(Notice::Migrated { from: 1, to: 2, backup: backup.clone() });
    assert_eq!(warning.code.as_str(), "store_migrated");
    assert_eq!(warning.details, json!({"from": 1, "to": 2, "backup": backup.display().to_string()}));

    for n in 2..=4 {
        drop(Store::open_with_migrations(&path, &init("ws-grown"), grown(n)).unwrap());
    }
    let mut kept: Vec<String> =
        std::fs::read_dir(&copies).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    kept.sort();
    assert_eq!(kept, ["store-v2.db", "store-v3.db", "store-v4.db"], "the last three copies are kept");
    drop(root);
}

#[test]
fn an_older_engine_says_restart_once() {
    let dir = tempfile::tempdir().unwrap();
    let (root, path) = workspace(dir.path());
    let mut store = Store::open(&path, &init("ws-newer")).unwrap();
    commit(&mut store, 1);
    drop(store);
    // A newer engine's migration, as far as this one can tell.
    let script = format!(
        "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); \
         c.execute(\"UPDATE meta SET value='9.9.0' WHERE key='engine_version'\"); \
         c.execute('PRAGMA user_version = {}'); c.commit()",
        STORE_VERSION + 1
    );
    let out = Command::new("python3").args(["-c", &script]).arg(&path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let data = dir.path().join("data");
    for command in [["focus", "--json"], ["map", "--json"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
            .args(command)
            .current_dir(&root)
            .env("PLANROOM_DATA", &data)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(7), "{command:?}");
        let env: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(env["ok"], false, "{env}");
        let error = &env["error"];
        assert_eq!(error["code"], "schema_mismatch", "{env}");
        assert_eq!(error["details"]["found"], STORE_VERSION + 1);
        assert_eq!(error["details"]["supported"], STORE_VERSION);
        assert_eq!(error["details"]["writer_version"], "9.9.0", "it names the engine that wrote the store");
        // The fix is the update the install docs name, as commands typed in Claude Code.
        assert_eq!(
            error["hint"],
            "/plugin marketplace update planroom, then /plugin update planroom@planroom, typed in Claude Code; then restart this session"
        );
    }
    // The older engine neither wrote to the store nor tried to migrate it.
    assert_eq!(sql(&path, "PRAGMA user_version"), (STORE_VERSION + 1).to_string());
    assert_eq!(sql(&path, "SELECT COUNT(*) FROM events"), "1");
    assert!(!data.join("backups").exists(), "no migration copy: it never migrates down");
}
