//! Actors and the person's authority (architecture.md, Actors and Actor rules; DR-4, DR-64,
//! DR-72, DR-75, DR-81): who is acting, person keys and one-time action tokens, and the terminal
//! path. `person init` registers a public key only when none is registered, and a person-only
//! command needs a fresh, unspent ECDSA P-256 token over its own command and arguments, or, where
//! the command takes it, a confirmation on a terminal. The test harness signs its own tokens with
//! test keys, and drives the terminal path through a pseudo-terminal it opens itself.

use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom::app::actor::{self, Bound, Caller, NoTerminal, Terminal, TerminalPath};
use planroom_contract::{ActorRule, Class, CommandSpec, ErrorCode, Idea, Undo};
use planroom_kernel::keys::{person_token_message, sha256_plain_hex};
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

/// A test key pair from a fixed scalar.
fn key(seed: u8) -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = seed;
    bytes[0] = 1;
    SigningKey::from_slice(&bytes).unwrap()
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

fn public(key: &SigningKey) -> String {
    b64url(&key.verifying_key().to_sec1_point(false).to_bytes())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// A token over `command` with `args`, at `at`.
fn signed(key: &SigningKey, command: &str, args: &[&str], at: u64) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let sig: Signature = key.sign(&person_token_message(command, &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// A token over `person init` with `args`, at `at`.
fn token(key: &SigningKey, args: &[&str], at: u64) -> String {
    signed(key, "person init", args, at)
}

/// The engine's command line in `root`, with `--json`, the data root under it and none of the
/// caller's actor, token or client. `open` finds no display, and its opener, if it looked for one,
/// is one that can't start, so no test opens a browser.
fn engine(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = text_mode(root, args, env);
    cmd.arg("--json");
    cmd
}

/// [`engine`] without `--json`.
fn text_mode(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args).current_dir(root);
    cmd.env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env("PLANROOM_TEST_OPENER", "/nonexistent/opener");
    // No browser can be seen unless the test gives a display: on macOS one always can, so the
    // run is an SSH session's there, as on a Linux host with no display.
    if !env.iter().any(|(k, _)| *k == "DISPLAY") {
        cmd.env("SSH_CONNECTION", "test");
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

/// `planroom ARGS --json` with `stdin` and the environment `env`: its exit code and envelope.
fn cli(root: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let mut child = engine(root, args, env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The command may refuse before it reads its stdin; its reply is what the test checks.
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

/// `planroom person init ARGS --json` with `stdin` and the environment `env`.
fn init(root: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let mut argv = vec!["person", "init"];
    argv.extend(args);
    cli(root, &argv, stdin, env)
}

/// `ptsname` keeps its answer in one static buffer.
static PTY: Mutex<()> = Mutex::new(());

/// `planroom ARGS --json` run as a person at a terminal would run it: in a session of its own
/// whose controlling terminal, and stdin, is a fresh pseudo-terminal. `typed` up to its last
/// end-of-input (`\x04`) is typed before it starts, as rows piped in at a terminal would be; the
/// rest is the person's answer, typed once the terminal shows the `[y/N]` question. The terminal
/// is read the whole time, so nothing the program writes there waits on a full buffer, and a run
/// past 60 s is stopped and fails with what the terminal showed. Returns its exit code, its
/// envelope from stdout (a pipe, as the mod reads it) and what it wrote on the terminal.
fn at_terminal(root: &Path, args: &[&str], typed: &str, env: &[(&str, &str)]) -> (i32, Value, String) {
    let (ahead, answer) = match typed.rfind('\x04') {
        Some(at) => typed.split_at(at + 1),
        None => ("", typed),
    };
    let (master, slave) = {
        let _one = PTY.lock().unwrap();
        // SAFETY: plain calls on descriptors this test owns; `ptsname`'s buffer is read under
        // the lock before any other call can reuse it.
        unsafe {
            let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            assert!(master >= 0, "posix_openpt");
            assert_eq!(libc::grantpt(master), 0);
            assert_eq!(libc::unlockpt(master), 0);
            let name = CStr::from_ptr(libc::ptsname(master)).to_owned();
            let slave = libc::open(name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
            assert!(slave >= 0, "open the pty's terminal side");
            (File::from(OwnedFd::from_raw_fd(master)), OwnedFd::from_raw_fd(slave))
        }
    };
    let mut cmd = engine(root, args, env);
    cmd.stdin(Stdio::from(slave)).stdout(Stdio::piped()).stderr(Stdio::piped());
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut writer = master.try_clone().unwrap();
    writer.write_all(ahead.as_bytes()).unwrap();
    let mut child = cmd.spawn().unwrap();
    drop(cmd);
    // SAFETY: a flag change on a descriptor this test owns.
    unsafe {
        let fd = master.as_raw_fd();
        libc::fcntl(fd, libc::F_SETFL, libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK);
    }
    let (mut master, mut shown, mut buf) = (master, Vec::new(), [0u8; 4096]);
    let mut pending = (!answer.is_empty()).then_some(answer);
    let deadline = Instant::now() + Duration::from_secs(60);
    let drain = |master: &mut File, shown: &mut Vec<u8>, buf: &mut [u8]| {
        while let Ok(n) = master.read(buf) {
            if n == 0 {
                break;
            }
            shown.extend_from_slice(&buf[..n]);
        }
    };
    loop {
        drain(&mut master, &mut shown, &mut buf);
        if let Some(text) = pending.filter(|_| String::from_utf8_lossy(&shown).contains("[y/N]")) {
            writer.write_all(text.as_bytes()).unwrap();
            pending = None;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "planroom {args:?} at a terminal ran past 60 s; answer still pending: {}; the terminal showed: {:?}",
                pending.is_some(),
                String::from_utf8_lossy(&shown)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output().unwrap();
    drain(&mut master, &mut shown, &mut buf);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env, String::from_utf8_lossy(&shown).into_owned())
}

fn code(env: &Value) -> &str {
    env["error"]["code"].as_str().unwrap_or("")
}

fn reason(env: &Value) -> &str {
    env["error"]["details"]["reason"].as_str().unwrap_or("")
}

fn scanned() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let out = engine(&root, &["scan"], &[]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    (tmp, root)
}

/// A scanned workspace with `key` registered as its person key.
fn registered(key: &SigningKey) -> (tempfile::TempDir, PathBuf) {
    let (tmp, root) = scanned();
    let (code, env) = init(&root, &[], &public(key), &[("PLANROOM_PERSON_TOKEN", &token(key, &[], now()))]);
    assert_eq!(code, 0, "{env}");
    (tmp, root)
}

/// Every `serve` the workspace's data root records, stopped when dropped: `open` starts one in
/// the background.
struct Serves(PathBuf);

impl Drop for Serves {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(self.0.join(".data/serve")).into_iter().flatten().flatten() {
            let record: Value =
                serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap_or_default())
                    .unwrap_or_default();
            if let Some(pid) = record["pid"].as_u64() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
        }
    }
}

/// A terminal that answers every question with `answer`, and counts them.
struct Answers {
    answer: Option<bool>,
    asked: usize,
}

impl Terminal for Answers {
    fn present(&mut self) -> bool {
        self.answer.is_some()
    }

    fn confirm(&mut self, _question: &str) -> Option<bool> {
        self.asked += 1;
        self.answer
    }
}

/// A leaf as the catalogue declares one; `edit` and the hand-off leaves join the catalogue in
/// later changes, and their rules are the actor module's already.
fn leaf(path: &'static [&'static str], actor: ActorRule, class: Class, needs_base: bool) -> CommandSpec {
    CommandSpec {
        path,
        idea: Idea::Intent,
        summary: "",
        actor,
        class,
        budget_tokens: 400,
        needs_base,
        spends: false,
        errors: &[],
        undo: Undo::Edit,
        tool: None,
        room: false,
    }
}

fn caller(argv: &[&str], actor: Option<&str>, token: Option<String>) -> Caller {
    Caller {
        token,
        actor: actor.map(str::to_owned),
        client: None,
        argv: argv.iter().map(|a| (*a).to_owned()).collect(),
    }
}

#[test]
fn person_init_registers_a_key_only_when_none_is_registered() {
    let (_tmp, root) = scanned();
    let (a, b) = (key(7), key(9));
    let (pa, pb) = (public(&a), public(&b));

    // No token, or an agent: never a person.
    let (code, env) = init(&root, &[], &pa, &[]);
    assert_eq!((code, env["error"]["code"].as_str(), reason(&env)), (5, Some("person_required"), "missing"));
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")]);
    assert_eq!(reason(&env), "not_a_person");

    // A token from another key, a stale one, or one for other arguments.
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&b, &[], now()))]);
    assert_eq!(reason(&env), "bad_signature");
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now() - 60))]);
    assert_eq!(reason(&env), "stale");
    let (_, env) = init(&root, &["--rekey"], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!(reason(&env), "bad_signature");
    let (code, env) = init(&root, &[], "not a key", &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")));

    // The first registration, signed by the key it registers, works once.
    let first = token(&a, &[], now());
    let (code, env) =
        init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &first), ("PLANROOM_CLIENT", "mod:s1")]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["surface"], "mod");
    assert_eq!(env["result"]["rekeyed"], false);
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &first)]);
    assert_eq!(reason(&env), "spent", "a token works once");

    // Another key is a mismatch, even signed by itself, until a rekey.
    let (code, env) = init(&root, &[], &pb, &[("PLANROOM_PERSON_TOKEN", &token(&b, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_key_mismatch")));
    assert_eq!(env["error"]["details"]["surface"], "mod");
    assert_eq!(env["error"]["retry"], "never");

    // The store holds only the public key.
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap().as_deref(), Some(pa.as_str()));
    drop(store);

    // A rekey under the old key replaces it.
    let (code, env) =
        init(&root, &["--rekey"], &pb, &[("PLANROOM_PERSON_TOKEN", &token(&a, &["--rekey"], now()))]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["rekeyed"], true);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap().as_deref(), Some(pb.as_str()));
}

#[test]
fn a_rekey_takes_the_mismatch_panes_press_under_the_new_key() {
    let (_tmp, root) = scanned();
    let (a, b) = (key(3), key(4));
    let (code, _) = init(&root, &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!(code, 0);
    let press = token(&b, &["--rekey"], now());
    let (code, env) = init(&root, &["--rekey"], &public(&b), &[("PLANROOM_PERSON_TOKEN", &press)]);
    assert_eq!(code, 0, "{env}");
    assert!(env["result"]["fingerprint"].as_str().unwrap().len() == 16);
}

#[test]
fn person_init_needs_a_scanned_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let a = key(5);
    let (code, env) =
        init(dir.path(), &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("workspace_not_found")));
}

// --- Resolution ---

#[test]
fn an_unnamed_write_with_no_terminal_is_actor_required() {
    // A write to user-authored state, as `edit` is, resolved as every write resolves its actor.
    let edit = leaf(&["edit"], ActorRule::Any, Class::Change, true);
    let argv = ["edit", "--base", "4", "rename", "x", "y"];
    let bound = Bound { idem: None, base: Some(4) };
    let a = key(21);
    let keys = [a.verifying_key()];

    // No PLANROOM_ACTOR, no token and no terminal: nobody stands behind it.
    let mut none = NoTerminal;
    let e = actor::writer(&caller(&argv, None, None), &edit, &bound, &keys, now(), &mut none).unwrap_err();
    assert_eq!(e.code, ErrorCode::ActorRequired);
    assert_eq!(e.code.exit_code(), 2);
    assert_eq!(e.hint.as_deref(), Some("export PLANROOM_ACTOR=agent:<id>:<role>"));

    // A terminal where nobody confirms is no one either.
    let mut declined = Answers { answer: Some(false), asked: 0 };
    let e =
        actor::writer(&caller(&argv, None, None), &edit, &bound, &keys, now(), &mut declined).unwrap_err();
    assert_eq!((e.code, declined.asked), (ErrorCode::ActorRequired, 1));

    // An agent that names itself writes as itself, and is never asked.
    let mut never = Answers { answer: Some(true), asked: 0 };
    let named = caller(&argv, Some("agent:1a2b3c4d:chat"), None);
    let acting = actor::writer(&named, &edit, &bound, &keys, now(), &mut never).unwrap();
    assert_eq!(
        acting.actor,
        planroom_contract::Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None }
    );
    assert_eq!((acting.digest, never.asked), (None, 0));
    let bad = caller(&argv, Some("agent:not an id"), None);
    let e = actor::writer(&bad, &edit, &bound, &keys, now(), &mut NoTerminal).unwrap_err();
    assert_eq!((e.code, e.details["field"].as_str()), (ErrorCode::InvalidArgument, Some("PLANROOM_ACTOR")));

    // A person's token, or a person who confirms at the terminal, writes as the person.
    let args = ["--base", "4", "rename", "x", "y"];
    let t = {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let at = now();
        let sig: Signature = a.sign(&person_token_message("edit", &args, "", "4", at));
        format!("{at}.{}", b64url(&sig.to_bytes()))
    };
    let acting =
        actor::writer(&caller(&argv, None, Some(t)), &edit, &bound, &keys, now(), &mut NoTerminal).unwrap();
    assert!(matches!(acting.actor, planroom_contract::Actor::Person { .. }));
    assert!(acting.digest.is_some(), "a token is spent when the write commits");
    let mut yes = Answers { answer: Some(true), asked: 0 };
    let acting = actor::writer(&caller(&argv, None, None), &edit, &bound, &keys, now(), &mut yes).unwrap();
    assert!(matches!(acting.actor, planroom_contract::Actor::Person { .. }));
    assert_eq!((acting.digest, yes.asked), (None, 1));
}

#[test]
fn a_pty_without_confirmation_is_no_person() {
    // `open` in a workspace no mod has registered a key for takes a terminal confirmation that
    // names it (DR-81): a terminal where nobody says yes makes no person.
    let (_tmp, root) = scanned();
    let _serves = Serves(root.clone());
    for typed in ["n\n", "\n", "maybe\n"] {
        let (code, env, shown) = at_terminal(&root, &["open"], typed, &[]);
        assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "not_confirmed"), "{env}");
        assert!(shown.contains("run open as"), "the question names the action: {shown:?}");
    }
    // With no terminal at all, there is no one to ask.
    let (code, env) = cli(&root, &["open"], "", &[]);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "missing"), "{env}");
    // Nothing was started for a refused open.
    assert!(
        !root.join(".data/serve").exists()
            || std::fs::read_dir(root.join(".data/serve")).unwrap().count() == 0
    );
}

#[test]
fn reads_need_no_actor() {
    let (_tmp, root) = scanned();
    for args in [
        &["map"][..],
        &["map", "--detail", "graph"],
        &["focus"],
        &["find", "orders"],
        &["schema", "--commands"],
        &["version"],
    ] {
        // No actor, no token, no terminal: a read just runs.
        let (code, env) = cli(&root, args, "", &[]);
        assert_eq!((code, &env["ok"]), (0, &json!(true)), "{args:?}: {env}");
        // Whatever the caller says it is, a read neither checks nor spends anything.
        let (code, env) = cli(&root, args, "", &[("PLANROOM_ACTOR", "person:mallory")]);
        assert_eq!((code, &env["ok"]), (0, &json!(true)), "{args:?}: {env}");
    }
}

#[test]
fn scan_needs_no_actor() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    // No actor, no token, no terminal: a scan only records what it observed.
    let out = engine(&root, &["scan"], &[]).stdin(Stdio::null()).output().unwrap();
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["ok"], true, "{env}");
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let events = store.events_after(0, 100).unwrap();
    assert!(!events.is_empty());
    for event in &events {
        assert!(
            matches!(&event.actor, planroom_contract::Actor::System { part } if part == "scan"),
            "{:?}",
            event.actor
        );
    }
    drop(store);
    // An agent's scan is the system's too.
    let out = engine(&root, &["scan"], &[("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn tool_handlers_never_send_a_person() {
    // A mod tool handler's call names the chat agent. Even with a valid token beside it, as when
    // one leaked into its environment, it is never the person's, and the token stays unspent.
    let a = key(11);
    let (_tmp, root) = registered(&a);
    // Signed seconds before the registration's own token, so it is another token.
    let t = token(&a, &[], now() - 10);
    let tool = [
        ("PLANROOM_ACTOR", "agent:1a2b3c4d:chat"),
        ("PLANROOM_CLIENT", "mod:5e55i0n1"),
        ("PLANROOM_PERSON_TOKEN", t.as_str()),
    ];
    let (code, env) = init(&root, &[], &public(&a), &tool);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "not_a_person"), "{env}");
    let (code, env) = cli(&root, &["open"], "", &tool);
    assert_eq!((code, reason(&env)), (5, "not_a_person"), "{env}");
    // The person's own press with the same token still works: the agent's call spent nothing.
    let (code, env) = init(&root, &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
}

// --- Imports ---

fn inbox_line(doc: &str, command: &str, request: Value) -> String {
    let row = json!({
        "format": "planroom.inbox/v1",
        "instance": "i1",
        "counter": 1,
        "command": command,
        "request": request,
        "base": 5,
        "at": "2026-10-27T10:00:00Z",
    });
    json!({"path": format!("inbox/u_1/events/{doc}"), "row": row, "version": 1}).to_string()
}

fn sync_dirs(root: &Path) -> usize {
    std::fs::read_dir(root.join(".planroom/share/sync")).map_or(0, Iterator::count)
}

#[test]
fn agent_cannot_import_inbox_rows() {
    let (_tmp, root) = scanned();
    let edit = inbox_line("d1", "edit", json!({"op": "rename", "target": "x", "name": "y"}));
    let read = inbox_line("d2", "map", json!({}));
    let rows = format!("{edit}\n{read}\n");
    let agent = [("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")];

    // An agent's import of a row that changes the workspace changes nothing at all.
    let (code, env) = cli(&root, &["import", "-"], &rows, &agent);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "not_a_person"), "{env}");
    assert_eq!(env["error"]["details"]["command"], "import");
    assert_eq!(sync_dirs(&root), 0, "nothing was written for the session to publish");

    // A read request alone is answered for an agent, as any sync answers it (DR-77).
    let (code, env) = cli(&root, &["import", "-"], &format!("{read}\n"), &agent);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["counts"]["imported"], 1);
    assert_eq!(env["result"]["person"], false);

    // The mod's tokenless sync, which names no one, leaves the change for a person's sync.
    let (code, env) = cli(&root, &["import", "-"], &rows, &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["counts"]["left"], 1);
    assert_eq!(env["result"]["person"], false);
}

#[test]
fn agent_cannot_import_an_event_export() {
    let (_tmp, root) = scanned();
    let seq = || Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap().seq().unwrap();
    let before = seq();
    let export = format!(
        "{}\n{}\n",
        json!({"format": "planroom.event/v1", "workspace_id": "w", "store_version": 1}),
        json!({"format": "planroom.event/v1", "seq": 1, "kind": "edit"})
    );
    let (code, env) = cli(&root, &["import", "-"], &export, &[("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")]);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "not_a_person"), "{env}");
    // Nor without a name, a token or a terminal: an export is never a sync's.
    let (code, env) = cli(&root, &["import", "-"], &export, &[]);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "missing"), "{env}");
    assert_eq!(seq(), before, "the store is unchanged");
    assert_eq!(sync_dirs(&root), 0);
}

// --- Person keys ---

#[test]
fn person_claim_without_key_is_refused() {
    let a = key(13);
    let (_tmp, root) = registered(&a);
    let claim = [("PLANROOM_ACTOR", "person:mallory")];
    // A claim to be a person is no key: refused before anything is read or opened.
    let (code, env) = cli(&root, &["open"], "", &claim);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "missing"), "{env}");
    let edit = inbox_line("d1", "edit", json!({}));
    let (code, env) = cli(&root, &["import", "-"], &format!("{edit}\n"), &claim);
    assert_eq!((code, reason(&env)), (5, "missing"), "{env}");
    let (code, env) = init(&root, &[], &public(&a), &claim);
    assert_eq!((code, reason(&env)), (5, "missing"), "{env}");
    // A write claimed for a person, with no token, is refused the same way, terminal or not.
    let edit = leaf(&["edit"], ActorRule::Any, Class::Change, true);
    let mut yes = Answers { answer: Some(true), asked: 0 };
    let e = actor::writer(
        &caller(&["edit"], Some("person:mallory"), None),
        &edit,
        &Bound::default(),
        &[],
        now(),
        &mut yes,
    )
    .unwrap_err();
    assert_eq!((e.code, yes.asked), (ErrorCode::PersonRequired, 0));
    // A key that isn't registered signs nothing here.
    let other = key(14);
    let (code, env) =
        cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &signed(&other, "open", &[], now()))]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
}

#[test]
fn a_mismatched_key_returns_person_key_mismatch() {
    let (a, b) = (key(15), key(16));
    let (_tmp, root) = registered(&a);
    let (code, env) = init(&root, &[], &public(&b), &[("PLANROOM_PERSON_TOKEN", &token(&b, &[], now()))]);
    assert_eq!((code, self::code(&env)), (5, "person_key_mismatch"), "{env}");
    assert_eq!(env["error"]["retry"], "never");
    assert_eq!(env["error"]["details"]["surface"], "cli");
    assert!(env["error"]["details"]["registered_at"].as_str().is_some_and(|at| at.ends_with('Z')));
    assert_eq!(env["error"]["hint"], "press Use this session's key in the Planroom pane");
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap(), Some(public(&a)), "the registered key stays");
}

#[test]
fn rekey_needs_the_old_key_or_a_press() {
    let (a, b, c) = (key(17), key(18), key(19));
    let (_tmp, root) = registered(&a);
    let pubkey = || {
        Store::open_existing(&root.join(".planroom/store.db"))
            .unwrap()
            .unwrap()
            .meta("person_pubkey")
            .unwrap()
    };
    // Neither the old key nor the new one: refused, and the key stays.
    let (code, env) =
        init(&root, &["--rekey"], &public(&b), &[("PLANROOM_PERSON_TOKEN", &token(&c, &["--rekey"], now()))]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
    assert_eq!(pubkey(), Some(public(&a)));
    // Under the old key: replaced.
    let (code, env) =
        init(&root, &["--rekey"], &public(&b), &[("PLANROOM_PERSON_TOKEN", &token(&a, &["--rekey"], now()))]);
    assert_eq!((code, &env["result"]["rekeyed"]), (0, &json!(true)), "{env}");
    assert_eq!(pubkey(), Some(public(&b)));
    // The mismatch pane's press, signed by the session's own key, the new one: replaced.
    let (code, env) =
        init(&root, &["--rekey"], &public(&c), &[("PLANROOM_PERSON_TOKEN", &token(&c, &["--rekey"], now()))]);
    assert_eq!((code, &env["result"]["rekeyed"]), (0, &json!(true)), "{env}");
    assert_eq!(pubkey(), Some(public(&c)));
}

#[test]
fn a_leaked_store_cannot_sign_a_person_action() {
    let a = key(20);
    let (_tmp, root) = registered(&a);
    // Everything the store keeps, its WAL included, holds the public key and nothing that signs.
    let mut bytes = Vec::new();
    for entry in std::fs::read_dir(root.join(".planroom")).unwrap().flatten() {
        if entry.file_name().to_string_lossy().starts_with("store.db") {
            bytes.extend(std::fs::read(entry.path()).unwrap());
        }
    }
    let secret = a.to_bytes();
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    let holds = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    assert!(holds(public(&a).as_bytes()), "the store holds the public key");
    for form in [&secret[..], b64url(&secret).as_bytes(), hex.as_bytes()] {
        assert!(!holds(form), "the store holds the private key");
    }
    // Whoever copies the store gets a key that checks tokens, never one that makes them: a token
    // signed with anything derived from what the store holds is refused.
    let leaked = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let held = leaked.meta("person_pubkey").unwrap().unwrap();
    drop(leaked);
    let derived = SigningKey::from_slice(
        &(0..32)
            .map(|i| u8::from_str_radix(&sha256_plain_hex(held.as_bytes())[2 * i..2 * i + 2], 16).unwrap())
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    let (code, env) =
        cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &signed(&derived, "open", &[], now()))]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
}

// --- Tokens ---

#[test]
fn a_token_works_once_and_only_for_its_command() {
    let a = key(22);
    let (_tmp, root) = registered(&a);
    let _serves = Serves(root.clone());
    // A token for `person init` doesn't open, and one for `import` doesn't either.
    let (code, env) = cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
    let (code, env) =
        cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &signed(&a, "import", &["-"], now()))]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
    // Nor does `open`'s token run `import`.
    let t = signed(&a, "open", &[], now());
    let (code, env) = cli(&root, &["import", "-"], "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!((code, reason(&env)), (5, "bad_signature"), "{env}");
    // Its own command takes it once.
    let (code, env) = cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let (code, env) = cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!((code, reason(&env)), (5, "spent"), "{env}");
}

/// A program that writes its environment to `log`, then runs `then` with its arguments.
fn recorder(dir: &Path, name: &str, log: &Path, then: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nenv >> '{}'\necho --- >> '{}'\nexec {then} \"$@\"\n",
            log.display(),
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    path
}

#[test]
fn a_token_the_mod_signed_verifies_in_the_engine() {
    // The key and the tokens `packages/mod/plugin-tests/engine-token.test.tsx` pins: what the mod
    // signs for a typed `/room open` and its `person init`, with its clock at `AT`. The engine's
    // own verifier accepts each for its own line, at that instant, and nothing else.
    const PUBLIC: &str =
        "BEN2rU90TjAtEsnOkmVgumbnM-L-SBpZEHpLvaMXvq100Ge8_e-8IK9n2XODxEPZc2Vd-9VrPveTcKq3WTF7ciU";
    const AT: u64 = 1_800_000_000;
    const OPEN: &str =
        "1800000000.bDwneg_gIbPTaRcJRk2rJGc3ILkYpsLsvTY5DWo63TYxmcKWoh7aLtn3qrSTFsjdqTtTfgQmCej_syhEqc1wiQ";
    const INIT: &str =
        "1800000000.3M57EZA28OLYF87JDWs9o1Db1n9PbHUbwWdmN3Zcm2gAJ5D0nZvcuQ3eaqCqt-dGTVMuun70YOOEbSihAArkbg";
    let key = actor::parse_key(PUBLIC).expect("the mod's public key parses");
    let mut secret = [0u8; 32];
    secret[0] = 1;
    secret[31] = 42;
    assert_eq!(public(&SigningKey::from_slice(&secret).unwrap()), PUBLIC, "the same key on both sides");
    let open = planroom_contract::Command::Open(Default::default());
    let init = planroom_contract::Command::PersonInit(Default::default());
    let check = |token: &str, argv: &[&str], spec: &CommandSpec, now: u64| {
        actor::check(&caller(argv, None, Some(token.to_owned())), spec, &Bound::default(), &[&key], now)
    };
    assert!(check(OPEN, &["open", "--json"], open.spec(), AT).is_ok());
    assert!(check(INIT, &["person", "init", "--json"], init.spec(), AT + 30).is_ok());
    // Only for its own line, and only within 30 s of its `at`.
    assert_eq!(
        check(OPEN, &["person", "init", "--json"], init.spec(), AT),
        Err(actor::Refused::BadSignature)
    );
    assert_eq!(
        check(INIT, &["person", "init", "--rekey"], init.spec(), AT),
        Err(actor::Refused::BadSignature)
    );
    assert_eq!(check(OPEN, &["open", "--json"], open.spec(), AT + 31), Err(actor::Refused::Stale));
    // And as the person resolution every person-only command runs.
    let acting = actor::person(
        &caller(&["open"], None, Some(OPEN.to_owned())),
        open.spec(),
        &Bound::default(),
        &[&key],
        AT,
        TerminalPath::Refused,
        &mut NoTerminal,
    )
    .unwrap();
    assert!(matches!(acting.actor, planroom_contract::Actor::Person { .. }));
}

#[test]
fn children_never_inherit_the_token() {
    let a = key(23);
    let (tmp, root) = registered(&a);
    let _serves = Serves(root.clone());
    let t = signed(&a, "open", &[], now());
    // git, found first on the caller's PATH, records what it was given, then runs.
    let real_git =
        String::from_utf8(Command::new("sh").args(["-c", "command -v git"]).output().unwrap().stdout)
            .unwrap()
            .trim()
            .to_owned();
    let gitlog = tmp.path().join("git.env");
    let shims = tmp.path().join("shims");
    recorder(&shims, "git", &gitlog, &real_git);
    let path = format!("{}:{}", shims.display(), std::env::var("PATH").unwrap());
    let out = engine(&root, &["scan"], &[("PATH", &path), ("PLANROOM_PERSON_TOKEN", &t)]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let seen = std::fs::read_to_string(&gitlog).unwrap();
    assert!(seen.contains("GIT_TERMINAL_PROMPT=0"), "the scan ran git through the recorder");
    assert!(!seen.contains("PLANROOM_PERSON_TOKEN"), "{seen}");
    assert!(!seen.contains(&t));

    // The browser opener gets none of it either.
    let openlog = tmp.path().join("opener.env");
    let opener = recorder(&tmp.path().join("opener"), "opener", &openlog, "true");
    let (code, env) = cli(
        &root,
        &["open"],
        "",
        &[
            ("PLANROOM_PERSON_TOKEN", &t),
            ("PLANROOM_TEST_OPENER", opener.to_str().unwrap()),
            ("DISPLAY", ":0"),
        ],
    );
    assert_eq!((code, &env["result"]["opened"]), (0, &json!(true)), "{env}");
    let seen = std::fs::read_to_string(&openlog).unwrap();
    assert!(!seen.contains("PLANROOM_PERSON_TOKEN") && !seen.contains(&t), "{seen}");

    // Nor does the `serve` that `open` started, which outlives it: once `open` has returned, no
    // process holds the token in its environment. (`/proc` is Linux's; elsewhere the opener's
    // check above is what runs.)
    if cfg!(target_os = "linux") {
        let mut serves = 0;
        for entry in std::fs::read_dir("/proc").unwrap().flatten() {
            let Ok(environ) = std::fs::read(entry.path().join("environ")) else { continue };
            let exe = std::fs::read_link(entry.path().join("exe")).unwrap_or_default();
            if exe.file_name().is_some_and(|n| n.to_string_lossy().starts_with("planroom")) {
                serves += 1;
            }
            let held = environ.windows(t.len()).any(|w| w == t.as_bytes());
            let pid = entry.file_name();
            assert!(!held, "process {} ({}) holds the token", pid.to_string_lossy(), exe.display());
        }
        assert!(serves > 0, "the serve open started is running");
    }
}

#[test]
fn person_required_names_a_path_that_exists() {
    // Each refusal names a way a person runs the command in this build: `/room open` for a leaf
    // `/room` serves, the mod's start for `person init`, and for `import`, which no pane button or
    // `/room` leaf runs yet, the person's own terminal, where it asks them to confirm.
    let a = key(24);
    let (_tmp, root) = registered(&a);
    let agent = [("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")];
    // Every page of the listing, following `meta.next`.
    let mut names: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["schema", "--commands"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c.as_str()]);
        }
        let (_, page) = cli(&root, &args, "", &[]);
        names.extend(
            page["result"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["name"].as_str().unwrap().to_owned()),
        );
        cursor = page["meta"]["next"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    let leaves: Vec<&str> = names.iter().map(String::as_str).collect();
    let room = |name: &str| planroom_contract::Command::SPECS.iter().any(|s| s.name() == name && s.room);

    let (_, env) = cli(&root, &["open"], "", &agent);
    assert_eq!(env["error"]["details"]["command"], "open");
    assert!(leaves.contains(&"open") && room("open"), "{leaves:?}");
    assert_eq!(env["error"]["hint"], "/room open, typed in Claude Code");

    let edit = inbox_line("d1", "edit", json!({}));
    let (_, env) = cli(&root, &["import", "-"], &format!("{edit}\n"), &agent);
    assert!(leaves.contains(&"import") && !room("import"));
    assert_eq!(env["error"]["hint"], "planroom import -, from your own terminal: it asks you to confirm");
    // That path exists, as the hint spells it: at a terminal, the person types the rows, then
    // confirms the import as theirs.
    let line = env["error"]["hint"].as_str().unwrap().split(',').next().unwrap().to_owned();
    let argv: Vec<&str> = line.split(' ').skip(1).collect();
    let (code, env, shown) = at_terminal(&root, &argv, &format!("{edit}\n\x04y\n"), &[]);
    assert_eq!(code, 0, "{env}");
    assert!(shown.contains("run import as"), "{shown:?}");
    assert_eq!(env["result"]["person"], true, "{env}");

    let (_, env) = init(&root, &[], &public(&a), &agent);
    assert!(leaves.contains(&"person init"));
    let hint = env["error"]["hint"].as_str().unwrap();
    assert!(hint.starts_with("claude, started in this workspace"), "{hint}");
    assert!(hint.contains("Use this session's key"), "the mismatch pane's button: {hint}");

    // With no mod key registered, `open` takes the terminal path too, and with mods off it is the
    // only one there is (DR-81): the refusal names it beside `/room open`.
    let (_tmp2, bare) = scanned();
    let _serves = Serves(bare.clone());
    let (_, env) = cli(&bare, &["open"], "", &[]);
    assert_eq!(reason(&env), "missing", "{env}");
    assert_eq!(
        env["error"]["hint"],
        "/room open, typed in Claude Code, or planroom open, from your own terminal: it asks you to confirm"
    );
    let (code, env, _) = at_terminal(&bare, &["open"], "y\n", &[]);
    assert_eq!(code, 0, "{env}");

    // Every leaf, with a mod key registered or not, follows its terminal rule: `/room NAME` only
    // for a leaf `/room` serves; the terminal never where its path is refused; and where it is
    // taken, the exact line, which parses as that leaf, for every leaf that resolves a person.
    const OPERANDS_FROM_THE_CALL: &[&str] =
        &["edit", "review", "intent propose", "intent version", "pack add", "pack remove"];
    for spec in planroom_contract::Command::SPECS {
        let name = spec.name();
        for registered in [false, true] {
            let path = actor::terminal_path(&name, registered);
            let hint = actor::person_path(&name, path);
            let at = format!("{name} (key registered: {registered}): {hint}");
            assert_eq!(hint.starts_with(&format!("/room {name},")), spec.room, "{at}");
            if !spec.room {
                assert!(!hint.contains("/room"), "{at}");
            }
            let line = actor::terminal_line(&name);
            match path {
                TerminalPath::Refused => {
                    assert!(!hint.contains("terminal") && !hint.contains("planroom "), "{at}")
                }
                TerminalPath::Takes => {
                    assert!(
                        hint.contains(&format!("{line}, from your own terminal: it asks you to confirm")),
                        "{at}"
                    );
                    // A leaf no actor rule binds and that writes nothing a person owns never
                    // resolves a person, so never returns this hint.
                    if spec.actor != ActorRule::Any || spec.needs_base {
                        let words: Vec<String> = line.split(' ').skip(1).map(str::to_owned).collect();
                        let leaf = &words[..spec.path.len()];
                        let parsed = planroom::cli::parse_leaf(&leaf.join(" "), &words[spec.path.len()..]);
                        // A leaf whose operands come from the call can't run bare: its refusal
                        // names the caller's own command instead, which edit.rs parses back.
                        if OPERANDS_FROM_THE_CALL.contains(&name.as_str()) {
                            assert!(parsed.is_err(), "{at}: runs bare, so name it bare");
                        } else {
                            assert!(parsed.is_ok_and(|c| c.spec().name() == name), "{at}");
                        }
                    }
                }
            }
        }
    }
}

// --- The terminal path ---

#[test]
fn repo_changing_commands_refuse_the_terminal_path() {
    // `handoff start`, `merge` and `revert` change repos, and `person pair` hands a device the
    // person's authority: anyone who controls a pty could confirm a prompt, so a confirmation
    // never serves for them, with a key registered or not, and nobody is even asked.
    for command in ["handoff start", "handoff merge", "handoff revert", "person pair"] {
        for registered in [false, true] {
            assert_eq!(actor::terminal_path(command, registered), TerminalPath::Refused, "{command}");
        }
        let path: &'static [&'static str] = match command {
            "handoff start" => &["handoff", "start"],
            "handoff merge" => &["handoff", "merge"],
            "handoff revert" => &["handoff", "revert"],
            _ => &["person", "pair"],
        };
        let spec = leaf(path, ActorRule::PersonOnly, Class::Handoff, false);
        let argv: Vec<&str> = path.to_vec();
        let mut yes = Answers { answer: Some(true), asked: 0 };
        let e = actor::writer(&caller(&argv, None, None), &spec, &Bound::default(), &[], now(), &mut yes)
            .unwrap_err();
        assert_eq!(
            (e.code, e.details["reason"].as_str(), yes.asked),
            (ErrorCode::PersonRequired, Some("terminal_refused"), 0)
        );
    }
}

#[test]
fn rekey_and_open_refuse_the_terminal_path() {
    let (a, b) = (key(25), key(26));
    let (_tmp, root) = registered(&a);
    let _serves = Serves(root.clone());
    // The key comes on stdin, which is the terminal here: a line, then end of input.
    let (code, env, shown) =
        at_terminal(&root, &["person", "init", "--rekey"], &format!("{}\n\x04", public(&b)), &[]);
    assert_eq!((code, self::code(&env), reason(&env)), (5, "person_required", "terminal_refused"), "{env}");
    assert!(!shown.contains("as "), "nobody was asked: {shown:?}");
    let (code, env, shown) = at_terminal(&root, &["open"], "y\n", &[]);
    assert_eq!((code, reason(&env)), (5, "terminal_refused"), "{env}");
    assert!(!shown.contains("run open"), "{shown:?}");
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap(), Some(public(&a)));
}

#[test]
fn open_takes_the_terminal_path_only_with_no_mod_key() {
    let (_tmp, root) = scanned();
    let _serves = Serves(root.clone());
    // No mod has registered a key: a confirmation that names the action serves (DR-81).
    let (code, env, shown) = at_terminal(&root, &["open"], "y\n", &[]);
    assert_eq!((code, &env["ok"]), (0, &json!(true)), "{env}");
    assert!(shown.contains("run open as"), "{shown:?}");
    // Once one has, only a press opens.
    let a = key(27);
    let (code, _) = init(&root, &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!(code, 0);
    let (code, env, _) = at_terminal(&root, &["open"], "y\n", &[]);
    assert_eq!((code, reason(&env)), (5, "terminal_refused"), "{env}");
    let (code, env) =
        cli(&root, &["open"], "", &[("PLANROOM_PERSON_TOKEN", &signed(&a, "open", &[], now()))]);
    assert_eq!(code, 0, "{env}");
}

// --- open ---

#[test]
fn open_is_person_only_and_prints_no_code() {
    let a = key(28);
    let (tmp, root) = registered(&a);
    let _serves = Serves(root.clone());
    let (code, env) = cli(&root, &["open"], "", &[("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")]);
    assert_eq!((code, self::code(&env)), (5, "person_required"), "{env}");
    let (code, env) = cli(&root, &["open"], "", &[]);
    assert_eq!((code, reason(&env)), (5, "missing"), "{env}");

    // The press opens the login page through the opener, which keeps a copy of what it was given.
    let copy = tmp.path().join("page.html");
    let opener = tmp.path().join("opener.sh");
    std::fs::write(&opener, format!("#!/bin/sh\ncat \"$1\" > '{}'\n", copy.display())).unwrap();
    std::fs::set_permissions(&opener, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    // Each press its own token, a second apart: two signed at one second over one line would be
    // the same token.
    let first = now();
    for json in [true, false] {
        let t = signed(&a, "open", &[], first - u64::from(!json));
        let press = [
            ("PLANROOM_TEST_OPENER", opener.to_str().unwrap()),
            ("DISPLAY", ":0"),
            ("PLANROOM_PERSON_TOKEN", &t),
        ];
        let mut cmd =
            if json { engine(&root, &["open"], &press) } else { text_mode(&root, &["open"], &press) };
        let out = cmd.stdin(Stdio::null()).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        let page = std::fs::read_to_string(&copy).unwrap();
        let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
        assert_eq!(code.len(), 32, "a 128-bit code");
        let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(!stdout.contains(&code) && !stderr.contains(&code), "{stdout}{stderr}");
        assert!(stdout.contains("Opened the canvas in your browser."), "{stdout}");
        if json {
            let env: Value = serde_json::from_str(stdout.trim()).unwrap();
            assert_eq!(env["result"], json!({"opened": true}));
        }
    }
}
