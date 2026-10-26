//! The git adapter runs nothing a scanned repo names: no lazy fetch from a partial clone's
//! promisor remote, so no `uploadpack` or `sshCommand` from its local config (correctness-15).

use std::path::Path;
use std::process::{Command, Output};

use planroom_testkit::git_repo;
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs")
}

fn scan(root: &Path) -> Value {
    let out = planroom(root, &["scan", ".", "--json"]);
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success() && env["ok"] == true, "{env}");
    env
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn code_ids(root: &Path) -> Vec<String> {
    let out = planroom(root, &["map", "--level", "code", "--json"]);
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut ids: Vec<String> = env["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids
}

/// A blob-less sparse clone of a three-directory repo, as `ws/orders`, holding only the
/// top-level file's blob.
fn partial_clone(dir: &Path) -> std::path::PathBuf {
    let src = dir.join("src");
    let files: Vec<(String, Vec<u8>)> = ["a/f1.txt", "a/f2.txt", "b/g1.txt", "top.txt"]
        .iter()
        .map(|p| (p.to_string(), format!("{p}\n").into_bytes()))
        .collect();
    git_repo(&src, &files);
    git(&src, &["config", "uploadpack.allowFilter", "true"]);
    let ws = dir.join("ws");
    std::fs::create_dir_all(ws.join(".planroom")).unwrap();
    let url = format!("file://{}", src.display());
    git(&ws, &["clone", "-q", "--filter=blob:none", "--sparse", &url, "orders"]);
    ws
}

#[test]
fn a_scan_of_a_partial_clone_never_fetches_and_reports_the_missing_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    let mark = dir.path().join("ran");
    let planted = format!("sh -c 'echo ran >> {}'; git-upload-pack", mark.display());
    git(&ws.join("orders"), &["config", "remote.origin.uploadpack", &planted]);

    let env = scan(&ws);
    assert!(!mark.exists(), "the repo's upload-pack command ran");
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    let partial = warnings.iter().find(|w| w["code"] == "partial_scan").expect("a partial_scan warning");
    assert_eq!(partial["details"]["repo"], "orders");
    assert_eq!(partial["details"]["uncovered"], serde_json::json!(["a/f1.txt", "a/f2.txt", "b/g1.txt"]));
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
    // Nothing was fetched into the clone either.
    let out = Command::new("git")
        .args(["rev-list", "--objects", "--missing=print", "--no-walk", "HEAD"])
        .current_dir(ws.join("orders"))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.starts_with('?')).count(), 3);
}

#[test]
fn a_partial_clone_that_gains_blobs_maps_them_at_the_same_commit() {
    // Which blobs a clone holds is an input of the walk, so it's part of the snapshot's key.
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    scan(&ws);
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
    // The person's own git widens the cone, which fetches the blobs.
    git(&ws.join("orders"), &["sparse-checkout", "add", "a"]);
    let env = scan(&ws);
    let partial =
        env["meta"]["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "partial_scan").cloned();
    assert_eq!(partial.unwrap()["details"]["uncovered"], serde_json::json!(["b/g1.txt"]));
    assert_eq!(code_ids(&ws), ["orders:file:a/f1.txt", "orders:file:a/f2.txt", "orders:file:top.txt"]);
}
