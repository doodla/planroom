//! Safe migrations and the older engine's line (contracts.md, Store schema and migrations;
//! DR-53, DR-69): a migration copies the store with `VACUUM INTO` first, runs every step in one
//! transaction and keeps the copy when it fails; an engine older than the store refuses it and
//! names the fix. Every person-authored commit appends the events above the backup file's last
//! `seq` to `DATA_ROOT/backups/<workspace_id>/events.ndjson`, so a one-shot process loses nothing
//! and a later one fills a gap. On a temporary host the same step commits a redacted copy of that
//! file to `refs/planroom/backup/<workspace_id>` in one workspace repo, and changes nothing else
//! there.
//!
//! The migration tests plant their own migrations through the store's debug-only constructor and
//! register this process's data root as the engine does at start-up, with the real filesystem
//! type. They share that registration and the store's notices, so they run one at a time. Their
//! writes are an agent's, so no event backup joins the directories they list. The backup tests
//! spawn the binary, each with a data root of its own, and sign each person write with a test
//! key registered through `person init`, as the mod does.

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
        planroom_store::restore::register_replay(planroom::app::doctor::restore::replay);
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
            actor: Actor::Agent { session: "agent:1a2b3c4d:chat".into(), role: "chat".into(), key: None },
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

// The event backup, through the binary.

/// A command in `root` with the data root `data`, and none of the variables that mark a temporary
/// host or name a writer.
fn planroom(root: &Path, data: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", data)
        .env("XDG_CONFIG_HOME", data.join("config"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT");
    for name in ["CLAUDE_CODE_REMOTE", "CODESPACES", "REMOTE_CONTAINERS"] {
        cmd.env_remove(name);
    }
    cmd
}

/// `planroom ARGS --json` with `env` and `stdin`; the exit code and the one envelope on stdout.
fn run(root: &Path, data: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    use std::io::Write as _;
    use std::process::Stdio;
    let mut cmd = planroom(root, data);
    cmd.args(args).arg("--json");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

/// A test key pair from a fixed scalar.
fn key() -> p256::ecdsa::SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = 7;
    bytes[0] = 1;
    p256::ecdsa::SigningKey::from_slice(&bytes).unwrap()
}

fn b64url(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// A person's token over `command` with the argv after it, as the mod signs one.
fn token(command: &str, args: &[&str], base: &str) -> String {
    use p256::ecdsa::Signature;
    use p256::ecdsa::signature::Signer;
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = now();
    let sig: Signature =
        key().sign(&planroom_kernel::keys::person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// Scan the workspace at `root` and register the test person's key.
fn person_init(root: &Path, data: &Path) {
    let (code, env) = run(root, data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    register_key(root, data);
}

/// Register the test person's key in the workspace's store, which a scan made.
fn register_key(root: &Path, data: &Path) {
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let (code, env) = run(
        root,
        data,
        &["person", "init"],
        &[("PLANROOM_PERSON_TOKEN", &token("person init", &[], ""))],
        &public,
    );
    assert_eq!(code, 0, "{env}");
}

/// A scanned copy of the fixture with the test person's key registered, and a data root beside it.
fn fixture_workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    planroom_testkit::fixture(&root);
    let data = tmp.path().join("data");
    person_init(&root, &data);
    (tmp, root, data)
}

/// `planroom edit --base BASE ARGS` as the person, signed; the `seq` it wrote.
fn edit(root: &Path, data: &Path, base: u64, op: &[&str]) -> u64 {
    signed_edit(root, data, base, op, &[])["meta"]["seq"].as_u64().unwrap()
}

/// [`edit`] with `env` set too; its whole envelope.
fn signed_edit(root: &Path, data: &Path, base: u64, op: &[&str], env: &[(&str, &str)]) -> Value {
    let base = base.to_string();
    let args: Vec<&str> = ["--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
    let signed = token("edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args.iter().copied()).collect();
    let vars: Vec<(&str, &str)> =
        env.iter().copied().chain([("PLANROOM_PERSON_TOKEN", signed.as_str())]).collect();
    let (code, env) = run(root, data, &argv, &vars, "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["ok"], true, "{env}");
    env
}

/// The store's head, as `map` reports it.
fn head(root: &Path, data: &Path) -> u64 {
    let (code, env) = run(root, data, &["map"], &[], "");
    assert_eq!(code, 0, "{env}");
    env["meta"]["seq"].as_u64().unwrap()
}

/// The workspace's store, opened in this process to read it.
fn open(state: &Path) -> Store {
    Store::open_existing(&state.join("store.db")).unwrap().expect("a store")
}

/// Every event `seq` in the store, in order.
fn store_seqs(state: &Path) -> Vec<u64> {
    open(state).events_after(0, 1_000_000).unwrap().iter().map(|e| e.seq).collect()
}

/// The backup file of the workspace whose state is at `state`.
fn backup_file(state: &Path, data: &Path) -> PathBuf {
    let id = open(state).meta("workspace_id").unwrap().unwrap();
    data.join("backups").join(id).join("events.ndjson")
}

/// The backup file's header and every event after it, each line whole.
fn backed_up(file: &Path) -> (planroom_contract::ExportHeader, Vec<planroom_contract::Event>) {
    let text = std::fs::read_to_string(file).unwrap();
    assert!(text.ends_with('\n'), "every line is whole");
    let mut lines = text.lines();
    let header = planroom_contract::format::read_str(lines.next().unwrap()).unwrap();
    let events = lines.map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect();
    (header, events)
}

fn seqs(events: &[planroom_contract::Event]) -> Vec<u64> {
    events.iter().map(|e| e.seq).collect()
}

fn backup_seq(state: &Path) -> Option<u64> {
    open(state).meta("backup_seq").unwrap().map(|v| v.parse().unwrap())
}

/// Run `statement` with `args` on the database at `path`, from a process of its own.
fn sql_exec(path: &Path, statement: &str, args: &[&str]) {
    let script = "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute(sys.argv[2], sys.argv[3:]); c.commit()";
    let out =
        Command::new("python3").args(["-c", script]).arg(path).arg(statement).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// The names of the code-level nodes, paging the map until `n` are found.
fn node_ids(root: &Path, data: &Path, n: usize) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..50 {
        let mut args = vec!["map", "--level", "code"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c.as_str()]);
        }
        let (code, env) = run(root, data, &args, &[], "");
        assert_eq!(code, 0, "{env}");
        for node in env["result"]["nodes"].as_array().unwrap() {
            let id = node["id"].as_str().unwrap().to_owned();
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.len() >= n {
            ids.truncate(n);
            return ids;
        }
        match env["meta"]["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    panic!("the map holds {} nodes, not {n}: {ids:?}", ids.len());
}

#[test]
fn a_burst_of_three_edits_is_fully_backed_up() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let file = backup_file(&state, &data);
    let mut base = head(&root, &data);
    for name in ["shopfront", "market", "bazaar"] {
        let written = edit(&root, &data, base, &["rename", "storefront:container:storefront", name]);
        // No debounce: the one-shot process left its event in the file before it exited.
        let (_, events) = backed_up(&file);
        let last = events.last().unwrap();
        assert_eq!(last.seq, written, "{name}");
        assert!(matches!(last.actor, Actor::Person { .. }), "{name}");
        base = written;
    }
    let (header, events) = backed_up(&file);
    assert_eq!(seqs(&events), store_seqs(&state), "every store event once, in order, scan events included");
    assert_eq!(header.workspace_id, open(&state).meta("workspace_id").unwrap().unwrap());
    assert_eq!(header.store_version, STORE_VERSION);
    assert_eq!(backup_seq(&state), Some(base), "the mark is the head");
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(file.parent().unwrap()), 0o700);
}

#[test]
fn concurrent_appenders_keep_order() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let ids = node_ids(&root, &data, 20);
    let h = head(&root, &data);
    std::thread::scope(|scope| {
        for (t, mine) in ids.chunks(10).enumerate() {
            let (root, data) = (&root, &data);
            scope.spawn(move || {
                for (i, id) in mine.iter().enumerate() {
                    edit(root, data, h, &["rename", id, &format!("t{t}-{i}")]);
                }
            });
        }
    });
    let all = store_seqs(&state);
    assert_eq!(all.iter().filter(|s| **s > h).count(), 20, "one event per edit");
    let (_, events) = backed_up(&backup_file(&state, &data));
    let backed = seqs(&events);
    assert!(backed.windows(2).all(|w| w[0] < w[1]), "strictly increasing: {backed:?}");
    assert_eq!(backed, all);
    assert_eq!(backup_seq(&state), all.last().copied());
}

#[test]
fn a_later_process_catches_up_a_gap() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let file = backup_file(&state, &data);
    let id = "storefront:container:storefront";
    let first = edit(&root, &data, head(&root, &data), &["rename", id, "one"]);
    let second = edit(&root, &data, first, &["rename", id, "two"]);

    // A kill between the commit and the append: the second edit's line and mark never landed.
    let text = std::fs::read_to_string(&file).unwrap();
    let cut = text.trim_end_matches('\n').rfind('\n').unwrap() + 1;
    std::fs::write(&file, &text[..cut]).unwrap();
    sql_exec(
        &state.join("store.db"),
        "UPDATE meta SET value = ?1 WHERE key = 'backup_seq'",
        &[&first.to_string()],
    );
    let third = edit(&root, &data, second, &["rename", id, "three"]);
    let (_, events) = backed_up(&file);
    assert_eq!(seqs(&events), store_seqs(&state), "the gap is filled, each event once");
    assert_eq!(backup_seq(&state), Some(third));

    // A kill partway through a line: the half line is cut before the next append.
    let text = std::fs::read_to_string(&file).unwrap();
    let start = text.trim_end_matches('\n').rfind('\n').unwrap() + 1;
    let half = start + (text.len() - start) / 2;
    std::fs::write(&file, &text[..half]).unwrap();
    let fourth = edit(&root, &data, third, &["rename", id, "four"]);
    let (_, events) = backed_up(&file);
    assert_eq!(seqs(&events), store_seqs(&state));
    assert_eq!(backup_seq(&state), Some(fourth));

    // A header that names another workspace: the file is renamed aside and a whole new one written.
    let text = std::fs::read_to_string(&file).unwrap();
    let (line, rest) = text.split_once('\n').unwrap();
    let mut header: Value = serde_json::from_str(line).unwrap();
    header["workspace_id"] = json!("someone-else");
    let planted = format!("{header}\n{rest}");
    std::fs::write(&file, &planted).unwrap();
    edit(&root, &data, fourth, &["rename", id, "five"]);
    let (header, events) = backed_up(&file);
    assert_eq!(header.workspace_id, open(&state).meta("workspace_id").unwrap().unwrap());
    assert_eq!(seqs(&events), store_seqs(&state), "a whole new file");
    let aside: Vec<String> = std::fs::read_dir(file.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("events-") && n.ends_with(".ndjson"))
        .collect();
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read_to_string(file.with_file_name(&aside[0])).unwrap(), planted, "kept as it was");
}

#[test]
fn git_clean_leaves_a_single_repos_state_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let app = tmp.path().join("app");
    planroom_testkit::git_repo(
        &app,
        &[
            ("docker-compose.yml".into(), b"services:\n  web:\n    image: nginx:1\n".to_vec()),
            ("src/main.rs".into(), b"fn main() {}\n".to_vec()),
        ],
    );
    let app = app.canonicalize().unwrap();
    person_init(&app, &data);
    let written = edit(&app, &data, head(&app, &data), &["rename", "app:repo:app", "shopfront"]);
    let state = app.join(".git/planroom");
    let file = backup_file(&state, &data);
    let renamed = |app: &Path| {
        let (code, map) = run(app, &data, &["map"], &[], "");
        assert_eq!(code, 0, "{map}");
        map["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == "app:repo:app").unwrap()["name"]
            .clone()
    };
    assert_eq!(renamed(&app), "shopfront");
    let (events, backup) = (store_seqs(&state), std::fs::read(&file).unwrap());
    assert_eq!(events.last().copied(), Some(written));

    std::fs::write(app.join("scratch.log"), "x").unwrap();
    let out = Command::new("git").args(["clean", "-xdf"]).current_dir(&app).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(!app.join("scratch.log").exists(), "git clean ran");
    assert_eq!(store_seqs(&state), events, "the store keeps every event");
    assert_eq!(renamed(&app), "shopfront", "the map keeps the edit");
    assert_eq!(std::fs::read(&file).unwrap(), backup, "the backup is unchanged");
}

/// A cloud session, as `CLAUDE_CODE_REMOTE` marks one: a temporary host.
const TEMPORARY_HOST: (&str, &str) = ("CLAUDE_CODE_REMOTE", "true");

/// Plain `git ARGS` in `dir`; its stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// The store's `meta.repos`: each repo's path from the root, as its key, and its namespace.
fn recorded_repos(state: &Path) -> std::collections::BTreeMap<String, String> {
    serde_json::from_str(&open(state).meta(planroom_store::META_REPOS).unwrap().unwrap()).unwrap()
}

/// The backup ref's segments in name order, concatenated.
fn backup_ref_text(repo: &Path, reference: &str) -> String {
    let names = git(repo, &["ls-tree", "--name-only", reference]);
    let mut names: Vec<&str> = names.lines().collect();
    names.sort_unstable();
    names.iter().map(|name| git(repo, &["cat-file", "-p", &format!("{reference}:{name}")])).collect()
}

/// Every value under a `target` key in `value`, depth first.
fn targets(value: &Value) -> Vec<Value> {
    match value {
        Value::Object(map) => {
            map.iter().flat_map(|(k, v)| if k == "target" { vec![v.clone()] } else { targets(v) }).collect()
        }
        Value::Array(items) => items.iter().flat_map(targets).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn a_temporary_host_writes_the_backup_ref() {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let reference = format!("refs/planroom/backup/{}", open(&state).meta("workspace_id").unwrap().unwrap());
    let repos = recorded_repos(&state);
    assert!(repos.len() > 1, "{repos:?}");
    let (first_key, first_ns) = repos.iter().min_by_key(|(_, ns)| ns.as_str()).unwrap();
    let first = root.join(first_key);

    // The repo's own config would sign the commit with a program of its own.
    let (marker, program) = (tmp.path().join("signed"), tmp.path().join("gpg.sh"));
    std::fs::write(&program, format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display())).unwrap();
    std::fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    git(&first, &["config", "commit.gpgSign", "true"]);
    git(&first, &["config", "gpg.program", program.to_str().unwrap()]);

    let looks = |repo: &Path| {
        ["rev-parse HEAD", "for-each-ref refs/heads", "status --porcelain --ignored"]
            .map(|args| git(repo, &args.split(' ').collect::<Vec<_>>()))
    };
    let before: Vec<_> = repos.keys().map(|key| looks(&root.join(key))).collect();

    let base = head(&root, &data);
    let rename = ["rename", "storefront:container:storefront"];
    let env = signed_edit(&root, &data, base, &[&rename[..], &["shopfront"]].concat(), &[TEMPORARY_HOST]);
    let warnings: Vec<&Value> = env["meta"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["code"] == "temporary_host")
        .collect();
    assert_eq!(warnings.len(), 1, "{env}");
    let details = warnings[0]["details"].as_object().unwrap();
    let mut keys: Vec<&String> = details.keys().collect();
    keys.sort_unstable();
    assert_eq!(keys, ["ref", "repo"], "{env}");
    assert_eq!(details["ref"], reference.as_str());
    assert_eq!(details["repo"], first_ns.as_str());
    assert_eq!(open(&state).meta("backup_ref").unwrap().as_ref(), Some(first_key));

    for key in repos.keys() {
        let refs = git(&root.join(key), &["for-each-ref", "--format=%(refname)", "refs/planroom/"]);
        let expected = if key == first_key { format!("{reference}\n") } else { String::new() };
        assert_eq!(refs, expected, "only the first repo by namespace holds the ref: {key}");
    }
    let commit = git(&first, &["rev-parse", &reference]).trim().to_owned();
    let object = git(&first, &["cat-file", "-p", &commit]);
    assert!(!object.lines().any(|l| l.starts_with("parent ")), "{object}");
    assert!(object.contains("\nauthor Planroom <planroom@invalid> "), "{object}");
    assert!(!object.contains("gpgsig"), "{object}");
    let names = git(&first, &["ls-tree", "--name-only", &reference]);
    assert!(names.lines().any(|n| n == "events-000001.ndjson"), "{names}");
    assert!(!marker.exists(), "the repo's signing program never ran");

    let written = env["meta"]["seq"].as_u64().unwrap();
    let env = signed_edit(&root, &data, written, &[&rename[..], &["market"]].concat(), &[TEMPORARY_HOST]);
    assert!(
        env["meta"]["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "temporary_host"),
        "{env}"
    );
    assert_ne!(git(&first, &["rev-parse", &reference]).trim(), commit, "a second edit moves the ref");
    let after: Vec<_> = repos.keys().map(|key| looks(&root.join(key))).collect();
    assert_eq!(after, before, "no HEAD, branch or working tree changes");
    assert!(!marker.exists(), "the repo's signing program never ran");
}

#[test]
fn the_backup_ref_is_redacted() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let secret = "sk_live_51PlantedBackupKey0000";
    let name = format!("STRIPE_SECRET_KEY={secret}");
    let base = head(&root, &data);
    signed_edit(&root, &data, base, &["rename", "storefront:container:storefront", &name], &[TEMPORARY_HOST]);

    let file = backup_file(&state, &data);
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(raw.contains(secret), "the data root's file holds the edit as written");

    let repo = root.join(open(&state).meta("backup_ref").unwrap().expect("a recorded ref repo"));
    let reference = format!("refs/planroom/backup/{}", open(&state).meta("workspace_id").unwrap().unwrap());
    let copy = backup_ref_text(&repo, &reference);
    assert!(!copy.contains(secret), "the ref never holds the secret");
    assert!(copy.contains(planroom_kernel::redact::MASK), "the ref holds the mask");
    assert_eq!(copy.lines().count(), raw.lines().count(), "one line for each of the file's");
    assert_eq!(copy.lines().next(), raw.lines().next(), "the header is the file's");
    for (copied, line) in copy.lines().zip(raw.lines()).skip(1) {
        let event: planroom_contract::Event =
            serde_json::from_str(copied).unwrap_or_else(|e| panic!("{e}: {copied}"));
        let (copied, line): (Value, Value) =
            (serde_json::from_str(copied).unwrap(), serde_json::from_str(line).unwrap());
        assert_eq!(serde_json::to_value(event.id).unwrap(), line["id"]);
        assert_eq!(copied["seq"], line["seq"]);
        assert_eq!(targets(&copied), targets(&line), "every target stays whole");
    }
}

/// `doctor`'s budget, as the catalogue gives it.
fn doctor_budget() -> u64 {
    use planroom_contract::{Command as Leaf, DoctorReq};
    u64::from(Leaf::Doctor(DoctorReq::default()).spec().budget_tokens)
}

/// `planroom doctor --json` in `dir`: ok, and its envelope.
fn doctor(dir: &Path, data: &Path, env: &[(&str, &str)]) -> Value {
    let (code, doctor) = run(dir, data, &["doctor"], env, "");
    assert_eq!(code, 0, "{doctor}");
    assert_eq!(doctor["ok"], true, "{doctor}");
    doctor
}

/// The `major.minor.patch` of the `git --version` on `PATH`.
fn git_version() -> String {
    let out = Command::new("git").arg("--version").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let word = text.trim().trim_start_matches("git version ").split_whitespace().next().unwrap().to_owned();
    let mut parts: Vec<&str> = word.split('.').take(3).collect();
    parts.resize(3, "0");
    parts.join(".")
}

#[test]
fn doctor_json_reports_every_check() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let seq = edit(&root, &data, head(&root, &data), &["rename", "storefront:container:storefront", "shop"]);

    let env = doctor(&root, &data, &[]);
    let result = &env["result"];
    assert_eq!(result["git"]["version"], git_version(), "{env}");
    assert_eq!(result["git"]["minimum"], "2.34.1");
    let store = &result["store"];
    assert_eq!(store["workspace_id"], open(&state).meta("workspace_id").unwrap().unwrap(), "{env}");
    assert_eq!(store["seq"], seq);
    assert_eq!(store["events"].as_u64(), Some(store_seqs(&state).len() as u64));
    let migrations = &result["migrations"];
    assert_eq!(migrations["store_version"], migrations["supported"], "{env}");
    assert_eq!(migrations["store_version"], STORE_VERSION);
    assert_eq!(migrations["copies"], json!([]));

    let code_pack = planroom_testkit::repo_root().join("packs/code");
    let (code, checked) = run(&root, &data, &["doctor", "--pack", code_pack.to_str().unwrap()], &[], "");
    assert_eq!(code, 0, "{checked}");
    let packs = &result["packs"];
    let listed = packs["packs"].as_array().unwrap();
    let code_record = listed.iter().find(|p| p["id"] == "code").expect("the code pack is listed");
    assert_eq!(code_record["source"], "builtin");
    assert_eq!(code_record["sha256"], checked["result"]["sha256"]);
    assert_eq!(packs["matches"], true, "{env}");
    assert_eq!(packs["recorded_hash"], packs["hash"]);

    let backup = &result["backup"];
    assert_eq!(backup["behind"], false, "{env}");
    assert_eq!(backup["temporary_host"], false);
    assert_eq!(backup["backup_ref"], Value::Null);
    assert_eq!(backup["last_authored"], seq);
    assert_eq!(env["meta"]["warnings"], Value::Null, "no warnings: {env}");
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= doctor_budget(), "{}", env["meta"]);

    let (code, schema) = run(&root, &data, &["schema", "--command", "doctor"], &[], "");
    assert_eq!(code, 0, "{schema}");
    assert_eq!(schema["result"]["result"], "DoctorResult", "{schema}");
    assert!(schema["result"]["defs"].get("DoctorReport").is_some(), "{schema}");

    // Outside any workspace: the git and pack checks alone, and nothing written.
    let elsewhere = tempfile::tempdir().unwrap();
    let away = elsewhere.path().join("away");
    std::fs::create_dir(&away).unwrap();
    let fresh = elsewhere.path().join("data");
    let env = doctor(&away, &fresh, &[]);
    let result = &env["result"];
    for check in ["store", "migrations", "backup"] {
        assert_eq!(result[check], Value::Null, "{check}: {env}");
    }
    assert_eq!(result["packs"]["matches"], Value::Null, "{env}");
    assert_eq!(result["packs"]["recorded_hash"], Value::Null);
    assert!(result["packs"]["packs"].as_array().unwrap().iter().any(|p| p["id"] == "code"));
    assert_eq!(std::fs::read_dir(&away).unwrap().count(), 0, "no state directory");
    assert!(!fresh.join("backups").exists(), "no backup directory");
}

/// A directory under `base` whose path from `base` is at most `len` bytes and within 100 of it,
/// starting with `first` and ending with `last`.
fn long_dir(base: &Path, first: &str, len: usize, last: &str) -> PathBuf {
    let mut dir = base.join(first);
    let mut i = 0;
    while dir.as_os_str().len() + 100 + 1 + last.len() <= base.as_os_str().len() + len {
        dir = dir.join(format!("{i:02}{}", "d".repeat(97)));
        i += 1;
    }
    dir.join(last)
}

#[test]
fn a_long_path_keeps_doctor_in_budget() {
    let secret = "sk_live_51PlantedDoctorKey0000";
    let named = format!("STRIPE_SECRET_KEY={secret}");
    let tmp = tempfile::tempdir().unwrap();
    // The root is canonical, since macOS's temporary directory is a link.
    let base = tmp.path().canonicalize().unwrap();
    // SQLite opens no file whose path is over 512 bytes, so the store's path is as long as it can
    // be, well over the 160 characters doctor shows whole. The data root and the packs' sources
    // go 3,000 bytes deep, or as deep as the platform's path limit allows with room left for the
    // files under them (4096 bytes on Linux, 1024 on macOS). The workspace's own name holds the
    // secret too, so the tail doctor shows of its path holds it.
    let deep = 3_000.min(usize::try_from(libc::PATH_MAX).unwrap() - 300 - base.as_os_str().len());
    let root = long_dir(&base, &named, 440, &named);
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    planroom_testkit::fixture(&root);
    let data = long_dir(&base.join("data"), &named, deep, "data");
    assert!(data.as_os_str().len() > 600, "{}", data.display());
    person_init(&root, &data);

    let packs_dir = long_dir(&base, "packs", deep, "src");
    for i in 0..8 {
        let id = format!("extra{i}");
        let dir = packs_dir.join(&id);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = format!(
            "format = \"planroom.pack/v1\"\nid = \"{id}\"\nversion = 1\ntitle = \"{id}\"\nrequires = []\n"
        );
        std::fs::write(dir.join("pack.toml"), manifest).unwrap();
        let args = [dir.to_str().unwrap()];
        let signed = token("pack add", &args, "");
        let (code, added) =
            run(&root, &data, &["pack", "add", args[0]], &[("PLANROOM_PERSON_TOKEN", &signed)], "");
        assert_eq!(code, 0, "{added}");
    }
    let recorded = open(&root.join(".planroom")).recorded_packs().unwrap().expect("a recorded set");
    assert!(recorded.packs.len() > 8, "{:?}", recorded.packs);

    let env = doctor(&root, &data, &[]);
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= doctor_budget(), "{}", env["meta"]);
    let packs = &env["result"]["packs"];
    let listed = packs["packs"].as_array().unwrap().len();
    assert!(listed <= 6, "{packs}");
    assert_eq!(listed as u64 + packs["more"].as_u64().unwrap(), recorded.packs.len() as u64, "{packs}");
    assert!(!env.to_string().contains(secret), "{env}");
    let path = env["result"]["store"]["path"].as_str().unwrap();
    assert!(path.contains(planroom_kernel::redact::MASK), "{path}");
}

#[test]
fn doctor_warns_when_the_backup_is_behind() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let file = backup_file(&state, &data);
    let id = "storefront:container:storefront";
    let first = edit(&root, &data, head(&root, &data), &["rename", id, "one"]);
    let second = edit(&root, &data, first, &["rename", id, "two"]);

    // A kill between the commit and the append: the second edit's line and mark never landed.
    let text = std::fs::read_to_string(&file).unwrap();
    let cut = text.trim_end_matches('\n').rfind('\n').unwrap() + 1;
    std::fs::write(&file, &text[..cut]).unwrap();
    sql_exec(
        &state.join("store.db"),
        "UPDATE meta SET value = ?1 WHERE key = 'backup_seq'",
        &[&first.to_string()],
    );

    let env = doctor(&root, &data, &[]);
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    let behind: Vec<&Value> = warnings.iter().filter(|w| w["code"] == "backup_behind").collect();
    assert_eq!(behind.len(), 1, "{env}");
    assert_eq!(
        behind[0]["details"],
        json!({"backup_seq": first, "file_seq": first, "last_authored": second})
    );
    assert_eq!(env["result"]["backup"]["behind"], true, "{env}");

    edit(&root, &data, second, &["rename", id, "three"]);
    let env = doctor(&root, &data, &[]);
    assert_eq!(env["meta"]["warnings"], Value::Null, "no warnings: {env}");
    assert_eq!(env["result"]["backup"]["behind"], false, "{env}");
}

/// A backup file the write-through would rename aside holds nothing of this store, so `doctor`
/// reports the backup behind even when the file's last `seq` is the store's head.
#[test]
fn doctor_counts_a_file_for_another_state_directory_as_holding_nothing() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let file = backup_file(&state, &data);
    let seq = edit(&root, &data, head(&root, &data), &["rename", "storefront:container:storefront", "one"]);
    let text = std::fs::read_to_string(&file).unwrap();
    let (line, rest) = text.split_once('\n').unwrap();
    let mut header: Value = serde_json::from_str(line).unwrap();
    header["state_dir"] = json!("/elsewhere/.planroom");
    std::fs::write(&file, format!("{header}\n{rest}")).unwrap();
    assert_eq!(planroom_store::file_seq(&file).unwrap(), seq, "the file's last line is the head");

    let env = doctor(&root, &data, &[]);
    let warnings = env["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("a warning: {env}"));
    let behind: Vec<&Value> = warnings.iter().filter(|w| w["code"] == "backup_behind").collect();
    assert_eq!(behind.len(), 1, "{env}");
    assert_eq!(behind[0]["details"], json!({"backup_seq": seq, "file_seq": 0, "last_authored": seq}));
    assert_eq!(env["result"]["backup"]["file_seq"], 0, "{env}");
    assert_eq!(env["result"]["backup"]["behind"], true, "{env}");
}

/// A workspace ID the write-through refuses, such as one with a dot, names no backup directory,
/// so no backup can exist: the person edit says the backup is behind, and so does `doctor`.
#[test]
fn an_id_the_backup_refuses_leaves_the_backup_behind() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let id = "storefront:container:storefront";
    let first = edit(&root, &data, head(&root, &data), &["rename", id, "one"]);
    sql_exec(&state.join("store.db"), "UPDATE meta SET value = ?1 WHERE key = 'workspace_id'", &["a.b"]);

    let reply = signed_edit(&root, &data, first, &["rename", id, "two"], &[]);
    let second = reply["meta"]["seq"].as_u64().unwrap();
    let warnings = reply["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("a warning: {reply}"));
    let behind: Vec<&Value> = warnings.iter().filter(|w| w["code"] == "backup_behind").collect();
    assert_eq!(behind.len(), 1, "{reply}");
    assert_eq!(behind[0]["details"], json!({"backup_seq": first, "file_seq": 0, "last_authored": second}));
    assert!(!data.join("backups/a.b").exists(), "nothing written for the refused ID");

    let env = doctor(&root, &data, &[]);
    assert_eq!(env["result"]["store"]["workspace_id"], "a.b", "{env}");
    let backup = &env["result"]["backup"];
    assert_eq!(backup["behind"], true, "{env}");
    assert_eq!(backup["file_seq"], 0, "{env}");
    assert_eq!(backup["last_authored"], second, "{env}");
    let warnings = env["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("a warning: {env}"));
    assert!(warnings.iter().any(|w| w["code"] == "backup_behind"), "{env}");
}

/// Whether `text` holds the word "push", in any case.
fn says_push(text: &str) -> bool {
    let lower = text.to_lowercase();
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    lower
        .match_indices("push")
        .any(|(at, _)| !word(lower[..at].chars().next_back()) && !word(lower[at + 4..].chars().next()))
}

/// `planroom ARGS` with `env`, in text mode or with `--json`; its stdout and stderr.
fn outputs(root: &Path, data: &Path, args: &[&str], env: &[(&str, &str)], json: bool) -> String {
    let mut cmd = planroom(root, data);
    cmd.args(args);
    if json {
        cmd.arg("--json");
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

#[test]
fn no_envelope_suggests_pushing_the_backup() {
    assert!(!says_push("pushed, or a push_x") && says_push("Push it") && says_push("then push."));
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    planroom_testkit::fixture(&root);
    let data = tmp.path().join("data");
    let mut said = Vec::new();
    said.push(outputs(&root, &data, &["scan"], &[TEMPORARY_HOST], true));
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let (code, init) = run(
        &root,
        &data,
        &["person", "init"],
        &[TEMPORARY_HOST, ("PLANROOM_PERSON_TOKEN", &token("person init", &[], ""))],
        &public,
    );
    assert_eq!(code, 0, "{init}");
    said.push(init.to_string());
    let base = head(&root, &data);
    let edited = signed_edit(
        &root,
        &data,
        base,
        &["rename", "storefront:container:storefront", "shop"],
        &[TEMPORARY_HOST],
    );
    assert!(edited["meta"]["warnings"].to_string().contains("temporary_host"), "{edited}");
    said.push(edited.to_string());
    for args in [&["focus"][..], &["map"], &["doctor"], &["schema", "--command", "doctor"]] {
        for json in [true, false] {
            said.push(outputs(&root, &data, args, &[TEMPORARY_HOST], json));
        }
    }
    for out in &said {
        assert!(!says_push(out), "{out}");
    }

    let catalog =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("text/en.toml")).unwrap();
    let mut section = "";
    let mut checked = 0;
    for line in catalog.lines() {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        let key = line.split('=').next().unwrap_or("").trim();
        let warned = section == "[warning]"
            && ["temporary_host", "temporary_host_copy_out", "backup_ref_failed"].contains(&key);
        if warned || (section == "[doctor]" && !key.is_empty()) {
            assert!(!says_push(line), "{section} {line}");
            checked += 1;
        }
    }
    assert!(checked >= 3 + 9, "the three warnings and doctor's nine lines: {checked}");
}

/// The UTC date, `YYYY-MM-DD`, of `secs` after the epoch.
fn utc_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Every restore card the store at `state` holds, as it was dealt, with the event that dealt it.
fn restore_cards(state: &Path) -> Vec<(planroom_contract::Event, planroom_contract::Card)> {
    open(state)
        .events_after(0, 1_000_000)
        .unwrap()
        .into_iter()
        .filter_map(|e| match &e.body {
            EventBody::Card(card) if card.kind.0 == "restore" => Some((e.clone(), card.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_lost_state_directory_then_scan_offers_one_restore_card() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let ids = node_ids(&root, &data, 3);
    let mut base = head(&root, &data);
    for (id, name) in ids.iter().zip(["one", "two", "three"]) {
        base = edit(&root, &data, base, &["rename", id.as_str(), name]);
    }
    let file = backup_file(&state, &data);
    let original = file.parent().unwrap().to_path_buf();
    let (header, events) = backed_up(&file);
    assert_eq!(events.iter().filter(|e| matches!(e.actor, Actor::Person { .. })).count(), 3);

    // Two directories that are no backup of this workspace: an ID no directory can have, and a
    // file that isn't an export.
    let backups = data.join("backups");
    std::fs::create_dir_all(backups.join("x..y")).unwrap();
    std::fs::write(backups.join("x..y/events.ndjson"), "any text\n").unwrap();
    std::fs::create_dir_all(backups.join("garbage_id")).unwrap();
    std::fs::write(backups.join("garbage_id/events.ndjson"), "not json\n").unwrap();

    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");

    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    let (_, card) = &cards[0];
    assert_eq!(card.subject, header.workspace_id);
    assert_eq!(card.vars.get("edits").map(String::as_str), Some("3"));
    assert_eq!(card.vars.get("date"), Some(&utc_date(now())));
    assert_eq!(card.vars.get("source").map(String::as_str), Some("backup"));
    assert_eq!(card.default, "reject", "a card nobody picks leaves the result as it is");
    let preselected: Vec<&str> =
        card.choices.iter().filter(|c| c.preselected).map(|c| c.id.as_str()).collect();
    assert_eq!(preselected, ["accept"], "Restore is the preselected pick");
    assert!(
        open(&state)
            .events_after(0, 1_000_000)
            .unwrap()
            .iter()
            .all(|e| !matches!(e.actor, Actor::Person { .. })),
        "a card nobody picks restores nothing"
    );

    let (code, env) = run(&root, &data, &["focus"], &[], "");
    assert_eq!(code, 0, "{env}");
    let decision = &env["result"]["decision"];
    assert!(decision["question"].as_str().unwrap().starts_with("Restore 3 edits from "), "{env}");
    let picked: Vec<(&str, &str)> = decision["choices"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["preselected"] == true)
        .map(|c| (c["id"].as_str().unwrap(), c["label"].as_str().unwrap()))
        .collect();
    assert_eq!(picked, [("accept", "Restore")], "{env}");

    // A second scan deals none.
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(restore_cards(&state).len(), 1);

    // A later backup of another workspace for this state directory: the card stays the one.
    let later = backups.join("zz_later");
    std::fs::create_dir_all(&later).unwrap();
    let text = std::fs::read_to_string(original.join("events.ndjson")).unwrap();
    let (first, rest) = text.split_once('\n').unwrap();
    let mut head_line: Value = serde_json::from_str(first).unwrap();
    head_line["workspace_id"] = json!("zz_later");
    let mut last: Value = serde_json::from_str(rest.lines().last().unwrap()).unwrap();
    last["id"] = json!("later-person-edit");
    last["seq"] = json!(base + 100);
    last["at"] = json!("2999-01-01T00:00:00Z");
    std::fs::write(later.join("events.ndjson"), format!("{head_line}\n{rest}{last}\n")).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "still one restore card");
    let focus = run(&root, &data, &["focus"], &[], "").1;
    assert!(
        focus["result"]["decision"]["question"].as_str().unwrap().starts_with("Restore 3 edits"),
        "{focus}"
    );
}

#[test]
fn a_linked_event_backup_deals_no_restore_card() {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let base = head(&root, &data);
    edit(&root, &data, base, &["rename", "storefront:container:storefront", "shop"]);
    // The backup file moves outside the data root, and a link to it takes its place.
    let file = backup_file(&state, &data);
    let outside = tmp.path().join("outside-events.ndjson");
    std::fs::rename(&file, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &file).unwrap();

    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    assert!(restore_cards(&state).is_empty(), "a link is no backup, so it deals no card");
}

#[test]
fn accepting_a_restore_card_never_reads_a_linked_backup() {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let (backup, _, card) = lost_with_a_card(&root, &data);
    // After the card was dealt, a link to a file outside the data root takes the backup's place.
    let outside = tmp.path().join("outside-events.ndjson");
    std::fs::rename(&backup, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &backup).unwrap();

    let (code, env) = as_person(&root, &data, "review", head(&root, &data), &[&card, "accept"], &[]);
    assert_eq!(env["error"]["code"], "not_found", "{env}");
    assert_ne!(code, 0, "{env}");
    assert!(
        open(&state)
            .events_after(0, 1_000_000)
            .unwrap()
            .iter()
            .all(|e| !matches!(e.actor, Actor::Person { .. })),
        "nothing was restored"
    );
}

#[test]
fn a_temporary_host_offers_the_backup_ref_when_the_data_root_lost_its_backup() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let old_id = open(&state).meta("workspace_id").unwrap().unwrap();
    let first = recorded_repos(&state).into_values().min().unwrap();
    let mut base = head(&root, &data);
    for name in ["one", "two"] {
        let op = ["rename", "storefront:container:storefront", name];
        base = signed_edit(&root, &data, base, &op, &[TEMPORARY_HOST])["meta"]["seq"].as_u64().unwrap();
    }

    // The container lost the data root's backup along with the state directory; the ref in the
    // repo is what is left.
    std::fs::remove_dir_all(data.join("backups")).unwrap();
    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[TEMPORARY_HOST], "");
    assert_eq!(code, 0, "{env}");
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    let card = &cards[0].1;
    assert_eq!(card.subject, old_id);
    assert_eq!(card.vars.get("source").map(String::as_str), Some("ref"));
    assert_eq!(card.vars.get("edits").map(String::as_str), Some("2"));
    assert_eq!(card.vars.get("repo"), Some(&first));
    assert_eq!(card.why, "card.restore_why_ref");
    assert_eq!(card.vars.get("ref"), Some(&format!("refs/planroom/backup/{old_id}")));
    assert_eq!(card.default, "reject");
}

/// An agent session's actor, as `PLANROOM_ACTOR` names it.
const AGENT: &str = "agent:aaaaaaaa:chat";

/// `planroom COMMAND --base BASE ARGS` as the person, with a token over the argv after the
/// command; the exit code and the envelope.
fn as_person(
    root: &Path,
    data: &Path,
    command: &str,
    base: u64,
    rest: &[&str],
    env: &[(&str, &str)],
) -> (i32, Value) {
    let base = base.to_string();
    let args: Vec<&str> = ["--base", base.as_str()].into_iter().chain(rest.iter().copied()).collect();
    let signed = token(command, &args, &base);
    let argv: Vec<&str> = std::iter::once(command).chain(args.iter().copied()).collect();
    let vars: Vec<(&str, &str)> =
        env.iter().copied().chain([("PLANROOM_PERSON_TOKEN", signed.as_str())]).collect();
    run(root, data, &argv, &vars, "")
}

/// An agent's `edit --base BASE OP`, which waits for a person as a ghost; its envelope.
fn agent_edit(root: &Path, data: &Path, base: u64, op: &[&str]) -> Value {
    let base = base.to_string();
    let argv: Vec<&str> = ["edit", "--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
    let (code, env) = run(root, data, &argv, &[("PLANROOM_ACTOR", AGENT)], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["status"], "ghost", "{env}");
    env
}

/// The name `map --level container` shows for the node `id`.
fn container_name(root: &Path, data: &Path, id: &str) -> Value {
    let (code, map) = run(root, data, &["map", "--level", "container"], &[], "");
    assert_eq!(code, 0, "{map}");
    map["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap()["name"].clone()
}

/// Every event but a scan, by ID, with its stored status.
fn held(state: &Path) -> std::collections::BTreeMap<String, Status> {
    open(state)
        .events_after(0, 1_000_000)
        .unwrap()
        .into_iter()
        .filter(|e| !matches!(e.body, EventBody::Scan { .. }))
        .map(|e| (e.id.0, e.status))
        .collect()
}

/// The pre-restore copies in the backup directory of the workspace `id`.
fn pre_restore_copies(data: &Path, id: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(data.join("backups").join(id)) else { return vec![] };
    let mut copies: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with("store-pre-restore-"))
        .collect();
    copies.sort();
    copies
}

#[test]
fn accepting_the_restore_card_restores() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let node = "storefront:container:storefront";
    let first = head(&root, &data);
    let renamed = edit(&root, &data, first, &["rename", node, "shop"]);
    let ghost = agent_edit(&root, &data, renamed, &["rename", "storefront:dir:src/cart", "basket"]);
    // The person's stale rename of the same node, from the older base: stored as a conflict.
    let (code, env) = as_person(&root, &data, "edit", first, &["rename", node, "store"], &[]);
    assert_eq!((code, env["error"]["code"].as_str()), (4, Some("stale_write")), "{env}");
    let conflict = env["error"]["details"]["event"].as_str().unwrap().to_owned();
    let recorded = held(&state);
    let ghost = ghost["result"]["event"].as_str().unwrap();
    assert_eq!(recorded.get(ghost), Some(&Status::Ghost));
    assert_eq!(recorded.get(&conflict), Some(&Status::Conflict));
    assert!(recorded.values().any(|s| *s == Status::Applied));

    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    register_key(&root, &data);
    let new_id = open(&state).meta("workspace_id").unwrap().unwrap();
    // An agent's ghost written after the scan is no person's edit, and doesn't stop the accept.
    agent_edit(&root, &data, head(&root, &data), &["rename", "storefront:dir:src/cart", "trolley"]);
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    let card = cards[0].0.id.0.clone();

    let (code, env) = as_person(&root, &data, "review", head(&root, &data), &[&card, "accept"], &[]);
    assert_eq!(code, 0, "{env}");
    assert!(env["summary"].as_str().unwrap().contains("Restored 2 edits from "), "{env}");
    let now_held = held(&state);
    for (id, status) in &recorded {
        assert_eq!(now_held.get(id), Some(status), "{id} comes back as it was stored");
    }
    assert_eq!(container_name(&root, &data, node), "shop");
    let (code, focus) = run(&root, &data, &["focus"], &[], "");
    assert_eq!(code, 0, "{focus}");
    assert!(
        !focus["result"]["decision"]["question"].as_str().unwrap_or_default().starts_with("Restore "),
        "no restore card is open: {focus}"
    );
    let copies = pre_restore_copies(&data, &new_id);
    assert_eq!(copies.len(), 1, "the accept wrote one pre-restore copy");
    // The restored events are person-authored, so the new store's own backup holds them.
    let (_, backed) = backed_up(&data.join("backups").join(&new_id).join("events.ndjson"));
    for id in recorded.keys() {
        assert!(backed.iter().any(|e| &e.id.0 == id), "{id} is in the new store's backup");
    }

    // An undo can't take the restore back; restoring the copy taken before it does.
    let review = env["result"]["event"].as_str().unwrap().to_owned();
    let (code, env) = as_person(&root, &data, "edit", head(&root, &data), &["undo", &review], &[]);
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["field"], "event", "{env}");
    let hint = env["error"]["hint"].as_str().unwrap();
    assert!(hint.starts_with("planroom doctor --restore "), "{env}");
    assert!(hint.contains(copies[0].file_name().unwrap().to_str().unwrap()), "{env}");
    assert_eq!(held(&state), now_held, "the refused undo wrote nothing");
    // Its hint, run by the person, takes the restore back: the card is open again, and the
    // restore's own events are no edit to bring back at the next upgrade.
    let copy = copies[0].to_str().unwrap();
    let (code, env) = restore_as_person(&root, &data, copy, &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["restored"], "copy", "{env}");
    assert_ne!(container_name(&root, &data, node), "shop", "the restored rename is gone");
    let (code, focus) = run(&root, &data, &["focus"], &[], "");
    assert_eq!(code, 0, "{focus}");
    assert!(
        focus["result"]["decision"]["question"].as_str().unwrap_or_default().starts_with("Restore "),
        "the restore card is open again: {focus}"
    );
    assert_eq!(open(&state).meta("replay_set").unwrap(), None, "the card restore's events stay out");

    // A second loss: a reject starts empty, and the next scan deals no card.
    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    register_key(&root, &data);
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    let card = cards[0].0.id.0.clone();
    let (code, env) = as_person(&root, &data, "review", head(&root, &data), &[&card, "reject"], &[]);
    assert_eq!(code, 0, "{env}");
    assert!(env["summary"].as_str().unwrap().contains("Started empty"), "{env}");
    let persons: Vec<EventBody> = open(&state)
        .events_after(0, 1_000_000)
        .unwrap()
        .into_iter()
        .filter(|e| matches!(e.actor, Actor::Person { .. }))
        .map(|e| e.body)
        .collect();
    assert!(matches!(persons.as_slice(), [EventBody::Review { .. }]), "only the reject: {persons:?}");
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(restore_cards(&state).len(), 1, "the next scan deals none");
}

#[test]
fn scan_restores_from_the_backup_ref() {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let secret = "sk_live_51PlantedRestoreKey00";
    let planted = format!("STRIPE_SECRET_KEY={secret}");
    let node = "storefront:container:storefront";
    let base = head(&root, &data);
    let base = signed_edit(&root, &data, base, &["rename", node, "shop"], &[TEMPORARY_HOST])["meta"]["seq"]
        .as_u64()
        .unwrap();
    signed_edit(&root, &data, base, &["rename", "storefront:dir:src/cart", &planted], &[TEMPORARY_HOST]);
    let recorded = held(&state);

    // The container lost the state directory and the data root; the ref is what is left.
    std::fs::remove_dir_all(&state).unwrap();
    let data = tmp.path().join("data2");
    let (code, env) = run(&root, &data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    assert!(restore_cards(&state).is_empty(), "off a temporary host the ref deals no card");
    let (code, env) = run(&root, &data, &["scan"], &[TEMPORARY_HOST], "");
    assert_eq!(code, 0, "{env}");
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    let (event, card) = &cards[0];
    assert_eq!(card.vars.get("source").map(String::as_str), Some("ref"));
    assert!(card.vars.get("repo").is_some_and(|r| !r.is_empty()));
    assert!(card.vars.get("ref").is_some_and(|r| r.starts_with("refs/planroom/backup/")));

    register_key(&root, &data);
    let (code, env) =
        as_person(&root, &data, "review", head(&root, &data), &[&event.id.0, "accept"], &[TEMPORARY_HOST]);
    assert_eq!(code, 0, "{env}");
    let now_held = held(&state);
    for (id, status) in &recorded {
        assert_eq!(now_held.get(id), Some(status), "{id} comes back as it was stored");
    }
    assert_eq!(container_name(&root, &data, node), "shop");
    let bodies = sql(&state.join("store.db"), "SELECT group_concat(body, char(10)) FROM events");
    assert!(!bodies.contains(secret), "the planted value comes back masked");
    assert!(bodies.contains(planroom_kernel::redact::MASK), "{bodies}");
}

/// `planroom doctor --restore BACKUP` as the person, with a token over `--restore BACKUP`.
fn restore_as_person(root: &Path, data: &Path, backup: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let signed = token("doctor", &["--restore", backup], "");
    restore_with(root, data, backup, &signed, env)
}

/// `planroom doctor --restore BACKUP` with the token `signed`.
fn restore_with(root: &Path, data: &Path, backup: &str, signed: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let vars: Vec<(&str, &str)> = env.iter().copied().chain([("PLANROOM_PERSON_TOKEN", signed)]).collect();
    run(root, data, &["doctor", "--restore", backup], &vars, "")
}

/// The workspace ID of the store at `state`.
fn workspace_id(state: &Path) -> String {
    open(state).meta("workspace_id").unwrap().unwrap()
}

/// A copy of the store at `state` made with `VACUUM INTO`, as `name` in its backup directory.
fn store_copy(state: &Path, data: &Path, name: &str) -> PathBuf {
    let dir = data.join("backups").join(workspace_id(state));
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join(name);
    sql_exec(&state.join("store.db"), "VACUUM INTO ?", &[copy.to_str().unwrap()]);
    copy
}

/// Plant a presence row from another session in the store copy at `copy`.
fn plant_presence(copy: &Path) {
    sql_exec(
        copy,
        "INSERT INTO presence (actor, surface, client_id, doing, focus, seen_at) VALUES (?, 'cli', 'planted', NULL, NULL, '2026-10-26T00:00:00Z')",
        &[r#"{"kind":"person","id":"planted"}"#],
    );
}

/// A workspace whose person edits a backup holds, whose state directory was then lost, and whose
/// next scan dealt a restore card: the path to its old event backup, its recorded events, and the
/// card's ID.
fn lost_with_a_card(
    root: &Path,
    data: &Path,
) -> (PathBuf, std::collections::BTreeMap<String, Status>, String) {
    let state = root.join(".planroom");
    let node = "storefront:container:storefront";
    let first = head(root, data);
    let renamed = edit(root, data, first, &["rename", node, "shop"]);
    edit(root, data, renamed, &["rename", "storefront:dir:src/cart", "basket"]);
    let recorded = held(&state);
    let backup = data.join("backups").join(workspace_id(&state)).join("events.ndjson");
    std::fs::remove_dir_all(&state).unwrap();
    let (code, env) = run(root, data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    register_key(root, data);
    let cards = restore_cards(&state);
    assert_eq!(cards.len(), 1, "one restore card");
    (backup, recorded, cards[0].0.id.0.clone())
}

#[test]
fn doctor_restore_of_the_event_backup_resolves_the_card() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let (backup, recorded, card) = lost_with_a_card(&root, &data);
    let backup = backup.to_str().unwrap();

    let (code, env) = restore_as_person(&root, &data, backup, &[]);
    assert_eq!(code, 0, "{env}");
    let result = &env["result"];
    assert_eq!(result["restored"], "events", "{env}");
    assert_eq!(result["card"], card.as_str(), "{env}");
    assert_eq!(result["events"], 2, "{env}");
    assert!(result["pre_restore"].as_str().is_some_and(|f| f.starts_with("store-pre-restore-")), "{env}");
    assert_eq!(container_name(&root, &data, "storefront:container:storefront"), "shop");
    // The card resolves in the restore's own commit: its review sits among the restored events.
    let events = open(&state).events_after(0, 1_000_000).unwrap();
    let restored: Vec<u64> =
        events.iter().filter(|e| recorded.contains_key(&e.id.0)).map(|e| e.seq).collect();
    assert_eq!(restored.len(), recorded.len(), "every recorded event came back");
    let review = events
        .iter()
        .find(|e| matches!(&e.body, EventBody::Review { of, .. } if of.0 == card))
        .expect("the card's review");
    let (lo, hi) = (*restored.iter().min().unwrap(), *restored.iter().max().unwrap());
    assert!(review.seq >= lo && review.seq <= hi + 1, "review {} within {lo}..={}", review.seq, hi + 1);

    let (code, env) = restore_as_person(&root, &data, backup, &[]);
    assert_ne!(code, 0, "{env}");
    assert_eq!(env["error"]["code"], "already_resolved", "{env}");
}

#[test]
fn doctor_restore_is_person_only() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let (backup, _, _) = lost_with_a_card(&root, &data);
    let id = workspace_id(&state);
    // A copy of the new store, so the copy form has one of this workspace's to put back.
    let copy = store_copy(&state, &data, "store-v1.db");
    let store = state.join("store.db");
    let before = std::fs::read(&store).unwrap();
    for given in [copy.to_str().unwrap(), backup.to_str().unwrap()] {
        let (code, env) = restore_as_person(&root, &data, given, &[("PLANROOM_ACTOR", AGENT)]);
        assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");
        let hint = env["error"]["hint"].as_str().unwrap();
        let line = format!("planroom doctor --restore {given}");
        assert!(hint.contains(&line), "the hint names {line}: {env}");
    }
    // A token signed for another command is no token for this one.
    let signed = token("edit", &["--restore", copy.to_str().unwrap()], "");
    let (code, env) = restore_with(&root, &data, copy.to_str().unwrap(), &signed, &[]);
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");
    assert_eq!(std::fs::read(&store).unwrap(), before, "the store is unchanged");
    assert!(pre_restore_copies(&data, &id).is_empty(), "no pre-restore copy was written");
}

#[test]
fn a_restore_is_undone_by_its_pre_restore_copy() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let node = "storefront:container:storefront";
    let id = workspace_id(&state);
    let mut base = head(&root, &data);
    for n in 1..=5 {
        base = edit(&root, &data, base, &["rename", node, &format!("n{n}")]);
    }
    let copy = store_copy(&state, &data, "store-v1.db");
    plant_presence(&copy);
    for n in 6..=8 {
        base = edit(&root, &data, base, &["rename", node, &format!("n{n}")]);
    }
    let original = held(&state);

    let copy = copy.to_str().unwrap().to_owned();
    let first_token = token("doctor", &["--restore", &copy], "");
    let (code, env) = restore_with(&root, &data, &copy, &first_token, &[]);
    assert_eq!(code, 0, "{env}");
    let result = &env["result"];
    assert_eq!(result["restored"], "copy", "{env}");
    assert_eq!(result["set_aside"], 3, "{env}");
    assert_eq!(result["from"], "store-v1.db", "{env}");
    let pre_restore = result["pre_restore"].as_str().expect("a pre-restore copy").to_owned();
    let copy_head = result["seq"].as_u64().unwrap();
    assert_eq!(container_name(&root, &data, node), "n5");

    let after = edit(&root, &data, head(&root, &data), &["rename", node, "n9"]);
    assert_eq!(after, copy_head + 1, "the next seq follows the copy's last");

    let pre = data.join("backups").join(&id).join(&pre_restore);
    let (code, env) = restore_as_person(&root, &data, pre.to_str().unwrap(), &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["set_aside"], 1, "the edit after the restore: {env}");
    assert_eq!(held(&state), original, "exactly the original events");
    assert_eq!(container_name(&root, &data, node), "n8");

    let (code, env) = restore_with(&root, &data, &copy, &first_token, &[]);
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");
    assert_eq!(env["error"]["details"]["reason"], "spent", "{env}");
    let planted = sql(&state.join("store.db"), "SELECT COUNT(*) FROM presence WHERE client_id = 'planted'");
    assert_eq!(planted, "0", "a copy's presence row never comes back");
}

#[test]
fn a_store_copy_of_another_workspace_or_a_damaged_one_is_a_bad_argument() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let node = "storefront:container:storefront";
    let base = edit(&root, &data, head(&root, &data), &["rename", node, "shop"]);
    let other = store_copy(&state, &data, "store-v1.db");
    sql_exec(&other, "UPDATE meta SET value = ? WHERE key = 'workspace_id'", &["another-workspace"]);
    let damaged = store_copy(&state, &data, "store-v2.db");
    std::fs::write(&damaged, "not a store\n").unwrap();
    edit(&root, &data, base, &["rename", node, "store"]);
    let before = held(&state);

    for copy in [&other, &damaged] {
        let (code, env) = restore_as_person(&root, &data, copy.to_str().unwrap(), &[]);
        assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
        assert_eq!(env["error"]["details"]["field"], "restore", "{env}");
    }
    assert_eq!(held(&state), before, "the live store is as it was");
    assert_eq!(container_name(&root, &data, node), "store");
}

#[test]
fn the_event_form_restores_the_backup_it_is_given() {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let secret = "sk_live_51PlantedRestoreKey00";
    let planted = format!("STRIPE_SECRET_KEY={secret}");
    let base = head(&root, &data);
    signed_edit(&root, &data, base, &["rename", "storefront:dir:src/cart", &planted], &[TEMPORARY_HOST]);
    let id = workspace_id(&state);
    let kept = std::fs::read(backup_file(&state, &data)).unwrap();

    // A new data root deals the card from the ref; then the data root's backup comes back.
    std::fs::remove_dir_all(&state).unwrap();
    let data = tmp.path().join("data2");
    let (code, env) = run(&root, &data, &["scan"], &[TEMPORARY_HOST], "");
    assert_eq!(code, 0, "{env}");
    let cards = restore_cards(&state);
    assert_eq!(cards[0].1.vars.get("source").map(String::as_str), Some("ref"));
    register_key(&root, &data);
    let file = data.join("backups").join(&id).join("events.ndjson");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, kept).unwrap();

    let (code, env) = restore_as_person(&root, &data, file.to_str().unwrap(), &[TEMPORARY_HOST]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["from"], "events.ndjson", "{env}");
    let bodies = sql(&state.join("store.db"), "SELECT group_concat(body, char(10)) FROM events");
    assert!(bodies.contains(secret), "the file given, not the masked ref, was restored");
}

#[test]
fn a_copy_restore_fences_a_held_gc_lease() {
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let node = "storefront:container:storefront";
    let base = edit(&root, &data, head(&root, &data), &["rename", node, "shop"]);
    let copy = store_copy(&state, &data, "store-v1.db");
    edit(&root, &data, base, &["rename", node, "store"]);
    let lease = open(&state).take_lease(planroom_store::GC_LEASE, "holder-a").unwrap().expect("the lease");

    let (code, env) = restore_as_person(&root, &data, copy.to_str().unwrap(), &[]);
    assert_eq!(code, 0, "{env}");
    let store = state.join("store.db");
    let epoch = sql(&store, "SELECT epoch FROM leases WHERE name = 'gc'");
    assert_eq!(epoch, (lease.epoch + 1).to_string(), "the lease's epoch moved on");
    open(&state).release_lease(&lease).unwrap();
    assert_eq!(
        sql(&store, "SELECT COUNT(*) FROM leases WHERE name = 'gc'"),
        "1",
        "the fenced holder freed nothing"
    );
}

#[test]
fn a_restore_beside_a_presence_writer_restores_no_presence() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let (_tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let (_, _, card) = lost_with_a_card(&root, &data);
    let store = state.join("store.db");

    let phase = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    // Each presence write: the restores done before it, and what it returned.
    type Touches = Mutex<Vec<(usize, Result<(), String>)>>;
    let results: Arc<Touches> = Arc::default();
    let thread = {
        let (phase, stop, results, store) = (phase.clone(), stop.clone(), results.clone(), store.clone());
        std::thread::spawn(move || {
            let writer = Store::open_existing(&store).unwrap().expect("a store");
            let row = planroom_contract::PresenceRow {
                actor: Actor::Person { id: "watcher".into(), name: None },
                surface: Surface::Cli,
                client_id: "presence-thread".into(),
                doing: None,
                focus: None,
                seen_at: Timestamp("2026-10-30T00:00:00Z".into()),
            };
            while !stop.load(Ordering::SeqCst) {
                let at = phase.load(Ordering::SeqCst);
                let outcome = match writer.touch_presence(&row) {
                    Ok(()) => Ok(()),
                    Err(planroom_store::StoreError::Busy) => Err("busy".to_owned()),
                    Err(e) => Err(format!("unexpected: {e}")),
                };
                results.lock().unwrap().push((at, outcome));
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        })
    };
    let wait_for_ok = |at: usize| {
        for _ in 0..100 {
            if results.lock().unwrap().iter().any(|(p, r)| *p == at && r.is_ok()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("no presence write landed after restore {at}");
    };
    wait_for_ok(0);

    let (code, env) = as_person(&root, &data, "review", head(&root, &data), &[&card, "accept"], &[]);
    assert_eq!(code, 0, "{env}");
    phase.store(1, Ordering::SeqCst);
    wait_for_ok(1);

    let copies = pre_restore_copies(&data, &workspace_id(&state));
    let copy = copies.last().expect("the accept's pre-restore copy");
    plant_presence(copy);
    let (code, env) = restore_as_person(&root, &data, copy.to_str().unwrap(), &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["restored"], "copy", "{env}");
    phase.store(2, Ordering::SeqCst);
    wait_for_ok(2);
    stop.store(true, Ordering::SeqCst);
    thread.join().unwrap();

    for (at, outcome) in results.lock().unwrap().iter() {
        assert!(outcome.as_ref().is_ok() || outcome.as_ref().unwrap_err() == "busy", "{at}: {outcome:?}");
    }
    assert_eq!(sql(&store, "SELECT COUNT(*) FROM presence"), "1", "only the thread's row");
    assert_eq!(sql(&store, "SELECT client_id FROM presence"), "presence-thread");
}

/// A workspace whose store copy holds five person edits and whose live store, migrated past it,
/// holds `n` person renames after them, put back to the copy by `doctor --restore`: the scratch
/// directory, the root, the data root, the IDs of the set-aside edits in order, the renamed node
/// IDs in the same order, and the restore's reply. The live store is then this build's version
/// again, with `meta.replay_set` naming the edits set aside.
fn restored_with_a_set(n: usize) -> (tempfile::TempDir, PathBuf, PathBuf, Vec<String>, Vec<String>, Value) {
    let (tmp, root, data) = fixture_workspace();
    let state = root.join(".planroom");
    let store = state.join("store.db");
    let container = "storefront:container:storefront";
    let mut base = head(&root, &data);
    for i in 1..=5 {
        base = edit(&root, &data, base, &["rename", container, &format!("early{i}")]);
    }
    let early = tmp.path().join("early.db");
    sql_exec(&store, "VACUUM INTO ?", &[early.to_str().unwrap()]);
    let nodes = node_ids(&root, &data, n);
    let mut set_aside = Vec::with_capacity(n);
    for (i, node) in nodes.iter().enumerate() {
        let before = held(&state);
        base = edit(&root, &data, base, &["rename", node, &format!("late{i}")]);
        let new: Vec<String> = held(&state).into_keys().filter(|id| !before.contains_key(id)).collect();
        assert_eq!(new.len(), 1, "one event per rename: {new:?}");
        set_aside.extend(new);
    }
    let id = workspace_id(&state);
    drop(Store::open_with_migrations(&store, &init(&id), grown(1)).unwrap());
    let dir = data.join("backups").join(&id);
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join("store-v1.db");
    std::fs::copy(&early, &copy).unwrap();

    let (code, env) = restore_as_person(&root, &data, copy.to_str().unwrap(), &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["restored"], "copy", "{env}");
    assert_eq!(env["result"]["set_aside"], n, "{env}");
    assert_eq!(env["result"]["made_on"], "0.2.0", "{env}");
    assert_eq!(sql(&store, "PRAGMA user_version"), "1");
    assert_eq!(sql(&store, "SELECT COUNT(*) FROM meta WHERE key = 'replay_set'"), "1");
    (tmp, root, data, set_aside, nodes, env)
}

/// The workspace's replay set, as `meta.replay_set` holds it.
fn replay_set(state: &Path) -> Value {
    serde_json::from_str(&sql(&state.join("store.db"), "SELECT value FROM meta WHERE key = 'replay_set'"))
        .unwrap()
}

/// Mark the replay set as made on a store older than this build's, as a migration past the
/// restored copy leaves it, and put its file where this process's data root keeps backups; the
/// set's JSON.
fn await_replay(state: &Path, data: &Path, this_data: &Path) -> Value {
    let store = state.join("store.db");
    sql_exec(
        &store,
        "UPDATE meta SET value = json_set(value, '$.store_version', 0) WHERE key = 'replay_set'",
        &[],
    );
    let set = replay_set(state);
    let id = workspace_id_raw(&store);
    let file = set["file"].as_str().unwrap();
    let here = this_data.join("backups").join(&id);
    std::fs::create_dir_all(&here).unwrap();
    std::fs::copy(data.join("backups").join(&id).join(file), here.join(file)).unwrap();
    set
}

/// The workspace ID of the store at `store`, read without opening it in this process.
fn workspace_id_raw(store: &Path) -> String {
    sql(store, "SELECT value FROM meta WHERE key = 'workspace_id'")
}

/// Every event the store at `state` holds, opened in this process.
fn all_events(state: &Path) -> Vec<planroom_contract::Event> {
    open(state).events_after(0, 1_000_000).unwrap()
}

/// The name `map --level code` shows for the node `id`, paging until it's found.
fn code_name(root: &Path, data: &Path, id: &str) -> Value {
    let mut cursor: Option<String> = None;
    for _ in 0..50 {
        let mut args = vec!["map", "--level", "code"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c.as_str()]);
        }
        let (code, env) = run(root, data, &args, &[], "");
        assert_eq!(code, 0, "{env}");
        if let Some(node) = env["result"]["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id) {
            return node["name"].clone();
        }
        match env["meta"]["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    panic!("the map holds no node {id}");
}

#[test]
fn restore_then_upgrade_replays_set_aside_edits() {
    let (this_data, _guard) = setup();
    let (_tmp, root, data, set_aside, _, env) = restored_with_a_set(40);
    let state = root.join(".planroom");
    let copy_head = env["result"]["seq"].as_u64().unwrap();
    let restored = held(&state);
    assert!(set_aside.iter().all(|id| !restored.contains_key(id)), "the restore set every edit aside");
    let set = await_replay(&state, &data, this_data);
    assert_eq!(set["base"], copy_head, "{set}");
    assert_eq!(set["count"], 40, "{set}");

    // The open that finds the store past the set's version replays it.
    let events = all_events(&state);
    for id in &set_aside {
        let event = events.iter().find(|e| &e.id.0 == id).unwrap_or_else(|| panic!("{id} came back"));
        assert_eq!(event.status, Status::Applied, "{id}");
        assert!(event.seq > copy_head, "{id} is written after the restored head");
        assert_eq!(event.client.id, format!("restore:{}", set["file"].as_str().unwrap()));
        let EventBody::Edit(edit) = &event.body else { panic!("{id} is an edit") };
        assert_eq!(edit.base, copy_head, "{id} is checked at the copy's head");
    }
    let replayed = events.len();
    assert_eq!(sql(&state.join("store.db"), "SELECT COUNT(*) FROM meta WHERE key = 'replay_set'"), "0");

    assert_eq!(all_events(&state).len(), replayed, "a second open adds none");
    sql_exec(
        &state.join("store.db"),
        "INSERT INTO meta (key, value) VALUES ('replay_set', ?)",
        &[&set.to_string()],
    );
    assert_eq!(all_events(&state).len(), replayed, "a replanted set adds none");
    assert_eq!(sql(&state.join("store.db"), "SELECT COUNT(*) FROM meta WHERE key = 'replay_set'"), "0");

    // A line whose ID the store already holds is skipped, and the rest still replay.
    let (_tmp, root, data, set_aside, _, _) = restored_with_a_set(3);
    let state = root.join(".planroom");
    let held_before = held(&state);
    let set = await_replay(&state, &data, this_data);
    let id = workspace_id_raw(&state.join("store.db"));
    let (_, backed) = backed_up(&data.join("backups").join(&id).join("events.ndjson"));
    let mut planted = backed
        .into_iter()
        .find(|e| held_before.contains_key(&e.id.0) && matches!(e.body, EventBody::Edit(_)))
        .expect("an edit the copy holds");
    planted.idem = "planted-replay-line".into();
    let file = this_data.join("backups").join(&id).join(set["file"].as_str().unwrap());
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&serde_json::to_string(&planted).unwrap());
    text.push('\n');
    std::fs::write(&file, text).unwrap();
    let events = all_events(&state);
    for id in &set_aside {
        let event = events.iter().find(|e| &e.id.0 == id).unwrap_or_else(|| panic!("{id} came back"));
        assert_eq!(event.status, Status::Applied, "{id}");
    }
    assert_eq!(events.iter().filter(|e| e.id == planted.id).count(), 1, "the held ID is skipped");
    assert!(events.iter().all(|e| e.idem != "planted-replay-line"), "nothing is written for it");
    assert_eq!(sql(&state.join("store.db"), "SELECT COUNT(*) FROM meta WHERE key = 'replay_set'"), "0");
}

#[test]
fn a_replayed_edit_conflicts_with_a_post_restore_edit() {
    let (this_data, _guard) = setup();
    let (_tmp, root, data, set_aside, nodes, _) = restored_with_a_set(4);
    let state = root.join(".planroom");
    let contested = &nodes[1];
    edit(&root, &data, head(&root, &data), &["rename", contested, "after-restore"]);
    await_replay(&state, &data, this_data);

    let events = all_events(&state);
    for (i, id) in set_aside.iter().enumerate() {
        let event = events.iter().find(|e| &e.id.0 == id).unwrap_or_else(|| panic!("{id} came back"));
        let wanted = if i == 1 { Status::Conflict } else { Status::Applied };
        assert_eq!(event.status, wanted, "{id} renamed {}", nodes[i]);
    }
    assert_eq!(code_name(&root, &data, contested), "after-restore", "the post-restore name holds");
    assert_eq!(code_name(&root, &data, &nodes[2]), "late2", "the others apply");
}
