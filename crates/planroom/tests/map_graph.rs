//! `map --detail graph`: every top-level block and lifted edge, with positions from the layout
//! cache when a layout is cached for the view's structure, and the key to cache one under.

use std::path::Path;
use std::process::Command;

use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

fn map(root: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .arg("map")
        .args(args)
        .arg("--json")
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn the_graph_carries_cached_positions_and_its_layout_key() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let scanned = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "--json"])
        .current_dir(&root)
        .env("PLANROOM_DATA", root.join(".data"))
        .output()
        .unwrap();
    assert!(scanned.status.success());

    let summary = map(&root, &[]);
    assert!(summary["result"].get("layout").is_none(), "only the graph names a layout key");
    let graph = map(&root, &["--detail", "graph"]);
    let view = &graph["result"];
    assert_eq!(view["nodes"], summary["result"]["nodes"], "the same top-level blocks");
    assert!(view["nodes"].as_array().unwrap().iter().all(|n| n["position"].is_null()));
    let key = &view["layout"];
    assert_eq!(key["view_key"], "map:system:runtime");

    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let positions = json!({"orders:repo:orders": {"x": 10.0, "y": 20.0}});
    store
        .put_layout("map:system:runtime", key["structure_hash"].as_str().unwrap(), &positions.to_string())
        .unwrap();
    // A layout cached for another structure is never used.
    store.put_layout("map:system:runtime", "stale", r#"{"kit:repo:kit": {"x": 1.0, "y": 1.0}}"#).unwrap();
    drop(store);

    let placed = map(&root, &["--detail", "graph"]);
    for node in placed["result"]["nodes"].as_array().unwrap() {
        if node["id"] == "orders:repo:orders" {
            assert_eq!(node["position"], json!({"x": 10.0, "y": 20.0}));
        } else {
            assert!(node["position"].is_null(), "{node}");
        }
    }
}
