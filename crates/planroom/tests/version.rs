//! `version --json` and `schema [--commands]`: what this engine is, matched by its catalogue hash.

use std::process::Command;

use planroom_testkit::repo_root;
use serde_json::Value;

fn json(args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom")).args(args).output().expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one JSON line: {stdout}");
    serde_json::from_str(&stdout).unwrap()
}

#[test]
fn version_reports_the_catalogue_hash_engine_env_names() {
    let env = json(&["version", "--json"]);
    assert_eq!(env["ok"], true);
    assert_eq!(env["command"], "version");
    let info = &env["result"];
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["schema_version"], 1);
    assert!(!info["target"].as_str().unwrap().is_empty());
    let hash = info["catalogue_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    // The resolver matches a checkout build by this line, which `cargo xtask gen` writes.
    let engine_env = std::fs::read_to_string(repo_root().join("plugin/scripts/engine.env")).unwrap();
    assert!(
        engine_env.lines().any(|l| l == format!("PLANROOM_CATALOGUE_HASH={hash}")),
        "engine.env names another catalogue; run cargo xtask gen\n{engine_env}"
    );
}

/// Every page of `schema --commands`, following `meta.next`, each checked within the budget.
fn listing_pages() -> Vec<Value> {
    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["schema", "--commands", "--json"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c.as_str()]);
        }
        let env = json(&args);
        assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 1_500, "{env}");
        cursor = env["meta"]["next"].as_str().map(str::to_owned);
        pages.push(env);
        if cursor.is_none() {
            return pages;
        }
        assert!(pages.len() < 50, "the listing ends");
    }
}

#[test]
fn schema_commands_pages_join_to_the_whole_catalogue() {
    let pages = listing_pages();
    let names: Vec<String> = pages
        .iter()
        .flat_map(|p| p["result"]["items"].as_array().unwrap().iter())
        .map(|i| i["name"].as_str().unwrap().to_owned())
        .collect();
    let want: Vec<String> = planroom_contract::Command::SPECS.iter().map(|s| s.name()).collect();
    assert_eq!(names, want, "the pages join, in order and each leaf once");
    // A cursor that isn't one is a usage error naming the field.
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["schema", "--commands", "--cursor", "nope", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["error"]["details"]["field"], "cursor", "{env}");
}

/// A cursor `schema --commands` never gave, such as a bare leaf index, is a usage error naming
/// the field: the cursor is opaque, tied to the catalogue its pages came from.
#[test]
fn a_schema_commands_cursor_no_page_gave_is_refused() {
    let given = listing_pages()[0]["meta"]["next"].as_str().unwrap().to_owned();
    for cursor in ["0", "9", "09", "+9", "21", &given[..given.len() - 2]] {
        let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
            .args(["schema", "--commands", "--cursor", cursor, "--json"])
            .output()
            .unwrap();
        let env: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(2), "--cursor {cursor}: {env}");
        assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
        assert_eq!(env["error"]["details"]["field"], "cursor", "{env}");
    }
}

#[test]
fn schema_commands_lists_every_leaf_with_its_handle() {
    let pages = listing_pages();
    let items: Vec<&Value> =
        pages.iter().flat_map(|p| p["result"]["items"].as_array().unwrap().iter()).collect();
    let names: Vec<&str> = items.iter().map(|i| i["name"].as_str().unwrap()).collect();
    for leaf in ["map", "focus", "scan", "watch", "schema", "version"] {
        assert!(names.contains(&leaf), "{leaf} missing from {names:?}");
    }
    for item in items {
        let args: Vec<&str> =
            item["schema"]["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        let mut argv = vec!["schema"];
        argv.extend(&args);
        argv.push("--json");
        let leaf = json(&argv);
        assert!(leaf["result"]["defs"][leaf["result"]["request"].as_str().unwrap()].is_object());
    }
}

#[test]
fn schema_alone_names_the_id_and_the_commands_handle() {
    let env = json(&["schema", "--json"]);
    let result = &env["result"];
    assert!(result["id"].as_str().unwrap().ends_with("schema/planroom.v1.schema.json"));
    assert_eq!(result["handles"][0]["args"][0], "--commands");
}

#[test]
fn version_reports_its_git_commit() {
    // build.rs stamps the checkout's HEAD when the build environment names no commit, and
    // rebuilds when HEAD moves, so the binary names the commit it was built from.
    let head = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(repo_root()).output().unwrap();
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap().trim().to_owned();
    let info = &json(&["version", "--json"])["result"];
    let expected = std::env::var("PLANROOM_GIT_COMMIT").ok().filter(|c| !c.trim().is_empty()).unwrap_or(head);
    assert_eq!(info["git_commit"], expected.as_str(), "{info}");
    assert_eq!(info["git_commit"].as_str().unwrap().len(), 40);
}
