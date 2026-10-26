//! WS3: the fixture flows to the store and to JSON. `scan` on the built fixture, the compose
//! containers and their evidence against `TRUTH.json`, the cold-scan budget, the scan-key rule on
//! a rescan, and `map --json` before and after a scan.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use planroom_store::{Shape, Store};
use planroom_testkit::{evidence_matches, fixture, truth};
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs")
}

/// Run a `--json` command and return its envelope, checking it is one line with a true size.
fn envelope(cwd: &Path, args: &[&str]) -> (Value, Output) {
    let out = planroom(cwd, args);
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "exactly one JSON line on stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(env["schema_version"], 1);
    assert_eq!(
        env["meta"]["size"]["bytes"].as_u64().unwrap() as usize,
        lines[0].len(),
        "meta.size measures the reply"
    );
    assert_eq!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() as usize, lines[0].len().div_ceil(4));
    (env, out)
}

fn scan(root: &Path) -> Value {
    let (env, out) = envelope(root, &["scan", root.to_str().unwrap(), "--json"]);
    assert!(out.status.success(), "{env}");
    assert_eq!(env["ok"], true, "{env}");
    env
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Every fact of the active snapshots, as JSON, read from the store (no `show` command yet).
fn active_facts(root: &Path) -> Vec<(Shape, Value)> {
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().expect("a store");
    let active = store.active_set().unwrap();
    store
        .facts(&active.snapshots())
        .unwrap()
        .into_iter()
        .map(|r| (r.shape, serde_json::from_str(&r.body).unwrap()))
        .collect()
}

fn scan_events(root: &Path) -> Vec<(u64, String)> {
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    store
        .events_after(0, 1000)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e.body {
            planroom_contract::EventBody::Scan { repo, .. } => Some((e.seq, repo)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_scan_finds_the_truths_compose_containers_with_evidence_on_their_lines() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let env = scan(&root);
    assert_eq!(env["meta"]["seq"], 5, "one scan event per repo: {env}");
    let facts = active_facts(&root);
    let nodes: Vec<&Value> = facts.iter().filter(|(s, _)| *s == Shape::Node).map(|(_, v)| v).collect();
    let truth = truth();

    // Every container the truth declares in the compose file, by its ID and name, with each of
    // the truth's compose evidence items on the exact line.
    let compose = "platform/docker-compose.yml";
    let mut checked = 0;
    for container in truth["containers"].as_array().unwrap() {
        if container["declared_in"] != compose {
            continue;
        }
        let name = container["name"].as_str().unwrap();
        let ns = container["repo"].as_str().unwrap_or("ws");
        let id = format!("{ns}:container:{name}");
        let node = nodes.iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("no node {id}"));
        assert_eq!(node["name"], name);
        assert_eq!(node["level"], "container");
        for want in container["evidence"].as_array().unwrap().iter().filter(|e| e["path"] == compose) {
            let got = node["evidence"].as_array().unwrap();
            assert!(got.iter().any(|g| evidence_matches(want, g)), "{id}: {want} not in {got:?}");
        }
        checked += 1;
    }
    assert_eq!(checked, 9, "eight compose services and the platform tooling container");
    for (name, kind) in [
        ("storefront", "code.service"),
        ("orders", "code.service"),
        ("worker", "code.service"),
        ("platform", "code.tooling"),
    ] {
        let id = format!("{name}:container:{name}");
        let node = nodes.iter().find(|n| n["id"] == id).unwrap();
        assert_eq!(node["kind"], kind, "{id}");
        assert_eq!(node["parent"], format!("{name}:repo:{name}"), "{id} joins its repo");
    }
    let orders = nodes.iter().find(|n| n["id"] == "orders:container:orders").unwrap();
    assert_eq!(orders["aliases"], serde_json::json!(["host:orders", "service:orders"]));
    assert_eq!(orders["props"]["image"], "acme/orders:1.4.0");

    // Every compose line the truth's relations cite is evidence on some fact: environment lines
    // on `code.env_set` hints, image lines on containers.
    let all_evidence: Vec<&Value> =
        facts.iter().flat_map(|(_, v)| v["evidence"].as_array().unwrap()).collect();
    for relation in truth["relations"].as_array().unwrap() {
        for want in relation["evidence"].as_array().unwrap().iter().filter(|e| e["path"] == compose) {
            assert!(all_evidence.iter().any(|g| evidence_matches(want, g)), "{}: {want}", relation["kind"]);
        }
    }
    let env_set: BTreeSet<(String, String)> = facts
        .iter()
        .filter(|(s, v)| *s == Shape::Hint && v["kind"] == "code.env_set")
        .map(|(_, v)| (v["scope"].as_str().unwrap().to_owned(), v["key"].as_str().unwrap().to_owned()))
        .collect();
    assert!(env_set.contains(&("worker:container:worker".into(), "DATABASE_URL".into())));
    assert!(env_set.contains(&("storefront:container:storefront".into(), "ORDERS_URL".into())));
    let depends: BTreeSet<(String, String)> = facts
        .iter()
        .filter(|(s, v)| *s == Shape::Hint && v["kind"] == "code.depends_on")
        .map(|(_, v)| (v["scope"].as_str().unwrap().to_owned(), v["key"].as_str().unwrap().to_owned()))
        .collect();
    assert!(depends.contains(&("orders:container:orders".into(), "orders-db".into())));
    assert!(depends.contains(&("ws:container:grafana".into(), "prometheus".into())));

    // Every repo and every component path the truth lists exists in the file tree [TRUTH about].
    let ids: BTreeSet<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    for repo in truth["repos"].as_object().unwrap().keys() {
        assert!(ids.contains(format!("{repo}:repo:{repo}").as_str()), "repo {repo}");
    }
    for component in truth["components"].as_array().unwrap() {
        let path = component["path"].as_str().unwrap();
        let (repo, rel) = path.split_once('/').unwrap();
        let found = ids.contains(format!("{repo}:dir:{rel}").as_str())
            || ids.contains(format!("{repo}:file:{rel}").as_str());
        assert!(found, "component {path}");
    }
    // Every fact carries evidence.
    assert!(facts.iter().all(|(_, v)| !v["evidence"].as_array().unwrap().is_empty()));
}

#[test]
fn a_cold_scan_of_the_fixture_takes_under_5_s() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let started = Instant::now();
    scan(&root);
    let took = started.elapsed();
    eprintln!("cold scan of the fixture: {took:?}");
    assert!(took < Duration::from_secs(5), "cold scan took {took:?}");
}

#[test]
fn a_rescan_writes_a_scan_event_only_when_a_head_moved() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let first = scan(&root);
    assert_eq!(first["meta"]["seq"], 5);
    let same = scan(&root);
    assert_eq!(same["meta"]["seq"], 5, "nothing moved, so no new scan event");
    assert_eq!(same["result"], first["result"], "the same map");

    git(
        &root.join("worker"),
        &["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "Touch"],
    );
    let moved = scan(&root);
    assert_eq!(moved["meta"]["seq"], 6, "one repo moved: one event");
    assert_eq!(scan_events(&root).last().unwrap(), &(6, "worker".to_owned()));

    // A move back to the earlier head still records a new scan (DR-8).
    git(&root.join("worker"), &["reset", "-q", "--hard", "HEAD~1"]);
    let back = scan(&root);
    assert_eq!(back["meta"]["seq"], 7);
    assert_eq!(scan(&root)["meta"]["seq"], 7);
}

#[test]
fn map_lists_the_repos_and_workspace_containers_with_counts() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    let (env, out) = envelope(&root.join("orders"), &["map", "--json"]);
    assert!(out.status.success());
    assert_eq!(env["command"], "map");
    assert_eq!(env["meta"]["seq"], 5);
    let map = &env["result"];
    assert_eq!(map["level"], "system");
    assert!(map.get("empty").is_none_or(Value::is_null));
    let names: Vec<&str> =
        map["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "kit",
            "orders",
            "platform",
            "storefront",
            "worker",
            "grafana",
            "orders-db",
            "otel-collector",
            "prometheus",
            "redis"
        ]
    );
    let orders = &map["nodes"][1];
    assert_eq!(orders["id"], "orders:repo:orders");
    assert_eq!(orders["counts"]["container"], 1);
    assert!(orders["counts"]["code"].as_u64().unwrap() > 10);
    assert!(orders["counts"]["component"].as_u64().unwrap() >= 6);
    assert_eq!(map["nodes"][9]["id"], "ws:container:redis");
}

#[test]
fn map_before_a_scan_offers_the_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let (env, out) = envelope(&root, &["map", "--json"]);
    assert!(out.status.success(), "{env}");
    assert_eq!(env["meta"]["seq"], 0);
    let empty = &env["result"]["empty"];
    assert_eq!(empty["reason"], "not_scanned");
    assert_eq!(empty["next"]["command"], "scan");
    assert_eq!(empty["next"]["label"], "Map these 5 repos? About 2 s.");
    assert_eq!(env["result"]["nodes"], serde_json::json!([]));
}

#[test]
fn map_outside_a_workspace_is_workspace_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (env, out) = envelope(dir.path(), &["map", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "workspace_not_found");
    assert_eq!(env["error"]["retry"], "never");
    assert_eq!(env["error"]["hint"], "planroom scan DIR");
}

#[test]
fn a_scan_of_a_directory_with_no_repos_is_workspace_not_found() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("empty")).unwrap();
    let (env, out) = envelope(dir.path(), &["scan", "empty", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(env["error"]["code"], "workspace_not_found");
    assert!(!dir.path().join("empty/.planroom").exists(), "no workspace until a scan finds repos");
}

#[test]
fn the_store_and_its_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&root.join(".planroom")), 0o700);
    assert_eq!(mode(&root.join(".planroom/store.db")), 0o600);
    assert_eq!(std::fs::read_to_string(root.join(".planroom/.gitignore")).unwrap(), "*\n");
}
