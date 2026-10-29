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
    // SQLite opens no file whose path is over 512 bytes, so the store's path is as long as it can
    // be, well over the 160 characters doctor shows whole; the data root and the packs' sources
    // are 3,000 bytes. The workspace's own name holds the secret too, so the tail doctor shows of
    // its path holds it.
    let root = long_dir(tmp.path(), &named, 440, &named);
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    planroom_testkit::fixture(&root);
    let data = long_dir(&tmp.path().join("data"), &named, 3_000, "data");
    person_init(&root, &data);

    let packs_dir = long_dir(tmp.path(), "packs", 3_000, "src");
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
