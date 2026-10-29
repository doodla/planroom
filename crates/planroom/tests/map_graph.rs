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
    // The same top-level blocks: the graph's evidence samples can take its blocks past one page,
    // so its pages, followed to the end, hold them.
    let mut blocks = view["nodes"].as_array().unwrap().clone();
    let mut next = graph["meta"]["next"].as_str().map(str::to_owned);
    while let Some(cursor) = next {
        let page = map(&root, &["--detail", "graph", "--cursor", &cursor]);
        blocks.extend(page["result"]["nodes"].as_array().unwrap().iter().cloned());
        next = page["meta"]["next"].as_str().map(str::to_owned);
    }
    let by_id = |nodes: &[Value]| -> std::collections::BTreeMap<String, Value> {
        nodes.iter().map(|n| (n["id"].as_str().unwrap().to_owned(), n.clone())).collect()
    };
    assert_eq!(
        by_id(&blocks),
        by_id(summary["result"]["nodes"].as_array().unwrap()),
        "the same top-level blocks"
    );
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

#[test]
fn graph_detail_carries_the_ladder_and_summary_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let ladder = |reply: &Value| -> Vec<(String, String, bool, String)> {
        reply["result"]["levels"]
            .as_array()
            .unwrap_or_else(|| panic!("the graph carries the ladder: {reply}"))
            .iter()
            .map(|l| {
                assert!(l["one"].as_str().unwrap().contains("{count}"), "{l}");
                assert!(l["many"].as_str().unwrap().contains("{count}"), "{l}");
                (
                    l["id"].as_str().unwrap().to_owned(),
                    l["label"].as_str().unwrap().to_owned(),
                    l["apart"].as_bool().unwrap(),
                    l["pack"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let want: Vec<(String, String, bool, String)> = [
        ("intent", true, "kernel"),
        ("system", false, "code"),
        ("container", false, "code"),
        ("component", false, "code"),
        ("code", false, "code"),
    ]
    .into_iter()
    .map(|(id, apart, pack)| (id.to_owned(), id.to_owned(), apart, pack.to_owned()))
    .collect();
    // Not scanned: the empty state's graph carries it too, so a surface can name the levels.
    let empty = map(&root, &["--detail", "graph"]);
    assert!(empty["result"]["empty"].is_object(), "{empty}");
    assert_eq!(ladder(&empty), want);
    assert!(map(&root, &[])["result"].get("levels").is_none(), "a summary carries no ladder");

    let scanned = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "--json"])
        .current_dir(&root)
        .env("PLANROOM_DATA", root.join(".data"))
        .output()
        .unwrap();
    assert!(scanned.status.success());
    for level in ["system", "container", "code"] {
        let graph = map(&root, &["--detail", "graph", "--level", level]);
        assert_eq!(ladder(&graph), want, "at the {level} level");
        let summary = map(&root, &["--level", level]);
        assert!(summary["result"].get("levels").is_none(), "a summary carries no ladder: {summary}");
    }
}
