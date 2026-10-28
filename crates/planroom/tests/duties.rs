//! Rescans that race: whichever order they commit in, and when two processes scan at once, the
//! link snapshot is built from the active set the commit finds, inside its transaction, so the
//! final link is the final active set's (DR-12).
//!
//! Every process runs with this test process's own data root, which `cargo xtask test` sets, and
//! the test reads scan events and the link row rather than counting `seq`s.

use std::fs;
use std::path::Path;
use std::process::Command;

use planroom::app::scan as app_scan;
use planroom_contract::{Edge, Evidence, ScanReq};
use planroom_store::{Shape, Store};
use planroom_testkit::fixture;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Name the jobs list `key` in the orders service's publisher, and commit.
fn publish_to(root: &Path, key: &str) {
    rewrite(root, "orders", "internal/queue/publisher.go", "const JobsKey = \"", key);
}

/// Name the jobs list `key` in the worker's consumer, and commit.
fn consume_from(root: &Path, key: &str) {
    rewrite(root, "worker", "worker/consumer.py", "JOBS_KEY = \"", key);
}

/// Replace the string that follows `prefix` on its line of `repo/path` with `key`, and commit.
fn rewrite(root: &Path, repo: &str, path: &str, prefix: &str, key: &str) {
    let file = root.join(repo).join(path);
    let text = fs::read_to_string(&file).unwrap();
    let lines: Vec<String> = text
        .lines()
        .map(|l| if l.starts_with(prefix) { format!("{prefix}{key}\"") } else { l.to_owned() })
        .collect();
    assert_ne!(lines.join("\n") + "\n", text, "{repo}/{path} names the jobs list");
    fs::write(&file, lines.join("\n") + "\n").unwrap();
    git(&root.join(repo), &["-c", "commit.gpgsign=false", "commit", "-q", "-am", &format!("Jobs on {key}")]);
}

/// The queue edges of the link the last scan event names, as `(kind, from, via)`, after checking
/// that the link is the one of the active set the store holds now and that no test file anchors
/// or cites one. Exactly the two edges a test expects: no other queue edge is linked.
fn final_link(root: &Path) -> Vec<(String, String, String)> {
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let active = store.active_set().unwrap();
    let link = active.link.expect("a link");
    let mut members: Vec<u64> = active.repos.values().copied().collect();
    members.sort_unstable();
    let packs_hash = store.meta("packs_hash").unwrap().unwrap();
    let key = store.snapshot_key(link).unwrap().unwrap();
    assert_eq!(
        key.commit_id,
        planroom_kernel::keys::link_key(&members, &packs_hash),
        "the active set's link"
    );
    let mut queues: Vec<(String, String, String)> = store
        .facts(&[link])
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Edge && r.kind.starts_with("code.queue_"))
        .map(|r| {
            let edge: Edge = serde_json::from_str(&r.body).unwrap();
            let from = serde_json::to_value(&edge.from).unwrap()["id"].as_str().unwrap().to_owned();
            // The worker's tests push onto the jobs list and pop from it: none of that is its
            // runtime wiring, so no test file anchors a queue edge or is cited as its caller.
            for e in &edge.evidence {
                if let Evidence::Line { path, line, .. } = e {
                    assert!(!path.contains("/tests/"), "{} cites the test file {path}:{line}", edge.kind.0);
                }
            }
            (edge.kind.0, from, edge.via.unwrap_or_default())
        })
        .collect();
    queues.sort();
    queues
}

fn both(key: &str) -> Vec<(String, String, String)> {
    vec![
        ("code.queue_consume".into(), "worker:container:worker".into(), key.into()),
        ("code.queue_publish".into(), "orders:container:orders".into(), key.into()),
    ]
}

#[test]
fn concurrent_rescans_link_the_final_active_set() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    app_scan::commit(app_scan::prepare(&ScanReq::default(), &root).unwrap()).unwrap();
    assert_eq!(final_link(&root), both("order-jobs"));

    // Two repos' heads move, each changing a linked edge; each scan reads after one move, and
    // they commit in the order they read.
    publish_to(&root, "jobs-a");
    let first = app_scan::prepare(&ScanReq::default(), &root).unwrap();
    consume_from(&root, "jobs-a");
    let second = app_scan::prepare(&ScanReq::default(), &root).unwrap();
    app_scan::commit(first).unwrap();
    app_scan::commit(second).unwrap();
    assert_eq!(final_link(&root), both("jobs-a"), "read first, committed first");

    // The other order: the scan that read both moves commits first, and the late one, which read
    // the worker before its move, moves nothing back.
    publish_to(&root, "jobs-b");
    let first = app_scan::prepare(&ScanReq::default(), &root).unwrap();
    consume_from(&root, "jobs-b");
    let second = app_scan::prepare(&ScanReq::default(), &root).unwrap();
    app_scan::commit(second).unwrap();
    app_scan::commit(first).unwrap();
    assert_eq!(final_link(&root), both("jobs-b"), "read last, committed first");

    // Two scan processes at once, after both moves.
    publish_to(&root, "jobs-c");
    consume_from(&root, "jobs-c");
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_planroom"))
            .args(["scan", "--json"])
            .current_dir(&root)
            .spawn()
            .unwrap()
    };
    let (mut a, mut b) = (spawn(), spawn());
    assert!(a.wait().unwrap().success());
    assert!(b.wait().unwrap().success());
    assert_eq!(final_link(&root), both("jobs-c"), "two processes at once");
}
