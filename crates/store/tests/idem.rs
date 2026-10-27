//! Idempotency (DR-8): a writer killed after its commit, before it could reply, loses nothing; its
//! retry with the same key returns the stored reply and writes nothing more. A command that wrote
//! several events, keyed `KEY#1`, `KEY#2` and so on, replays as the whole command.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use planroom_contract::{Actor, Client, EventBody, EventId, Status, Surface, Timestamp};
use planroom_store::{Init, NewEvent, Store, StoreError, Write};
use serde_json::json;

fn event(id: &str, label: &str) -> NewEvent {
    NewEvent {
        id: EventId(id.into()),
        at: Timestamp("2026-10-26T00:00:00Z".into()),
        actor: Actor::Person { id: "person:ann".into(), name: None },
        client: Client { surface: Surface::Test, id: "tab_1".into() },
        project: None,
        cause: None,
        status: Status::Applied,
        target: None,
        body: EventBody::Version { project: None, label: label.into() },
    }
}

#[test]
fn multi_event_replay_returns_the_whole_reply() {
    let dir = tempfile::tempdir().unwrap();
    let init = Init {
        workspace_id: "idem".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    };
    let mut store = Store::open(&dir.path().join("store.db"), &init).unwrap();
    // A regroup: one command, three events, one reply naming all three.
    let regroup = || Write {
        idem: "regroup".into(),
        request_hash: "h-regroup".into(),
        events: vec![event("ev-put", "put"), event("ev-move-1", "move"), event("ev-move-2", "move")],
    };
    let first = store.commit(regroup(), |seqs| json!({"events": seqs, "line": "Grouped 2 blocks"})).unwrap();
    assert_eq!(first.seqs, vec![1, 2, 3]);
    // Another write whose key merely starts with this one's, as a scan's `KEY#after:SEQ` does, and
    // a key one character longer, are other commands.
    for (idem, id) in [("regroup#after:3", "ev-scan"), ("regroup2", "ev-other")] {
        let other = Write { idem: idem.into(), request_hash: "h-other".into(), events: vec![event(id, "x")] };
        store.commit(other, |_| json!({})).unwrap();
    }

    let replay = store.commit(regroup(), |_| panic!("a replay builds no new reply")).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.seqs, vec![1, 2, 3], "the replay names every event of the command, and only those");
    assert_eq!(replay.reply, json!({"events": [1, 2, 3], "line": "Grouped 2 blocks"}));
    assert_eq!(store.seq().unwrap(), 5, "the replay wrote nothing");

    // The same key with another request is refused, naming the command's first event.
    let other = Write { request_hash: "h-changed".into(), ..regroup() };
    let refused = store.commit(other, |_| json!(null)).unwrap_err();
    assert!(
        matches!(&refused, StoreError::IdempotencyMismatch { stored_event, stored_seq: 1, .. } if stored_event == "ev-put"),
        "{refused:?}"
    );
}

fn write(request_hash: &str) -> Write {
    Write {
        idem: "kill-after-commit".into(),
        request_hash: request_hash.into(),
        events: vec![NewEvent {
            id: EventId("ev-kill".into()),
            at: Timestamp("2026-10-26T00:00:00Z".into()),
            actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
            client: Client { surface: Surface::Test, id: "agent:1a2b3c4d".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: "v1".into() },
        }],
    }
}

/// The writer: commit, say so, then hang until killed, so it never replies.
#[test]
fn committer_child() {
    let Ok(path) = std::env::var("WS4_KILL_STORE") else { return };
    let mut store = Store::open_existing(Path::new(&path)).unwrap().unwrap();
    let done = store.commit(write("h1"), |seqs| json!({"seq": seqs[0], "label": "v1"})).unwrap();
    println!("COMMITTED {}", done.seqs[0]);
    std::thread::sleep(Duration::from_secs(120));
}

#[test]
fn a_retry_after_a_kill_after_commit_returns_the_stored_reply() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let init = Init {
        workspace_id: "ws4".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    };
    drop(Store::open(&path, &init).unwrap());

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "committer_child", "--nocapture", "--test-threads", "1"])
        .env("WS4_KILL_STORE", &path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let committed: u64 = loop {
        let line = lines.next().expect("the child says it committed").unwrap();
        if let Some(seq) = line.find("COMMITTED ").map(|at| &line[at + 10..]) {
            break seq.parse().unwrap();
        }
    };
    child.kill().unwrap(); // SIGKILL: no destructor, no reply
    assert!(!child.wait().unwrap().success());

    let mut store = Store::open_existing(&path).unwrap().unwrap();
    let retry = store.commit(write("h1"), |_| panic!("a replay builds no new reply")).unwrap();
    assert!(retry.replayed);
    assert_eq!(retry.seqs, vec![committed]);
    assert_eq!(retry.reply, json!({"seq": committed, "label": "v1"}));
    assert_eq!(store.seq().unwrap(), committed, "the retry wrote nothing");

    let other = store.commit(write("h2"), |_| json!(null)).unwrap_err();
    assert!(matches!(other, StoreError::IdempotencyMismatch { stored_seq, .. } if stored_seq == committed));
}
