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
fn map_outside_a_workspace_is_the_empty_state_focus_shows() {
    let dir = tempfile::tempdir().unwrap();
    let (map, out) = envelope(dir.path(), &["map", "--json"]);
    assert!(out.status.success(), "{map}");
    let (focus, _) = envelope(dir.path(), &["focus", "--json"]);
    let empty = &map["result"]["empty"];
    assert_eq!(empty["reason"], "no_repos");
    assert_eq!(empty, &focus["result"]["empty"], "map and focus agree");
    assert_eq!(map["result"]["nodes"], serde_json::json!([]));
    assert_eq!(map["summary"], empty["next"]["label"]);
    // Its one next action works: no scan that would answer workspace_not_found.
    let next = &empty["next"];
    assert_ne!(next["command"], "scan");
    let mut args = vec![next["command"].as_str().unwrap().to_owned()];
    args.extend(next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()));
    args.push("--json".into());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (ran, out) = envelope(dir.path(), &args);
    assert!(out.status.success() && ran["ok"] == true, "{ran}");
    assert!(!dir.path().join(".planroom").exists(), "a read created a state directory");
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

/// The IDs `map --level LEVEL --json` lists, or plain `map --json` for an empty level.
fn map_ids(root: &Path, level: &str) -> BTreeSet<String> {
    let args: &[&str] =
        if level.is_empty() { &["map", "--json"] } else { &["map", "--level", level, "--json"] };
    let (env, out) = envelope(root, args);
    assert!(out.status.success(), "{env}");
    env["result"]["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_owned()).collect()
}

/// Every level's IDs and every active fact, sorted: what two stores at the same heads share.
fn the_map(root: &Path) -> (Vec<BTreeSet<String>>, Vec<String>) {
    let levels = ["", "system", "container", "component", "code"].iter().map(|l| map_ids(root, l)).collect();
    let mut facts: Vec<String> = active_facts(root).iter().map(|(_, v)| v.to_string()).collect();
    facts.sort();
    (levels, facts)
}

#[test]
fn a_dirty_first_scan_then_a_clean_rescan_gives_the_map_a_fresh_store_gives() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(&dir.path().join("dirty")).root;
    let compose = root.join("platform/docker-compose.yml");
    let committed = std::fs::read_to_string(&compose).unwrap();
    std::fs::write(&compose, format!("# local note\n{committed}")).unwrap();
    let first = scan(&root);
    assert!(
        first["warnings"].as_array().is_none_or(|w| w.iter().all(|w| w["code"] != "evidence_dropped")),
        "the dirty edit never reaches extraction: {first}"
    );
    let dirty = the_map(&root);
    std::fs::write(&compose, committed).unwrap();
    scan(&root);
    let rescanned = the_map(&root);
    assert_eq!(rescanned, dirty, "the working tree never changed the map");

    let fresh = fixture(&dir.path().join("fresh")).root;
    scan(&fresh);
    assert_eq!(the_map(&fresh), rescanned, "the same heads give the same map");
    assert!(rescanned.0[0].contains("ws:container:redis"), "{:?}", rescanned.0[0]);
}

#[test]
fn an_untracked_directory_is_never_mapped_and_never_lingers() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let untracked = |ids: &BTreeSet<String>| -> Vec<String> {
        ids.iter().filter(|id| id.contains("newdir") || id.contains("EMPTY_UNTRACKED")).cloned().collect()
    };
    // Present at the first scan of the commit.
    std::fs::create_dir_all(root.join("kit/newdir/deep")).unwrap();
    std::fs::write(root.join("kit/newdir/deep/f1.py"), "x = 1\n").unwrap();
    std::fs::write(root.join("kit/EMPTY_UNTRACKED"), "").unwrap();
    scan(&root);
    let with = the_map(&root);
    for level in &with.0 {
        assert_eq!(untracked(level), Vec::<String>::new());
    }
    assert!(with.1.iter().all(|f| !f.contains("newdir") && !f.contains("EMPTY_UNTRACKED")));
    // Deleted: nothing lingers, and the map is the one it was.
    std::fs::remove_dir_all(root.join("kit/newdir")).unwrap();
    std::fs::remove_file(root.join("kit/EMPTY_UNTRACKED")).unwrap();
    scan(&root);
    assert_eq!(the_map(&root), with);
    // Created after the first scan: still never mapped.
    std::fs::create_dir_all(root.join("kit/newdir")).unwrap();
    std::fs::write(root.join("kit/newdir/f2.py"), "y = 2\n").unwrap();
    scan(&root);
    assert_eq!(the_map(&root), with);
}

#[test]
fn a_file_removed_from_the_working_tree_but_in_head_stays_mapped() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    std::fs::remove_file(root.join("kit/CHANGELOG.md")).unwrap();
    scan(&root);
    assert!(map_ids(&root, "code").contains("kit:file:CHANGELOG.md"));
}

/// The warnings of a reply with `code`.
fn warnings_of<'a>(env: &'a Value, code: &str) -> Vec<&'a Value> {
    env["meta"]["warnings"]
        .as_array()
        .map_or_else(Vec::new, |w| w.iter().filter(|w| w["code"] == code).collect())
}

#[test]
fn moving_a_repo_out_and_back_gives_the_map_a_fresh_store_gives() {
    // correctness-13: compose joins a service to a repo of the workspace by name, so the repo set
    // is an input of every snapshot's facts, and part of its key.
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(&dir.path().join("moved")).root;
    std::fs::rename(root.join("orders"), dir.path().join("orders-away")).unwrap();
    scan(&root);
    assert!(map_ids(&root, "container").contains("ws:container:orders"), "no orders repo to join");
    std::fs::rename(dir.path().join("orders-away"), root.join("orders")).unwrap();
    scan(&root);
    let back = the_map(&root);
    assert!(back.0[2].contains("orders:container:orders"), "{:?}", back.0[2]);
    assert!(!back.0[2].contains("ws:container:orders"), "{:?}", back.0[2]);

    let fresh = fixture(&dir.path().join("fresh")).root;
    scan(&fresh);
    assert_eq!(the_map(&fresh), back, "the same heads and repos give the same map");
}

#[test]
fn deleting_the_branch_a_snapshot_was_read_on_leaves_no_evidence_naming_it() {
    // correctness-13: the branch is no input of a snapshot, so no fact names it; the scan event does.
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    git(&root.join("kit"), &["checkout", "-q", "-b", "feature"]);
    git(&root.join("kit"), &["branch", "-q", "-D", "main"]);
    let env = scan(&root);
    assert_eq!(env["meta"]["seq"], 5, "the same commit: nothing moved");
    for (_, fact) in active_facts(&root) {
        for evidence in fact["evidence"].as_array().unwrap() {
            assert!(evidence.get("ref").is_none_or(Value::is_null), "{fact}");
            assert!(evidence.get("branch").is_none_or(Value::is_null), "{fact}");
        }
    }
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let kit = store.events_after(0, 100).unwrap().into_iter().find_map(|e| match e.body {
        planroom_contract::EventBody::Scan { repo, branch, .. } if repo == "kit" => Some(branch),
        _ => None,
    });
    assert_eq!(kit.as_deref(), Some("main"), "the scan event records the branch it read");
}

#[test]
fn a_scan_that_read_an_older_head_never_moves_the_repo_back() {
    // correctness-14: the first scan pauses between extraction and commit, the seam a slow scan
    // opens, while a commit and a second scan land.
    use planroom::app::scan as app_scan;
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    let slow = app_scan::prepare(&planroom_contract::ScanReq::default(), &root).unwrap();
    let commit = ["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "Newer"];
    git(&root.join("platform"), &commit);
    let newer = scan(&root);
    assert_eq!(newer["meta"]["seq"], 6, "the newer scan moved platform");
    let late = app_scan::commit(slow).unwrap();
    assert_eq!(late.seq, 6, "the late scan wrote nothing");
    let superseded: Vec<&planroom_contract::Warning> =
        late.warnings.iter().filter(|w| w.code == planroom_contract::WarningCode::ScanSuperseded).collect();
    assert_eq!(superseded.len(), 1, "{:?}", late.warnings);
    assert_eq!(superseded[0].details["repo"], "platform");
    assert_eq!(scan_events(&root).last().unwrap(), &(6, "platform".to_owned()), "no revert in the log");
    // The repo stays at HEAD: a rescan finds nothing to move.
    assert_eq!(scan(&root)["meta"]["seq"], 6);
}

/// Make the repo at `dir` one git refuses: a config it can't parse stands in for any per-repo
/// git failure.
fn break_repo(dir: &Path) {
    git(dir.parent().unwrap(), &["init", "-q", "-b", "main", dir.file_name().unwrap().to_str().unwrap()]);
    std::fs::write(dir.join(".git/config"), "[core\nthis is not a config\n").unwrap();
}

#[test]
fn an_unborn_and_an_unreadable_repo_leave_the_rest_of_the_workspace_mapped() {
    // correctness-16, unasked-r2-1, ux-a.
    use std::os::unix::fs::MetadataExt;
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    git(&root, &["init", "-q", "-b", "main", "newrepo"]);
    break_repo(&root.join("broken"));
    // As root, a repo another user owns is refused by git's ownership check as well.
    let as_root = std::fs::metadata(root.join("kit")).unwrap().uid() == 0;
    if as_root {
        let out = Command::new("chown").args(["-R", "nobody", "worker"]).current_dir(&root).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let readable = if as_root { 5 } else { 6 };

    // Before any scan, focus offers to map only what git can read.
    let (focus, _) = envelope(&root, &["focus", "--json"]);
    assert_eq!(focus["result"]["next"]["command"], "scan", "{focus}");
    assert_eq!(focus["result"]["next"]["label"], format!("Map these {readable} repos? About 2 s."));

    let env = scan(&root);
    let unborn = warnings_of(&env, "repo_unborn");
    assert_eq!(unborn.len(), 1, "{env}");
    assert_eq!(unborn[0]["details"], serde_json::json!({"repo": "newrepo", "branch": "main"}));
    assert_eq!(unborn[0]["message"], "newrepo has no commits yet; it maps after its first commit.");
    let skipped = warnings_of(&env, "repo_skipped");
    let reasons: BTreeSet<(String, String)> = skipped
        .iter()
        .map(|w| {
            (w["details"]["repo"].as_str().unwrap().into(), w["details"]["reason"].as_str().unwrap().into())
        })
        .collect();
    let mut want = BTreeSet::from([("broken".to_owned(), "git_failed".to_owned())]);
    if as_root {
        want.insert(("worker".into(), "other_owner".into()));
    }
    assert_eq!(reasons, want, "{env}");
    let broken = skipped.iter().find(|w| w["details"]["repo"] == "broken").unwrap();
    let fix = broken["details"]["fix"].as_str().unwrap();
    assert!(fix.contains(&format!("git -C {} status", root.join("broken").display())), "{fix}");

    let top = map_ids(&root, "");
    assert!(
        !top.contains("newrepo:repo:newrepo"),
        "an unborn repo maps nothing until its first commit: {top:?}"
    );
    assert!(top.contains("orders:repo:orders") && top.contains("ws:container:redis"), "{top:?}");
    assert!(!top.contains("broken:repo:broken"));
    // focus now shows the map, and its next action is no scan.
    let (focus, _) = envelope(&root, &["focus", "--json"]);
    assert!(focus["result"]["empty"].is_null(), "{focus}");
    assert_ne!(focus["result"]["next"]["command"], "scan");

    // After its first commit, the new repo maps its files and the warning goes.
    std::fs::write(root.join("newrepo/README.md"), "# New\n").unwrap();
    git(&root.join("newrepo"), &["add", "README.md"]);
    git(&root.join("newrepo"), &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
    let env = scan(&root);
    assert!(warnings_of(&env, "repo_unborn").is_empty(), "{env}");
    assert!(map_ids(&root, "code").contains("newrepo:file:README.md"));
    assert!(map_ids(&root, "").contains("newrepo:repo:newrepo"));
}

#[test]
fn a_workspace_git_can_read_none_of_offers_no_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(&root).unwrap();
    break_repo(&root.join("broken"));
    let (focus, _) = envelope(&root, &["focus", "--json"]);
    let empty = &focus["result"]["empty"];
    assert_eq!(empty["reason"], "repos_unreadable", "{focus}");
    assert_eq!(empty["next"]["command"], "focus");
    assert!(empty["next"]["label"].as_str().unwrap().contains("broken"), "{empty}");
    // The scan itself answers ok, with the warning, and writes no store.
    let env = scan(&root);
    assert_eq!(warnings_of(&env, "repo_skipped").len(), 1, "{env}");
    assert_eq!(env["result"]["map"]["empty"]["reason"], "repos_unreadable");
    assert!(!root.join(".planroom/store.db").exists());
}
