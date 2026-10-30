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
    let seen = |at: &str, doing: &str, client: &str| PresenceRow {
        actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
        surface: Surface::Mod,
        client_id: client.into(),
        doing: Some(doing.into()),
        focus: None,
        seen_at: Timestamp(at.into()),
    };
    store.touch_presence(&seen("2000-01-01T00:00:00.000000Z", "long gone", "mod:old")).unwrap();
    let now = planroom::clock::now();
    store.touch_presence(&seen(&now.0, "planning retries", "mod:s1")).unwrap();

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

#[test]
fn focus_leaves_out_its_own_session() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    planroom(&root, &["scan", "--json"]);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let now = planroom::clock::now();
    for client in ["mod:s1", "mod:s2"] {
        let row = PresenceRow {
            actor: Actor::Person { id: "ann".into(), name: None },
            surface: Surface::Mod,
            client_id: client.into(),
            doing: None,
            focus: None,
            seen_at: now.clone(),
        };
        store.touch_presence(&row).unwrap();
    }
    let who = |args: &[&str]| -> Vec<Value> {
        let mut ids: Vec<Value> = planroom(&root, args)["result"]["who"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["client_id"].clone())
            .collect();
        ids.sort_by_key(Value::to_string);
        ids
    };
    // The mod's session sees everyone else; the CLI, which has no session, sees every row.
    assert_eq!(who(&["focus", "--session", "s1", "--json"]), ["mod:s2"]);
    assert_eq!(who(&["focus", "--json"]), ["mod:s1", "mod:s2"]);
}

/// `planroom ARGS` in text mode: what it prints on stdout.
fn text(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// The envelope without what no two runs repeat: `meta.took_ms`, and `meta.size`, which measures
/// the line.
fn steady(mut env: Value) -> Value {
    let meta = env["meta"].as_object_mut().unwrap();
    meta.remove("took_ms");
    meta.remove("size");
    env
}

#[test]
fn focus_detail_summary_drops_the_minimap_and_keeps_the_rest() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    planroom(&root, &["scan", "--json"]);

    // The summary is the full reply less its mini-map.
    let mut full = steady(planroom(&root, &["focus", "--json"]));
    let summary = steady(planroom(&root, &["focus", "--detail", "summary", "--json"]));
    assert!(full["result"]["minimap"]["blocks"].as_array().is_some_and(|b| !b.is_empty()), "{full}");
    assert!(summary["result"].get("minimap").is_none(), "{summary}");
    full["result"].as_object_mut().unwrap().remove("minimap");
    assert_eq!(summary, full);
    let named = planroom(&root, &["focus", "--detail", "full", "--json"]);
    assert_eq!(named["result"]["minimap"]["level"], "system", "{named}");

    // Not yet a workspace: no mini-map either.
    let empty = tempfile::tempdir().unwrap();
    let outside = planroom(empty.path(), &["focus", "--detail", "summary", "--json"]);
    assert_eq!(outside["result"]["empty"]["reason"], "no_repos", "{outside}");
    assert!(outside["result"].get("minimap").is_none(), "{outside}");

    // Text mode still names the blocks, at either detail.
    let lines = text(&root, &["focus", "--detail", "summary"]);
    assert!(lines.lines().any(|l| l.contains("at the top:") && l.contains("orders")), "{lines}");
    assert_eq!(lines, text(&root, &["focus"]));
}

// ---------------------------------------------------------------------------------------------
// `focus --picker`

const AGENT: [(&str, &str); 1] = [("PLANROOM_ACTOR", "agent:5e55i0n1:chat")];

/// `planroom ARGS` with `env` added, its one JSON line parsed; the run may fail.
fn planroom_with(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .env_remove("PLANROOM_RAW")
        .envs(env.iter().copied())
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)))
}

/// A scanned fixture workspace.
fn scanned() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    planroom(&root, &["scan", "--json"]);
    (tmp, root)
}

/// An agent starts the plan project `name`, which waits as a ghost.
fn new_project(root: &Path, name: &str) {
    let env = planroom_with(root, &["intent", "new", name, "--json"], &AGENT);
    assert_eq!(env["ok"], true, "{env}");
}

fn picker(root: &Path, which: &str, more: &[&str]) -> Value {
    let mut args = vec!["focus", "--picker", which];
    args.extend_from_slice(more);
    args.push("--json");
    let env = planroom_with(root, &args, &[]);
    assert_eq!(env["ok"], true, "{env}");
    env
}

#[test]
fn the_lens_picker_lists_every_lens_with_the_current_marked() {
    let (_tmp, root) = scanned();
    let env = picker(&root, "lens", &[]);
    let result = &env["result"];
    assert_eq!(result["name"], "lens");
    assert!(result.get("estimate").is_none(), "a lens costs nothing: {result}");
    let entries = result["entries"].as_array().unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e["handle"]["args"][1].as_str().unwrap()).collect();
    assert_eq!(ids.last(), Some(&"all"), "{ids:?}");
    assert!(ids.len() > 2, "every loaded lens, then all: {ids:?}");
    for entry in entries {
        assert_eq!(entry["handle"]["command"], "map");
        assert_eq!(entry["handle"]["args"][0], "--lens");
    }
    // The pane's lens is marked, first, and only it.
    let pane = planroom(&root, &["focus", "--json"]);
    let lens = pane["result"]["focus"]["lens"].as_str().unwrap();
    let current: Vec<&Value> = entries.iter().filter(|e| e["current"] == true).collect();
    assert_eq!(current.len(), 1, "{entries:?}");
    assert_eq!(current[0]["handle"]["args"][1], lens);
    assert_eq!(entries[0]["handle"]["args"][1], lens, "the default lens comes first");
    assert!(entries[0]["detail"].as_str().unwrap().ends_with(" level"), "{entries:?}");
    assert_eq!(result["more"], 0);
    assert_eq!(env["meta"]["seq"], pane["meta"]["seq"]);
}

#[test]
fn the_stage_picker_is_the_stage_runs_cost_card() {
    let (_tmp, root) = scanned();
    new_project(&root, "Add retries to the worker");
    let env = picker(&root, "stage", &[]);
    let result = &env["result"];
    assert_eq!(result["name"], "stage");
    let question = result["question"].as_str().unwrap();
    assert!(question.contains("8,300") && question.contains("$0.10"), "{question}");
    assert_eq!(result["estimate"], serde_json::json!({"tokens": 8300, "usd": 0.1, "cached": true}));
    let entries = result["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 7, "{entries:?}");
    // The plan lacks every stage, so the first is the next, and it is marked.
    assert_eq!(entries[0]["label"], "Intent");
    assert_eq!(entries[0]["handle"]["command"], "intent next");
    assert_eq!(entries[0]["handle"]["args"], serde_json::json!(["--stage", "intent"]));
    assert!(entries[0]["detail"].as_str().unwrap().starts_with("next"), "{entries:?}");
    assert_eq!(entries[1]["label"], "Outcome");
    let last = &entries[6];
    assert_eq!(last["label"], "Lint");
    assert_eq!(last["handle"]["command"], "intent lint");
    assert_eq!(last["handle"]["args"], serde_json::json!([]));
    // A project named in the request is named in each handle.
    let named = picker(&root, "stage", &["--project", "Add retries to the worker"]);
    let first = &named["result"]["entries"][0]["handle"]["args"];
    assert_eq!(first[0], "--stage");
    assert!(first.as_array().unwrap().iter().any(|a| a == "--project"), "{first}");
}

#[test]
fn the_project_picker_lists_ghost_projects() {
    let (_tmp, root) = scanned();
    new_project(&root, "Add retries to the worker");
    let env = picker(&root, "project", &[]);
    let entries = env["result"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{env}");
    assert_eq!(entries[0]["label"], "Add retries to the w…", "a label is cut to 20 characters");
    assert_eq!(entries[0]["current"], true, "the only project is the current one");
    assert_eq!(entries[0]["handle"]["command"], "intent list");
    assert_eq!(entries[0]["handle"]["args"][0], "--project");
    assert!(entries[0]["detail"].as_str().unwrap().contains("0 items"), "{entries:?}");
}

#[test]
fn a_picker_with_nothing_to_pick_has_no_entries() {
    let (_tmp, root) = scanned();
    for which in ["project", "stage"] {
        let env = picker(&root, which, &[]);
        let result = &env["result"];
        assert_eq!(result["entries"].as_array().unwrap().len(), 0, "{which}: {result}");
        assert_eq!(result["more"], 0);
        let why = result["why"].as_str().unwrap();
        assert!(why.contains("intent new"), "{which}: {why}");
    }
}

#[test]
fn the_stage_picker_with_several_projects_asks_for_one() {
    let (_tmp, root) = scanned();
    new_project(&root, "Add retries to the worker");
    new_project(&root, "Move orders to the new queue");
    let env = picker(&root, "stage", &[]);
    let result = &env["result"];
    assert_eq!(result["entries"].as_array().unwrap().len(), 0, "{result}");
    assert_eq!(result["why"], "Pick a project with p first");
    // Naming one lists its stages.
    let named = picker(&root, "stage", &["--project", "Move orders to the new queue"]);
    assert_eq!(named["result"]["entries"].as_array().unwrap().len(), 7);
}

#[test]
fn a_picker_outside_a_workspace_is_the_empty_pane() {
    let dir = tempfile::tempdir().unwrap();
    for which in ["lens", "project", "stage"] {
        let env = planroom(dir.path(), &["focus", "--picker", which, "--json"]);
        assert_eq!(env["result"]["empty"]["reason"], "no_repos", "{which}: {env}");
        assert!(env["result"].get("entries").is_none(), "{env}");
    }
    assert!(!dir.path().join(".planroom").exists(), "a picker created a state directory");
}

#[test]
fn a_picker_keeps_to_focus_budget() {
    let (_tmp, root) = scanned();
    for i in 0..40 {
        new_project(&root, &format!("{i:02} {}", "p".repeat(197)));
    }
    for raw in [false, true] {
        let env = planroom_with(
            &root,
            &["focus", "--picker", "project", "--json"],
            if raw { &[("PLANROOM_RAW", "1")] } else { &[] },
        );
        assert_eq!(env["ok"], true, "{env}");
        let result = &env["result"];
        let shown = result["entries"].as_array().unwrap().len() as u64;
        assert!(shown <= 16, "{shown} entries");
        assert_eq!(shown + result["more"].as_u64().unwrap(), 40, "every project is shown or counted");
        let tokens = env["meta"]["size"]["tokens_est"].as_u64().unwrap();
        assert!(tokens <= 1_000, "raw {raw}: {tokens} tokens");
    }
}

#[test]
fn a_project_name_in_a_picker_is_masked() {
    let (_tmp, root) = scanned();
    new_project(&root, "db_PASSWORD=hunter2");
    let masked = picker(&root, "project", &[]);
    assert_eq!(masked["result"]["entries"][0]["label"], "db_PASSWORD=[redacted]", "{masked}");
    assert!(!masked.to_string().contains("hunter2"), "{masked}");
    let raw = planroom_with(&root, &["focus", "--picker", "project", "--json"], &[("PLANROOM_RAW", "1")]);
    assert_eq!(raw["result"]["entries"][0]["label"], "db_PASSWORD=hunter2", "{raw}");
}
