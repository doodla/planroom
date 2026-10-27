//! `find TEXT` (architecture.md, "Lenses and questions": the `find` shape; contracts.md, `FindResult`):
//! fuzzy search over the map's node names and IDs, best first, each match with its ID, level and
//! kind and handles that run today; no match, or no scan yet, is an empty state.

use std::path::Path;
use std::process::Command;

use planroom_contract::{FindReq, FindResult};
use planroom_kernel::redact;
use planroom_testkit::fixture;
use serde_json::Value;

/// A `--json` reply's envelope, which must be `ok`, on one line.
fn ok(cwd: &Path, args: &[&str]) -> Value {
    ok_with(cwd, args, &[])
}

/// [`ok`] with the environment variables `vars` set.
fn ok_with(cwd: &Path, args: &[&str], vars: &[(&str, &str)]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .env_remove("PLANROOM_RAW")
        .envs(vars.iter().copied())
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"));
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    env
}

/// Run a handle as the CLI runs it.
fn run(cwd: &Path, handle: &Value) -> Value {
    let command = handle["command"].as_str().unwrap();
    let args = handle["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap());
    let words: Vec<&str> = command.split(' ').chain(args).collect();
    ok(cwd, &words)
}

#[test]
fn find_returns_ids_and_handles() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);

    // Before a scan, find has nothing to search, and says what to do.
    let before = ok(&root, &["find", "worker"]);
    assert_eq!(before["result"]["empty"]["reason"], "not_scanned", "{before}");
    assert!(before["result"]["items"].as_array().unwrap().is_empty());

    ok(&root, &["scan"]);
    for (text, name, kind) in [
        ("worker", "worker", "code.service"),
        ("consumer.py", "consumer.py", "code.file"),
        ("orders-db", "orders-db", "code.datastore"),
    ] {
        let env = ok(&root, &["find", text]);
        let found = &env["result"];
        assert!(found["empty"].is_null(), "{text}: {env}");
        assert!(env["summary"].as_str().is_some_and(|s| !s.is_empty()), "{text}: a summary line");
        let items = found["items"].as_array().unwrap();
        assert!(!items.is_empty(), "{text}: {env}");
        // An exact name ranks first, among blocks of that name the one of the kind asked for.
        let first = items.iter().find(|i| i["kind"] == kind).unwrap_or_else(|| panic!("{text}: {env}"));
        assert_eq!(first["name"], name, "{text}: {env}");
        assert_eq!(items[0]["name"], name, "{text}: the exact name first: {env}");
        for item in items {
            let id = item["id"].as_str().unwrap();
            assert_eq!(id.splitn(3, ':').count(), 3, "{text}: a node ID: {id}");
            assert!(item["level"].is_string() && item["kind"].is_string(), "{item}");
            let handles = item["handles"].as_array().unwrap();
            assert!(!handles.is_empty(), "{text}: every match carries handles: {item}");
            for handle in handles {
                assert!(handle["label"].as_str().is_some_and(|l| !l.is_empty()), "{handle}");
            }
        }
        // The first handle shows the match on the map, among the blocks beside it.
        let shown = run(&root, &first["handles"][0]);
        let ids: Vec<&str> =
            shown["result"]["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
        assert!(
            ids.contains(&first["id"].as_str().unwrap()),
            "{text}: {} shows {}: {shown}",
            first["handles"][0],
            first["id"]
        );
        assert_eq!(shown["result"]["level"], first["level"], "{text}: at its own level");
    }

    // A match by ID as well as by name.
    let by_id = ok(&root, &["find", "storefront:container:storefront"]);
    assert_eq!(by_id["result"]["items"][0]["id"], "storefront:container:storefront", "{by_id}");

    // Nothing matches: an empty state whose next action runs.
    let none = ok(&root, &["find", "qqqzzzxxjj"]);
    assert_eq!(none["result"]["empty"]["reason"], "no_match", "{none}");
    assert!(none["result"]["items"].as_array().unwrap().is_empty());
    let next = run(&root, &none["result"]["empty"]["next"]);
    assert!(next["result"]["empty"].is_null(), "{next}");
}

#[test]
fn every_find_reply_stays_within_its_budget_whatever_the_search_text() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    let budget = u64::from(planroom_contract::Command::Find(FindReq::default()).spec().budget_tokens);
    let long_match = "src ".repeat(600);
    let long_miss = "zq".repeat(1500);
    for (text, matches) in [(long_match.as_str(), true), (long_miss.as_str(), false), ("o", true)] {
        let env = ok(&root, &["find", text]);
        let found = &env["result"];
        assert_eq!(found["empty"].is_null(), matches, "{env}");
        let tokens = env["meta"]["size"]["tokens_est"].as_u64().unwrap();
        assert!(tokens <= budget, "{} chars: {tokens} tokens over {budget}: {env}", text.len());
        let warnings = env["meta"]["warnings"].as_array().map(Vec::as_slice).unwrap_or_default();
        assert!(warnings.iter().all(|w| w["code"] != "truncated"), "{} chars: {warnings:?}", text.len());
        // The summary echoes a short prefix of a long search text, never all of it.
        let summary = found["summary"].as_str().unwrap();
        assert!(summary.chars().count() < 200, "{summary}");
        if text.len() > 100 {
            assert!(summary.contains('…'), "{summary}");
            let line = found["empty"]["line"].as_str().unwrap_or(summary);
            assert!(line.chars().count() < 200, "{line}");
        }
    }
}

/// The names of `result`'s matches.
fn names(result: &FindResult) -> Vec<&str> {
    result.items.iter().map(|i| i.name.as_str()).collect()
}

/// Commits `path`, with one line, in the fixture repo `repo` through git's index alone, and keeps
/// it out of the working tree's status: a path longer than a filesystem allows (macOS's 1,024
/// bytes) still fits in a tree, as a repo committed elsewhere holds it, and a scan reads the tree.
fn commit_in_the_index(repo: &Path, path: &str) {
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(repo).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let line = repo.join("line.ts");
    std::fs::write(&line, "export const x = 1;\n").unwrap();
    let blob = git(&["hash-object", "-w", "line.ts"]);
    std::fs::remove_file(&line).unwrap();
    git(&["update-index", "--add", "--cacheinfo", &format!("100644,{},{path}", blob.trim())]);
    git(&["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "more"]);
    git(&["update-index", "--skip-worktree", path]);
}

#[test]
fn a_find_whose_match_is_too_long_to_list_stays_within_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    // A file 14 directories deep, each name 200 characters long, so its ID alone passes the
    // budget.
    let repo = root.join("storefront");
    let mut deep = String::from("src");
    for i in 0..14 {
        deep.push_str(&format!("/{i:02}{}", "d".repeat(198)));
    }
    commit_in_the_index(&repo, &format!("{deep}/zebrafinch.ts"));
    ok(&root, &["scan"]);

    let budget = u64::from(planroom_contract::Command::Find(FindReq::default()).spec().budget_tokens);
    let env = ok(&root, &["find", "zebrafinch"]);
    let tokens = env["meta"]["size"]["tokens_est"].as_u64().unwrap();
    assert!(tokens <= budget, "{tokens} tokens over {budget}: {env}");
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    let over = text_of("warning", "reply_over_budget");
    let over = &over[..over.find('{').unwrap()];
    assert!(warnings.iter().all(|w| !w["message"].as_str().unwrap().starts_with(over)), "{warnings:?}");
    // No match is listed, the summary counts the one there is, and a truncated warning points
    // to a narrower search that runs.
    let found = &env["result"];
    assert!(found["items"].as_array().unwrap().is_empty(), "{env}");
    assert!(found["empty"].is_null(), "{env}");
    assert!(found["summary"].as_str().unwrap().starts_with("1 block matches zebrafinch"), "{env}");
    let truncated = warnings.iter().find(|w| w["code"] == "truncated").expect("a truncated warning");
    assert_eq!(truncated["details"]["total"], 1, "{truncated}");
    let next = &truncated["details"]["next"];
    assert_eq!(next["command"], "find", "{next}");
    run(&root, next);
}

#[test]
fn one_match_is_counted_in_the_singular() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    let env = ok(&root, &["find", "storefront:container:storefront"]);
    let summary = env["result"]["summary"].as_str().unwrap();
    assert!(summary.starts_with("1 block matches "), "{summary}");
}

/// The template at `section.key` in the string catalog.
fn text_of(section: &str, key: &str) -> String {
    let catalog = std::fs::read_to_string(planroom_testkit::repo_root().join("crates/planroom/text/en.toml"));
    let catalog: toml::Table = toml::from_str(&catalog.unwrap()).unwrap();
    catalog[section][key].as_str().unwrap().to_owned()
}

/// A secret in a block's name is masked in find's output, its summary as in each match's name,
/// through the CLI and by the contract's marks alike, while IDs and handles stay whole; the truth
/// comparator reads it raw.
#[test]
fn a_secret_in_a_block_name_is_masked_in_finds_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    // A directory whose name holds a secret, committed, so the scan maps it as a block.
    let repo = root.join("storefront");
    let dir = repo.join("src").join("db_PASSWORD=hunter2");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.ts"), "export const x = 1;\n").unwrap();
    for args in [
        vec!["add", "-A"],
        vec!["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "db"],
    ] {
        let out = Command::new("git").args(&args).current_dir(&repo).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    ok(&root, &["scan"]);

    // The truth comparator reads the names raw, on a read command's --json stdout alone.
    let raw = ok_with(&root, &["find", "db_PASSWORD"], &[("PLANROOM_RAW", "1")]);
    let found: FindResult = serde_json::from_value(raw["result"].clone()).unwrap();
    assert!(found.summary.contains("db_PASSWORD=hunter2"), "the summary names the match: {raw}");
    assert!(names(&found).contains(&"db_PASSWORD=hunter2"), "{raw}");

    // Every other output masks the secret in the summary as in each match's name, by the
    // contract's redaction marks.
    let mut masked = found.clone();
    redact::output(&mut masked);
    assert!(!masked.summary.contains("hunter2"), "the summary is masked: {}", masked.summary);
    assert!(masked.summary.contains(redact::MASK), "{}", masked.summary);
    assert!(names(&masked).iter().all(|n| !n.contains("hunter2")), "{:?}", names(&masked));
    assert!(names(&masked).iter().any(|n| n.contains(redact::MASK)), "{:?}", names(&masked));
    // IDs and handles carry no mark, so they stay whole for the next command.
    let ids = |r: &FindResult| r.items.iter().map(|i| (i.id.clone(), i.handles.clone())).collect::<Vec<_>>();
    assert_eq!(ids(&masked), ids(&found));

    // The CLI's own --json masks the same fields, and the envelope's summary with them.
    let env = ok(&root, &["find", "db_PASSWORD"]);
    let shown: FindResult = serde_json::from_value(env["result"].clone()).unwrap();
    let summary = env["summary"].as_str().unwrap();
    for line in [shown.summary.as_str(), summary] {
        assert!(!line.contains("hunter2") && line.contains(redact::MASK), "{line}");
    }
    assert!(names(&shown).iter().all(|n| !n.contains("hunter2")), "{env}");
    assert_eq!(ids(&shown), ids(&found), "IDs and handles stay whole on the CLI");
}
