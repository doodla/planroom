//! Golden CLI outputs from real runs on the built fixture, which the mod's and the canvas's tests
//! read (DR-6, DR-79): `tests/golden/cli/focus.json`, `tests/golden/cli/map-graph.json` and
//! `tests/golden/cli/map-empty.json`.
//!
//! A plain run compares the engine's output with the committed files. `cargo xtask gen --golden`
//! runs this test with `PLANROOM_GOLDEN=write`, which writes them instead. The one value a run
//! can't repeat, `meta.took_ms`, is left out, and `meta.size` is measured again without it.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use planroom_contract::{MapView, PaneModel, Position};
use planroom_store::Store;
use planroom_testkit::{fixture, repo_root};
use serde_json::Value;

fn run(root: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_CLIENT")
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// The envelope without `meta.took_ms`, with `meta.size` measuring the line that is left, as the
/// engine measures its own.
fn normalized(mut env: Value) -> String {
    env["meta"].as_object_mut().unwrap().remove("took_ms");
    let mut line = env.to_string();
    for _ in 0..4 {
        let bytes = line.len() as u64;
        let size = serde_json::json!({"bytes": bytes, "tokens_est": bytes.div_ceil(4)});
        if env["meta"]["size"] == size {
            break;
        }
        env["meta"]["size"] = size;
        line = env.to_string();
    }
    line + "\n"
}

/// `value` with every string naming the temporary workspace root rewritten to `/home/you/shop`,
/// so a golden doesn't carry the test's own temp path.
fn rooted(value: Value, root: &Path) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace(&*root.to_string_lossy(), "/home/you/shop")),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| rooted(v, root)).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, rooted(v, root))).collect()),
        other => other,
    }
}

fn check(name: &str, output: String) {
    let path = repo_root().join("tests/golden/cli").join(name);
    if std::env::var("PLANROOM_GOLDEN").as_deref() == Ok("write") {
        std::fs::write(&path, &output).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == output,
        "{} differs from the engine's output; run cargo xtask gen --golden\nengine:    {output}committed: {committed}",
        path.display()
    );
}

#[test]
fn golden_outputs_equal_engine_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let empty = run(&root, &["map", "--detail", "graph", "--json"]);
    let unscanned: MapView = serde_json::from_value(empty["result"].clone()).expect("map returns a MapView");
    assert!(unscanned.nodes.is_empty() && unscanned.empty.is_some(), "an unscanned workspace maps empty");
    check("map-empty.json", normalized(rooted(empty, &root)));

    run(&root, &["scan", "--json"]);

    let focus = run(&root, &["focus", "--json"]);
    let _: PaneModel = serde_json::from_value(focus["result"].clone()).expect("focus returns a PaneModel");
    check("focus.json", normalized(focus));

    // Cache a layout the way the canvas does, so the graph carries positions.
    let unplaced = run(&root, &["map", "--detail", "graph", "--json"]);
    let view: MapView = serde_json::from_value(unplaced["result"].clone()).expect("map returns a MapView");
    let key = view.layout.expect("--detail graph names its layout key");
    let positions: BTreeMap<_, _> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), Position { x: (i % 5) as f64 * 240.0, y: (i / 5) as f64 * 160.0 }))
        .collect();
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    store
        .put_layout(&key.view_key, &key.structure_hash, &serde_json::to_string(&positions).unwrap())
        .unwrap();
    drop(store);

    let graph = run(&root, &["map", "--detail", "graph", "--json"]);
    let placed: MapView = serde_json::from_value(graph["result"].clone()).unwrap();
    assert!(placed.nodes.iter().all(|n| n.position == positions.get(&n.id).copied()));
    check("map-graph.json", normalized(graph));
}
