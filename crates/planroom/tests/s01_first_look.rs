//! S1, first look: a cold scan of the fixture workspace, and the map it gives.
//!
//! A test here that asserts a wall-clock budget is listed in `crates/xtask/nextest.toml`, so
//! nextest runs it with no other test beside it, and takes [`SERIAL`], so `cargo test`, which runs
//! this file's tests as threads of one process, never times it beside another.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use planroom_contract::{EventBody, Level};
use planroom_store::Store;
use planroom_testkit::truth::{self, Cli, Report, ScanRead, Scope};
use planroom_testkit::{fixture, fixture_with_packs};
use serde_json::Value;

/// Held by every test in this file, so a timed one runs alone under `cargo test` too.
static SERIAL: Mutex<()> = Mutex::new(());

/// `planroom ARGS --json` in `cwd` with the data root `data`: the one envelope, which is `ok`.
fn planroom(cwd: &Path, data: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", data)
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap();
    assert!(out.status.success() && env["ok"] == true, "{args:?}: {env}");
    env
}

/// The first scan of a fresh copy of the fixture, with no store and an empty data root, takes
/// under 5 s from the command's start to its reply, and maps the five repos.
#[test]
fn cold_scan_under_5_s() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    let data = tmp.path().join("data");
    let fx = fixture(&root);
    let dir = fx.root.display().to_string();
    let started = Instant::now();
    let env = planroom(&fx.root, &data, &["scan", &dir]);
    let took = started.elapsed();
    assert!(took < Duration::from_secs(5), "the cold scan took {took:?}");
    let nodes = env["result"]["map"]["nodes"].as_array().unwrap();
    let repos = nodes.iter().filter(|n| n["level"] == "system").count();
    assert_eq!(repos, 5, "the five repos at the top: {}", env["result"]["map"]);
}

/// A scanned copy of the fixture workspace as every CI path scans it, with the Ruby pack its config
/// names (DR-13): the temporary directory that holds the copy and its data root.
struct Scanned {
    _dir: tempfile::TempDir,
    root: PathBuf,
    data: PathBuf,
}

impl Scanned {
    fn new() -> Scanned {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let ruby = planroom_testkit::repo_root().join("examples/packs/ruby");
        let root = fixture_with_packs(&dir.path().join("shop"), &data, &[&ruby]).root;
        planroom(&root, &data, &["scan", &root.display().to_string()]);
        Scanned { _dir: dir, root, data }
    }

    /// How the truth comparator runs the engine on this copy.
    fn cli(&self) -> Cli<'_> {
        Cli { bin: Path::new(env!("CARGO_BIN_EXE_planroom")), cwd: &self.root, data: &self.data }
    }

    /// The scan events: the commit each read and the branch HEAD named. No read leaf shows them
    /// yet, so they come from the store, as `scan.rs` reads its facts.
    fn scans(&self) -> Vec<ScanRead> {
        let store = Store::open_existing(&self.root.join(".planroom/store.db")).unwrap().expect("a store");
        store
            .events_after(0, 10_000)
            .unwrap()
            .into_iter()
            .filter_map(|e| match e.body {
                EventBody::Scan { repo, commit, branch, .. } => Some(ScanRead { repo, branch, commit }),
                _ => None,
            })
            .collect()
    }

    /// The truth comparison of this copy's whole map, within the vocabulary the views read.
    fn report(&self) -> Report {
        let observed = truth::read(self.cli(), self.scans());
        let scope = Scope::loaded(planroom::packs::vocabulary().expect("the built-in packs load"));
        truth::compare(&planroom_testkit::truth(), &observed, &scope)
    }
}

/// The blocks and the lifted edges of every page, joined.
fn joined(pages: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for page in pages {
        nodes.extend(page["result"]["nodes"].as_array().unwrap().iter().cloned());
        edges.extend(page["result"]["edges"].as_array().unwrap().iter().cloned());
    }
    (nodes, edges)
}

/// Zooming from the containers to the code under one block and back gives the same container
/// view, and the roll-up is faithful: at each level below a block, the zoom read through every
/// page shows exactly the blocks under it at that level that the whole map holds, as many as its
/// counts say, nothing from outside it, and each lifted edge counts the leaf edges it carries.
#[test]
fn zoom_to_code_and_back_is_faithful() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let scanned = Scanned::new();
    let cli = scanned.cli();
    let observed = truth::read(cli, Vec::new());
    let parents: BTreeMap<&str, Option<&str>> =
        observed.nodes.iter().map(|n| (n.id.as_str(), n.parent.as_deref())).collect();
    let under = |id: &str, focus: &str| {
        let mut at = Some(id);
        for _ in 0..100 {
            match at {
                Some(node) if node == focus => return true,
                Some(node) => at = parents.get(node).copied().flatten(),
                None => return false,
            }
        }
        panic!("{id}'s parents end");
    };

    let containers = |detail: &str| cli.pages(&["map", "--level", "container", "--detail", detail]);
    let before = [containers("summary"), containers("graph")];

    // S1's zoom, then the same from the repo that holds that container.
    let mut zoomed = 0;
    for focus in ["storefront:container:storefront", "storefront:repo:storefront"] {
        let block =
            observed.nodes.iter().find(|n| n.id == focus).unwrap_or_else(|| panic!("{focus} is mapped"));
        let (level, below): (&str, &[&str]) = match block.level {
            Level::System => ("system", &["container", "component", "code"]),
            _ => ("container", &["component", "code"]),
        };
        let (shown, _) = joined(&cli.pages(&["map", "--level", level, "--detail", "graph"]));
        let counts = &shown.iter().find(|n| n["id"] == focus).unwrap()["counts"];
        for &below in below {
            let pages = cli.pages(&["map", "--level", below, "--focus", focus, "--detail", "graph"]);
            let (nodes, edges) = joined(&pages);
            // A level's view also keeps a shallower block with nothing at that level above it,
            // such as a file at a repo's root, so the counts are of the blocks at the level.
            let at: BTreeSet<&str> =
                nodes.iter().filter(|n| n["level"] == below).map(|n| n["id"].as_str().unwrap()).collect();
            assert_eq!(
                at.len() as u64,
                counts[below].as_u64().unwrap_or(0),
                "the {below} blocks under {focus}, through {} pages, are as many as its counts say",
                pages.len()
            );
            let whole: BTreeSet<&str> = observed
                .nodes
                .iter()
                .filter(|n| {
                    serde_json::to_value(n.level).unwrap() == below && n.id != focus && under(&n.id, focus)
                })
                .map(|n| n.id.as_str())
                .collect();
            assert_eq!(at, whole, "the zoom shows every {below} block under {focus} the whole map holds");
            for node in &nodes {
                let id = node["id"].as_str().unwrap();
                assert!(id != focus && under(id, focus), "{id} sits under {focus}");
            }
            for edge in &edges {
                assert_eq!(
                    edge["count"].as_u64().unwrap() as usize,
                    edge["children"].as_array().unwrap().len(),
                    "a lifted edge counts its leaf edges: {edge}"
                );
            }
            zoomed += nodes.len();
        }
    }
    assert!(zoomed > 0, "a zoom shows blocks");

    let after = [containers("summary"), containers("graph")];
    let results = |pages: &[Value]| pages.iter().map(|p| p["result"].clone()).collect::<Vec<_>>();
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(
            results(before),
            results(after),
            "back at the containers, the view is the one before the zoom"
        );
    }
}

/// The recall floor: the truth checks the comparator found when it was last raised.
const RECALL_FLOOR: &str = "tests/s01_recall_floor.json";

/// The comparator's recall never falls below the committed floor: each check the floor names
/// still holds. A change that raises recall raises the floor with it; nothing lowers it (DR-89).
#[test]
fn truth_recall_never_drops() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let report = Scanned::new().report();
    let recall = report.recall();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(RECALL_FLOOR);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{RECALL_FLOOR} reads ({e}); today's recall: {}",
            serde_json::to_string_pretty(&recall).unwrap()
        )
    });
    let floor: Value = serde_json::from_str(&text).unwrap();
    let floor: BTreeSet<String> =
        floor["recall"].as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_owned()).collect();
    assert!(!floor.is_empty(), "the floor names what the comparator finds");
    let dropped: Vec<&String> = floor.difference(&recall).collect();
    assert!(dropped.is_empty(), "recall dropped below the floor: {dropped:?}\n{}", report.describe());
    // A change that raises recall raises the floor in the same change, so what it gained is
    // held from then on.
    let raised: Vec<&String> = recall.difference(&floor).collect();
    assert!(raised.is_empty(), "recall rose above the floor; add these to {RECALL_FLOOR}: {raised:?}");
}

impl Scanned {
    /// Every node of the active set, parsed.
    fn nodes(&self) -> Vec<Value> {
        let store = Store::open_existing(&self.root.join(".planroom/store.db")).unwrap().expect("a store");
        store
            .facts(&store.active_set().unwrap().snapshots())
            .unwrap()
            .into_iter()
            .filter(|r| r.shape == planroom_store::Shape::Node)
            .map(|r| serde_json::from_str(&r.body).unwrap())
            .collect()
    }
}

/// `find` finds every truth entity in its one reply, untruncated: each of the 11 containers by its
/// name, and each of the 24 components by its repo and its whole path, since a last segment
/// repeats (`api`) or is a common word.
#[test]
fn search_finds_every_truth_entity() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let scanned = Scanned::new();
    let truth = planroom_testkit::truth();
    let nodes = scanned.nodes();
    let find = |query: &str, expected: &str| {
        let env = planroom(&scanned.root, &scanned.data, &["find", query]);
        let warnings = env["meta"]["warnings"].as_array().cloned().unwrap_or_default();
        assert!(!warnings.iter().any(|w| w["code"] == "truncated"), "find {query}: truncated: {env}");
        let ids: Vec<&str> =
            env["result"]["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&expected), "find {query}: {expected} not among {ids:?}");
    };
    let containers = truth["containers"].as_array().unwrap();
    assert_eq!(containers.len(), 11);
    for container in containers {
        let name = container["name"].as_str().unwrap();
        let node = nodes
            .iter()
            .find(|n| n["level"] == "container" && n["name"] == name)
            .unwrap_or_else(|| panic!("the container {name} is mapped"));
        find(name, node["id"].as_str().unwrap());
    }
    let components = truth["components"].as_array().unwrap();
    assert_eq!(components.len(), 24);
    for component in components {
        let path = component["path"].as_str().unwrap();
        let (repo, rest) = path.split_once('/').unwrap();
        let node = nodes
            .iter()
            .find(|n| {
                let id = n["id"].as_str().unwrap();
                n["level"] != "system"
                    && id.starts_with(&format!("{repo}:"))
                    && id
                        .split_once(':')
                        .and_then(|(_, tail)| tail.split_once(':'))
                        .is_some_and(|(_, key)| key == rest)
            })
            .unwrap_or_else(|| panic!("the component {path} is mapped"));
        // Its repo and its path in the repo, as two words: `find` matches each word, fuzzily, in a
        // block's name or ID, and an ID holds both.
        find(&format!("{repo} {rest}"), node["id"].as_str().unwrap());
    }
    // The flags wait on the flags pack; until then `find` answers.
    planroom(&scanned.root, &scanned.data, &["find", "gift"]);
}

/// Each repo's languages hold the truth's, `ruby` among `platform`'s through the Ruby pack.
#[test]
fn repo_languages_match_truth() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let scanned = Scanned::new();
    let truth = planroom_testkit::truth();
    let nodes = scanned.nodes();
    for (repo, want) in truth["repos"].as_object().unwrap() {
        let node = nodes
            .iter()
            .find(|n| n["level"] == "system" && n["id"] == format!("{repo}:repo:{repo}"))
            .unwrap_or_else(|| panic!("the repo {repo} is mapped"));
        let have: BTreeSet<&str> =
            node["props"]["languages"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
        for language in want["languages"].as_array().unwrap() {
            assert!(have.contains(language.as_str().unwrap()), "{repo}: {language} not in {have:?}");
        }
    }
}
