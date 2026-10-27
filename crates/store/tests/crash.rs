//! A writer killed mid-commit (DR-1): with WAL and `synchronous = FULL`, a commit is all or
//! nothing, so a process killed inside its transaction leaves no part of it, and every commit it
//! finished stays. The store reopens whole, passes `integrity_check`, and takes the next commit.
//!
//! The writer is this test binary run again with `CRASH_WRITER_STORE` set, as `concurrency.rs`
//! runs its writers. It reports each finished commit, and then, inside one more commit's
//! transaction, after its events are inserted and before it commits, says so and waits, so the
//! kill lands mid-commit every time rather than when it happens to.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use planroom_contract::{Actor, Client, EventBody, EventId, Status, Surface, Timestamp};
use planroom_store::{Init, NewEvent, Store, Write};
use serde_json::json;

/// The commits the writer finishes before the one it is killed in.
const FINISHED: usize = 40;

fn init() -> Init {
    Init {
        workspace_id: "ws-crash".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

fn event(n: usize, part: usize) -> NewEvent {
    NewEvent {
        id: EventId(format!("e{n}-{part}")),
        at: Timestamp("2026-10-26T00:00:00Z".into()),
        actor: Actor::System { part: "crash".into() },
        client: Client { surface: Surface::Test, id: "crash-writer".into() },
        project: None,
        cause: None,
        status: Status::Applied,
        target: None,
        // A body large enough that each commit spans several WAL pages.
        body: EventBody::Version { project: None, label: format!("{n}-{part}-{}", "x".repeat(6_000)) },
    }
}

/// Each write is three events, so a half-written one would show as a count not divisible by 3.
fn write(n: usize) -> Write {
    Write { idem: format!("w{n}"), request_hash: "h".into(), events: (0..3).map(|p| event(n, p)).collect() }
}

/// The writer process: `FINISHED` commits, each reported once it returns, then one more that
/// reports `INSIDE` from within its transaction and never returns.
#[test]
fn crash_writer_child() {
    let Ok(path) = std::env::var("CRASH_WRITER_STORE") else { return };
    let mut store = Store::open_existing(Path::new(&path)).unwrap().unwrap();
    for n in 0..FINISHED {
        let done = store.commit(write(n), |seqs| json!({"seqs": seqs})).unwrap();
        println!("CRASH committed {}", done.seqs[2]);
    }
    let _ = store.commit(write(FINISHED), |_| {
        // Inside the IMMEDIATE transaction: the three events are inserted, nothing is committed.
        println!("CRASH INSIDE");
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });
    unreachable!("the parent kills this process inside its last commit");
}

#[test]
fn a_kill_mid_commit_then_reopen_keeps_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    drop(Store::open(&path, &init()).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_writer_child", "--nocapture", "--test-threads", "1"])
        .env("CRASH_WRITER_STORE", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut committed = 0;
    let mut inside = false;
    for line in BufReader::new(child.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        // libtest may print "test crash_writer_child ... " ahead of the first line.
        if line.contains("CRASH committed") {
            committed += 1;
        } else if line.contains("CRASH INSIDE") {
            inside = true;
            // SIGKILL: no destructor, no rollback, no close; the transaction is simply cut off.
            child.kill().unwrap();
            break;
        }
    }
    let status = child.wait().unwrap();
    assert!(inside, "the writer reached its last commit's transaction");
    assert_eq!(committed, FINISHED, "every finished commit was reported before the kill");
    assert!(!status.success(), "the writer was killed, not finished: {status:?}");

    // The killed process left its WAL behind; reopening recovers from it.
    let mut store = Store::open_existing(&path).unwrap().expect("the store is still there");
    let events = store.events_after(0, 10_000).unwrap();
    assert_eq!(events.len(), FINISHED * 3, "every finished commit stays, and none of the killed one");
    let seqs: Vec<u64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, (1..=(FINISHED * 3) as u64).collect::<Vec<_>>(), "no seq lost or repeated");
    assert!(
        events.iter().all(|e| !e.id.0.starts_with(&format!("e{FINISHED}-"))),
        "no event of the killed commit"
    );

    let conn = rusqlite::Connection::open(&path).unwrap();
    let verdict: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(verdict, "ok");
    drop(conn);

    // The store takes the next commit, and a retry of the killed one writes it whole.
    let next = store.commit(write(FINISHED), |seqs| json!({"seqs": seqs})).unwrap();
    assert!(!next.replayed, "the killed commit left no key behind to replay");
    let first = (FINISHED * 3) as u64 + 1;
    assert_eq!(next.seqs, vec![first, first + 1, first + 2]);
}
