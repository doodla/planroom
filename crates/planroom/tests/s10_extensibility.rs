//! S10 Extensibility (KICKOFF.md; architecture.md, "Lenses and questions"; DR-15): `ask` alone is
//! the catalogue of what is loaded, so a pack added after the build is still found.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use planroom::app::ask;
use planroom::packs::{self, Origin, Source};
use planroom_contract::{AskResult, Cursor};
use serde_json::Value;

/// A `--json` reply's envelope, which must be `ok`, on one line, within the leaf's budget.
fn ok(cwd: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .env_remove("PLANROOM_RAW")
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"));
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 1_500, "{args:?}: {env}");
    env
}

/// Every page of the listing `args` gives, following `meta.next`.
fn pages(cwd: &Path, args: &[&str]) -> Vec<Value> {
    let mut out = vec![ok(cwd, args)];
    while let Some(next) = out.last().unwrap()["meta"]["next"].as_str().map(str::to_owned) {
        let mut paged: Vec<&str> = args.to_vec();
        paged.extend(["--cursor", &next]);
        out.push(ok(cwd, &paged));
        assert!(out.len() < 50, "the listing never ends");
    }
    out
}

/// Each item's ID across `pages`, with its item.
fn items(pages: &[Value]) -> BTreeMap<String, Value> {
    let mut all = BTreeMap::new();
    for page in pages {
        for item in page["result"]["items"].as_array().unwrap() {
            let id = item["id"].as_str().unwrap().to_owned();
            assert!(all.insert(id.clone(), item.clone()).is_none(), "{id} listed twice");
        }
    }
    all
}

/// What the loaded set declares: each level of its ladder, each kind, qualified, each lens and
/// each question, with the built-in questions this build serves: `kinds`, and `stage` while a
/// pack declares stages.
fn declared(loaded: &packs::Loaded) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = BTreeSet::from(["kinds".to_owned()]);
    ids.extend(loaded.vocabulary.levels.iter().map(|l| l.id.as_str().to_owned()));
    if has_stages(loaded) {
        ids.insert("stage".to_owned());
    }
    for pack in &loaded.packs {
        ids.extend(pack.file.kinds.iter().map(|k| packs::qualify(pack.id(), &k.id)));
        ids.extend(pack.file.lenses.iter().map(|l| l.id.clone()));
        ids.extend(pack.file.questions.iter().map(|q| q.id.clone()));
    }
    ids
}

fn has_stages(loaded: &packs::Loaded) -> bool {
    loaded.packs.iter().any(|p| !p.file.stages.is_empty())
}

#[test]
fn ask_lists_what_is_loaded() {
    // Outside any workspace: the listing reads the packs, not a map.
    let tmp = tempfile::tempdir().unwrap();
    let loaded = packs::builtins().unwrap();
    let listed = items(&pages(tmp.path(), &["ask"]));
    let ids: BTreeSet<String> = listed.keys().cloned().collect();
    assert_eq!(
        ids,
        declared(&loaded),
        "ask alone lists every loaded level, kind, lens and question, and nothing else"
    );
    // The code pack's questions, each with its arguments, its summary and its pack.
    for (id, usage) in [("path", "path A B"), ("why", "why A B"), ("impact", "impact X")] {
        let item = &listed[id];
        assert_eq!(item["name"], usage, "{item}");
        assert!(item["why"].as_str().unwrap().ends_with("from the code pack."), "{item}");
    }
    assert!(listed["path"]["why"].as_str().unwrap().starts_with("How A reaches B"), "{}", listed["path"]);
    // The built-in questions are marked, `stage` with the plan pack; the ones whose packs or checks
    // aren't loaded aren't listed.
    assert!(listed["kinds"]["why"].as_str().unwrap().starts_with("Built in"));
    assert!(listed["stage"]["why"].as_str().unwrap().starts_with("Built in"));
    for unserved in ["lint", "missing", "drift"] {
        assert!(!listed.contains_key(unserved), "{unserved} isn't loaded");
    }
    // A lens's handle shows the map through it, and a kind's its level.
    assert_eq!(listed["flow"]["handles"][0]["args"], serde_json::json!(["--lens", "flow"]));
    assert_eq!(listed["code.service"]["handles"][0]["args"], serde_json::json!(["--level", "container"]));
    // ask kinds gives the same listing.
    assert_eq!(items(&pages(tmp.path(), &["ask", "kinds"])), listed);

    // A pack added after the build is found the same way: its kinds, lens and question, however
    // many and however long their labels, page under the leaf's budget with each listed once.
    let mut body = String::new();
    for i in 0..60 {
        body.push_str(&format!(
            "[[kinds]]\nid = \"widget_{i:02}\"\nid_prefix = \"widget{i:02}\"\nlevel = \"component\"\nlabel = \"{}\"\n\n",
            "W".repeat(400)
        ));
    }
    body.push_str(
        "[[lenses]]\nid = \"widgets\"\nlabel = \"Widgets\"\nrelations = [\"code.imports\"]\nlevel = \"component\"\n\n\
         [[questions]]\nid = \"widget-users\"\nsummary = \"What uses {W}\"\nargs = [{ name = \"W\" }]\n\n\
         [[questions.steps]]\nshape = \"neighbors\"\nparams = { ids = [\"$W\"], lens = \"widgets\", direction = \"backward\" }\n",
    );
    let manifest = format!(
        "format = \"planroom.pack/v1\"\nid = \"widgets\"\nversion = 1\ntitle = \"Widgets\"\nrequires = [\"code\"]\n\n{body}"
    );
    let mut sources = packs::embed::sources();
    sources.push(Source {
        origin: Origin::OutOfTree,
        label: "mem/widgets".into(),
        dir: None,
        recorded: None,
        files: BTreeMap::from([("pack.toml".to_owned(), manifest.into_bytes())]),
    });
    let wider = packs::resolve(sources).unwrap();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cursor: Option<Cursor> = None;
    let mut count = 0;
    loop {
        let page = ask::listing(&wider, cursor.as_ref()).unwrap();
        let result: &AskResult = &page.reply.result;
        let bytes = serde_json::to_string(result).unwrap().len()
            + result.summary.len()
            + serde_json::to_string(&page.reply.warnings).unwrap().len();
        assert!(bytes.div_ceil(4) + 60 <= 1_500, "a page of {bytes} bytes");
        assert!(!result.items.is_empty());
        for item in &result.items {
            assert!(seen.insert(item.id.clone()), "{} listed twice", item.id);
            assert!(item.name.chars().count() <= 41, "{}", item.name);
        }
        count += 1;
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert!(count > 1, "60 long-labelled kinds take more than one page");
    assert_eq!(seen, declared(&wider));
    assert!(seen.contains("widget-users") && seen.contains("widgets") && seen.contains("widgets.widget_00"));
}

/// Every page of the listing over `loaded`, each checked against the leaf's budget as the CLI
/// measures it: the result, the summary the envelope repeats, the warnings and the cursor.
/// Every page of the listing, each within its budget, and how many entries the pages left out.
fn listed_pages(loaded: &packs::Loaded) -> (Vec<AskResult>, u64) {
    let mut out = Vec::new();
    let mut left_out = 0;
    let mut cursor: Option<Cursor> = None;
    loop {
        let page = ask::listing(loaded, cursor.as_ref()).unwrap();
        let result = page.reply.result.clone();
        let bytes = serde_json::to_string(&result).unwrap().len()
            + result.summary.len()
            + serde_json::to_string(&page.reply.warnings).unwrap().len()
            + page.next.as_ref().map_or(0, |c| c.0.len());
        assert!(bytes.div_ceil(4) + 60 <= 1_500, "page {} is {bytes} bytes", out.len());
        let gone: u64 = page.reply.warnings.iter().filter_map(|w| w.details["left_out"].as_u64()).sum();
        assert!(!result.items.is_empty() || gone > 0, "a page moves on");
        left_out += gone;
        out.push(result);
        assert!(out.len() < 200, "the listing never ends");
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    (out, left_out)
}

#[test]
fn a_listing_page_keeps_to_its_budget_however_long_the_ids() {
    // An out-of-tree pack whose kind, lens and question IDs are as long as the loader allows, and
    // sixty more kinds as long, so the listing takes several pages.
    let long = "w".repeat(packs::ID_MAX);
    let mut many = String::new();
    for i in 0..60 {
        let id = format!("{}{i:02}", "k".repeat(packs::ID_MAX - 2));
        many.push_str(&format!(
            "[[kinds]]\nid = \"{id}\"\nid_prefix = \"k{i:02}\"\nlevel = \"component\"\nlabel = \"Many\"\n\n"
        ));
    }
    let manifest = format!(
        "format = \"planroom.pack/v1\"\nid = \"longids\"\nversion = 1\ntitle = \"Long IDs\"\nrequires = [\"code\"]\n\n\
         [[kinds]]\nid = \"{long}\"\nid_prefix = \"longw\"\nlevel = \"component\"\nlabel = \"Long\"\n\n\
         [[kinds]]\nid = \"short_one\"\nid_prefix = \"shortw\"\nlevel = \"component\"\nlabel = \"Short\"\n\n\
         {many}\
         [[lenses]]\nid = \"{long}\"\nlabel = \"Long\"\nrelations = [\"code.imports\"]\nlevel = \"component\"\n\n\
         [[questions]]\nid = \"{long}\"\nsummary = \"What uses {{W}}\"\nargs = [{{ name = \"W\" }}]\n\n\
         [[questions.steps]]\nshape = \"neighbors\"\nparams = {{ ids = [\"$W\"], relations = [\"code.imports\"] }}\n"
    );
    let mut sources = packs::embed::sources();
    sources.push(Source {
        origin: Origin::OutOfTree,
        label: "mem/longids".into(),
        dir: None,
        recorded: None,
        files: BTreeMap::from([("pack.toml".to_owned(), manifest.into_bytes())]),
    });
    // And one whose own ID is that long, so its kind's qualified ID is longer still.
    let named = format!(
        "format = \"planroom.pack/v1\"\nid = \"{long}\"\nversion = 1\ntitle = \"Long name\"\nrequires = [\"code\"]\n\n\
         [[kinds]]\nid = \"tiny\"\nid_prefix = \"tinyw\"\nlevel = \"component\"\nlabel = \"Tiny\"\n"
    );
    sources.push(Source {
        origin: Origin::OutOfTree,
        label: "mem/longname".into(),
        dir: None,
        recorded: None,
        files: BTreeMap::from([("pack.toml".to_owned(), named.into_bytes())]),
    });
    let loaded = packs::resolve(sources).unwrap();
    let (pages, left_out) = listed_pages(&loaded);
    assert!(pages.len() > 1, "the listing pages");
    let items: Vec<&planroom_contract::AskItem> = pages.iter().flat_map(|p| &p.items).collect();
    // Every entry is listed once, its ID whole, since a caller passes it back; none goes.
    let expected = declared(&loaded);
    let listed: BTreeSet<&str> = items.iter().map(|i| i.id.as_str()).collect();
    // A lens and a question may share an ID, so entries are counted, not IDs.
    let entries: usize = 1
        + usize::from(has_stages(&loaded))
        + loaded.vocabulary.levels.len()
        + loaded
            .packs
            .iter()
            .map(|p| p.file.kinds.len() + p.file.lenses.len() + p.file.questions.len())
            .sum::<usize>();
    assert_eq!(items.len(), entries);
    assert_eq!(listed, expected.iter().map(String::as_str).collect());
    assert_eq!(left_out, 0);
    let long_ones: Vec<_> = items.iter().filter(|i| i.id.contains(&long)).collect();
    assert_eq!(long_ones.len(), 4, "the long kind, lens and question, and the long pack's kind");
    for item in long_ones {
        assert!(item.why.chars().count() < 400, "{item:?}");
    }
    assert!(items.iter().any(|i| i.id == "longids.short_one" && !i.handles.is_empty()));
}

// ---------------------------------------------------------------------------------------------
// Ruby through the extractor contract (architecture.md, A new language and an out-of-tree pack;
// DR-13, DR-62): the fixture without the Ruby pack has no `healthcheck`, and a person's
// `pack add` of a copy outside the tree brings it, with its two `http` edges, to every later
// command.

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_store::{Shape, Store};

/// `planroom ARGS --json` in `cwd` with the data root `data` and `env`: the envelope, which must
/// be `ok`.
fn run(cwd: &Path, data: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let env = envelope(cwd, data, args, env);
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    env
}

/// The `--json` envelope `args` prints, `ok` or not.
fn envelope(cwd: &Path, data: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", data)
        .env_remove("PLANROOM_RAW")
        .env_remove("PLANROOM_PACKS")
        .env_remove("PLANROOM_ACTOR")
        .envs(env.iter().copied())
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"))
}

fn person_key() -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = 7;
    bytes[0] = 1;
    SigningKey::from_slice(&bytes).unwrap()
}

fn b64url(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// A person's token under `key` over `command` with the argv after it, as the mod signs one.
fn token(key: &SigningKey, command: &str, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key.sign(&planroom_kernel::keys::person_token_message(command, &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// Register `key` in the workspace at `ws`, as the mod's first start does.
fn register(ws: &Path, data: &Path, key: &SigningKey) {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["person", "init", "--json"])
        .current_dir(ws)
        .env("PLANROOM_DATA", data)
        .env("PLANROOM_PERSON_TOKEN", token(key, "person init", &[]))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let public = b64url(&key.verifying_key().to_sec1_point(false).to_bytes());
    child.stdin.take().unwrap().write_all(public.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

/// Every fact of the workspace's active set, parsed, by shape.
fn facts(ws: &Path) -> Vec<(Shape, Value)> {
    let store = Store::open_existing(&ws.join(".planroom/store.db")).unwrap().expect("a store");
    store
        .facts(&store.active_set().unwrap().snapshots())
        .unwrap()
        .into_iter()
        .map(|r| (r.shape, serde_json::from_str(&r.body).unwrap()))
        .collect()
}

/// The containers `map --level container` shows: name to node.
fn containers(ws: &Path, data: &Path) -> BTreeMap<String, Value> {
    let map = run(ws, data, &["map", "--level", "container"], &[]);
    map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["name"].as_str().unwrap().to_owned(), n.clone()))
        .collect()
}

#[test]
fn ruby_absent_without_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let ws = planroom_testkit::fixture(&tmp.path().join("shop")).root;
    run(&ws, &data, &["scan"], &[]);
    assert!(!containers(&ws, &data).contains_key("healthcheck"));
    let facts = facts(&ws);
    let node = |id: &str| {
        facts
            .iter()
            .find(|(s, f)| *s == Shape::Node && f["id"] == id)
            .map(|(_, f)| f.clone())
            .unwrap_or_else(|| panic!("{id} is mapped"))
    };
    let script = node("platform:file:tools/healthcheck.rb");
    assert!(script["props"].get("language").is_none(), "a universal-tier file with no language: {script}");
    assert_eq!(script["provenance"]["extractor"], "code/files", "{script}");
    let platform = node("platform:repo:platform");
    let languages: Vec<&str> =
        platform["props"]["languages"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert!(!languages.contains(&"ruby"), "{languages:?}");
}

#[test]
fn healthcheck_edges_through_the_ruby_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let ws = planroom_testkit::fixture(&tmp.path().join("shop")).root;
    run(&ws, &data, &["scan"], &[]);
    // A copy of the pack outside the tree, added by a person.
    let pack = tmp.path().join("packs/ruby");
    std::fs::create_dir_all(pack.parent().unwrap()).unwrap();
    let copied = Command::new("cp")
        .arg("-R")
        .arg(planroom_testkit::repo_root().join("examples/packs/ruby"))
        .arg(&pack)
        .status()
        .unwrap();
    assert!(copied.success());
    let key = person_key();
    register(&ws, &data, &key);
    let dir = pack.to_str().unwrap();
    let added = run(
        &ws,
        &data,
        &["pack", "add", dir],
        &[("PLANROOM_PERSON_TOKEN", &token(&key, "pack add", &[dir]))],
    );
    assert_eq!(added["result"]["id"], "ruby", "{added}");
    run(&ws, &data, &["scan"], &[]);

    // The map: healthcheck under platform, with an `http` edge, GET /healthz, to each service.
    let containers = containers(&ws, &data);
    let healthcheck = containers.get("healthcheck").unwrap_or_else(|| panic!("{containers:?}"));
    assert_eq!(healthcheck["parent"], "platform:repo:platform", "{healthcheck}");
    let truth = planroom_testkit::truth();
    let facts = facts(&ws);
    for want in truth["relations"].as_array().unwrap().iter().filter(|r| r["from"] == "healthcheck") {
        let to = &containers[want["to"].as_str().unwrap()]["id"];
        let edge = facts
            .iter()
            .find(|(s, e)| *s == Shape::Edge && e["from"]["id"] == healthcheck["id"] && e["to"]["id"] == *to)
            .map(|(_, e)| e)
            .unwrap_or_else(|| panic!("healthcheck -> {to}"));
        assert_eq!(edge["kind"], "code.http", "{edge}");
        assert_eq!(edge["via"], want["via"], "{edge}");
        for evidence in want["evidence"].as_array().unwrap() {
            let found = edge["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| planroom_testkit::evidence_matches(evidence, e));
            assert!(found, "{evidence} on {edge}");
        }
        // Provenance: the Ruby pack's extractor, through the code pack's linker.
        assert_eq!(edge["provenance"]["linker"], "code/runtime_wiring", "{edge}");
        let hints = edge["provenance"]["hints"].as_array().unwrap();
        assert!(hints.iter().any(|h| h["extractor"] == "ruby/ruby-http"), "{edge}");
    }

    // Every later command sees it, with no variable set. `path` reads through the lens its
    // question names, so `ask` takes no `--lens`.
    let path = run(&ws, &data, &["ask", "path", "healthcheck", "orders"], &[]);
    assert!(path["result"]["items"].as_array().is_some_and(|i| !i.is_empty()), "{path}");
    assert!(path.to_string().contains("healthcheck"), "{path}");
    let found = run(&ws, &data, &["find", "healthcheck"], &[]);
    assert!(
        found["result"]["items"].as_array().unwrap().iter().any(|i| i["id"] == healthcheck["id"]),
        "{found}"
    );
}

// ---------------------------------------------------------------------------------------------
// Levels are pack data (architecture.md, Levels and roll-ups): `ask` alone lists them first, in
// ladder order, and a pack added after the build puts a level into the ladder its set resolves
// to, with no change to the engine. Every read of the workspace loads the set its store records
// (DR-114), so through the CLI `map`, its graph's ladder and `ask` show that level too.

#[test]
fn ask_alone_lists_the_levels_in_ladder_order() {
    let tmp = tempfile::tempdir().unwrap();
    let first = ok(tmp.path(), &["ask"]);
    let summary = first["summary"].as_str().unwrap();
    assert!(summary.starts_with("5 levels, "), "{summary}");
    let items = first["result"]["items"].as_array().unwrap();
    let levels: Vec<&str> = items.iter().take(5).map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(
        levels,
        ["intent", "system", "container", "component", "code"],
        "levels first, in ladder order"
    );
    assert_eq!(items[0]["why"], "The plan's level, beside the others.");
    for (n, item) in items[1..5].iter().enumerate() {
        assert_eq!(item["why"], format!("Level {} of 4, from the code pack.", n + 1), "{item}");
    }
    for item in &items[..5] {
        let id = item["id"].as_str().unwrap();
        assert_eq!(item["name"], id, "a built-in level's label is its ID: {item}");
        let handle = &item["handles"][0];
        assert_eq!(handle["command"], "map", "{item}");
        assert_eq!(handle["args"], serde_json::json!(["--level", id]), "{item}");
        assert_eq!(handle["label"], "Show that level", "{item}");
    }
    // Kinds follow the levels.
    assert!(items[5]["id"].as_str().unwrap().contains('.'), "{}", items[5]);
}

/// A toy pack that puts `zone` below `container`, labelled "service area", with a kind at it: one
/// block per Go package, under its repo's container.
const ZONES: &str = r#"format = "planroom.pack/v1"
id = "zones"
version = 1
title = "Zones"
requires = ["code"]

[[levels]]
id = "zone"
label = "service area"
one = "{count} service area"
many = "{count} service areas"
below = "container"

[[kinds]]
id = "zone"
id_prefix = "zone"
level = "zone"
label = "Zone"

[[extractors]]
id = "go-packages"
files = ["**/*.go"]
regex = "(?m)^package (\\w+)"
emit = [{ node = "zones.zone", key = "$1", name = "$1", parent = "@id:code.service:$repo" }]
"#;

/// Each block `map ARGS` shows, with its counts, by ID.
fn blocks(ws: &Path, data: &Path, args: &[&str]) -> BTreeMap<String, Value> {
    let map = run(ws, data, args, &[]);
    assert!(map["meta"]["next"].is_null(), "one page: {map}");
    map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_str().unwrap().to_owned(), n.clone()))
        .collect()
}

#[test]
fn a_packs_level_sits_on_the_ladder_and_every_read_shows_it() {
    use planroom_contract::LevelId;
    use planroom_kernel::ladder::Ladder;

    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let ws = planroom_testkit::fixture(&tmp.path().join("shop")).root;
    run(&ws, &data, &["scan"], &[]);
    let before = blocks(&ws, &data, &["map", "--level", "container"]);

    // The toy pack, in this test's own directory, added by a person, and a scan that extracts it.
    let pack = tmp.path().join("packs/zones");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("pack.toml"), ZONES).unwrap();
    let key = person_key();
    register(&ws, &data, &key);
    let dir = pack.to_str().unwrap();
    let added = run(
        &ws,
        &data,
        &["pack", "add", dir],
        &[("PLANROOM_PERSON_TOKEN", &token(&key, "pack add", &[dir]))],
    );
    assert_eq!(added["result"]["id"], "zones", "{added}");
    run(&ws, &data, &["scan"], &[]);

    // The set the workspace now records: the built-ins and the toy pack. Its ladder holds the new
    // level directly below `container`, and the engine's own fold places the scan's facts on it.
    let mut sources = packs::embed::sources();
    sources.push(Source {
        origin: Origin::OutOfTree,
        label: dir.into(),
        dir: Some(pack.clone()),
        recorded: None,
        files: BTreeMap::from([("pack.toml".to_owned(), ZONES.as_bytes().to_vec())]),
    });
    let loaded = packs::resolve(sources).unwrap();
    let ladder = Ladder::of(&loaded.vocabulary);
    let ids: Vec<&str> = ladder.levels().iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["intent", "system", "container", "zone", "component", "code"]);
    let store = Store::open_existing(&ws.join(".planroom/store.db")).unwrap().expect("a store");
    let model = planroom::live::model(&store.snapshot().unwrap(), &loaded.vocabulary).unwrap();
    let at = |level: &str| -> BTreeMap<String, planroom_contract::MapNode> {
        planroom_kernel::tree::at_level(&model.nodes, &ladder, &LevelId::from(level))
            .into_iter()
            .map(|n| (n.id.0.clone(), n))
            .collect()
    };

    // The new level shows its blocks, each under a container.
    let zones = at("zone");
    assert!(!zones.is_empty(), "the zone level shows its blocks");
    for (id, zone) in &zones {
        assert_eq!(zone.kind.0, "zones.zone", "{zone:?}");
        let parent = zone.parent.as_ref().map(|p| p.0.as_str()).unwrap_or_default();
        assert!(before.contains_key(parent), "{id} sits under a container: {zone:?}");
    }

    // The container level shows the same blocks, each with its old counts and its zones counted,
    // and the roll-up is faithful: a container's zone count is the zones under it.
    let after = at("container");
    assert_eq!(after.keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "the same blocks");
    for (id, node) in &after {
        let under = zones.values().filter(|z| z.parent.as_ref().is_some_and(|p| p.0 == *id)).count();
        let mut counts: BTreeMap<String, u64> =
            node.counts.iter().map(|(level, n)| (level.clone(), u64::from(*n))).collect();
        let zone = counts.remove("zone").unwrap_or(0);
        assert_eq!(zone, under as u64, "{id}'s zone count");
        let old: BTreeMap<String, u64> = serde_json::from_value(before[id]["counts"].clone()).unwrap();
        assert_eq!(counts, old, "{id}'s other counts");
    }
    assert!(after["orders:container:orders"].counts.get("zone").is_some_and(|n| *n > 0));

    // `ask` alone over the set lists the level in its place: its line names its label, its
    // handle its ID; so does its kind's.
    let first = ask::listing(&loaded, None).unwrap().reply.result;
    let ids: Vec<&str> = first.items.iter().take(6).map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["intent", "system", "container", "zone", "component", "code"]);
    let zone = &first.items[3];
    assert_eq!(zone.name, "service area");
    assert_eq!(zone.why, "Level 3 of 5, from the zones pack.");
    assert_eq!(zone.handles[0].args, ["--level", "zone"]);
    assert!(first.summary.starts_with("6 levels, "), "{}", first.summary);
    let kind = pages_of(&loaded).into_iter().find(|i| i.id == "zones.zone").expect("the kind is listed");
    assert_eq!(kind.why, "A kind at the service area level, from the zones pack.");
    assert_eq!(kind.handles[0].args, ["--level", "zone"]);

    // Through the CLI, every read loads the set the workspace records, so it shows the same level
    // with the same blocks and counts as the engine's own fold over that set.
    let shown = blocks(&ws, &data, &["map", "--level", "zone"]);
    assert_eq!(shown.keys().collect::<Vec<_>>(), zones.keys().collect::<Vec<_>>(), "the zone blocks");
    for (id, node) in &shown {
        assert_eq!(node["kind"], "zones.zone", "{id}: {node}");
    }
    let shown = blocks(&ws, &data, &["map", "--level", "container"]);
    assert_eq!(shown.keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "the same blocks");
    for (id, node) in &shown {
        let counts: BTreeMap<String, u64> = serde_json::from_value(node["counts"].clone()).unwrap();
        let folded: BTreeMap<String, u64> =
            after[id].counts.iter().map(|(level, n)| (level.clone(), u64::from(*n))).collect();
        assert_eq!(counts, folded, "{id}'s counts, `zone` included");
    }
    let graph = run(&ws, &data, &["map", "--level", "zone", "--detail", "graph"], &[]);
    let levels: Vec<&str> =
        graph["result"]["levels"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(levels, ["intent", "system", "container", "zone", "component", "code"], "{graph}");
    let listed = run(&ws, &data, &["ask"], &[]);
    let ids: Vec<&str> = listed["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .take(6)
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["intent", "system", "container", "zone", "component", "code"], "{listed}");
    let summary = listed["result"]["summary"].as_str().unwrap();
    assert!(summary.starts_with("6 levels, "), "{summary}");
    assert!(summary.contains("zones"), "{summary}");
}

/// Every item `ask` alone lists over `loaded`, page after page.
fn pages_of(loaded: &packs::Loaded) -> Vec<planroom_contract::AskItem> {
    let mut items = Vec::new();
    let mut cursor: Option<Cursor> = None;
    loop {
        let page = ask::listing(loaded, cursor.as_ref()).unwrap();
        items.extend(page.reply.result.items);
        match page.next {
            Some(next) => cursor = Some(next),
            None => return items,
        }
    }
}
