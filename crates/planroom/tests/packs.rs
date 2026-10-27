//! Packs: every pack under `packs/` validates and passes its conformance kit, requires point
//! inward by origin, ID prefixes and merged-table keys belong to one pack, extractor names are
//! pack-qualified, one default lens, no hint kind that nothing joins, no suggestion that can't be
//! undone, and a compose service's kind and role come from the code pack's image roles (DR-9,
//! DR-46, DR-54, DR-62).
//!
//! A pack a person adds is checked through `planroom doctor --pack DIR --json`; a set that holds
//! more than one extra pack, or a built-in that isn't in `packs/`, through the loader itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use planroom::packs::{self, Origin, Source};
use planroom_store::{Shape, Store};
use planroom_testkit::{fixture, repo_root};
use serde_json::Value;

/// `planroom doctor --pack DIR --json`: its envelope.
fn doctor(dir: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["doctor", "--pack", dir.to_str().unwrap(), "--json"])
        .current_dir(dir.parent().unwrap())
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap();
    let code = out.status.code().unwrap();
    if env["ok"] == true {
        assert_eq!(code, 0, "{env}");
    } else {
        assert_eq!(code, 8, "pack_invalid exits 8: {env}");
    }
    env
}

/// A pack directory under `root` holding `pack.toml` with `body` after its header.
fn pack_dir(root: &Path, id: &str, requires: &[&str], body: &str) -> PathBuf {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("pack.toml"), manifest(id, requires, body)).unwrap();
    dir
}

fn manifest(id: &str, requires: &[&str], body: &str) -> String {
    let requires: Vec<String> = requires.iter().map(|r| format!("\"{r}\"")).collect();
    format!(
        "format = \"planroom.pack/v1\"\nid = \"{id}\"\nversion = 1\ntitle = \"{id}\"\nrequires = [{}]\n\n{body}",
        requires.join(", ")
    )
}

/// A pack the loader reads from memory, with the origin given.
fn source(origin: Origin, id: &str, requires: &[&str], body: &str) -> Source {
    Source {
        origin,
        label: format!("mem/{id}"),
        dir: None,
        files: BTreeMap::from([("pack.toml".to_owned(), manifest(id, requires, body).into_bytes())]),
    }
}

/// The built-ins, then `extra`.
fn with_builtins(extra: Vec<Source>) -> Result<packs::Loaded, planroom::app::error::AppError> {
    let mut sources = packs::embed::sources();
    sources.extend(extra);
    packs::resolve(sources)
}

/// The error a pack set fails with: its code, its message and its details.
fn refused(result: Result<packs::Loaded, planroom::app::error::AppError>) -> (String, String, Value) {
    let e = result.expect_err("the set is refused");
    (e.code.as_str().to_owned(), e.message.clone(), e.details.clone())
}

fn assert_invalid(env: &Value, reference: &str, says: &str) {
    assert_eq!(env["ok"], false, "{env}");
    assert_eq!(env["error"]["code"], "pack_invalid", "{env}");
    assert_eq!(env["error"]["details"]["reference"], reference, "{env}");
    assert!(env["error"]["details"]["line"].as_u64().unwrap() >= 1, "{env}");
    let message = env["error"]["message"].as_str().unwrap();
    assert!(message.contains(says), "{message}");
}

/// Every pack directory in the tree: the built-ins under `packs/`, then `examples/packs/`.
fn pack_dirs() -> Vec<PathBuf> {
    let root = repo_root();
    let mut dirs = Vec::new();
    for parent in ["packs", "examples/packs"] {
        let Ok(entries) = fs::read_dir(root.join(parent)) else { continue };
        let mut found: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        found.sort();
        dirs.extend(found);
    }
    dirs
}

#[test]
fn every_pack_validates() {
    let loaded = packs::builtins().expect("the built-in packs load");
    // build.rs embeds every directory under packs/, so a new built-in pack needs no code edit.
    let mut dirs: Vec<String> = fs::read_dir(repo_root().join("packs"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    assert_eq!(packs::embed::dirs(), dirs);
    let ids: BTreeSet<&str> = loaded.packs.iter().map(|p| p.id()).collect();
    assert_eq!(ids.len(), dirs.len(), "one pack per directory");
    assert!(loaded.packs.iter().all(|p| p.origin == Origin::BuiltIn));
    assert_eq!(loaded.packs[0].id(), "code", "the base pack loads first");
    // Every embedded pack is the pack its directory holds, file for file.
    for pack in &loaded.packs {
        let dir = repo_root().join(&pack.source.label);
        assert_eq!(packs::read_dir(&dir).unwrap().sha256(), pack.sha256, "{}", pack.source.label);
    }
    // Every out-of-tree example pack loads beside the built-ins.
    for dir in pack_dirs().into_iter().filter(|d| d.starts_with(repo_root().join("examples"))) {
        with_builtins(vec![packs::read_dir(&dir).unwrap()])
            .unwrap_or_else(|e| panic!("{dir:?}: {}", e.message));
    }
    // The vocabulary qualifies every kind and relation, and names the code pack's default lens.
    let vocabulary = &loaded.vocabulary;
    assert!(vocabulary.kinds.iter().all(|k| k.kind.0.split_once('.').is_some()));
    assert!(vocabulary.relations.iter().all(|r| r.kind.0.split_once('.').is_some()));
    let defaults: Vec<&str> = vocabulary.lenses.iter().filter(|l| l.default).map(|l| l.id.as_str()).collect();
    assert_eq!(defaults.len(), 1, "{defaults:?}");
}

#[test]
fn requires_is_inward_only() {
    // A built-in pack requires `code` or nothing: one that requires another built-in is refused.
    let observability = source(Origin::BuiltIn, "observability", &["code"], "");
    let release = source(Origin::BuiltIn, "release", &["observability"], "");
    let (code, message, details) = refused(with_builtins(vec![observability.clone(), release]));
    assert_eq!(code, "pack_invalid");
    assert_eq!(details["reference"], "observability");
    assert!(message.contains("may require only code"), "{message}");
    assert!(with_builtins(vec![observability.clone()]).is_ok());

    // An out-of-tree pack may not require another out-of-tree pack.
    let flags = source(Origin::OutOfTree, "flags", &["code"], "");
    let rollout = source(Origin::OutOfTree, "rollout", &["flags"], "");
    let (code, message, details) = refused(with_builtins(vec![flags, rollout]));
    assert_eq!(code, "pack_invalid");
    assert_eq!(details["reference"], "flags");
    assert!(message.contains("may require only built-in packs"), "{message}");

    // A pack may reference only its own kinds, those of the packs it requires, and the kernel's.
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack_dir(
        tmp.path(),
        "lonely",
        &[],
        "[[kinds]]\nid = \"thing\"\nid_prefix = \"thing\"\nlevel = \"code\"\nlabel = \"Thing\"\nparents = [\"code.file\"]\n",
    );
    assert_invalid(&doctor(&dir), "code.file", "doesn't require");
    let dir = pack_dir(
        tmp.path(),
        "grouped",
        &[],
        "[[kinds]]\nid = \"thing\"\nid_prefix = \"thing\"\nlevel = \"code\"\nlabel = \"Thing\"\nparents = [\"kernel.group\"]\n",
    );
    assert_eq!(doctor(&dir)["ok"], true);
}

#[test]
fn an_out_of_tree_pack_may_require_any_built_in() {
    // Every built-in this build embeds, and one more built-in ring beside `code`.
    let mut builtins: Vec<String> =
        packs::builtins().unwrap().packs.iter().map(|p| p.id().to_owned()).collect();
    let observability = source(Origin::BuiltIn, "observability", &["code"], "");
    builtins.push("observability".into());
    let required: Vec<&str> = builtins.iter().map(String::as_str).collect();
    let alerts = source(
        Origin::OutOfTree,
        "alerts",
        &required,
        "[[relations]]\nid = \"pages\"\nfrom = [\"code.service\"]\nto = [\"code.service\"]\nlabel = \"pages\"\n",
    );
    let loaded = with_builtins(vec![observability, alerts]).expect("an out-of-tree pack requires built-ins");
    let order: Vec<&str> = loaded.packs.iter().map(|p| p.id()).collect();
    assert_eq!(order.first(), Some(&"code"));
    assert_eq!(order.last(), Some(&"alerts"), "out-of-tree packs load after the built-ins");
    assert!(loaded.vocabulary.relations.iter().any(|r| r.kind.0 == "alerts.pages"));

    // Through doctor: an added pack that requires the code pack.
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack_dir(
        tmp.path(),
        "healthcheck",
        &["code"],
        "[[kinds]]\nid = \"probe\"\nid_prefix = \"container\"\nlevel = \"container\"\nlabel = \"Probe\"\nparents = [\"code.repo\"]\ntone = \"infra\"\n",
    );
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["origin"], "out_of_tree");
    assert_eq!(env["result"]["kinds"], serde_json::json!(["healthcheck.probe"]));
}

#[test]
fn id_prefixes_are_owned_by_one_pack() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = |id: &str, prefix: &str, level: &str| {
        format!(
            "[[kinds]]\nid = \"{id}\"\nid_prefix = \"{prefix}\"\nlevel = \"{level}\"\nlabel = \"{id}\"\n\n"
        )
    };
    // The code pack owns `file`; another pack can't take it.
    let dir = pack_dir(tmp.path(), "docs", &["code"], &kind("page", "file", "code"));
    assert_invalid(&doctor(&dir), "file", "already owned by the code pack");
    // Every Container-level kind uses the kernel's `container`, and only those do.
    let dir = pack_dir(tmp.path(), "boxes", &["code"], &kind("box", "box", "container"));
    assert_invalid(&doctor(&dir), "box", "must be container");
    let dir = pack_dir(tmp.path(), "crates", &["code"], &kind("crate", "container", "component"));
    assert_invalid(&doctor(&dir), "container", "the kernel's prefix for containers");
    // Several kinds of one pack may share a prefix, and any pack's container kind shares the
    // kernel's.
    let body = kind("page", "page", "code")
        + &kind("draft", "page", "code")
        + &kind("site", "container", "container");
    let dir = pack_dir(tmp.path(), "wiki", &["code"], &body);
    assert_eq!(doctor(&dir)["ok"], true);
    // The kernel's own prefix for hand-made groups is no pack's.
    let dir = pack_dir(tmp.path(), "teams", &["code"], &kind("team", "group", "component"));
    assert_invalid(&doctor(&dir), "group", "is the kernel's own");
}

#[test]
fn extractor_names_are_pack_qualified() {
    let loaded = packs::builtins().unwrap();
    let registry: BTreeMap<&str, &str> =
        planroom_extract::registry::all().map(|b| (b.builtin, b.name)).collect();
    assert!(!loaded.extractors.is_empty());
    for declared in &loaded.extractors {
        let (pack, id) = declared.name.split_once('/').expect("a pack-qualified name");
        assert_eq!(pack, declared.pack);
        assert!(!id.is_empty());
        if let planroom_contract::ExtractorDecl::Builtin { builtin, .. } = &declared.decl {
            assert_eq!(registry.get(builtin.as_str()), Some(&declared.name.as_str()), "{}", declared.name);
        }
    }
    let names: BTreeSet<&str> = loaded.extractors.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.len(), loaded.extractors.len(), "no name twice");

    // A built-in runs under the one pack-qualified name its facts' provenance gives, so another
    // pack can't claim it, under its own ID or the code pack's.
    let tmp = tempfile::tempdir().unwrap();
    let decl = |id: &str| {
        format!("[[extractors]]\nid = \"{id}\"\nbuiltin = \"compose\"\nfiles = [\"**/compose.yml\"]\n")
    };
    let dir = pack_dir(tmp.path(), "stack", &["code"], &decl("compose"));
    assert_invalid(&doctor(&dir), "compose", "runs as code/compose");
    let dir = pack_dir(
        tmp.path(),
        "nope",
        &["code"],
        "[[extractors]]\nid = \"x\"\nbuiltin = \"cobol\"\nfiles = [\"**\"]\n",
    );
    assert_invalid(&doctor(&dir), "cobol", "no built-in extractor named cobol");
}

#[test]
fn duplicate_table_keys_are_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    // One file extension, claimed by the code pack and another.
    let dir =
        pack_dir(tmp.path(), "deno", &["code"], "[[languages]]\nid = \"deno\"\nextensions = [\"ts\"]\n");
    assert_invalid(&doctor(&dir), "ts", "already claimed by the code pack");
    // One image glob, claimed twice.
    let dir = pack_dir(
        tmp.path(),
        "cache",
        &["code"],
        "[[image_roles]]\nimage = \"redis*\"\nrole = \"cache\"\nkind = \"code.datastore\"\n",
    );
    assert_invalid(&doctor(&dir), "redis*", "already claimed by the code pack");
    // A language file under languages/ merges with the manifest, and its keys count too.
    let dir = pack_dir(tmp.path(), "ruby", &["code"], "");
    fs::create_dir_all(dir.join("languages")).unwrap();
    fs::write(
        dir.join("languages/ruby.toml"),
        "[[languages]]\nid = \"ruby\"\nextensions = [\"rb\", \"py\"]\n",
    )
    .unwrap();
    let env = doctor(&dir);
    assert_invalid(&env, "py", "already claimed by the code pack");
    assert!(env["error"]["details"]["file"].as_str().unwrap().ends_with("languages/ruby.toml"), "{env}");
    // A kind declared twice in one pack.
    let twice =
        "[[kinds]]\nid = \"flag\"\nid_prefix = \"flag\"\nlevel = \"code\"\nlabel = \"Flag\"\n\n".repeat(2);
    let dir = pack_dir(tmp.path(), "flags", &["code"], &twice);
    let env = doctor(&dir);
    assert_invalid(&env, "flag", "declares flag twice");
    assert!(env["error"]["details"]["line"].as_u64().unwrap() > 7, "the second declaration: {env}");
    // A distinct extension and glob load.
    let dir = pack_dir(
        tmp.path(),
        "ruby2",
        &["code"],
        "[[languages]]\nid = \"ruby\"\nextensions = [\"rb\"]\n\n[[image_roles]]\nimage = \"memcached*\"\nrole = \"queue\"\nkind = \"code.queue\"\n",
    );
    assert_eq!(doctor(&dir)["ok"], true);
}

#[test]
fn two_default_lenses_are_pack_invalid() {
    let lens = |id: &str| {
        format!(
            "[[lenses]]\nid = \"{id}\"\nlabel = \"{id}\"\nrelations = [\"code.http\"]\nlevel = \"container\"\ndefault = true\n"
        )
    };
    // One out-of-tree default beside the code pack's: the code pack's stays the default.
    let loaded = with_builtins(vec![source(Origin::OutOfTree, "ops", &["code"], &lens("ops"))]).unwrap();
    let defaults: Vec<&str> =
        loaded.vocabulary.lenses.iter().filter(|l| l.default).map(|l| l.id.as_str()).collect();
    assert_eq!(defaults, ["runtime"]);
    // Two defaults, neither the code pack's, are refused.
    let (code, message, details) = refused(with_builtins(vec![
        source(Origin::OutOfTree, "ops", &["code"], &lens("ops")),
        source(Origin::OutOfTree, "sre", &["code"], &lens("sre")),
    ]));
    assert_eq!(code, "pack_invalid");
    assert_eq!(details["reference"], "sre");
    assert!(message.contains("both the default"), "{message}");
    // So are two in one pack.
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack_dir(tmp.path(), "views", &["code"], &(lens("one") + "\n" + &lens("two")));
    assert_invalid(&doctor(&dir), "two", "both the default");
}

#[test]
fn an_unconsumed_hint_kind_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack_dir(tmp.path(), "pinger", &["code"], "[[hints]]\nid = \"ping\"\nattrs = [\"host\"]\n");
    let env = doctor(&dir);
    assert_invalid(&env, "ping", "no loaded linker joins the hint kind pinger.ping");
    let message = env["error"]["message"].as_str().unwrap();
    assert!(message.contains("@alias:") && message.contains("@ext:"), "names the same-key joins: {message}");
    // Emitting an existing hint kind is how an out-of-tree pack reaches a linker.
    let dir = pack_dir(
        tmp.path(),
        "rubyish",
        &["code"],
        "[[extractors]]\nid = \"http\"\nfiles = [\"**/*.rb\"]\nregex = \"'(https?)://([a-z0-9-]+)'\"\nemit = [{ hint = \"code.url\", scope = \"@id:code.file:$file\", key = \"$2\" }]\n",
    );
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    // A bare path where a reference belongs is refused.
    let dir = pack_dir(
        tmp.path(),
        "bare",
        &["code"],
        "[[extractors]]\nid = \"http\"\nfiles = [\"**/*.rb\"]\nregex = \"'(https?)://([a-z0-9-]+)'\"\nemit = [{ hint = \"code.url\", scope = \"$file\", key = \"$2\" }]\n",
    );
    assert_invalid(&doctor(&dir), "$file", "isn't a reference");
}

/// A `[[decisions]]` table about `subject`, whose precheck asks for no incoming `when` edge, whose
/// slice lifts `slice` and whose yes puts an `effect` edge.
fn decision(subject: &str, when: &str, slice: &str, effect: &str) -> String {
    format!(
        "[[decisions]]\nid = \"stale\"\nversion = 1\nsubject = \"{subject}\"\n\
         when = {{ \"props.enabled\" = false, without_edge = {{ relation = \"{when}\", direction = \"in\" }} }}\n\
         question = {{ type = \"boolean\", instructions = \"Is it stale?\" }}\n\
         slice = {{ query = \"neighbors\", relations = [\"{slice}\"], max_tokens = 500 }}\n\
         effect = {{ on = \"true\", do = \"put_edge\", relation = \"{effect}\", to = \"$pick\" }}\n\
         stakes = \"person\"\ndefault = false\n\
         policy = {{ card_band = [0.15, 0.85], escalate_band = [0.35, 0.65] }}\n\
         budget = {{ max_calls_per_scan = 5 }}\n"
    )
}

#[test]
fn a_decision_kind_references_only_kinds_in_scope() {
    let tmp = tempfile::tempdir().unwrap();
    // Its subject, its precheck's relation, its slice's and its effect's, all in scope, load.
    let dir = pack_dir(
        tmp.path(),
        "flags",
        &["code"],
        &decision("code.file", "code.imports", "code.imports", "code.owned_by"),
    );
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    // An edge kind is a subject too, and the kernel's kinds and relation are every pack's.
    let dir = pack_dir(
        tmp.path(),
        "edges",
        &["code"],
        &decision("code.imports", "kernel.touches", "code.imports", "kernel.touches"),
    );
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    // A subject no loaded pack declares.
    let dir = pack_dir(
        tmp.path(),
        "ghosts",
        &["code"],
        &decision("code.flag", "code.imports", "code.imports", "code.imports"),
    );
    assert_invalid(&doctor(&dir), "code.flag", "no loaded pack declares the kind or relation code.flag");
    // A subject of a pack it doesn't require.
    let dir = pack_dir(
        tmp.path(),
        "lonely",
        &[],
        &decision("code.file", "kernel.touches", "kernel.touches", "kernel.touches"),
    );
    assert_invalid(&doctor(&dir), "code.file", "doesn't require");
    // The precheck's relation, the slice's and the effect's are references too.
    let dir = pack_dir(
        tmp.path(),
        "unread",
        &["code"],
        &decision("code.file", "code.reads", "code.imports", "code.imports"),
    );
    assert_invalid(&doctor(&dir), "code.reads", "no loaded pack declares the relation code.reads");
    let dir = pack_dir(
        tmp.path(),
        "sliced",
        &["code"],
        &decision("code.file", "code.imports", "code.calls", "code.imports"),
    );
    assert_invalid(&doctor(&dir), "code.calls", "no loaded pack declares the relation code.calls");
    let dir = pack_dir(
        tmp.path(),
        "items",
        &[],
        &decision("kernel.item", "kernel.touches", "kernel.touches", "code.imports"),
    );
    assert_invalid(&doctor(&dir), "code.imports", "doesn't require");
}

#[test]
fn a_pack_found_on_a_path_never_takes_a_built_in_ring() {
    // The loader: an out-of-tree pack under a built-in's ID is refused, whether or not that
    // built-in is loaded, so it never takes the base pack's place.
    for result in [
        with_builtins(vec![source(Origin::OutOfTree, "code", &[], "")]),
        packs::resolve(vec![source(Origin::OutOfTree, "code", &[], "")]),
    ] {
        let (code, message, details) = refused(result);
        assert_eq!(code, "pack_invalid");
        assert_eq!(details["reference"], "code");
        assert!(message.contains("is a built-in pack's ID"), "{message}");
    }

    // doctor: a directory that names itself code, with a hint kind no linker joins, which only an
    // out-of-tree pack is refused for.
    let tmp = tempfile::tempdir().unwrap();
    let posing =
        pack_dir(&tmp.path().join("posing"), "code", &[], "[[hints]]\nid = \"ping\"\nattrs = [\"host\"]\n");
    assert_invalid(&doctor(&posing), "code", "aren't the code pack this build embeds");
    // A copy of the code pack with one file changed is refused the same way.
    let changed = tmp.path().join("changed/code");
    copy_dir(&repo_root().join("packs/code"), &changed);
    let manifest = changed.join("pack.toml");
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap() + "\n# changed\n").unwrap();
    assert_invalid(&doctor(&changed), "code", "aren't the code pack this build embeds");
    // An unchanged copy holds the built-in's own files, so it checks the built-in.
    let same = tmp.path().join("same/code");
    copy_dir(&repo_root().join("packs/code"), &same);
    let env = doctor(&same);
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["origin"], "builtin");
}

#[test]
fn an_invalid_extractor_regex_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let body = |regex: &str| {
        format!(
            "[[extractors]]\nid = \"http\"\nfiles = [\"**/*.rb\"]\nregex = '{regex}'\nemit = [{{ hint = \"code.url\", scope = \"@id:code.file:$file\", key = \"$1\" }}]\n"
        )
    };
    let dir = pack_dir(tmp.path(), "broken", &["code"], &body("(https?://"));
    let env = doctor(&dir);
    assert_invalid(&env, "(https?://", "the regex of broken/http doesn't compile");
    assert_eq!(env["error"]["details"]["line"], 10, "the regex's own line: {env}");
    let message = env["error"]["message"].as_str().unwrap();
    assert!(!message.contains('\n'), "one line: {message}");
    // The same extractor with a pattern that compiles loads.
    let dir = pack_dir(tmp.path(), "fixed", &["code"], &body("(https?)://"));
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
}

#[test]
fn an_invalid_regex_is_reported_at_its_own_line() {
    let tmp = tempfile::tempdir().unwrap();
    let extractor = |id: &str, regex: &str| {
        format!(
            "[[extractors]]\nid = \"{id}\"\nfiles = [\"**/*.rb\"]\nregex = {regex}\nemit = [{{ hint = \"code.url\", scope = \"@id:code.file:$file\", key = \"$1\" }}]\n"
        )
    };
    // An earlier extractor's pattern holds the broken one's text: the error names line 16, the
    // broken extractor's own `regex` key, never line 10.
    let body = extractor("scheme", "'(a)b'") + "\n" + &extractor("broken", "'(a'");
    let dir = pack_dir(tmp.path(), "shadowed", &["code"], &body);
    let env = doctor(&dir);
    assert_invalid(&env, "(a", "the regex of shadowed/broken doesn't compile");
    assert_eq!(env["error"]["details"]["line"], 16, "the broken extractor's regex line: {env}");
    // A basic string's escape spells a pattern that no line holds as such.
    let body = "\n\n".to_owned() + &extractor("escaped", "\"\\u0041(\"");
    let dir = pack_dir(tmp.path(), "escaped", &["code"], &body);
    let env = doctor(&dir);
    assert_invalid(&env, "A(", "the regex of escaped/escaped doesn't compile");
    assert_eq!(env["error"]["details"]["line"], 12, "the regex's own line: {env}");
}

/// A `[[questions]]` table whose first step filters `kinds` with no incoming `without` edge, and
/// whose second step walks `relations` under the lens `lens`.
fn question(kinds: &str, without: &str, relations: &str, lens: &str) -> String {
    format!(
        "[[questions]]\nid = \"unwatched\"\nsummary = \"Services nothing calls\"\n\n\
         [[questions.steps]]\nshape = \"filter\"\n\
         params = {{ kinds = [\"{kinds}\"], without_edge = {{ relation = \"{without}\", direction = \"in\" }} }}\n\n\
         [[questions.steps]]\nshape = \"neighbors\"\n\
         params = {{ relations = [\"{relations}\"], lens = \"{lens}\", depth = 1, direction = \"both\" }}\n"
    )
}

#[test]
fn a_question_step_references_only_kinds_in_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, requires: &[&str], body: String| pack_dir(tmp.path(), id, requires, &body);
    // Its kinds, its predicate's relation, its relations and its lens, all in scope, load.
    let dir = pack("callers", &["code"], question("code.service", "code.http", "code.imports", "runtime"));
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    // A kind no loaded pack declares.
    // A filter's kinds are node kinds, so the refusal names a kind.
    let dir = pack("ghosts", &["code"], question("code.flag", "code.http", "code.imports", "runtime"));
    assert_invalid(&doctor(&dir), "code.flag", "no loaded pack declares the kind code.flag");
    // A kind of a pack it doesn't require.
    let dir = pack("lonely", &[], question("code.service", "kernel.touches", "kernel.touches", "runtime"));
    assert_invalid(&doctor(&dir), "code.service", "doesn't require");
    // The predicate's relation and the walked relations are references too.
    let dir = pack("unseen", &["code"], question("code.service", "code.watches", "code.imports", "runtime"));
    assert_invalid(&doctor(&dir), "code.watches", "no loaded pack declares the relation code.watches");
    let dir = pack("walked", &["code"], question("code.service", "code.http", "code.calls", "runtime"));
    assert_invalid(&doctor(&dir), "code.calls", "no loaded pack declares the relation code.calls");
    // So is the lens: one no loaded pack declares, and one of a pack it doesn't require.
    let dir = pack("blind", &["code"], question("code.service", "code.http", "code.imports", "flows"));
    assert_invalid(&doctor(&dir), "flows", "no loaded pack declares the lens flows");
    let dir = pack("items", &[], question("kernel.item", "kernel.touches", "kernel.touches", "runtime"));
    assert_invalid(&doctor(&dir), "runtime", "runtime belongs to code, which this pack doesn't require");
}

/// A `[[rules]]` table of rule type `kind` over `subject`, with `params` as an inline table's body.
fn rule(kind: &str, subject: &str, params: &str) -> String {
    format!(
        "[[rules]]\nid = \"checked\"\nkind = \"{kind}\"\nsubject = \"{subject}\"\nparams = {{ {params} }}\n\
         message = \"It lacks a part\"\n"
    )
}

/// A one-step `[[questions]]` table of shape `shape`, with `params` as an inline table's body.
/// The question declares the arguments `X`, `A` and `B`.
fn step(shape: &str, params: &str) -> String {
    format!(
        "[[questions]]\nid = \"asked\"\nsummary = \"One step\"\n\
         args = [{{ name = \"X\" }}, {{ name = \"A\" }}, {{ name = \"B\" }}]\n\n\
         [[questions.steps]]\nshape = \"{shape}\"\nparams = {{ {params} }}\n"
    )
}

#[test]
fn a_parameter_key_its_shape_or_rule_type_doesnt_declare_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, body: String| pack_dir(tmp.path(), id, &["code"], &body);
    // Every key the shape or rule type declares loads.
    let dir = pack(
        "declared",
        step(
            "neighbors",
            "ids = [\"$X\"], lens = \"runtime\", relations = [\"code.http\"], depth = 2, direction = \"in\"",
        ) + "\n"
            + &rule(
                "min_children",
                "code.service",
                "min = 1, kinds = [\"code.file\"], \"props.role\" = \"service\"",
            ),
    );
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
    // A misspelt key names nothing the shape reads, so it would be ignored where it runs.
    let dir = pack("misspelt", step("neighbors", "relations = [\"code.http\"], dept = 2"));
    assert_invalid(&doctor(&dir), "dept", "dept isn't a parameter of the neighbors shape");
    // A key one shape declares isn't another's: path takes no relations.
    let dir = pack("borrowed", step("path", "from = \"$A\", to = \"$B\", relations = [\"code.http\"]"));
    assert_invalid(&doctor(&dir), "relations", "relations isn't a parameter of the path shape");
    // Inside a filter's edge test too.
    let dir = pack("nested", step("filter", "without_edge = { relation = \"code.http\", towards = \"in\" }"));
    assert_invalid(&doctor(&dir), "towards", "towards isn't a parameter of without_edge");
    // A rule's params go through its rule type's table.
    let dir = pack("ruled", rule("requires_edge", "code.service", "relation = \"code.http\", count = 2"));
    assert_invalid(&doctor(&dir), "count", "count isn't a parameter of the requires_edge rule type");
    // So do a decision kind's candidates, a query shape over its subject.
    let choice = |params: &str| {
        decision("code.file", "code.imports", "code.imports", "code.imports").replace(
            "question = { type = \"boolean\", instructions = \"Is it stale?\" }",
            &format!(
                "question = {{ type = \"choice\", instructions = \"Which one?\", \
                 options_from = {{ shape = \"neighbors\", params = {{ {params} }}, max = 3 }} }}"
            ),
        )
    };
    let env = doctor(&pack("chosen", choice("relations = [\"code.imports\"], depth = 1")));
    assert_eq!(env["ok"], true, "{env}");
    let dir = pack("unchosen", choice("relations = [\"code.imports\"], hops = 1"));
    assert_invalid(&doctor(&dir), "hops", "hops isn't a parameter of the neighbors shape");
}

#[test]
fn a_rule_naming_an_unknown_relation_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, params: &str| {
        pack_dir(tmp.path(), id, &["code"], &rule("requires_edge", "code.service", params))
    };
    // A relation in scope loads, the kernel's among them.
    let env = doctor(&pack("called", "relation = \"code.http\", direction = \"in\""));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack("touched", "relation = \"kernel.touches\", direction = \"in\""));
    assert_eq!(env["ok"], true, "{env}");
    // One no loaded pack declares.
    let dir = pack("watched", "relation = \"code.watches\", direction = \"in\"");
    assert_invalid(&doctor(&dir), "code.watches", "no loaded pack declares the relation code.watches");
    // A kind where the rule names a relation.
    let dir = pack("filed", "relation = \"code.file\"");
    assert_invalid(&doctor(&dir), "code.file", "no loaded pack declares the relation code.file");
}

#[test]
fn a_rule_naming_a_kind_of_a_pack_it_doesnt_require_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let body = |kinds: &str| rule("min_children", "kernel.item", &format!("min = 1, kinds = [\"{kinds}\"]"));
    // The kernel's kinds are every pack's, and a required pack's are this one's.
    let env = doctor(&pack_dir(tmp.path(), "planned", &[], &body("kernel.item")));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack_dir(tmp.path(), "filed", &["code"], &body("code.file")));
    assert_eq!(env["ok"], true, "{env}");
    // A kind of the code pack, which this pack doesn't require.
    let dir = pack_dir(tmp.path(), "lonely", &[], &body("code.file"));
    assert_invalid(&doctor(&dir), "code.file", "code.file belongs to code, which this pack doesn't require");
    // So is a requires_edge rule's other end, and a covers rule's children.
    let dir = pack_dir(
        tmp.path(),
        "touching",
        &[],
        &rule("requires_edge", "kernel.item", "relation = \"kernel.touches\", kinds = [\"code.service\"]"),
    );
    assert_invalid(&doctor(&dir), "code.service", "doesn't require");
    let dir = pack_dir(
        tmp.path(),
        "covering",
        &[],
        &rule("covers", "kernel.item", "prop = \"scope\", kinds = [\"code.module\"]"),
    );
    assert_invalid(&doctor(&dir), "code.module", "doesn't require");
}

#[test]
fn a_literal_id_whose_kind_short_no_kind_in_scope_declares_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let edge = |to: &str| {
        format!(
            "[[extractors]]\nid = \"links\"\nfiles = [\"**/*.rb\"]\nregex = '(\\w+)'\n\
             emit = [{{ edge = \"code.imports\", from = \"@id:code.file:$file\", to = \"{to}\" }}]\n"
        )
    };
    // A kind-short that is the id_prefix of a kind in scope loads: the code pack's, and the
    // kernel's prefix every Container-level kind shares.
    let env = doctor(&pack_dir(tmp.path(), "linked", &["code"], &edge("web:file:src/$1.ts")));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack_dir(tmp.path(), "hosted", &["code"], &edge("ws:container:$1")));
    assert_eq!(env["ok"], true, "{env}");
    // One no loaded kind declares.
    let dir = pack_dir(tmp.path(), "widgets", &["code"], &edge("ws:widget:$1"));
    assert_invalid(&doctor(&dir), "ws:widget:$1", "the ID ws:widget:$1 has the kind-short widget");
    // One that a pack this pack doesn't require declares.
    let parent = format!(
        "[[kinds]]\nid = \"flag\"\nid_prefix = \"flag\"\nlevel = \"code\"\nlabel = \"Flag\"\n\n\
         [[extractors]]\nid = \"flags\"\nfiles = [\"**/*.rb\"]\nregex = '(\\w+)'\n\
         emit = [{{ node = \"lonely.flag\", key = \"$1\", parent = \"{}\" }}]\n",
        "web:pkg:src"
    );
    let dir = pack_dir(tmp.path(), "lonely", &[], &parent);
    assert_invalid(
        &doctor(&dir),
        "web:pkg:src",
        "kind-short pkg, which belongs to code, which this pack doesn't require",
    );
    // A question step's from and to are node IDs too.
    let path = |from: &str| step("path", &format!("from = \"{from}\", to = \"$B\", lens = \"flow\""));
    let env = doctor(&pack_dir(tmp.path(), "routed", &["code"], &path("web:endpoint:get-cart")));
    assert_eq!(env["ok"], true, "{env}");
    let dir = pack_dir(tmp.path(), "unrouted", &["code"], &path("web:route:get-cart"));
    assert_invalid(&doctor(&dir), "web:route:get-cart", "the ID web:route:get-cart has the kind-short route");
}

#[test]
fn a_filter_given_a_relation_where_it_takes_node_kinds_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let filter = |kinds: &str| {
        step("filter", &format!("kinds = [\"{kinds}\"], \"props.role\" = {{ in = [\"service\"] }}"))
    };
    let env = doctor(&pack_dir(tmp.path(), "services", &["code"], &filter("code.service")));
    assert_eq!(env["ok"], true, "{env}");
    let dir = pack_dir(tmp.path(), "calls", &["code"], &filter("code.http"));
    assert_invalid(&doctor(&dir), "code.http", "code.http is a relation, but kinds names node kinds");
    // The kernel's relation is a relation too.
    let dir = pack_dir(tmp.path(), "touches", &[], &filter("kernel.touches"));
    assert_invalid(
        &doctor(&dir),
        "kernel.touches",
        "kernel.touches is a relation, but kinds names node kinds",
    );
}

#[test]
fn a_suggestion_cannot_name_an_irreversible_command() {
    let tmp = tempfile::tempdir().unwrap();
    let suggestion = |command: &str| {
        format!("[[suggestions]]\ntext = \"Do it\"\naction = {{ command = \"{command}\" }}\npriority = 1\n")
    };
    // `share` can't be undone: a viewer may already have seen what it published (DR-54).
    let dir = pack_dir(tmp.path(), "publish", &["code"], &suggestion("share"));
    assert_invalid(&doctor(&dir), "share", "can't name share");
    // A leaf this build doesn't have is refused too.
    let dir = pack_dir(tmp.path(), "ghost", &["code"], &suggestion("teleport"));
    assert_invalid(&doctor(&dir), "teleport", "can't name teleport");
    let dir = pack_dir(tmp.path(), "look", &["code"], &suggestion("map"));
    assert_eq!(doctor(&dir)["ok"], true);
    // `find` is a read, so a suggestion may name it, with the text it searches for; a flag it
    // doesn't take is refused as the CLI refuses it.
    let action = |args: &str| {
        format!(
            "[[suggestions]]\ntext = \"Search\"\naction = {{ command = \"find\", args = [{args}] }}\npriority = 1\n"
        )
    };
    let dir = pack_dir(tmp.path(), "search", &["code"], &action("\"checkout\""));
    assert_eq!(doctor(&dir)["ok"], true, "a suggestion may name find");
    let dir = pack_dir(tmp.path(), "searched", &["code"], &action("\"checkout\", \"--lens\", \"runtime\""));
    assert_invalid(&doctor(&dir), "find", "the action find checkout --lens runtime doesn't parse");
    // Every suggestion a built-in pack ships names a leaf the band may name: one that can be
    // undone, hands nothing off and never spends, so the pane's next action always is one.
    let loaded = packs::builtins().expect("the built-in packs load");
    assert!(!loaded.vocabulary.suggestions.is_empty(), "the code pack suggests a next action");
    for s in &loaded.vocabulary.suggestions {
        let spec = planroom_contract::Command::SPECS
            .iter()
            .find(|spec| spec.name() == s.action.command.0)
            .unwrap_or_else(|| panic!("{} is a leaf", s.action.command.0));
        assert!(!matches!(spec.undo, planroom_contract::Undo::NotUndoable(_)), "{}", spec.name());
        assert_ne!(spec.class, planroom_contract::Class::Handoff, "{}", spec.name());
        assert!(!spec.spends, "{}", spec.name());
    }
}

#[test]
fn doctor_pack_passes_on_every_pack() {
    let dirs = pack_dirs();
    assert!(dirs.iter().any(|d| d.ends_with("packs/code")), "{dirs:?}");
    for dir in dirs {
        let env = doctor(&dir);
        assert_eq!(env["ok"], true, "{dir:?}: {env}");
        assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 1_500, "doctor keeps to its budget");
        let checks: Vec<&str> = env["result"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["check"].as_str().unwrap())
            .collect();
        assert_eq!(checks.first(), Some(&"validate"), "{dir:?}");
        if dir.join("tests/inputs").is_dir() {
            for check in ["expected", "determinism", "evidence"] {
                assert!(checks.contains(&check), "{dir:?}: {checks:?}");
            }
        }
    }
    // The code pack ships a conformance kit, and a changed expectation fails it.
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("code");
    copy_dir(&repo_root().join("packs/code"), &copy);
    let expected = copy.join("tests/expected.jsonl");
    let text = fs::read_to_string(&expected).unwrap();
    assert!(text.lines().count() >= 2, "the code pack's kit has facts to compare");
    fs::write(&expected, text.replacen("code.queue", "code.service", 1)).unwrap();
    let env = doctor(&copy);
    assert_eq!(env["error"]["code"], "pack_invalid", "{env}");
    assert_eq!(env["error"]["details"]["reference"], "tests/expected.jsonl");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(entry.file_name()));
        } else {
            fs::copy(&path, to.join(entry.file_name())).unwrap();
        }
    }
}

#[test]
fn a_compose_image_takes_its_role_from_image_roles() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture(&tmp.path().join("shop")).root;
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", root.to_str().unwrap(), "--json"])
        .current_dir(&root)
        .env("PLANROOM_DATA", tmp.path().join("data"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().expect("a store");
    let active = store.active_set().unwrap();
    let containers: Vec<Value> = store
        .facts(&active.snapshots())
        .unwrap()
        .into_iter()
        .filter(|r| r.shape == Shape::Node)
        .map(|r| serde_json::from_str::<Value>(&r.body).unwrap())
        .filter(|n| n["level"] == "container" && n["props"]["image"].is_string())
        .collect();
    assert!(containers.len() >= 6, "{containers:?}");

    // The code pack's [[image_roles]], read from its manifest: first match in order.
    let manifest: toml::Table =
        fs::read_to_string(repo_root().join("packs/code/pack.toml")).unwrap().parse().unwrap();
    let roles: Vec<(String, String, String)> = manifest["image_roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let field = |k: &str| r[k].as_str().unwrap().to_owned();
            (field("image"), field("role"), field("kind"))
        })
        .collect();
    let mut matched = BTreeSet::new();
    for node in &containers {
        let image = node["props"]["image"].as_str().unwrap();
        let name = image.rsplit('/').next().unwrap();
        let rule = roles
            .iter()
            .find(|(glob, _, _)| name.starts_with(glob.trim_end_matches('*')) && glob.ends_with('*'));
        let (role, kind) = rule.map_or(("service", "code.service"), |(_, r, k)| (r.as_str(), k.as_str()));
        assert_eq!(node["props"]["role"], role, "{node}");
        assert_eq!(node["kind"], kind, "{node}");
        // Whatever role classifies it, a container keeps the kernel's prefix.
        assert!(node["id"].as_str().unwrap().contains(":container:"), "{node}");
        if rule.is_some() {
            matched.insert(node["name"].as_str().unwrap().to_owned());
        }
    }
    assert_eq!(
        matched,
        BTreeSet::from(["orders-db".to_owned(), "redis".to_owned()]),
        "the fixture's datastore and queue"
    );
    let redis = containers.iter().find(|n| n["name"] == "redis").unwrap();
    assert_eq!(redis["id"], "ws:container:redis");
    assert_eq!(
        (redis["kind"].as_str(), redis["props"]["role"].as_str()),
        (Some("code.queue"), Some("queue"))
    );
}

/// The line `env`'s error names.
fn line(env: &Value) -> u64 {
    env["error"]["details"]["line"].as_u64().expect("an error with a line")
}

/// A `[[decisions]]` table about `subject` (a quoted kind or an inline table), with `when` and
/// `effect` as inline tables' bodies. Its lines, from the manifest's line 7: the header, `id`,
/// `version`, `subject` (10), `when` (11), `question` (12), `slice` (13), `effect` (14), `stakes`,
/// `default`, `policy` (17) and `budget`.
fn decision_with(subject: &str, when: &str, effect: &str) -> String {
    format!(
        "[[decisions]]\nid = \"stale\"\nversion = 1\nsubject = {subject}\nwhen = {{ {when} }}\n\
         question = {{ type = \"boolean\", instructions = \"Is it stale?\" }}\n\
         slice = {{ query = \"neighbors\", relations = [\"kernel.touches\"], max_tokens = 500 }}\n\
         effect = {{ {effect} }}\n\
         stakes = \"person\"\ndefault = false\n\
         policy = {{ card_band = [0.15, 0.85], escalate_band = [0.35, 0.65] }}\n\
         budget = {{ max_calls_per_scan = 5 }}\n"
    )
}

#[test]
fn a_decision_effect_takes_only_its_effects_parameters() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, effect: &str| {
        pack_dir(tmp.path(), id, &["code"], &decision_with("\"code.file\"", "", effect))
    };
    // Each effect with the keys its table declares loads, `$pick` naming the chosen candidate.
    for (id, effect) in [
        ("tagged", "on = \"true\", do = \"set_prop\", prop = \"stale\", value = true"),
        ("moved", "on = \"true\", do = \"set_prop\", parent = \"$pick\""),
        ("linked", "do = \"put_edge\", relation = \"code.owned_by\", to = \"$pick\""),
        ("unlinked", "do = \"retract_edge\", relation = \"code.imports\", from = \"web:file:src/a.ts\""),
        ("linted", "on = \"false\", do = \"lint\", rule = \"vague\""),
        ("kept", "do = \"none\""),
    ] {
        let env = doctor(&pack(id, effect));
        assert_eq!(env["ok"], true, "{id}: {env}");
    }
    // A key the effect doesn't declare would be read by nothing, so it's refused at its line.
    let env = doctor(&pack("misspelt", "do = \"set_prop\", prop = \"stale\", valu = true"));
    assert_invalid(
        &env,
        "valu",
        "valu isn't a parameter of the set_prop effect; it takes prop, value, parent",
    );
    assert_eq!(line(&env), 14, "the effect's line: {env}");
    let env = doctor(&pack("idle", "do = \"none\", rule = \"vague\""));
    assert_invalid(&env, "rule", "rule isn't a parameter of the none effect; it takes no parameters");
    let env = doctor(&pack("rerouted", "do = \"retarget\", to = \"$pick\""));
    assert_invalid(&env, "to", "to isn't a parameter of the retarget effect");
    // A relation, and a literal node ID's kind-short, are references in scope.
    let env = doctor(&pack("unknown", "do = \"put_edge\", relation = \"code.watches\", to = \"$pick\""));
    assert_invalid(&env, "code.watches", "no loaded pack declares the relation code.watches");
    let env =
        doctor(&pack("widgets", "do = \"put_edge\", relation = \"code.imports\", to = \"web:widget:x\""));
    assert_invalid(&env, "web:widget:x", "the ID web:widget:x has the kind-short widget");
    let body = decision_with("\"kernel.item\"", "", "do = \"set_prop\", parent = \"web:pkg:src\"");
    let env = doctor(&pack_dir(tmp.path(), "items", &[], &body));
    assert_invalid(
        &env,
        "web:pkg:src",
        "kind-short pkg, which belongs to code, which this pack doesn't require",
    );
}

#[test]
fn a_when_predicate_takes_only_declared_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, subject: &str, when: &str| {
        pack_dir(tmp.path(), id, &["code"], &decision_with(subject, when, "do = \"none\""))
    };
    // Every key the predicate declares loads.
    let when = "kind = \"code.file\", id = \"web:file:src/a.ts\", parent = \"web:dir:src\", \
                status = \"live\", confidence = { lt = 0.8 }, \"props.enabled\" = false, \
                with_edge = { relation = \"code.imports\", direction = \"out\" }, \
                without_edge = { relation = \"code.owned_by\", direction = \"out\", layer = \"intended\" }";
    let env = doctor(&pack("full", "\"code.file\"", when));
    assert_eq!(env["ok"], true, "{env}");
    // An edge kind's subjects are edges, so their kinds are relations.
    let env =
        doctor(&pack("edged", "\"code.imports\"", "kinds = [\"code.imports\"], confidence = { lt = 1.0 }"));
    assert_eq!(env["ok"], true, "{env}");
    // A misspelt key, at the top and inside an edge test, is refused at its line.
    let env = doctor(&pack("misspelt", "\"code.file\"", "\"prop.enabled\" = false"));
    assert_invalid(&env, "prop.enabled", "prop.enabled isn't a parameter of a when predicate");
    assert_eq!(line(&env), 11, "the when line: {env}");
    let deep = "without_edge = { relation = \"code.imports\", direction = \"in\", lyer = \"intended\" }";
    let env = doctor(&pack("deep", "\"code.file\"", deep));
    assert_invalid(&env, "lyer", "lyer isn't a parameter of without_edge");
    // A node ID in it is in scope.
    let env = doctor(&pack("widgets", "\"code.file\"", "parent = \"web:widget:x\""));
    assert_invalid(&env, "web:widget:x", "the ID web:widget:x has the kind-short widget");
    // A node kind's subjects are nodes, so a relation where the predicate names their kind is
    // refused, and a node kind where an edge kind's subjects are named.
    let env = doctor(&pack("related", "\"code.file\"", "kind = \"code.imports\""));
    assert_invalid(&env, "code.imports", "code.imports is a relation, but kind names node kinds");
    let env = doctor(&pack("noded", "\"code.imports\"", "kinds = [\"code.file\"]"));
    assert_invalid(&env, "code.file", "no loaded pack declares the relation code.file");
    // A suggestion's when goes through the same table.
    let suggestion = |when: &str| {
        format!("[[suggestions]]\nwhen = {{ {when} }}\ntext = \"Look\"\naction = {{ command = \"map\" }}\n")
    };
    let env = doctor(&pack_dir(tmp.path(), "looked", &["code"], &suggestion("status = \"live\"")));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack_dir(tmp.path(), "unlooked", &["code"], &suggestion("statsu = \"live\"")));
    assert_invalid(&env, "statsu", "statsu isn't a parameter of a when predicate");
    assert_eq!(line(&env), 8, "the suggestion's when line: {env}");
}

#[test]
fn a_value_form_no_check_reads_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, when: &str| {
        pack_dir(tmp.path(), id, &["code"], &decision_with("\"code.file\"", when, "do = \"none\""))
    };
    // The forms the checks read load: a name, a list or an in list; a value or one comparison.
    let forms = "kinds = { in = [\"code.file\"] }, \"props.role\" = { in = [\"a\", \"b\"] }, \
                 confidence = { gt = 0.2 }, status = \"live\"";
    let env = doctor(&pack("forms", forms));
    assert_eq!(env["ok"], true, "{env}");
    // A table without an in list, or an in that isn't a list, names nothing a check reads.
    let env = doctor(&pack("eq", "kind = { eq = \"code.file\" }"));
    assert_invalid(&env, "kind", "kind names something: give a name, a list of names or an in list");
    let env = doctor(&pack("scalar_in", "kinds = { in = \"code.file\" }"));
    assert_invalid(&env, "kinds", "kinds names something");
    // A field test is a value or one comparison.
    let env = doctor(&pack("nin", "\"props.role\" = { nin = [\"a\"] }"));
    assert_invalid(&env, "props.role", "props.role is a field test");
    let env = doctor(&pack("in_scalar", "status = { in = \"live\" }"));
    assert_invalid(&env, "status", "status is a field test");
    // So in a filter step's prop tests, and in an effect's reference parameters.
    let filter = step("filter", "kinds = [\"code.service\"], \"props.role\" = { eq = \"service\" }");
    let env = doctor(&pack_dir(tmp.path(), "filtered", &["code"], &filter));
    assert_invalid(&env, "props.role", "props.role is a field test");
    let effect = "do = \"put_edge\", relation = { eq = \"code.imports\" }, to = \"$pick\"";
    let env =
        doctor(&pack_dir(tmp.path(), "effected", &["code"], &decision_with("\"code.file\"", "", effect)));
    assert_invalid(&env, "relation", "relation names something");
}

#[test]
fn a_suggestion_action_parses_as_the_cli_parses_it() {
    let tmp = tempfile::tempdir().unwrap();
    let suggestion = |args: &str| {
        format!(
            "[[suggestions]]\ntext = \"Look\"\naction = {{ command = \"map\", args = [{args}] }}\npriority = 1\n"
        )
    };
    let pack =
        |id: &str, requires: &[&str], args: &str| pack_dir(tmp.path(), id, requires, &suggestion(args));
    // Arguments the leaf takes, naming what's in scope, load: `all` names no lens, and a name
    // or an ID suffix names no kind-short.
    for (id, args) in [
        ("lensed", "\"--lens\", \"runtime\""),
        ("joined", "\"--lens=flow\""),
        ("everything", "\"--lens\", \"all\", \"--level\", \"container\""),
        ("focused", "\"--focus\", \"web:file:src/a.ts\""),
        ("named", "\"--focus\", \"orders\""),
    ] {
        let env = doctor(&pack(id, &["code"], args));
        assert_eq!(env["ok"], true, "{id}: {env}");
    }
    // A flag the leaf doesn't take, and a value its flag refuses.
    let env = doctor(&pack("flagged", &["code"], "\"--bogus\""));
    assert_invalid(
        &env,
        "map",
        "the action map --bogus doesn't parse as the CLI parses it: unexpected argument",
    );
    assert_eq!(line(&env), 9, "the action's line: {env}");
    let env = doctor(&pack("leveled", &["code"], "\"--level\", \"galaxy\""));
    assert_invalid(&env, "map", "the action map --level galaxy doesn't parse as the CLI parses it");
    // The lens and the node ID it names are in scope.
    let env = doctor(&pack("blind", &["code"], "\"--lens\", \"flows\""));
    assert_invalid(&env, "flows", "no loaded pack declares the lens flows");
    let env = doctor(&pack("lonely", &[], "\"--lens\", \"runtime\""));
    assert_invalid(&env, "runtime", "runtime belongs to code, which this pack doesn't require");
    let env = doctor(&pack("widgets", &["code"], "\"--focus\", \"web:widget:x\""));
    assert_invalid(&env, "web:widget:x", "the ID web:widget:x has the kind-short widget");
}

#[test]
fn an_unknown_key_anywhere_in_a_pack_is_pack_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    // Lines 7 to 11, then a blank line, then the extractor at 13 to 17.
    let kind = "[[kinds]]\nid = \"flag\"\nid_prefix = \"flag\"\nlevel = \"code\"\nlabel = \"Flag\"\n";
    let extractor = "[[extractors]]\nid = \"flags\"\nfiles = [\"**/*.yaml\"]\nregex = '(\\w+)'\n\
                     emit = [{ node = \"code.file\", key = \"$1\" }]\n";
    let role = decision_with("{ role = \"rescan.retarget\" }", "", "do = \"none\"");
    // Spelled as the manifest declares, everything loads.
    let env = doctor(&pack_dir(tmp.path(), "spelled", &["code"], &format!("{kind}\n{extractor}")));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack_dir(tmp.path(), "roled", &["code"], &role));
    assert_eq!(env["ok"], true, "{env}");
    // Each misspelt key is refused at its own line: at the top, in a declaration, in an
    // extractor and an emit (which untagged enums read), in a role subject (another), in a policy
    // and in a question (a tagged enum).
    let cases = [
        ("top", format!("colour = \"red\"\n{kind}"), "colour", 7, "colour isn't a key of pack.toml"),
        ("kind", kind.replace("label", "lable"), "lable", 11, "lable isn't a key of [[kinds]]; it takes"),
        (
            "extractor",
            format!("{kind}\n{}", extractor.replace("regex =", "langauge = \"go\"\nregex =")),
            "langauge",
            16,
            "langauge isn't a key of [[extractors]]",
        ),
        (
            "emit",
            format!("{kind}\n{}", extractor.replace("key = \"$1\"", "key = \"$1\", nmae = \"x\"")),
            "nmae",
            17,
            "nmae isn't a key of [[extractors]] emit",
        ),
        (
            "subject",
            role.replace("rescan.retarget\"", "rescan.retarget\", extra = 1"),
            "extra",
            10,
            "extra isn't a key of [[decisions]] subject",
        ),
        (
            "policy",
            role.replace("policy = { card_band", "policy = { auto = 0.9, card_band"),
            "auto",
            17,
            "auto isn't a key of [[decisions]] policy",
        ),
        (
            "question",
            role.replace("\"Is it stale?\"", "\"Is it stale?\", critera = { true = \"a\", false = \"b\" }"),
            "critera",
            12,
            "critera isn't a key of [[decisions]] question",
        ),
    ];
    for (id, body, key, at, says) in cases {
        let env = doctor(&pack_dir(tmp.path(), id, &["code"], &body));
        assert_invalid(&env, key, says);
        assert_eq!(line(&env), at, "{id}: {env}");
    }
    // A languages file's keys count too, at their own line.
    let dir = pack_dir(tmp.path(), "langs", &["code"], "");
    fs::create_dir_all(dir.join("languages")).unwrap();
    let file = dir.join("languages/flags.toml");
    fs::write(&file, format!("{}limit = 3\n", extractor)).unwrap();
    let env = doctor(&dir);
    assert_invalid(&env, "limit", "limit isn't a key of [[extractors]]");
    assert_eq!(line(&env), 6, "{env}");
    assert!(env["error"]["details"]["file"].as_str().unwrap().ends_with("languages/flags.toml"), "{env}");
    fs::write(&file, format!("{extractor}\n{kind}")).unwrap();
    let env = doctor(&dir);
    assert_invalid(&env, "kinds", "a languages file holds only [[languages]] and [[extractors]], not kinds");
    assert_eq!(line(&env), 7, "the [[kinds]] header: {env}");
    fs::write(&file, extractor).unwrap();
    let env = doctor(&dir);
    assert_eq!(env["ok"], true, "{env}");
}

#[test]
fn a_question_step_names_only_its_declared_arguments() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, args: &str, params: &str| {
        let body = format!(
            "[[questions]]\nid = \"asked\"\nsummary = \"How {{A}} reaches {{B}}\"\nargs = [{args}]\n\n\
             [[questions.steps]]\nshape = \"path\"\nparams = {{ {params} }}\n"
        );
        pack_dir(tmp.path(), id, &["code"], &body)
    };
    // Each argument a step names, whole or inside an ID, is one the question declares.
    let declared = "{ name = \"A\" }, { name = \"B\", optional = true }";
    let env = doctor(&pack("declared", declared, "from = \"$A\", to = \"$B:dir:src\", lens = \"flow\""));
    assert_eq!(env["ok"], true, "{env}");
    // A step that names an argument the question doesn't declare would run with nothing bound.
    let env = doctor(&pack("misnamed", declared, "from = \"$A\", to = \"$C\""));
    assert_invalid(
        &env,
        "$C",
        "$C names the argument C, which the question doesn't declare; it declares A, B",
    );
    let env = doctor(&pack("inside", declared, "from = \"$A\", to = \"storefront:dir:$D\""));
    assert_invalid(&env, "storefront:dir:$D", "names the argument D, which the question doesn't declare");
    let env = doctor(&pack("bare", "", "from = \"$A\", to = \"$B\""));
    assert_invalid(
        &env,
        "$A",
        "$A names the argument A, which the question doesn't declare; it declares no arguments",
    );
}

#[test]
fn a_parameter_that_names_nothing_refuses_a_table() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, body: String| pack_dir(tmp.path(), id, &["code"], &body);
    // A number, a word or a list of them loads where a key names nothing.
    let env = doctor(&pack("plain", step("subgraph", "lens = \"runtime\", props = [\"role\", \"version\"]")));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack("worded", step("neighbors", "ids = [\"$X\"], depth = 2, direction = \"both\"")));
    assert_eq!(env["ok"], true, "{env}");
    // A table there has keys nothing reads, so it is refused, as is a list that holds one.
    let env = doctor(&pack("tabled", step("neighbors", "ids = [\"$X\"], depth = { max = 2 }")));
    assert_invalid(
        &env,
        "depth",
        "depth names nothing, so it takes a number, a word or text, or a list of them, never a table",
    );
    let env = doctor(&pack("listed", step("subgraph", "props = [{ name = \"role\" }]")));
    assert_invalid(&env, "props", "props names nothing");
    // Inside a table parameter too: an edge test's direction.
    let env = doctor(&pack(
        "nested",
        step("filter", "without_edge = { relation = \"code.http\", direction = { of = \"in\" } }"),
    ));
    assert_invalid(&env, "direction", "direction names nothing");
    // And in a rule type's parameters.
    let env = doctor(&pack("ruled", rule("min_children", "code.service", "min = { at_least = 1 }")));
    assert_invalid(&env, "min", "min names nothing");
}

#[test]
fn a_placeholder_stands_in_only_where_the_docs_allow_one() {
    let tmp = tempfile::tempdir().unwrap();
    let pack = |id: &str, body: String| pack_dir(tmp.path(), id, &["code"], &body);
    let edge = |to: &str| {
        format!(
            "[[extractors]]\nid = \"links\"\nfiles = [\"**/*.rb\"]\nregex = '(\\w+)'\n\
             emit = [{{ edge = \"code.imports\", from = \"@id:code.file:$file\", to = \"{to}\" }}]\n"
        )
    };
    // Where the docs allow one: a question's argument in a step, `$pick` in an effect, and an
    // emit's captures around a literal kind-short.
    for (id, body) in [
        ("argued", step("neighbors", "ids = [\"$X\", \"web:file:$X\"], depth = 1")),
        (
            "picked",
            decision_with(
                "\"code.file\"",
                "",
                "do = \"put_edge\", relation = \"code.imports\", to = \"$pick\"",
            ),
        ),
        ("captured", edge("$repo:file:$1")),
    ] {
        let env = doctor(&pack(id, body));
        assert_eq!(env["ok"], true, "{id}: {env}");
    }
    // A kind, a relation or a lens is always spelled out.
    let env = doctor(&pack("kinded", step("filter", "kinds = [\"$K\"]")));
    assert_invalid(&env, "$K", "$K holds a placeholder, but a kind, relation or lens is always spelled out");
    let env = doctor(&pack("lensed", step("neighbors", "ids = [\"$X\"], lens = \"$L\"")));
    assert_invalid(&env, "$L", "always spelled out");
    let env = doctor(&pack("related", step("neighbors", "relations = [\"code.$R\"]")));
    assert_invalid(&env, "code.$R", "always spelled out");
    // A placeholder beside a literal kind-short still has the kind-short checked.
    let env = doctor(&pack("mixed", step("neighbors", "ids = [\"$X:widget:k\"]")));
    assert_invalid(&env, "$X:widget:k", "the ID $X:widget:k has the kind-short widget");
    let env = doctor(&pack("captured_widget", edge("$repo:widget:$1")));
    assert_invalid(&env, "$repo:widget:$1", "the ID $repo:widget:$1 has the kind-short widget");
    let env = doctor(&pack("shortless", edge("ws:$1:x")));
    assert_invalid(&env, "ws:$1:x", "has a placeholder for its kind-short");
    // Nowhere else: not in a candidates query, a predicate, or an effect beside `$pick`.
    let choice = decision_with("\"code.file\"", "", "do = \"none\"").replace(
        "question = { type = \"boolean\", instructions = \"Is it stale?\" }",
        "question = { type = \"choice\", instructions = \"Which?\", \
         options_from = { shape = \"neighbors\", params = { ids = [\"$X\"] }, max = 3 } }",
    );
    let env = doctor(&pack("chosen", choice));
    assert_invalid(&env, "$X", "$X is a placeholder where none stands in");
    let env = doctor(&pack("whened", decision_with("\"code.file\"", "parent = \"$X\"", "do = \"none\"")));
    assert_invalid(&env, "$X", "$X is a placeholder where none stands in");
    let effect = "do = \"put_edge\", relation = \"code.imports\", to = \"$X\"";
    let env = doctor(&pack("unpicked", decision_with("\"code.file\"", "", effect)));
    assert_invalid(&env, "$X", "$X is a placeholder where none stands in");
}

#[test]
fn pack_invalid_names_the_line_of_its_own_key_or_value() {
    let tmp = tempfile::tempdir().unwrap();
    // An earlier line holds the unknown kind's text, as a label: the error names the parents
    // line, 18, not line 11.
    let body = "[[kinds]]\nid = \"flag\"\nid_prefix = \"flag\"\nlevel = \"code\"\nlabel = \"code.flags\"\n\n\
                [[kinds]]\nid = \"toggle\"\nid_prefix = \"toggle\"\nlevel = \"code\"\nlabel = \"Toggle\"\n\
                parents = [\"code.flags\"]\n";
    let env = doctor(&pack_dir(tmp.path(), "labels", &["code"], body));
    assert_invalid(&env, "code.flags", "no loaded pack declares the kind code.flags");
    assert_eq!(line(&env), 18, "{env}");
    // An earlier step sets the same key, which that step's shape declares: the error names the
    // second step's params line, 18, not line 14.
    let body = "[[questions]]\nid = \"asked\"\nsummary = \"Two steps\"\n\
                args = [{ name = \"A\" }, { name = \"B\" }]\n\n\
                [[questions.steps]]\nshape = \"neighbors\"\nparams = { depth = 1 }\n\n\
                [[questions.steps]]\nshape = \"path\"\nparams = { from = \"$A\", to = \"$B\", depth = 2 }\n";
    let env = doctor(&pack_dir(tmp.path(), "depths", &["code"], body));
    assert_invalid(&env, "depth", "depth isn't a parameter of the path shape");
    assert_eq!(line(&env), 18, "{env}");
    // A later line holds a claimed key's text: the error names the claim's own line, 9.
    let body = "[[languages]]\nid = \"deno\"\nextensions = [\"ts\"]\n\n\
                [[languages]]\nid = \"other\"\nextensions = [\"zz\"]\nline_comment = \"ts\"\n";
    let env = doctor(&pack_dir(tmp.path(), "claims", &["code"], body));
    assert_invalid(&env, "ts", "already claimed by the code pack");
    assert_eq!(line(&env), 9, "{env}");
    // An item of an array that spans lines is named at its own line.
    let body = "[[lenses]]\nid = \"watch\"\nlabel = \"Watch\"\nrelations = [\n  \"code.http\",\n  \"code.watches\",\n]\n\
                level = \"container\"\n";
    let env = doctor(&pack_dir(tmp.path(), "spread", &["code"], body));
    assert_invalid(&env, "code.watches", "no loaded pack declares the relation code.watches");
    assert_eq!(line(&env), 12, "{env}");
}

/// One parameter as architecture.md's tables spell it: its label, or a nested table's keys.
#[derive(Debug, PartialEq)]
enum Spelled {
    Label(String),
    Table(Vec<(String, Spelled)>),
}

/// A contract table as the docs spell it, sorted by key: `props.*` is `props.<name>`.
fn spelled(table: &[planroom_contract::Param]) -> Vec<(String, Spelled)> {
    use planroom_contract::ParamNames;
    let mut out: Vec<(String, Spelled)> = table
        .iter()
        .map(|p| {
            let label = |l: &str| Spelled::Label(l.to_owned());
            let names = match p.names {
                ParamNames::Nothing => label(""),
                ParamNames::Test => label("field test"),
                ParamNames::NodeKind => label("node kinds"),
                ParamNames::SubjectKind => label("the subject's kinds"),
                ParamNames::Relation => label("relations"),
                ParamNames::Lens => label("lens"),
                ParamNames::NodeId => label("node ID"),
                ParamNames::Table(inner) => Spelled::Table(spelled(inner)),
            };
            (p.key.replace(".*", ".<name>"), names)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A Parameters cell, sorted by key: each backticked key at its top level with the label in
/// the parentheses after it, and `a table of ...` read the same way. `none` holds no keys.
fn parse_cell(cell: &str) -> Vec<(String, Spelled)> {
    let chars: Vec<char> = cell.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '`' {
            i += 1;
            continue;
        }
        let end = i + 1 + chars[i + 1..].iter().position(|c| *c == '`').expect("a closing backtick");
        let key: String = chars[i + 1..end].iter().collect();
        i = end + 1;
        let mut label = String::new();
        if chars[i..].starts_with(&[' ', '(']) {
            let (mut depth, mut j) = (0, i + 1);
            loop {
                match chars[j] {
                    '(' => depth += 1,
                    ')' if depth == 1 => break,
                    ')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            label = chars[i + 2..j].iter().collect();
            i = j + 1;
        }
        let names = match label.strip_prefix("a table of ") {
            Some(inner) => Spelled::Table(parse_cell(inner)),
            None => Spelled::Label(label.replace("node IDs", "node ID")),
        };
        out.push((key, names));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The rows of the architecture.md table whose header line starts with `header`, as cells.
fn doc_rows(text: &str, header: &str) -> Vec<Vec<String>> {
    let mut lines = text.lines().skip_while(|l| !l.starts_with(header));
    assert!(lines.next().is_some(), "architecture.md has the table {header}");
    lines
        .skip(1)
        .take_while(|l| l.starts_with('|'))
        .map(|l| l.trim().trim_matches('|').split(" | ").map(|c| c.trim().to_owned()).collect())
        .collect()
}

/// A table of named rows: each row's name to its Parameters cell.
fn doc_table(text: &str, header: &str) -> BTreeMap<String, Vec<(String, Spelled)>> {
    doc_rows(text, header)
        .into_iter()
        .map(|r| (r[0].trim_matches('`').to_owned(), parse_cell(&r[1])))
        .collect()
}

/// A shape's, a rule type's or an effect's name as a pack spells it.
fn serde_name<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value).unwrap().as_str().unwrap().to_owned()
}

#[test]
fn the_parameter_tables_in_the_docs_match_the_contract() {
    use planroom_contract::{EffectOp, RuleType, Shape};
    let text = fs::read_to_string(repo_root().join("docs/contributing/architecture.md")).unwrap();
    // Each list is checked whole: a new variant fails to compile in its match until it joins.
    let shapes = {
        use Shape::*;
        let all = [Node, Find, Neighbors, Path, Why, Filter, Compare, Order, Subgraph, Tree, Machine, Checks];
        for s in all {
            match s {
                Node | Find | Neighbors | Path | Why | Filter | Compare | Order | Subgraph | Tree
                | Machine | Checks => {}
            }
        }
        all
    };
    let doc = doc_table(&text, "| Shape | Parameters |");
    assert_eq!(doc.len(), shapes.len(), "one row per shape: {:?}", doc.keys());
    for shape in shapes {
        let name = serde_name(shape);
        assert_eq!(doc.get(&name), Some(&spelled(shape.params())), "the {name} shape's row");
    }
    let rules = [RuleType::RequiresProp, RuleType::RequiresEdge, RuleType::MinChildren, RuleType::Covers];
    for rule in rules {
        match rule {
            RuleType::RequiresProp | RuleType::RequiresEdge | RuleType::MinChildren | RuleType::Covers => {}
        }
    }
    let doc = doc_table(&text, "| Rule type | Parameters |");
    assert_eq!(doc.len(), rules.len(), "one row per rule type: {:?}", doc.keys());
    for rule in rules {
        let name = serde_name(rule);
        assert_eq!(doc.get(&name), Some(&spelled(rule.params())), "the {name} rule type's row");
    }
    let effects = {
        use EffectOp::*;
        let all = [None, Lint, SetProp, PutEdge, RetractEdge, Retarget];
        for e in all {
            match e {
                None | Lint | SetProp | PutEdge | RetractEdge | Retarget => {}
            }
        }
        all
    };
    let doc = doc_table(&text, "| Effect | Parameters |");
    assert_eq!(doc.len(), effects.len(), "one row per effect: {:?}", doc.keys());
    for effect in effects {
        let name = serde_name(effect);
        assert_eq!(doc.get(&name), Some(&spelled(effect.params())), "the {name} effect's row");
    }
    // The predicate's table: a row's keys share its label.
    let mut doc: Vec<(String, Spelled)> = Vec::new();
    for row in doc_rows(&text, "| Predicate key | What it names |") {
        for (key, _) in parse_cell(&row[0]) {
            let names = match row[1].strip_prefix("a table of ") {
                Some(inner) => Spelled::Table(parse_cell(inner)),
                None => Spelled::Label(row[1].clone()),
            };
            doc.push((key, names));
        }
    }
    doc.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(doc, spelled(planroom_contract::PREDICATE_PARAMS), "the predicate's table");
}

/// Every declared ID, a question's included, is at most 64 characters, so a reply can always show
/// it whole: a longer one is refused at load, where the pack names it.
#[test]
fn a_declared_id_past_64_characters_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let kind = |id: &str| {
        format!(
            "[[kinds]]\nid = \"{id}\"\nid_prefix = \"thing\"\nlevel = \"code\"\nlabel = \"Thing\"\nparents = [\"code.file\"]\n"
        )
    };
    let question = |id: &str| {
        format!(
            "[[questions]]\nid = \"{id}\"\nsummary = \"What touches {{A}}\"\nargs = [{{ name = \"A\" }}]\n\n\
             [[questions.steps]]\nshape = \"neighbors\"\nparams = {{ ids = [\"$A\"] }}\n"
        )
    };
    let (fits, long) = ("k".repeat(64), "k".repeat(65));
    let env = doctor(&pack_dir(tmp.path(), "fits", &["code"], &(kind(&fits) + "\n" + &question(&fits))));
    assert_eq!(env["ok"], true, "{env}");
    let env = doctor(&pack_dir(tmp.path(), "long_kind", &["code"], &kind(&long)));
    assert_invalid(&env, &long, "is longer than 64 characters");
    let env = doctor(&pack_dir(tmp.path(), "long_question", &["code"], &question(&long)));
    assert_invalid(&env, &long, "is longer than 64 characters");
}
