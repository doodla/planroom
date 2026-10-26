//! `focus --json`: the `PaneModel` from the store, and the not-yet-a-workspace empty state that
//! creates nothing (DR-57).

use std::path::Path;
use std::process::Command;

use planroom_contract::{Actor, PresenceRow, Surface, Timestamp};
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        1,
        "one JSON line: {stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env: Value = serde_json::from_str(&stdout).unwrap();
    assert!(out.status.success(), "{env}");
    env
}

#[test]
fn focus_outside_a_workspace_is_an_empty_state_and_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let env = planroom(dir.path(), &["focus", "--json"]);
    assert_eq!(env["ok"], true);
    let pane = &env["result"];
    assert_eq!(pane["empty"]["reason"], "no_repos");
    assert_eq!(pane["next"], pane["empty"]["next"]);
    assert_eq!(pane["progress"]["scan"], "never");
    assert!(!dir.path().join(".planroom").exists(), "focus created a state directory");
}

#[test]
fn focus_shows_the_scanned_workspace_and_who_is_present() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);

    let before = planroom(&root, &["focus", "--json"]);
    assert_eq!(before["result"]["empty"]["reason"], "not_scanned");
    assert_eq!(before["result"]["next"]["command"], "scan");
    assert!(!root.join(".planroom/store.db").exists(), "focus created a store");

    planroom(&root, &["scan", "--json"]);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let seen = |at: &str, doing: &str| PresenceRow {
        actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
        surface: Surface::Mod,
        doing: Some(doing.into()),
        focus: None,
        seen_at: Timestamp(at.into()),
    };
    store.touch_presence(&seen("2000-01-01T00:00:00.000000Z", "long gone"), "mod:old").unwrap();
    let now = planroom::clock::now();
    store.touch_presence(&seen(&now.0, "planning retries"), "mod:s1").unwrap();

    let env = planroom(&root, &["focus", "--json"]);
    let pane = &env["result"];
    assert!(pane["empty"].is_null(), "{pane}");
    assert_eq!(pane["focus"]["breadcrumb"][0]["name"], "shop");
    assert_eq!(pane["focus"]["seq"], env["meta"]["seq"]);
    assert_eq!(pane["progress"]["scan"], "done");
    assert_eq!(pane["next"]["command"], "map");
    assert!(pane["decision"].is_null());
    let who = pane["who"].as_array().unwrap();
    assert_eq!(who.len(), 1, "only rows seen in the last 30 s: {who:?}");
    assert_eq!(who[0]["doing"], "planning retries");

    // The mini-map rows are the map's top-level blocks.
    let map = planroom(&root, &["map", "--json"]);
    let top: Vec<&Value> = map["result"]["nodes"].as_array().unwrap().iter().map(|n| &n["id"]).collect();
    let blocks: Vec<&Value> =
        pane["minimap"]["blocks"].as_array().unwrap().iter().map(|b| &b["id"]).collect();
    assert_eq!(blocks, top);
    assert!(blocks.iter().any(|b| *b == "orders:repo:orders"));

    let bytes = serde_json::to_string(pane).unwrap().len();
    assert!(bytes <= 4_096, "the pane is {bytes} bytes");
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 1_000);
}
