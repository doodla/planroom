//! `planroom watch --json [--since SEQ]` as the mod runs it (contracts.md, HTTP API and live
//! stream; architecture.md, Live within 1 s): a `ping` first, then the replay after `--since`,
//! then each commit; one `reset` for a client the store can't catch up; and an exit with
//! `schema_mismatch` when a newer engine migrates the store under it (DR-78).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use planroom_contract::{Actor, Client, EventBody, EventId, Status, StreamFrame, Surface, Timestamp};
use planroom_store::{Init, NewEvent, STORE_VERSION, Store, Write};
use serde_json::Value;

/// A workspace whose store holds `events` version events, and nothing else.
fn workspace(dir: &Path, events: usize) -> PathBuf {
    let root = dir.join("shop");
    std::fs::create_dir_all(root.join(".planroom")).unwrap();
    let init = Init {
        workspace_id: "watch-test".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    };
    let mut store = Store::open(&root.join(".planroom/store.db"), &init).unwrap();
    for i in 0..events {
        commit(&mut store, i);
    }
    root
}

fn commit(store: &mut Store, i: usize) -> u64 {
    let write = Write {
        idem: format!("v-{i}"),
        request_hash: "h".into(),
        events: vec![NewEvent {
            id: EventId(format!("v-{i}")),
            at: Timestamp("2026-10-26T00:00:00Z".into()),
            actor: Actor::System { part: "test".into() },
            client: Client { surface: Surface::Test, id: "test".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: format!("v{i}") },
        }],
    };
    store.commit(write, |_| Value::Null).unwrap().seqs[0]
}

/// `watch --json` and a channel of its stdout lines, each with when it arrived.
fn watch(root: &Path, args: &[&str]) -> (Child, mpsc::Receiver<(String, Instant)>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["watch", "--json"])
        .args(args)
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out: ChildStdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            if tx.send((line, Instant::now())).is_err() {
                break;
            }
        }
    });
    (child, rx)
}

fn frame(rx: &mpsc::Receiver<(String, Instant)>) -> StreamFrame {
    let (line, _) = rx.recv_timeout(Duration::from_secs(10)).expect("a frame within 10 s");
    serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line}: {e}"))
}

fn seq(frame: &StreamFrame) -> Option<u64> {
    match frame {
        StreamFrame::Event { seq, .. } | StreamFrame::Reset { seq } => Some(*seq),
        StreamFrame::Ping => None,
    }
}

#[test]
fn watch_replays_after_since_then_streams_each_new_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = workspace(dir.path(), 4);
    let (mut child, rx) = watch(&root, &["--since", "2", "--session", "s1"]);
    assert_eq!(frame(&rx), StreamFrame::Ping, "the stream opens with a ping");
    assert_eq!(seq(&frame(&rx)), Some(3), "the replay starts after --since");
    assert_eq!(seq(&frame(&rx)), Some(4));

    let mut store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let committed = Instant::now();
    let next = commit(&mut store, 4);
    let got = frame(&rx);
    assert_eq!(seq(&got), Some(next));
    assert!(committed.elapsed() < Duration::from_secs(1), "a commit reaches the stream within 1 s");
    let StreamFrame::Event { kind, actor, status, summary, .. } = got else { unreachable!() };
    assert_eq!(kind.as_str(), "version");
    assert_eq!(actor, Actor::System { part: "test".into() });
    assert_eq!(status, Status::Applied);
    assert!(!summary.is_empty() && summary.len() <= 2048);
    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn a_client_ahead_of_the_store_gets_one_reset_at_the_stores_seq() {
    let dir = tempfile::tempdir().unwrap();
    let root = workspace(dir.path(), 2);
    let (mut child, rx) = watch(&root, &["--since", "40"]);
    assert_eq!(frame(&rx), StreamFrame::Ping);
    assert_eq!(frame(&rx), StreamFrame::Reset { seq: 2 });
    let mut store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    commit(&mut store, 2);
    assert_eq!(seq(&frame(&rx)), Some(3), "after the reset, the stream goes on from the store's seq");
    child.kill().unwrap();
    let _ = child.wait();
}

#[test]
fn watch_outside_a_store_prints_one_workspace_not_found_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let (mut child, rx) = watch(dir.path(), &[]);
    let (line, _) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "workspace_not_found");
    assert_eq!(child.wait().unwrap().code(), Some(3));
}

/// A newer engine's migration, as far as an older reader can tell: `user_version` moves past
/// this build's, in one commit, through a connection of another process.
fn migrate_past_this_build(store: &Path) {
    let script = format!(
        "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); \
         c.execute(\"UPDATE meta SET value='9.9.9' WHERE key='engine_version'\"); \
         c.execute('PRAGMA user_version = {}'); c.commit()",
        STORE_VERSION + 1
    );
    let out = Command::new("python3").args(["-c", &script]).arg(store).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn an_older_watch_exits_with_schema_mismatch_within_one_poll_of_a_migration() {
    let dir = tempfile::tempdir().unwrap();
    let root = workspace(dir.path(), 1);
    let (mut child, rx) = watch(&root, &[]);
    assert_eq!(frame(&rx), StreamFrame::Ping);

    migrate_past_this_build(&root.join(".planroom/store.db"));
    // The migration has committed by the time its process exits.
    let migrated = Instant::now();
    let (line, at) = rx.recv_timeout(Duration::from_secs(5)).expect("watch reports the mismatch");
    let status = child.wait().unwrap();
    let took = at.saturating_duration_since(migrated);
    eprintln!("schema_mismatch after {} ms", took.as_millis());

    let env: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(env["ok"], false, "{env}");
    assert_eq!(env["command"], "watch");
    assert_eq!(env["error"]["code"], "schema_mismatch");
    assert_eq!(env["error"]["details"]["found"], STORE_VERSION + 1);
    assert_eq!(env["error"]["details"]["supported"], STORE_VERSION);
    assert_eq!(env["error"]["details"]["writer_version"], "9.9.9");
    assert_eq!(status.code(), Some(7));
    // One 200 ms poll, plus the error and the exit; the slack is for a loaded CI host.
    assert!(took < Duration::from_millis(400), "watch took {} ms to exit", took.as_millis());
}

#[test]
fn watch_refuses_a_store_newer_than_this_engine_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let root = workspace(dir.path(), 1);
    migrate_past_this_build(&root.join(".planroom/store.db"));
    let (mut child, rx) = watch(&root, &[]);
    let (line, _) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(env["error"]["code"], "schema_mismatch", "{env}");
    assert_eq!(child.wait().unwrap().code(), Some(7));
}
