//! `gc` after a scan (architecture.md, Duties and Plan versions; DR-47): the process that scans
//! prunes old snapshots under the `gc` lease, at most two per repo a run, and keeps each repo's
//! last 20, the active set and what a `version` event refers to, while a read that began before a
//! prune still reads everything it began with.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_store::{GC_LEASE, Store};
use serde_json::Value;

/// `planroom ARGS --json` in `root` with the data root `data`, `env` and `stdin`: the exit code
/// and the one envelope.
fn run(root: &Path, data: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", data)
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .args(args)
        .arg("--json");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

fn key() -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = 7;
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

/// A person's token over `command` with the argv after it, as the mod signs one.
fn token(command: &str, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key().sign(&person_token_message(command, &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// `planroom COMMAND ARGS --json` as the person, signed.
fn person(root: &Path, data: &Path, command: &str, args: &[&str], stdin: &str) -> Value {
    let signed = token(command, args);
    let argv: Vec<&str> = command.split(' ').chain(args.iter().copied()).collect();
    let (code, env) = run(root, data, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], stdin);
    assert_eq!(code, 0, "{command}: {env}");
    env
}

fn scan(root: &Path, data: &Path) {
    let (code, env) = run(root, data, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
}

/// Commit a change to `repo`, the `n`th since its first commit.
fn commit(repo: &Path, n: usize) {
    std::fs::write(repo.join("src").join(format!("m{n}.ts")), format!("export const m{n} = {n};\n")).unwrap();
    for args in [&["add", "-A"][..], &["commit", "-q", "-m", &format!("change {n}")]] {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// `repo`'s snapshot IDs, newest first, read fresh.
fn ids(store_path: &Path, repo: &str) -> Vec<u64> {
    let store = Store::open_existing(store_path).unwrap().unwrap();
    store.snapshot().unwrap().snapshot_ids(repo).unwrap()
}

#[test]
fn gc_keeps_what_a_reader_may_need() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, data) = (tmp.path().join("ws"), tmp.path().join("data"));
    let svc = root.join("svc");
    planroom_testkit::git_repo(&svc, &[("src/index.ts".into(), b"export const start = 1;\n".to_vec())]);
    let store_path = root.join(".planroom").join("store.db");

    // Scan 1, then a read that holds the store as scan 1 left it.
    scan(&root, &data);
    let (s1, l1) = (ids(&store_path, "svc")[0], ids(&store_path, "ws")[0]);
    let held_store = Store::open_existing(&store_path).unwrap().unwrap();
    let held = held_store.snapshot().unwrap();
    let s1_facts = held.facts(&[s1]).unwrap();
    assert!(!s1_facts.is_empty());

    // Scan 2, then a version, which refers to scan 2's snapshot and link.
    commit(&svc, 2);
    scan(&root, &data);
    let (s2, l2) = (ids(&store_path, "svc")[0], ids(&store_path, "ws")[0]);
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    person(&root, &data, "person init", &[], &public);
    person(&root, &data, "intent new", &["Add retries to the worker"], "");
    let version = person(&root, &data, "intent version", &["v1"], "");
    assert_eq!(version["result"]["status"], "applied", "{version}");

    // Scans 3 to 23.
    let (mut svc_ids, mut ws_ids) = (vec![s1, s2], vec![l1, l2]);
    for n in 3..=23 {
        commit(&svc, n);
        scan(&root, &data);
        svc_ids.push(ids(&store_path, "svc")[0]);
        ws_ids.push(ids(&store_path, "ws")[0]);
    }

    // Scan 24, while another process holds the `gc` lease: it prunes nothing.
    let mut other = Store::open_existing(&store_path).unwrap().unwrap();
    let lease = other.take_lease(GC_LEASE, r#"{"pid":1}"#).unwrap().expect("the lease is free");
    let before = ids(&store_path, "svc").len();
    commit(&svc, 24);
    scan(&root, &data);
    assert_eq!(ids(&store_path, "svc").len(), before + 1, "a held lease leaves the run to its holder");
    svc_ids.push(ids(&store_path, "svc")[0]);
    ws_ids.push(ids(&store_path, "ws")[0]);
    other.release_lease(&lease).unwrap();

    // Scan 25.
    commit(&svc, 25);
    scan(&root, &data);
    svc_ids.push(ids(&store_path, "svc")[0]);
    ws_ids.push(ids(&store_path, "ws")[0]);

    // Each repo keeps the version's snapshot and its last 20; the first and third to fifth went,
    // at most two a run.
    let kept = |all: &[u64]| -> Vec<u64> {
        let mut kept: Vec<u64> = std::iter::once(all[1]).chain(all[5..].iter().copied()).collect();
        kept.reverse();
        kept
    };
    assert_eq!(ids(&store_path, "svc"), kept(&svc_ids));
    assert_eq!(ids(&store_path, "ws"), kept(&ws_ids));
    assert_eq!(ids(&store_path, "svc").len(), 21);
    let fresh = Store::open_existing(&store_path).unwrap().unwrap();
    let now = fresh.snapshot().unwrap();
    assert!(now.facts(&[s1]).unwrap().is_empty(), "the first snapshot's facts went with it");
    let active = now.active_set().unwrap();
    assert_eq!(active.repos.get("svc"), Some(&svc_ids[24]));
    assert_eq!(active.link, Some(ws_ids[24]));
    assert!(!now.facts(&[svc_ids[24]]).unwrap().is_empty());
    // The read that began before any prune still reads all it began with.
    assert_eq!(held.facts(&[s1]).unwrap(), s1_facts);
}
