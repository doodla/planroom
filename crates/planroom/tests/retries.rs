//! Keys that make retries safe (DR-8, DR-81, DR-86): a write that needs a base derives its key
//! from the request, so a retry replays it; every other write mints a fresh key per invocation,
//! so a repeat runs again. A scan keys each repo's event by the repo, its commit, the extraction
//! inputs and the snapshot it moves away from, so a head that moves records each move, a move back
//! to an earlier head included, and two scans of one head join on one event.

use std::path::Path;
use std::process::Command;

use planroom::app::edit::write_key;
use planroom_contract::{Class, Command as Leaf, EventBody};
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

fn planroom(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args)
        .arg("--json")
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_PERSON_TOKEN");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)))
}

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn scanned() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture(&tmp.path().join("shop")).root;
    assert_eq!(planroom(&root, &["scan"], &[])["ok"], true);
    (tmp, root)
}

/// Each `scan` event of `repo`: its `seq`, key, request hash and commit.
fn scan_events(root: &Path, repo: &str) -> Vec<(u64, String, String, String)> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    store
        .events_after(0, 10_000)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e.body {
            EventBody::Scan { repo: r, commit, .. } if r == repo => {
                Some((e.seq, e.idem, e.request_hash, commit))
            }
            _ => None,
        })
        .collect()
}

/// A scan key's shape: `scan:` and 64 hex characters, optionally `#after:SEQ`.
fn scan_key_shape(idem: &str) -> (String, Option<u64>) {
    let (stem, after) = match idem.split_once("#after:") {
        Some((stem, seq)) => (stem, Some(seq.parse().expect("#after: takes a seq"))),
        None => (idem, None),
    };
    let hash = stem.strip_prefix("scan:").expect("a scan key starts scan:");
    assert!(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()), "{idem}");
    (stem.to_owned(), after)
}

/// The writer a key is derived for.
const WHO: &str = "agent:1a2b3c4d:chat";

#[test]
fn base_less_commands_mint_a_fresh_key() {
    let args = json!({"args": {"op": "rename", "id": "x", "name": "y"}});
    for spec in Leaf::SPECS.iter().filter(|s| s.class != Class::Read) {
        let (first, derived_first) = write_key(spec, None, WHO, &args, Some(9)).unwrap();
        let (second, derived_second) = write_key(spec, None, WHO, &args, Some(9)).unwrap();
        if spec.needs_base {
            // The base pins the intent: the same request from the same view is the same write.
            assert!(derived_first && derived_second, "{}", spec.name());
            assert_eq!(first, second, "{}: a retry derives the same key", spec.name());
            let (other, _) = write_key(spec, None, WHO, &args, Some(10)).unwrap();
            assert_ne!(first, other, "{}: another base is another write", spec.name());
        } else {
            // Nothing pins the intent, so each invocation is its own write.
            assert!(!derived_first && !derived_second, "{}", spec.name());
            assert_ne!(first, second, "{}: a repeat mints a fresh key", spec.name());
            for key in [&first, &second] {
                let id = uuid::Uuid::parse_str(key)
                    .unwrap_or_else(|_| panic!("{}: {key} is no uuid", spec.name()));
                assert_eq!(id.get_version_num(), 7, "{}: {key}", spec.name());
            }
        }
        // A caller's own key always wins, and may not hold the `#` a command's events are keyed by.
        assert_eq!(write_key(spec, Some("k-1"), "a", &args, Some(9)).unwrap(), ("k-1".to_owned(), false));
        assert!(write_key(spec, Some("k#2"), "a", &args, Some(9)).is_err(), "{}", spec.name());
    }

    // Through the CLI: the same edit from the same base, with no key, replays the first; with a
    // new key it is a new write.
    let (_tmp, root) = scanned();
    let base = planroom(&root, &["map"], &[])["meta"]["seq"].as_u64().unwrap().to_string();
    let agent = [("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")];
    let op = ["edit", "--base", &base, "rename", "orders:container:orders", "orders-api"];
    let first = planroom(&root, &op, &agent);
    assert_eq!(first["ok"], true, "{first}");
    let retry = planroom(&root, &op, &agent);
    assert_eq!(retry["meta"]["replayed"], true, "{retry}");
    assert_eq!(retry["meta"]["replayed_from"], first["meta"]["seq"], "{retry}");
    assert_eq!(retry["result"], first["result"], "the retry returns the stored reply whole");
    let keyed =
        ["edit", "--base", &base, "--idem", "second-try", "rename", "orders:container:orders", "orders-api"];
    let again = planroom(&root, &keyed, &agent);
    assert_eq!(again["meta"].get("replayed"), None, "{again}");
    assert!(again["meta"]["seq"].as_u64() > first["meta"]["seq"].as_u64(), "{again}");
}

#[test]
fn a_rescan_after_a_head_move_writes_a_new_scan_event() {
    let (_tmp, root) = scanned();
    let worker = root.join("worker");
    let before = scan_events(&root, "worker");
    assert_eq!(before.len(), 1);
    let (first_key, after) = scan_key_shape(&before[0].1);
    assert_eq!(after, None, "a first scan's key has no #after");
    assert_eq!(
        first_key,
        format!("scan:{}", before[0].2),
        "the key and its request hash cover the same inputs"
    );

    // Nothing moved: a rescan joins the event it already wrote.
    let seq = planroom(&root, &["scan"], &[])["meta"]["seq"].clone();
    assert_eq!(scan_events(&root, "worker"), before);

    git(&worker, &["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "Move the head"]);
    let head = git(&worker, &["rev-parse", "HEAD"]);
    let moved = planroom(&root, &["scan"], &[]);
    assert!(moved["meta"]["seq"].as_u64() > seq.as_u64(), "{moved}");
    let after_move = scan_events(&root, "worker");
    assert_eq!(after_move.len(), 2, "the moved head records its own scan event");
    let (seq_b, key_b, _, commit_b) = &after_move[1];
    assert_eq!(commit_b, &head);
    assert_eq!(moved["meta"]["seq"].as_u64(), Some(*seq_b));
    assert_ne!(key_b, &before[0].1, "a new head is a new key");
    assert_eq!(scan_key_shape(key_b).1, None);
    // Every other repo stayed where it was: no event for them.
    for repo in ["kit", "orders", "platform", "storefront"] {
        assert_eq!(scan_events(&root, repo).len(), 1, "{repo}");
    }
}

#[test]
fn a_move_back_to_an_earlier_head_records_a_new_scan() {
    let (_tmp, root) = scanned();
    let worker = root.join("worker");
    let a = git(&worker, &["rev-parse", "HEAD"]);
    git(&worker, &["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "B"]);
    let b = git(&worker, &["rev-parse", "HEAD"]);
    planroom(&root, &["scan"], &[]);
    // A -> B -> A: the move back reads a snapshot the store already holds, and still records it.
    git(&worker, &["reset", "-q", "--hard", &a]);
    planroom(&root, &["scan"], &[]);
    // -> B again: the same commit and the same previous snapshot as the first move to B, so the
    // same key; the scan keys it after the event it moves away from.
    git(&worker, &["reset", "-q", "--hard", &b]);
    let last = planroom(&root, &["scan"], &[]);
    assert_eq!(last["ok"], true, "{last}");

    let events = scan_events(&root, "worker");
    let commits: Vec<&str> = events.iter().map(|e| e.3.as_str()).collect();
    assert_eq!(commits, [a.as_str(), b.as_str(), a.as_str(), b.as_str()], "each move records its own event");
    let keys: Vec<(String, Option<u64>)> = events.iter().map(|e| scan_key_shape(&e.1)).collect();
    assert_eq!(keys[3].0, keys[1].0, "the second move to B meets the first one's key");
    assert_eq!(keys[3].1, Some(events[2].0), "and is keyed #after: the seq of the scan it moves away from");
    assert_eq!(keys[1].1, None);
    assert_ne!(keys[2].0, keys[0].0, "back on A after B is another key than A's first scan");
    // Every key is distinct, so no move replays another.
    let mut unique: Vec<&String> = events.iter().map(|e| &e.1).collect();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4);

    // The map follows the last move: the repo's active snapshot is B's.
    let again = planroom(&root, &["scan"], &[]);
    assert_eq!(again["meta"]["seq"], last["meta"]["seq"], "a rescan of B joins B's event");
}
