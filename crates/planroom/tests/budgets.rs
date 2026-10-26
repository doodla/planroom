//! Output budgets: `map` honors the budget its catalogue entry declares, on a synthetic repo of
//! 20 directories of 100 `.ts` files each (unasked-8). Over the budget it keeps the blocks that
//! fit, sets `meta.truncated` with a `truncated` warning, and hands a drill-down that works today.
//! The canvas's layout input, `--detail graph` over `serve`, takes the whole graph.

use std::path::Path;
use std::process::Command;

use planroom_contract::{Command as Leaf, Detail, Level, MapReq};
use planroom_testkit::git_repo;
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap();
    assert!(out.status.success(), "{args:?}: {env}");
    env
}

/// A workspace holding one repo of 20 directories × 100 `.ts` files, scanned.
fn large_workspace(root: &Path) {
    let files: Vec<(String, Vec<u8>)> = (1..=20)
        .flat_map(|d| {
            (1..=100).map(move |f| {
                let next = f % 100 + 1;
                let body = format!(
                    "import {{ f{next} }} from './file{next}';\nexport function f{f}() {{ return f{next}; }}\n"
                );
                (format!("src/mod{d}/file{f}.ts"), body.into_bytes())
            })
        })
        .collect();
    git_repo(&root.join("app"), &files);
    planroom(root, &["scan", "--json"]);
}

#[test]
fn map_keeps_to_its_budget_and_says_where_to_go_next() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let budget = u64::from(Leaf::Map(MapReq::default()).spec().budget_tokens);
    for detail in ["summary", "graph"] {
        let env = planroom(tmp.path(), &["map", "--level", "code", "--detail", detail, "--json"]);
        let meta = &env["meta"];
        assert!(meta["size"]["tokens_est"].as_u64().unwrap() <= budget, "{detail}: {meta}");
        assert_eq!(meta["truncated"], true, "{detail}");
        let warning = meta["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "truncated").unwrap();
        let kept = env["result"]["nodes"].as_array().unwrap().len() as u64;
        assert!(kept > 0, "{detail}: the first page is kept");
        assert_eq!(warning["details"]["kept"], kept);
        assert_eq!(warning["details"]["total"], 2_000);
        // Every edge kept joins two kept blocks.
        let ids: Vec<&Value> = env["result"]["nodes"].as_array().unwrap().iter().map(|n| &n["id"]).collect();
        for edge in env["result"]["edges"].as_array().unwrap() {
            assert!(ids.contains(&&edge["from"]) && ids.contains(&&edge["to"]), "{edge}");
        }
        // The drill-down is a handle that runs today.
        let next = &warning["details"]["next"];
        assert_eq!(next["command"], "map");
        let mut args: Vec<&str> =
            next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        args.insert(0, "map");
        args.push("--json");
        let coarser = planroom(tmp.path(), &args);
        assert!(coarser["meta"]["size"]["tokens_est"].as_u64().unwrap() <= budget);
        assert!(coarser["summary"].as_str().unwrap().starts_with("21 at the top"), "{coarser}");
    }
}

#[test]
fn a_map_within_its_budget_is_whole_and_untruncated() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let env = planroom(tmp.path(), &["map", "--json"]);
    assert!(env["meta"].get("truncated").is_none(), "{env}");
    assert!(env["meta"].get("warnings").is_none(), "{env}");
}

#[test]
fn the_canvas_graph_over_serve_takes_every_block() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let req = MapReq { level: Some(Level::Code), detail: Detail::Graph, ..MapReq::default() };
    let reply = planroom::app::map::for_canvas(&req, tmp.path()).unwrap();
    assert_eq!(reply.result.nodes.len(), 2_000);
    assert!(reply.warnings.is_empty());
    // A summary over serve is still an agent's read, and keeps to the budget.
    let req = MapReq { level: Some(Level::Code), ..MapReq::default() };
    let reply = planroom::app::map::for_canvas(&req, tmp.path()).unwrap();
    assert!(reply.result.nodes.len() < 2_000 && !reply.warnings.is_empty());
}

/// `scan`'s reply carries the top-level map, which an agent reads too: it keeps to `scan`'s own
/// budget. One compose file of 200 services puts 200 parentless containers at the top.
#[test]
fn scan_keeps_its_top_level_map_to_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let services: String =
        (1..=200).map(|n| format!("  service{n}:\n    image: example/service{n}:1\n")).collect();
    git_repo(
        &tmp.path().join("platform"),
        &[("docker-compose.yml".into(), format!("services:\n{services}").into_bytes())],
    );
    let env = planroom(tmp.path(), &["scan", "--json"]);
    let budget = u64::from(Leaf::Scan(planroom_contract::ScanReq::default()).spec().budget_tokens);
    let meta = &env["meta"];
    assert!(meta["size"]["tokens_est"].as_u64().unwrap() <= budget, "{meta}");
    assert_eq!(meta["truncated"], true);
    let warning = meta["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "truncated").unwrap();
    assert_eq!(warning["details"]["total"], 201, "the repo and its 200 containers");
    let kept = env["result"]["map"]["nodes"].as_array().unwrap().len() as u64;
    assert!(kept > 0 && kept < 201, "the first page is kept: {kept}");
    assert_eq!(warning["details"]["kept"], kept);
}
