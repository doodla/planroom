//! Lenses (architecture.md, "Lenses and questions"; DR-15, DR-46): the default lens is the code
//! pack's `runtime`; `--lens all` is about every kind of every loaded pack and lifts every
//! relation; a lens alone takes its level; an unknown lens is refused; every block carries its
//! kind's tone from the pack that declares it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use planroom_contract::{Kind, LiftedEdge, MapView, NodeId};
use planroom_kernel::lens;
use planroom_store::{Shape, Store};
use planroom_testkit::{fixture, repo_root};
use serde_json::Value;

fn json(cwd: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"))
}

fn ok(cwd: &Path, args: &[&str]) -> Value {
    let env = json(cwd, args);
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    env
}

/// The code pack's kinds: each qualified kind's level and tone, as its manifest declares them.
fn code_kinds() -> BTreeMap<String, (String, Option<String>)> {
    let text = std::fs::read_to_string(repo_root().join("packs/code/pack.toml")).unwrap();
    let pack: toml::Value = toml::from_str(&text).unwrap();
    pack["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let id = format!("code.{}", k["id"].as_str().unwrap());
            let tone = k.get("tone").and_then(|t| t.as_str()).map(str::to_owned);
            (id, (k["level"].as_str().unwrap().to_owned(), tone))
        })
        .collect()
}

/// The code pack's default lens and each lens's level.
fn code_lenses() -> (String, BTreeMap<String, String>) {
    let text = std::fs::read_to_string(repo_root().join("packs/code/pack.toml")).unwrap();
    let pack: toml::Value = toml::from_str(&text).unwrap();
    let lenses = pack["lenses"].as_array().unwrap();
    let default =
        lenses.iter().find(|l| l.get("default").and_then(toml::Value::as_bool) == Some(true)).unwrap();
    let levels = lenses
        .iter()
        .map(|l| (l["id"].as_str().unwrap().to_owned(), l["level"].as_str().unwrap().to_owned()))
        .collect();
    (default["id"].as_str().unwrap().to_owned(), levels)
}

/// The yarn each code lens declares.
fn code_lens_yarns() -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(repo_root().join("packs/code/pack.toml")).unwrap();
    let pack: toml::Value = toml::from_str(&text).unwrap();
    pack["lenses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["id"].as_str().unwrap().to_owned(), l["yarn"].as_str().unwrap().to_owned()))
        .collect()
}

/// The kinds of the scanned nodes at each level, read from the store's active snapshots.
fn stored_kinds(root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let snapshot = store.snapshot().unwrap();
    let active = snapshot.active_set().unwrap();
    let mut kinds: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in snapshot.facts(&active.snapshots()).unwrap() {
        if row.shape == Shape::Node {
            let node: Value = serde_json::from_str(&row.body).unwrap();
            kinds.entry(node["level"].as_str().unwrap().to_owned()).or_default().insert(row.kind);
        }
    }
    kinds
}

#[test]
fn each_lens_map_carries_its_own_yarn_and_lens_all_carries_none() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    let declared = code_lens_yarns();
    assert_eq!(declared.len(), 4, "{declared:?}");
    let distinct: BTreeSet<&String> = declared.values().collect();
    assert_eq!(distinct.len(), 4, "the four lenses draw in four yarns: {declared:?}");

    for (lens, yarn) in &declared {
        let env = ok(&root, &["map", "--lens", lens]);
        assert_eq!(env["result"]["yarn"], yarn.as_str(), "{lens}: {env}");
    }
    let all = ok(&root, &["map", "--lens", "all"]);
    assert!(all["result"].get("yarn").is_none(), "{all}");
}

#[test]
fn lens_all_shows_every_loaded_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    let loaded = code_kinds();
    let stored = stored_kinds(&root);
    assert!(stored.contains_key("container"), "{stored:?}");

    for level in ["system", "container", "component", "code"] {
        // Every page of the level, following each `meta.next` (DR-86).
        let mut nodes: Vec<Value> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut args = vec!["map", "--level", level, "--lens", "all", "--detail", "graph"];
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor.as_str()]);
            }
            let env = ok(&root, &args);
            let view = &env["result"];
            assert_eq!(view["lens"], "all", "{env}");
            assert_eq!(view["level"], level);
            nodes.extend(view["nodes"].as_array().unwrap().iter().cloned());
            match env["meta"]["next"].as_str() {
                Some(next) if cursor.as_deref() != Some(next) => cursor = Some(next.to_owned()),
                Some(next) => panic!("{level}: the cursor {next} moves on"),
                None => break,
            }
        }
        let shown: BTreeSet<String> = nodes.iter().map(|n| n["kind"].as_str().unwrap().to_owned()).collect();
        // Every kind the scan holds at this level shows, and every one is a loaded pack's kind.
        for kind in stored.get(level).into_iter().flatten() {
            assert!(shown.contains(kind), "{level}: {kind} shows under --lens all: {shown:?}");
        }
        for node in nodes {
            let kind = node["kind"].as_str().unwrap();
            let (_, tone) = loaded.get(kind).unwrap_or_else(|| panic!("{kind} is a loaded kind"));
            // Each block carries its kind's tone, so surfaces style from the view model (DR-46).
            assert_eq!(node["tone"].as_str(), tone.as_deref(), "{level}: {node}");
        }
    }
    // Every kind at the `container` level the scan holds has a tone.
    let containers = ok(&root, &["map", "--level", "container", "--lens", "all"]);
    for node in containers["result"]["nodes"].as_array().unwrap() {
        assert!(node["tone"].is_string(), "a container's block has a tone: {node}");
    }

    // With no --lens, the map is the default lens's, at the top of the map.
    let (default, levels) = code_lenses();
    let top = ok(&root, &["map"]);
    assert_eq!(top["result"]["lens"], default.as_str(), "{top}");
    assert_eq!(top["result"]["level"], "system");
    let pane = ok(&root, &["focus"]);
    assert_eq!(pane["result"]["focus"]["lens"], default.as_str(), "{pane}");

    // A lens alone takes its own level; with --level, the level asked for.
    for (lens, level) in &levels {
        let alone = ok(&root, &["map", "--lens", lens]);
        assert_eq!(alone["result"]["level"], level.as_str(), "{lens}: {alone}");
        assert_eq!(alone["result"]["lens"], lens.as_str());
        let at = ok(&root, &["map", "--lens", lens, "--level", "system"]);
        assert_eq!(at["result"]["level"], "system", "{lens}: {at}");
    }

    // A lens no loaded pack declares is refused, naming it.
    let unknown = json(&root, &["map", "--lens", "telepathy"]);
    assert_eq!(unknown["ok"], false, "{unknown}");
    assert_eq!(unknown["error"]["code"], "invalid_argument", "{unknown}");
    assert!(unknown["error"]["message"].as_str().unwrap().contains("telepathy"), "{unknown}");

    // `all` is no lens's ID, so no pack may declare a lens under it.
    let pack = tmp.path().join("everything");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("pack.toml"),
        "format = \"planroom.pack/v1\"\nid = \"everything\"\nversion = 1\ntitle = \"Everything\"\n\
         requires = [\"code\"]\n\n[[lenses]]\nid = \"all\"\nlabel = \"All\"\nrelations = [\"code.http\"]\n\
         level = \"container\"\n",
    )
    .unwrap();
    let refused = json(tmp.path(), &["doctor", "--pack", pack.to_str().unwrap()]);
    assert_eq!(refused["error"]["code"], "pack_invalid", "{refused}");
    assert_eq!(refused["error"]["details"]["reference"], "all", "{refused}");
}

/// Every block and lifted edge of `map ARGS --detail graph`, every page of it, in page order.
fn whole_map(root: &Path, args: &[&str]) -> MapView {
    let mut args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    args.extend(["--detail".into(), "graph".into()]);
    let mut whole: Option<MapView> = None;
    let mut cursor: Option<String> = None;
    loop {
        let mut page_args = args.clone();
        if let Some(cursor) = &cursor {
            page_args.extend(["--cursor".into(), cursor.clone()]);
        }
        let page_args: Vec<&str> = page_args.iter().map(String::as_str).collect();
        let env = ok(root, &page_args);
        let page: MapView = serde_json::from_value(env["result"].clone()).unwrap();
        match &mut whole {
            None => whole = Some(page),
            Some(view) => {
                view.nodes.extend(page.nodes);
                view.edges.extend(page.edges);
            }
        }
        match env["meta"]["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => return whole.unwrap(),
        }
    }
}

/// A lifted edge of relation `kind` between two blocks, as a scan would lift it.
fn lifted(kind: &str, from: &NodeId, to: &NodeId) -> LiftedEdge {
    LiftedEdge {
        kind: Kind(kind.into()),
        from: from.clone(),
        to: to.clone(),
        count: 1,
        evidence_sample: vec![],
        children: vec![],
    }
}

/// `id` and every ancestor of it that `view` holds.
fn chain(view: &MapView, id: &NodeId) -> BTreeSet<NodeId> {
    let mut chain = BTreeSet::new();
    let mut at = view.nodes.iter().find(|n| &n.id == id);
    while let Some(node) = at {
        chain.insert(node.id.clone());
        at = node.parent.as_ref().and_then(|p| view.nodes.iter().find(|n| &n.id == p));
    }
    chain
}

/// The IDs of `view`'s blocks of kind `kind`, in the view's order.
fn of_kind(view: &MapView, kind: &str) -> Vec<NodeId> {
    view.nodes.iter().filter(|n| n.kind.0 == kind).map(|n| n.id.clone()).collect()
}

#[test]
fn a_lens_with_kinds_shows_them_and_the_blocks_its_relations_touch_with_their_parent_chains() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);

    // An out-of-tree pack, through the real loader: a lens about datastores that lifts
    // `code.http`, and one about queues, which no component is, that lifts `code.imports`.
    let dir = tmp.path().join("stores");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("pack.toml"),
        "format = \"planroom.pack/v1\"\nid = \"stores\"\nversion = 1\ntitle = \"Stores\"\n\
         requires = [\"code\"]\n\n\
         [[lenses]]\nid = \"stores\"\nlabel = \"Stores\"\nrelations = [\"code.http\"]\n\
         kinds = [\"code.datastore\"]\nlevel = \"container\"\n\n\
         [[lenses]]\nid = \"imports\"\nlabel = \"Imports\"\nrelations = [\"code.imports\"]\n\
         kinds = [\"code.queue\"]\nlevel = \"component\"\n",
    )
    .unwrap();
    let mut sources = planroom::packs::embed::sources();
    sources.push(planroom::packs::read_dir(&dir).unwrap());
    let vocabulary = planroom::packs::resolve(sources).unwrap().vocabulary;
    let lens_named = |id: &str| vocabulary.lenses.iter().find(|l| l.id == id).unwrap();

    // The containers, with two services joined by `code.http`, and a third joined to a queue by
    // `code.db`, which the lens doesn't lift.
    let mut containers = whole_map(&root, &["map", "--level", "container", "--lens", "all"]);
    let stores = of_kind(&containers, "code.datastore");
    let services = of_kind(&containers, "code.service");
    let queues = of_kind(&containers, "code.queue");
    assert!(!stores.is_empty() && services.len() >= 3 && !queues.is_empty(), "{containers:?}");
    containers.edges =
        vec![lifted("code.http", &services[0], &services[1]), lifted("code.db", &services[2], &queues[0])];
    let seen = lens::apply(containers, &vocabulary, Some(lens_named("stores")));
    let ids: Vec<NodeId> = seen.nodes.iter().map(|n| n.id.clone()).collect();
    // Its kinds' blocks come first, then the blocks its relations touch, and nothing else.
    assert_eq!(ids[..stores.len()], stores[..], "{ids:?}");
    let mut want: BTreeSet<NodeId> = stores.iter().cloned().collect();
    want.extend([services[0].clone(), services[1].clone()]);
    assert_eq!(ids.iter().cloned().collect::<BTreeSet<_>>(), want, "{ids:?}");
    let lifted_kinds: Vec<&str> = seen.edges.iter().map(|e| e.kind.0.as_str()).collect();
    assert_eq!(lifted_kinds, ["code.http"]);

    // The components nest, so a block its relations touch shows with its parent chain.
    let mut components = whole_map(&root, &["map", "--level", "component", "--lens", "all"]);
    assert!(of_kind(&components, "code.queue").is_empty());
    let held: BTreeSet<NodeId> = components.nodes.iter().map(|n| n.id.clone()).collect();
    let parent_of =
        |id: &NodeId| components.nodes.iter().find(|n| &n.id == id).and_then(|n| n.parent.clone());
    let deep = held
        .iter()
        .find(|id| parent_of(id).and_then(|p| parent_of(&p)).is_some_and(|g| held.contains(&g)))
        .expect("a component two folders deep")
        .clone();
    let top = held.iter().find(|id| parent_of(id).is_none_or(|p| !held.contains(&p))).unwrap().clone();
    components.edges = vec![lifted("code.imports", &deep, &top)];
    let seen = lens::apply(components.clone(), &vocabulary, Some(lens_named("imports")));
    let shown: BTreeSet<NodeId> = seen.nodes.iter().map(|n| n.id.clone()).collect();
    let mut want = chain(&components, &deep);
    want.extend(chain(&components, &top));
    assert!(want.len() >= 4 && want.len() < held.len(), "{want:?}");
    assert_eq!(shown, want);
}

#[test]
fn a_lens_whose_kinds_is_empty_hides_no_block() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    let (_, levels) = code_lenses();
    let ids = |view: &MapView| -> BTreeSet<NodeId> { view.nodes.iter().map(|n| n.id.clone()).collect() };
    for level in ["system", "container", "component", "code"] {
        let all = ids(&whole_map(&root, &["map", "--level", level, "--lens", "all"]));
        assert!(!all.is_empty(), "{level}");
        for lens in levels.keys() {
            let seen = ids(&whole_map(&root, &["map", "--level", level, "--lens", lens]));
            assert_eq!(seen, all, "{lens} at the {level} level shows every block");
        }
    }
}

#[test]
fn an_unknown_lens_lists_the_loaded_lenses_under_a_declared_details_key() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let env = json(&root, &["map", "--lens", "telepathy"]);
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    let details = env["error"]["details"].as_object().unwrap();
    // Every key it sends is one the error registry declares for the code, as errors.md lists it.
    let declared = planroom_contract::ErrorCode::InvalidArgument.spec().details;
    for key in details.keys() {
        assert!(declared.contains(&key.as_str()), "{key} is declared: {declared:?}");
    }
    let (_, levels) = code_lenses();
    let mut want: Vec<&str> = vec!["all"];
    want.extend(levels.keys().map(String::as_str));
    want.sort_unstable();
    let mut listed: Vec<&str> =
        details["lenses"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    listed.sort_unstable();
    assert_eq!(listed, want);
}

#[test]
fn a_lens_with_kinds_that_leaves_no_block_shows_an_empty_state_whose_next_action_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);

    // An out-of-tree pack, through the real loader, with a lens about queues, which no
    // component is, that lifts `code.imports`.
    let dir = tmp.path().join("queues");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("pack.toml"),
        "format = \"planroom.pack/v1\"\nid = \"queues\"\nversion = 1\ntitle = \"Queues\"\n\
         requires = [\"code\"]\n\n\
         [[lenses]]\nid = \"queues\"\nlabel = \"Queues\"\nrelations = [\"code.imports\"]\n\
         kinds = [\"code.queue\"]\nlevel = \"component\"\n",
    )
    .unwrap();
    let mut sources = planroom::packs::embed::sources();
    sources.push(planroom::packs::read_dir(&dir).unwrap());
    let vocabulary = planroom::packs::resolve(sources).unwrap().vocabulary;

    // The components, with no `code.imports` edge among them, through the lens as `map` reads
    // them: no block is left, and the view says so instead of showing nothing.
    let mut components = whole_map(&root, &["map", "--level", "component", "--lens", "all"]);
    assert!(!components.nodes.is_empty());
    components.edges.clear();
    let seen = planroom::app::map::lens::filter(components, &vocabulary, "queues");
    assert!(seen.nodes.is_empty(), "{seen:?}");
    let empty = seen.empty.expect("a lens that leaves no block has an empty state");
    assert_eq!(empty.reason.0, "lens_shows_nothing");
    let line = empty.line.unwrap();
    assert!(line.contains("queues") && line.contains("component"), "{line}");
    assert_eq!(empty.next.command.0, "map");
    assert_eq!(empty.next.args, ["--level", "component", "--lens", "all"]);
    assert!(!empty.next.label.is_empty());

    // The next action runs through the CLI and shows the blocks at that level.
    let mut words = vec![empty.next.command.0.as_str()];
    words.extend(empty.next.args.iter().map(String::as_str));
    let next = ok(&root, &words);
    assert!(next["result"]["empty"].is_null(), "{next}");
    assert!(!next["result"]["nodes"].as_array().unwrap().is_empty(), "{next}");
}
