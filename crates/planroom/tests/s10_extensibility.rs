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

/// What the loaded set declares: each kind, qualified, each lens and each question, with the
/// built-in questions this build serves: `kinds`, and `stage` while a pack declares stages.
fn declared(loaded: &packs::Loaded) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = BTreeSet::from(["kinds".to_owned()]);
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
        "ask alone lists every loaded kind, lens and question, and nothing else"
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
