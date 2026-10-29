//! The workspace link snapshot: one per active set, built in the scan's transaction from the
//! member snapshots' stored facts, and keyed by them and the packs hash (DR-12, DR-86).
//!
//! Every process runs with this test process's own data root, which `cargo xtask test` sets, and
//! the tests read scan events and the link row rather than counting `seq`s.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use planroom_contract::{Edge, EventBody, Node, Provenance, Ref, Tier};
use planroom_extract::link::WorkspaceFacts;
use planroom_store::{Shape, Store};
use planroom_testkit::{fixture, git_repo};

fn scan(root: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "--json"])
        .current_dir(root)
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

fn store(root: &Path) -> Store {
    Store::open_existing(&root.join(".planroom/store.db")).unwrap().expect("a store")
}

/// Each repo's last scan event, by repo: its event ID and the snapshot it made active.
fn last_scans(store: &Store) -> BTreeMap<String, (String, u64)> {
    let mut last = BTreeMap::new();
    for event in store.events_after(0, 100_000).unwrap() {
        if let EventBody::Scan { repo, snapshot, .. } = event.body {
            last.insert(repo, (event.id.0, snapshot));
        }
    }
    last
}

/// The link the code pack's linkers make of the active set's member snapshots, as stored: what
/// the linkers read, selected as the scan selects it, every loaded linker rerun over the store.
fn relink(store: &Store) -> Vec<planroom_extract::link::Linked> {
    let active = store.active_set().unwrap();
    let mut members: Vec<u64> = active.repos.values().copied().collect();
    members.sort_unstable();
    let packs = planroom::packs::builtins().unwrap();
    let rows = store.facts(&members).unwrap();
    let facts: WorkspaceFacts = packs
        .link_selection()
        .facts(active.repos.keys().cloned(), rows.iter().map(planroom::packs::link_member))
        .unwrap();
    let runs = packs.linker_runs();
    let builtins: Vec<&str> = runs.iter().map(|r| r.builtin.as_str()).collect();
    assert_eq!(builtins, ["runtime_wiring", "imports"], "the code pack's linkers");
    runs.iter().map(|run| run.link(&facts).unwrap()).collect()
}

#[test]
fn the_link_snapshot_matches_the_active_set() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    let store = store(&root);
    let active = store.active_set().unwrap();
    let link = active.link.expect("the last scan event names a link");

    // Its row: the workspace's, keyed by the sorted active snapshot IDs and the packs hash.
    let key = store.snapshot_key(link).unwrap().expect("the link's row");
    let packs_hash = store.meta("packs_hash").unwrap().expect("the packs hash");
    let mut members: Vec<u64> = active.repos.values().copied().collect();
    members.sort_unstable();
    assert_eq!((key.repo.as_str(), key.purpose.as_str()), ("ws", "link"));
    assert_eq!(key.commit_id, planroom_kernel::keys::link_key(&members, &packs_hash));
    assert_eq!(key.packs_hash, packs_hash);
    for (repo, snapshot) in &active.repos {
        let head = store.snapshot_key(*snapshot).unwrap().unwrap();
        assert_eq!((head.repo.as_str(), head.purpose.as_str()), (repo.as_str(), "head"));
    }

    // Its facts: every linker rerun over the stored member facts, fact for fact.
    let linked = relink(&store);
    for one in &linked {
        assert!(one.unresolved.is_empty(), "{:?}", one.unresolved);
    }
    let nodes: Vec<&Node> = linked.iter().flat_map(|l| &l.batch.nodes).collect();
    let edges: Vec<&Edge> = linked.iter().flat_map(|l| &l.batch.edges).collect();
    let mut want: Vec<(String, String)> = nodes
        .iter()
        .map(|n| (n.id.0.clone(), serde_json::to_string(n).unwrap()))
        .chain(edges.iter().map(|e| (e.id.0.clone(), serde_json::to_string(e).unwrap())))
        .collect();
    want.sort();
    let got: Vec<(String, String)> =
        store.facts(&[link]).unwrap().into_iter().map(|r| (r.id, r.body)).collect();
    assert_eq!(got, want);
    let kinds: std::collections::BTreeSet<&str> = edges.iter().map(|e| e.kind.0.as_str()).collect();
    for kind in ["code.db", "code.queue_publish", "code.queue_consume", "code.http", "code.imports"] {
        assert!(kinds.contains(kind), "{kind}: {kinds:?}");
    }
    // storefront's call to orders, whose base its constructor takes and whose host a variable
    // read in another file of its container names.
    assert!(
        edges.iter().any(|e| {
            e.kind.0 == "code.http"
                && e.from == Ref::Id(planroom_contract::NodeId("storefront:container:storefront".into()))
                && e.via.as_deref() == Some("POST /orders")
        }),
        "{edges:#?}"
    );

    // No head snapshot holds a node the link mints at a tier that outranks it: a repo's root
    // manifest gives its container at the universal tier, which the link's kind and role win
    // (DR-42). A shared edge ID is how a linked edge adds its evidence to an extracted one.
    let minted: BTreeMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), *n)).collect();
    for row in store.facts(&members).unwrap() {
        if row.shape != Shape::Node || !minted.contains_key(row.id.as_str()) {
            continue;
        }
        let node: Node = serde_json::from_str(&row.body).unwrap();
        assert!(
            matches!(node.provenance, Provenance::Extracted { tier: Tier::Universal, .. }),
            "a head snapshot holds the link's {} above the universal tier: {node:?}",
            row.id
        );
    }
}

#[test]
fn adding_a_repo_rekeys_only_the_link_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    scan(&root);
    let before = {
        let store = store(&root);
        (last_scans(&store), store.active_set().unwrap())
    };
    assert_eq!(before.0.len(), 5);

    git_repo(&root.join("redis"), &[("README.md".into(), b"# Redis\n".to_vec())]);
    scan(&root);
    let store = store(&root);
    let after = last_scans(&store);
    let active = store.active_set().unwrap();
    // The five repos keep their snapshots and their scan events; the new repo has one event.
    for (repo, last) in &before.0 {
        assert_eq!(after.get(repo), Some(last), "{repo} keeps its snapshot and its scan event");
        assert_eq!(active.repos.get(repo), before.1.repos.get(repo));
    }
    let new: Vec<&String> = after.keys().filter(|r| !before.0.contains_key(*r)).collect();
    assert_eq!(new, ["redis"]);
    let scans = store
        .events_after(0, 100_000)
        .unwrap()
        .into_iter()
        .filter(|e| matches!(e.body, EventBody::Scan { .. }))
        .count();
    assert_eq!(scans, 6, "one new scan event");
    // The link is new, and joins the redis service to the new repo.
    let link = active.link.unwrap();
    assert_ne!(Some(link), before.1.link);
    let facts = store.facts(&[link]).unwrap();
    assert!(facts.iter().any(|r| r.id == "redis:container:redis"));
    assert!(!facts.iter().any(|r| r.id == "ws:container:redis"));
}

/// Commit every change in the repo at `dir`.
fn commit_all(dir: &Path, message: &str) {
    for args in [&["add", "-A"][..], &["-c", "commit.gpgsign=false", "commit", "-q", "-m", message]] {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.test")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.test")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
}

/// A Rust unit test module inside a runtime file, the language's usual layout: its queue push
/// links nothing, and its call of the runtime function is never cited as that function's caller,
/// while the runtime push above it links as before.
#[test]
fn an_inline_rust_test_module_never_anchors_or_cites_a_join() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let lib = "\
pub fn drain(conn: &mut redis::Connection) {
    let _: () = conn.lpush(\"runtime-jobs\", 1).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes() {
        let mut conn = connect();
        drain(&mut conn);
        let _: () = conn.lpush(\"probe-jobs\", 1).unwrap();
    }
}
";
    std::fs::create_dir_all(root.join("worker/src")).unwrap();
    std::fs::write(root.join("worker/src/lib.rs"), lib).unwrap();
    commit_all(&root.join("worker"), "A Rust drain with its unit tests");
    scan(&root);
    let store = store(&root);
    let link = store.active_set().unwrap().link.unwrap();
    let edges: Vec<planroom_contract::Edge> = store
        .facts(&[link])
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Edge)
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    let vias: Vec<&str> = edges.iter().filter_map(|e| e.via.as_deref()).collect();
    assert!(vias.contains(&"runtime-jobs"), "the runtime push links: {vias:?}");
    assert!(!vias.contains(&"probe-jobs"), "the test's push links nothing: {vias:?}");
    for edge in &edges {
        for e in &edge.evidence {
            if let planroom_contract::Evidence::Line { path, line, .. } = e {
                assert!(
                    !(path == "worker/src/lib.rs" && (5..=15).contains(line)),
                    "{} {:?} cites the test module's {path}:{line}",
                    edge.kind.0,
                    edge.via
                );
            }
        }
    }
}

/// Rust's test modules in files of their own and its test functions: `#[cfg(test)] mod test;`,
/// a `#[path]` test module, a file below the test module's directory, and functions `#[test]` or
/// `#[tokio::test]` marks with no `cfg(test)` around them. None of their pushes links, and none is
/// cited, while the runtime pushes beside them, a runtime module among them, link as before.
#[test]
fn rust_test_modules_and_test_functions_never_anchor_a_join() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let lib = "\
pub mod jobs;

pub fn drain(c: &mut redis::Connection) {
    let _: () = c.lpush(\"runtime-jobs\", 1).unwrap();
}

#[test]
fn bare_test() {
    let _: () = connect().lpush(\"probe-bare-test\", 1).unwrap();
}

#[tokio::test]
async fn tokio_test() {
    let _: () = connect().lpush(\"probe-tokio-test\", 1).unwrap();
}

#[cfg(test)]
mod test;

#[cfg(test)]
#[path = \"lib_tests.rs\"]
mod lib_tests;
";
    let push =
        |key: &str| format!("fn t(c: &mut C) {{\n    let _: () = c.lpush(\"{key}\", 1).unwrap();\n}}\n");
    let files = [
        ("worker/src/lib.rs", lib.to_owned()),
        ("worker/src/jobs.rs", push("runtime-module-jobs")),
        ("worker/src/test.rs", format!("mod helpers;\n{}", push("probe-testrs"))),
        ("worker/src/test/helpers.rs", push("probe-helpers")),
        ("worker/src/lib_tests.rs", push("probe-pathfile")),
    ];
    for (path, text) in &files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    commit_all(&root.join("worker"), "A Rust drain with its tests");
    scan(&root);
    let store = store(&root);
    let link = store.active_set().unwrap().link.unwrap();
    let edges: Vec<planroom_contract::Edge> = store
        .facts(&[link])
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Edge)
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    let vias: Vec<&str> = edges.iter().filter_map(|e| e.via.as_deref()).collect();
    for runtime in ["runtime-jobs", "runtime-module-jobs"] {
        assert!(vias.contains(&runtime), "{runtime} links: {vias:?}");
    }
    let probes: Vec<&&str> = vias.iter().filter(|v| v.starts_with("probe-")).collect();
    assert!(probes.is_empty(), "test code links nothing: {probes:?}");
    for edge in &edges {
        for e in &edge.evidence {
            if let planroom_contract::Evidence::Line { path, line, .. } = e {
                let test_line = path == "worker/src/lib.rs" && (7..=23).contains(line);
                let test_file =
                    ["worker/src/test.rs", "worker/src/test/helpers.rs", "worker/src/lib_tests.rs"]
                        .contains(&path.as_str());
                assert!(!test_line && !test_file, "{} {:?} cites {path}:{line}", edge.kind.0, edge.via);
            }
        }
    }
}

/// The scan's reply, as JSON.
fn scan_reply(root: &Path) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "--json"])
        .current_dir(root)
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A second compose service built from the orders repo gives it two containers, so its database
/// open and its queue push could each join either: no edge, and one `unresolved_ref` from the
/// scan that builds the link, never from a rescan that finds the link already built.
#[test]
fn a_join_two_containers_fit_warns_once_from_the_scan_that_links() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let compose = root.join("platform/docker-compose.yml");
    let text = std::fs::read_to_string(&compose).unwrap();
    std::fs::write(
        &compose,
        text.replacen("services:\n", "services:\n  orders-admin:\n    build: ../orders\n", 1),
    )
    .unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root.join("platform"))
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.test")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.test")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["-c", "commit.gpgsign=false", "commit", "-q", "-am", "An admin service built from orders"]);
    let warned = |reply: &serde_json::Value| -> Vec<serde_json::Value> {
        reply["meta"]["warnings"]
            .as_array()
            .map(|w| w.iter().filter(|w| w["code"] == "unresolved_ref").cloned().collect())
            .unwrap_or_default()
    };
    let first = scan_reply(&root);
    let [warning] = &warned(&first)[..] else { panic!("one unresolved_ref: {first}") };
    assert_eq!(warning["details"]["count"], 2, "the database open and the queue push: {warning}");
    for item in warning["details"]["items"].as_array().unwrap() {
        assert_eq!(item["linker"], "code/runtime_wiring");
        assert_eq!(
            item["candidates"],
            serde_json::json!(["orders:container:orders", "orders:container:orders-admin"])
        );
        assert_eq!(item["candidate_count"], 2);
    }
    let store = store(&root);
    let link = store.active_set().unwrap().link.unwrap();
    let from_orders = store
        .facts(&[link])
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Edge && r.body.contains("\"from\":{\"id\":\"orders:"))
        .count();
    assert_eq!(from_orders, 0, "no edge from either orders container");
    drop(store);
    assert!(warned(&scan_reply(&root)).is_empty(), "a rescan builds no link, so it warns of nothing");
}
