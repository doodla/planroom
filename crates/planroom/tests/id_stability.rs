//! IDs stay stable across releases (DR-74): every node ID the scanned fixture had in the last
//! release still exists. Until v0.1.0 commits a release's list, `id_baseline.json` stands in for
//! it; it changes only with a decision record that says how overrides on a changed ID carry over.
//! This test only reads it.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use planroom_store::{Shape, Store};
use planroom_testkit::fixture;

const BASELINE: &str = "crates/planroom/tests/id_baseline.json";

fn baseline() -> Vec<String> {
    let path = planroom_testkit::repo_root().join(BASELINE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{BASELINE}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{BASELINE}: {e}"))
}

/// Every node ID of the store's active snapshots after a scan of the fixture at `root`.
fn scanned_ids(root: &Path) -> BTreeSet<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", ".", "--json"])
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_RAW")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().expect("a store");
    let active = store.active_set().unwrap();
    store
        .facts(&active.snapshots())
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Node)
        .map(|r| r.id)
        .collect()
}

#[test]
fn fixture_ids_match_the_last_release() {
    let listed = baseline();
    let mut sorted = listed.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(listed, sorted, "{BASELINE} lists each ID once, sorted");
    assert!(listed.len() > 100, "{BASELINE} lists the fixture's nodes, not a handful");

    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let now = scanned_ids(&root);
    let gone: Vec<&String> = listed.iter().filter(|id| !now.contains(*id)).collect();
    assert!(
        gone.is_empty(),
        "{} node IDs of the last release no longer exist, so overrides on them would detach. A change \
         that means it needs a decision record saying how they carry over, and then a new {BASELINE}: {gone:?}",
        gone.len()
    );
}
