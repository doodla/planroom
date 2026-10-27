//! S1, first look: a cold scan of the fixture workspace, and the map it gives.
//!
//! A test here that asserts a wall-clock budget is listed in `crates/xtask/nextest.toml`, so
//! nextest runs it with no other test beside it, and takes [`SERIAL`], so `cargo test`, which runs
//! this file's tests as threads of one process, never times it beside another.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use planroom_testkit::fixture;
use serde_json::Value;

/// Held by every test in this file, so a timed one runs alone under `cargo test` too.
static SERIAL: Mutex<()> = Mutex::new(());

/// `planroom ARGS --json` in `cwd` with the data root `data`: the one envelope, which is `ok`.
fn planroom(cwd: &Path, data: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", data)
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap();
    assert!(out.status.success() && env["ok"] == true, "{args:?}: {env}");
    env
}

/// The first scan of a fresh copy of the fixture, with no store and an empty data root, takes
/// under 5 s from the command's start to its reply, and maps the five repos.
#[test]
fn cold_scan_under_5_s() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    let data = tmp.path().join("data");
    let fx = fixture(&root);
    let dir = fx.root.display().to_string();
    let started = Instant::now();
    let env = planroom(&fx.root, &data, &["scan", &dir]);
    let took = started.elapsed();
    assert!(took < Duration::from_secs(5), "the cold scan took {took:?}");
    let nodes = env["result"]["map"]["nodes"].as_array().unwrap();
    let repos = nodes.iter().filter(|n| n["level"] == "system").count();
    assert_eq!(repos, 5, "the five repos at the top: {}", env["result"]["map"]);
}
