//! One namespace per repo (DR-74): a repo's namespace is its directory's name, escaped so every
//! ID it starts is a valid `NodeId`; the later of two repos of one name adds `-` and 4 hex of the
//! sha256 of its path from the workspace root; and `meta.repos` fixes each root's namespace at its
//! first scan, so a repo discovered later never takes another's.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use planroom::app::workspace;
use planroom_contract::NodeId;
use planroom_store::{META_REPOS, Shape, Store};
use planroom_testkit::git_repo;
use serde_json::Value;

/// The test's own data root beside the workspace, so nothing lands in the repos.
fn workspace_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("data-root")).unwrap();
    let ws = dir.path().join("ws");
    fs::create_dir(&ws).unwrap();
    (dir, ws.canonicalize().unwrap())
}

fn repo(dir: &Path) {
    git_repo(
        dir,
        &[
            ("README.md".to_owned(), b"# Repo\n".to_vec()),
            ("src/main.rs".to_owned(), b"fn main() {}\n".to_vec()),
        ],
    );
}

fn scan(ws: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", ".", "--json"])
        .current_dir(ws)
        .env("PLANROOM_DATA", ws.parent().unwrap().join("data-root"))
        .env_remove("PLANROOM_RAW")
        .output()
        .unwrap();
    let env: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    assert_eq!(env["ok"], true, "{env}\n{}", String::from_utf8_lossy(&out.stderr));
    env
}

/// Every node ID of the store's active snapshots.
fn node_ids(ws: &Path) -> BTreeSet<String> {
    let store = Store::open_existing(&ws.join(".planroom/store.db")).unwrap().expect("a store");
    let active = store.active_set().unwrap();
    store
        .facts(&active.snapshots())
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Node)
        .map(|r| r.id)
        .collect()
}

fn namespaces(ws: &Path) -> BTreeSet<String> {
    node_ids(ws).iter().map(|id| id.split(':').next().unwrap().to_owned()).collect()
}

fn suffix(rel: &str) -> String {
    planroom_kernel::keys::sha256_plain_hex(rel.as_bytes())[..4].to_owned()
}

#[test]
fn same_named_repos_get_distinct_namespaces() {
    let (_dir, ws) = workspace_dir();
    repo(&ws.join("team-a/api"));
    repo(&ws.join("team-b/api"));
    scan(&ws);
    let b = format!("api-{}", suffix("team-b/api"));
    assert_eq!(
        namespaces(&ws),
        BTreeSet::from(["api".to_owned(), b.clone()]),
        "the later in path order is suffixed"
    );
    let ids = node_ids(&ws);
    assert!(ids.contains("api:repo:api") && ids.contains(&format!("{b}:repo:{b}")), "{ids:?}");
    assert!(
        ids.contains("api:file:src/main.rs") && ids.contains(&format!("{b}:file:src/main.rs")),
        "two sets of nodes"
    );

    // A repo of the same name discovered later, earlier in path order, never takes a namespace
    // a root already holds: meta.repos fixed them at their first scan.
    repo(&ws.join("team-0/api"));
    scan(&ws);
    let zero = format!("api-{}", suffix("team-0/api"));
    assert_eq!(namespaces(&ws), BTreeSet::from(["api".to_owned(), b.clone(), zero.clone()]));
    let store = Store::open_existing(&ws.join(".planroom/store.db")).unwrap().unwrap();
    let fixed: serde_json::Map<String, Value> =
        serde_json::from_str(&store.meta(META_REPOS).unwrap().expect("meta.repos")).unwrap();
    assert_eq!(fixed["team-a/api"], "api");
    assert_eq!(fixed["team-b/api"], Value::String(b));
    assert_eq!(fixed["team-0/api"], Value::String(zero));

    // A repo named for a namespace Planroom's own IDs use is suffixed too.
    let (_dir, ws) = workspace_dir();
    repo(&ws.join("ws"));
    repo(&ws.join("plan"));
    scan(&ws);
    let names = namespaces(&ws);
    assert_eq!(
        names,
        BTreeSet::from([format!("plan-{}", suffix("plan")), format!("ws-{}", suffix("ws"))]),
        "no repo takes `ws` or `plan`"
    );
}

#[test]
fn a_path_with_a_space_or_colon_gives_a_valid_node_id() {
    let (_dir, ws) = workspace_dir();
    let names = ["my app", "svc:v2", "100%", "tab\there", "nbsp\u{a0}x", "caf\u{e9}"];
    for name in names {
        repo(&ws.join(name));
    }
    scan(&ws);
    // NodeId as the published schema defines it.
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(planroom_testkit::repo_root().join("schema/planroom.v1.schema.json")).unwrap(),
    )
    .unwrap();
    let node_id = jsonschema::draft202012::new(&schema["$defs"]["NodeId"]).unwrap();
    let ids = node_ids(&ws);
    for id in &ids {
        assert!(node_id.is_valid(&Value::String(id.clone())), "{id} breaks NodeId's pattern");
        assert!(!id.split(':').next().unwrap().contains('\u{feff}'), "{id}");
        let parsed: NodeId = id.parse().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&parsed.0, id);
    }
    let spaces: BTreeSet<String> = namespaces(&ws);
    let want: BTreeSet<String> = ["my%20app", "svc%3Av2", "100%25", "tab%09here", "nbsp%C2%A0x", "caf\u{e9}"]
        .map(str::to_owned)
        .into();
    assert_eq!(spaces, want);
    // Each namespace gives its directory's name back.
    let back: BTreeSet<String> =
        spaces.iter().map(|ns| String::from_utf8(workspace::unescape(ns).unwrap()).unwrap()).collect();
    assert_eq!(back, names.map(str::to_owned).into());
    assert!(ids.contains("my%20app:repo:my%20app"), "{ids:?}");
}

#[test]
fn escaping_round_trips_every_name() {
    for name in
        [&b"plain"[..], b"a b", b"a:b", b"%", b"%25", b"\x01ctl", b"bom\xef\xbb\xbf", b"not\xffutf8", b"\xc3"]
    {
        let ns = workspace::escape(name);
        assert!(
            !ns.is_empty() && !ns.contains(':') && !ns.chars().any(|c| c.is_whitespace() || c == '\u{feff}'),
            "{ns}"
        );
        assert_eq!(workspace::unescape(&ns).as_deref(), Some(name), "{ns}");
    }
    assert_eq!(workspace::escape(b"a b:c%"), "a%20b%3Ac%25");
}

#[test]
fn two_scans_that_named_a_repo_set_differently_never_give_two_repos_one_namespace() {
    // One scan reads the workspace while team-b/api is its only `api`; a second reads it after
    // team-a/api joins, earlier in path order, so it names team-a `api` and team-b suffixed. The
    // first commits, fixing team-b as `api`; the second then commits a naming that no longer holds.
    use planroom::app::scan as app_scan;
    let (_dir, ws) = workspace_dir();
    let api = |dir: &Path, file: &str| {
        git_repo(dir, &[("README.md".to_owned(), b"# Api\n".to_vec()), (file.to_owned(), b"x\n".to_vec())])
    };
    api(&ws.join("team-b/api"), "src/b.rs");
    let first = app_scan::prepare(&planroom_contract::ScanReq::default(), &ws).unwrap();
    api(&ws.join("team-a/api"), "src/a.rs");
    let second = app_scan::prepare(&planroom_contract::ScanReq::default(), &ws).unwrap();
    app_scan::commit(first).unwrap();
    let late = app_scan::commit(second).unwrap();

    let a = format!("api-{}", suffix("team-a/api"));
    let b = format!("api-{}", suffix("team-b/api"));
    let superseded: BTreeSet<(String, String)> = late
        .warnings
        .iter()
        .filter(|w| w.code == planroom_contract::WarningCode::ScanSuperseded)
        .map(|w| {
            (w.details["repo"].as_str().unwrap().to_owned(), w.details["reason"].as_str().unwrap().to_owned())
        })
        .collect();
    assert_eq!(
        superseded,
        BTreeSet::from([
            ("api".to_owned(), "repos_changed".to_owned()),
            (b.clone(), "repos_changed".to_owned())
        ]),
        "both of the late scan's names are stale: {:?}",
        late.warnings
    );
    let store = Store::open_existing(&ws.join(".planroom/store.db")).unwrap().unwrap();
    let fixed: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&store.meta(META_REPOS).unwrap().expect("meta.repos")).unwrap();
    drop(store);
    assert_eq!(
        fixed,
        [("team-b/api".to_owned(), "api".to_owned())].into(),
        "only the first scan's naming is fixed"
    );
    let ids = node_ids(&ws);
    assert!(ids.contains("api:file:src/b.rs") && !ids.contains("api:file:src/a.rs"), "{ids:?}");
    assert!(!ids.iter().any(|id| id.starts_with(&format!("{b}:"))), "team-b is mapped once: {ids:?}");

    // The next scan gives team-a its own namespace, and the two never share one.
    scan(&ws);
    assert_eq!(namespaces(&ws), BTreeSet::from(["api".to_owned(), a.clone()]));
    let ids = node_ids(&ws);
    assert!(ids.contains("api:file:src/b.rs") && ids.contains(&format!("{a}:file:src/a.rs")), "{ids:?}");
    assert!(!ids.contains("api:file:src/a.rs"), "{ids:?}");
    let mut discovered: Vec<(String, PathBuf)> =
        workspace::discover(&ws).unwrap().repos.into_iter().map(|r| (r.name, r.path)).collect();
    discovered.sort();
    let mut want = vec![("api".to_owned(), ws.join("team-b/api")), (a, ws.join("team-a/api"))];
    want.sort();
    assert_eq!(discovered, want);
}
