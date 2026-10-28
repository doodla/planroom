//! The extractor contract for the built-ins (`files`, `compose`, `manifest`, `openapi` and `git`)
//! and the declarative runner: deterministic, sorted, tolerant, bounded in sorted path order, code
//! and never comments, emit props typed by the prop schema, and every fact's evidence verifies
//! against the commit.

mod code_pack;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use planroom_contract::{
    ErrorCode, Evidence, ExtractorDecl, FactBatch, LanguageDecl, Level, NodeId, PartialFile, PartialReason,
    Ref, Vocabulary,
};
use planroom_extract::declarative::{
    CAPTURE_FLOOR, CAPTURE_PER_BYTE, Declarative, Invalid, Spec, Step, TEMPLATE_MAX_VARS, Why,
};
use planroom_extract::git::{Git, is_repo};
use planroom_extract::link::WorkspaceFacts;
use planroom_extract::verify::{self, Verifier};
use planroom_extract::walk::Blobs;
use planroom_extract::{RepoPath, RepoView, builtin, select, walk};
use serde_json::json;

const COMPOSE: &str = "\
# All services on one host.
name: shop

services:
  web:
    image: acme/web:2.0.0
    environment:
      - API_URL=http://api:8080
    depends_on:
      - api

  api:
    build: ../api
    environment:
      DATABASE_URL: postgres://db:5432/app
    depends_on:
      db:
        condition: service_started

  db:
    image: postgres:16.4
";

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .env("GIT_AUTHOR_DATE", "2026-10-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-10-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A committed repo named `deploy` holding a compose file, a nested directory, an empty file, a
/// file that isn't UTF-8, a binary and an ignored file.
fn repo(dir: &Path) -> RepoView {
    let root = dir.join("deploy");
    fs::create_dir_all(root.join("src/app")).unwrap();
    fs::write(root.join("docker-compose.yml"), COMPOSE).unwrap();
    fs::write(root.join("src/app/main.ts"), "export const x = 1;\n").unwrap();
    fs::write(root.join("src/empty.txt"), "").unwrap();
    fs::write(root.join("src/latin1.txt"), [0xe9u8, b'\n']).unwrap();
    fs::write(root.join("logo.png"), [0x89u8, 0, 1, 2]).unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("debug.log"), "noise\n").unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["add", "-A"]);
    git(&root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
    view(&root)
}

/// The repo at `root` at its `HEAD`, holding the walk's files.
fn view(root: &Path) -> RepoView {
    let git = Git::new(root);
    let head = git.head().unwrap();
    let blobs = walk::walk(&git, &head.commit, &walk::Exclude::none()).unwrap().blobs;
    RepoView { name: "deploy".into(), root: root.to_path_buf(), commit: head.commit, blobs }
}

fn walked(view: &RepoView) -> walk::Walked {
    walk::walk(&Git::new(&view.root), &view.commit, &walk::Exclude::none()).unwrap()
}

/// Run the built-in `name` with `opts`, plus the kinds the code pack names for it, as the engine
/// fills them in from the loaded vocabulary.
fn run(view: &RepoView, name: &str, files: &[RepoPath], opts: serde_json::Value) -> FactBatch {
    let kind = |kind: &str, id_prefix: &str, level: &str| json!({"kind": kind, "id_prefix": id_prefix, "level": level});
    let repo = kind("code.repo", "repo", "system");
    let directory = kind("code.directory", "dir", "component");
    let kinds = match name {
        "files" => json!({"repo": repo, "directory": directory, "file": kind("code.file", "file", "code")}),
        "manifest" => json!({"package": kind("code.package", "pkg", "component"),
                             "library": kind("code.library", "container", "container"),
                             "repo": repo, "directory": directory}),
        "openapi" => json!({"spec": kind("code.api_spec", "spec", "component"), "repo": repo,
                            "directory": directory}),
        "git" => json!({"repo": repo, "file": kind("code.file", "file", "code"),
                        "person": kind("code.person", "person", "component")}),
        _ => json!({"service": kind("code.service", "container", "container"),
                    "tooling": kind("code.tooling", "container", "container"), "repo": repo,
                    "file": kind("code.file", "file", "code")}),
    };
    let mut opts = if opts.is_null() { json!({}) } else { opts };
    opts["kinds"] = kinds;
    if name == "git" {
        opts["relations"] = json!({"co_change": "code.co_change", "owned_by": "code.owned_by"});
    }
    builtin(name).unwrap().run(view, files, &opts).unwrap()
}

fn compose_files(files: &[RepoPath]) -> Vec<RepoPath> {
    files.iter().filter(|f| f.0.ends_with("docker-compose.yml")).cloned().collect()
}

#[test]
fn files_emit_the_repo_its_directories_and_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    assert!(is_repo(&view.root));
    let walked = walked(&view);
    let paths: Vec<&str> = walked.files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        paths,
        [".gitignore", "docker-compose.yml", "src/app/main.ts", "src/empty.txt", "src/latin1.txt"]
    );
    let opts = json!({"languages": [{"id": "typescript", "extensions": ["ts"]}, {"id": "yaml", "extensions": ["yml"]}]});
    let batch = run(&view, "files", &walked.files, opts);
    let ids: Vec<&str> = batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(
        ids,
        [
            "deploy:dir:src",
            "deploy:dir:src/app",
            "deploy:file:.gitignore",
            "deploy:file:docker-compose.yml",
            "deploy:file:src/app/main.ts",
            "deploy:file:src/empty.txt",
            "deploy:file:src/latin1.txt",
            "deploy:repo:deploy",
        ]
    );
    let node = |id: &str| batch.nodes.iter().find(|n| n.id.0 == id).unwrap();
    let main = node("deploy:file:src/app/main.ts");
    assert_eq!((main.level, main.parent.as_ref().unwrap().0.as_str()), (Level::Code, "deploy:dir:src/app"));
    assert_eq!(main.props["language"], "typescript");
    assert!(
        matches!(&main.evidence[0], Evidence::Line { line: 1, contains: Some(c), .. } if c == "export const x = 1;")
    );
    assert!(matches!(node("deploy:file:src/empty.txt").evidence[0], Evidence::Ref { .. }));
    assert_eq!(node("deploy:dir:src").parent.as_ref().unwrap().0, "deploy:repo:deploy");
    assert_eq!(node("deploy:repo:deploy").props["languages"], json!(["typescript", "yaml"]));
    assert_eq!(batch.partial.len(), 1, "the non-UTF-8 file is partial");
}

/// The code pack's merged `[[languages]]` table, as the pack loader merges it: `pack.toml`'s
/// own, then each `languages/*.toml` file's in path order; and the IDs each file declares.
fn code_languages() -> (Vec<LanguageDecl>, BTreeMap<String, Vec<String>>) {
    #[derive(serde::Deserialize)]
    struct Table {
        #[serde(default)]
        languages: Vec<LanguageDecl>,
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/code");
    // Every other table is left unread.
    let read = |path: &Path| -> Vec<LanguageDecl> {
        toml::from_str::<Table>(&fs::read_to_string(path).unwrap()).unwrap().languages
    };
    let mut merged = read(&dir.join("pack.toml"));
    let mut by_file = BTreeMap::new();
    let mut paths: Vec<_> = fs::read_dir(dir.join("languages"))
        .map(|d| d.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    paths.sort();
    for path in paths {
        let declared = read(&path);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        by_file.insert(name, declared.iter().map(|l| l.id.clone()).collect());
        merged.extend(declared);
    }
    (merged, by_file)
}

#[test]
fn files_language_from_the_languages_table() {
    let (languages, by_file) = code_languages();
    // Each of the six grammars' languages is one file, beside the extractors that read it.
    for id in ["typescript", "tsx", "javascript", "python", "go", "rust"] {
        assert_eq!(by_file.get(&format!("{id}.toml")), Some(&vec![id.to_owned()]), "{by_file:?}");
    }
    let files = [
        ("src/a.ts", "typescript"),
        ("src/b.mts", "typescript"),
        ("src/c.cts", "typescript"),
        ("src/d.tsx", "tsx"),
        ("src/e.js", "javascript"),
        ("src/f.mjs", "javascript"),
        ("src/g.cjs", "javascript"),
        ("src/h.jsx", "javascript"),
        ("src/i.py", "python"),
        ("src/j.go", "go"),
        ("src/k.rs", "rust"),
        ("deploy/l.yml", "yaml"),
        ("deploy/m.yaml", "yaml"),
        ("deploy/n.json", "json"),
    ];
    // A file whose extension no loaded pack declares has no language: Ruby comes from its pack.
    let undeclared = ["tools/check.rb", "Makefile", "README.md"];
    let texts: Vec<(&str, &str)> =
        files.iter().map(|(p, _)| (*p, "x\n")).chain(undeclared.iter().map(|p| (*p, "x\n"))).collect();
    let (view, paths) = memory(&texts);
    let batch = run(&view, "files", &paths, json!({ "languages": languages }));
    let language = |path: &str| {
        let node = batch.nodes.iter().find(|n| n.id.0 == format!("web:file:{path}")).unwrap();
        node.props.get("language").and_then(|l| l.as_str()).map(str::to_owned)
    };
    for (path, want) in files {
        assert_eq!(language(path).as_deref(), Some(want), "{path}");
    }
    for path in undeclared {
        assert_eq!(language(path), None, "{path}");
    }
    // The repo's languages are the union over its files.
    let repo = batch.nodes.iter().find(|n| n.id.0 == "web:repo:web").unwrap();
    assert_eq!(
        repo.props["languages"],
        json!(["go", "javascript", "json", "python", "rust", "tsx", "typescript", "yaml"])
    );
}

/// The `runtime_wiring` linker's options, with the kinds and relations the code pack names for it,
/// as the engine fills them in from the loaded vocabulary.
fn link_opts() -> serde_json::Value {
    let kind = |kind: &str, id_prefix: &str, level: &str| json!({"kind": kind, "id_prefix": id_prefix, "level": level});
    let from = json!(["code.service", "code.library", "code.tooling"]);
    json!({
        "kinds": {"service": kind("code.service", "container", "container"),
                  "repo": kind("code.repo", "repo", "system"),
                  "endpoint": kind("code.endpoint", "endpoint", "code")},
        "relations": {"http": {"kind": "code.http", "from": from, "to": ["code.service", "code.library"]},
                      "queue_publish": {"kind": "code.queue_publish", "from": from, "to": ["code.queue"]},
                      "queue_consume": {"kind": "code.queue_consume", "from": from, "to": ["code.queue"]},
                      "db": {"kind": "code.db", "from": from, "to": ["code.datastore"]}},
    })
}

/// Compose reads one repo: the tooling container and a hint for each service, its environment and
/// its dependencies, all scoped to the compose file. The `runtime_wiring` linker makes each
/// service's container from its hint and joins it to the repo its build context or image names.
#[test]
fn compose_emits_containers_aliases_and_hints_on_their_lines() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let files = compose_files(&walked(&view).files);
    let opts =
        json!({"image_roles": [{"image": "postgres*", "role": "datastore", "kind": "code.datastore"}]});
    let batch = run(&view, "compose", &files, opts);
    let facts = WorkspaceFacts {
        repos: ["deploy", "api", "web"].iter().map(|r| (*r).to_owned()).collect(),
        nodes: batch.nodes.clone(),
        hints: batch.hints.clone(),
    };
    let linked =
        planroom_extract::link::builtin("runtime_wiring").unwrap().link(&facts, &link_opts()).unwrap();
    assert_eq!(linked.unresolved, []);
    let mut ids: Vec<&str> = batch.nodes.iter().chain(&linked.batch.nodes).map(|n| n.id.0.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["api:container:api", "deploy:container:deploy", "web:container:web", "ws:container:db"]);
    let node = |id: &str| batch.nodes.iter().chain(&linked.batch.nodes).find(|n| n.id.0 == id).unwrap();
    let web = node("web:container:web");
    assert_eq!(web.aliases, ["host:web", "service:web"]);
    assert_eq!(web.parent.as_ref().unwrap().0, "web:repo:web");
    assert_eq!(web.props["image"], "acme/web:2.0.0");
    assert!(
        matches!(&web.evidence[0], Evidence::Line { line: 6, contains: Some(c), .. } if c == "    image: acme/web:2.0.0")
    );
    let db = node("ws:container:db");
    assert_eq!((db.kind.0.as_str(), db.props["role"].as_str()), ("code.datastore", Some("datastore")));
    assert_eq!(db.parent, None);
    assert_eq!(node("api:container:api").kind.0, "code.service");
    assert!(matches!(&node("deploy:container:deploy").evidence[0], Evidence::Line { line: 2, .. }));
    let file = "deploy:file:docker-compose.yml";
    let env: Vec<(&str, &str, &str, Option<&str>)> = batch
        .hints
        .iter()
        .filter(|h| h.kind.0 == "code.env_set")
        .map(|h| {
            (h.scope.0.as_str(), h.attrs["service"].as_str().unwrap(), h.key.as_str(), h.value.as_deref())
        })
        .collect();
    assert_eq!(env.len(), 2);
    assert!(env.contains(&(file, "web", "API_URL", Some("http://api:8080"))));
    assert!(env.contains(&(file, "api", "DATABASE_URL", Some("postgres://db:5432/app"))));
    let deps: BTreeSet<(&str, &str, &str)> = batch
        .hints
        .iter()
        .filter(|h| h.kind.0 == "code.depends_on")
        .map(|h| (h.scope.0.as_str(), h.attrs["service"].as_str().unwrap(), h.key.as_str()))
        .collect();
    assert_eq!(deps, BTreeSet::from([(file, "web", "api"), (file, "api", "db")]));
    let services: BTreeSet<(&str, &str)> = batch
        .hints
        .iter()
        .filter(|h| h.kind.0 == "code.compose_service")
        .map(|h| (h.scope.0.as_str(), h.key.as_str()))
        .collect();
    assert_eq!(services, BTreeSet::from([(file, "api"), (file, "db"), (file, "web")]));
    let hint_ids: Vec<&String> = batch.hints.iter().map(|h| &h.id).collect();
    let mut sorted = hint_ids.clone();
    sorted.sort();
    assert_eq!(hint_ids, sorted, "hints are sorted by ID");
}

#[test]
fn extraction_is_deterministic_and_every_fact_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let files = walked(&view).files;
    let refs = Git::new(&view.root).refs().unwrap();
    for (name, files, opts) in
        [("files", files.clone(), json!(null)), ("compose", compose_files(&files), json!(null))]
    {
        let first = run(&view, name, &files, opts.clone());
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&run(&view, name, &files, opts)).unwrap()
        );
        let mut checked = first.clone();
        let mut verifier =
            Verifier::new(&view.name, &view.commit, &refs, Git::new(&view.root).objects().unwrap());
        let dropped = verifier.verify(&mut checked).unwrap();
        assert!(dropped.is_empty(), "{name}: {dropped:?}");
        assert_eq!(checked, first);
        assert!(checked.nodes.iter().all(|n| !n.evidence.is_empty()));
    }
}

#[test]
fn evidence_that_differs_from_the_commit_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut view = repo(dir.path());
    // Contents that aren't the commit's, however an extractor came by them.
    let files = walked(&view).files;
    let mut contents: BTreeMap<String, Vec<u8>> =
        files.iter().map(|f| (f.0.clone(), view.read(f).unwrap().to_vec())).collect();
    contents.insert("src/app/main.ts".into(), b"export const y = 2;\n".to_vec());
    view.blobs = Blobs::new(contents);
    let mut batch = run(&view, "files", &files, json!(null));
    let refs = Git::new(&view.root).refs().unwrap();
    let mut verifier =
        Verifier::new(&view.name, &view.commit, &refs, Git::new(&view.root).objects().unwrap());
    let dropped = verifier.verify(&mut batch).unwrap();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].fact, "deploy:file:src/app/main.ts");
    assert!(dropped[0].fact_dropped);
    assert!(batch.nodes.iter().all(|n| n.id.0 != "deploy:file:src/app/main.ts"));
}

/// Every path the tree of `view`'s commit holds, files and the directories above them.
fn tree_paths(view: &RepoView) -> BTreeSet<String> {
    let out = Command::new("git")
        .args(["ls-tree", "-r", "-t", "--name-only", "-z", &view.commit])
        .current_dir(&view.root)
        .output()
        .unwrap();
    assert!(out.status.success());
    out.stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

#[test]
fn the_working_tree_never_reaches_the_facts() {
    let dir = tempfile::tempdir().unwrap();
    let clean = repo(dir.path());
    let clean_files = walked(&clean).files;
    let before: Vec<FactBatch> =
        ["files", "compose"].iter().map(|name| run(&clean, name, &clean_files, json!({}))).collect();

    // A dirty edit, a deleted file, an untracked directory and an empty untracked file.
    let root = &clean.root;
    fs::write(root.join("docker-compose.yml"), format!("# local note\n{COMPOSE}")).unwrap();
    fs::remove_file(root.join("src/app/main.ts")).unwrap();
    fs::create_dir_all(root.join("scratch/deep")).unwrap();
    fs::write(root.join("scratch/deep/notes.ts"), "export const n = 1;\n").unwrap();
    fs::write(root.join("EMPTY_UNTRACKED"), "").unwrap();
    let dirty = view(root);
    assert_eq!(dirty, clean, "the same commit gives the same view");
    let files = walked(&dirty).files;
    assert_eq!(files, clean_files);
    let after: Vec<FactBatch> =
        ["files", "compose"].iter().map(|name| run(&dirty, name, &files, json!({}))).collect();
    assert_eq!(serde_json::to_string(&after).unwrap(), serde_json::to_string(&before).unwrap());

    // Every directory and file node names a path in the commit, and every fact verifies.
    let paths = tree_paths(&dirty);
    let refs = Git::new(root).refs().unwrap();
    let mut verifier = Verifier::new(&dirty.name, &dirty.commit, &refs, Git::new(root).objects().unwrap());
    for mut batch in after {
        for node in &batch.nodes {
            if let Some(path) =
                node.id.0.strip_prefix("deploy:dir:").or_else(|| node.id.0.strip_prefix("deploy:file:"))
            {
                assert!(paths.contains(path), "{} isn't in the commit", node.id.0);
            }
        }
        assert_eq!(verifier.verify(&mut batch).unwrap(), []);
    }
}

#[test]
fn a_broken_compose_file_is_partial_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut view = repo(dir.path());
    view.blobs =
        Blobs::new(BTreeMap::from([("docker-compose.yml".into(), b"services: [unclosed\n".to_vec())]));
    let batch = run(&view, "compose", &[RepoPath("docker-compose.yml".into())], json!(null));
    assert!(batch.nodes.is_empty());
    assert_eq!(batch.partial.len(), 1);
}

#[test]
fn unknown_options_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let err = builtin("compose").unwrap().run(&view, &[], &json!({"nope": 1})).unwrap_err();
    assert!(err.to_string().contains("code/compose"), "{err}");
}

#[test]
fn evidence_verifies_at_ingest() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let files = walked(&view).files;
    let mut batch = run(&view, "files", &files, json!(null));
    let commit = view.commit.clone();
    let line = |path: &str, line: u32, contains: &str| Evidence::Line {
        path: path.into(),
        line,
        contains: Some(contains.into()),
        commit: Some(commit.clone()),
        branch: None,
    };
    let at_ref = |commit: &str, ref_name: Option<&str>| Evidence::Ref {
        commit: commit.into(),
        ref_name: ref_name.map(str::to_owned),
    };
    let missing = "0".repeat(40);
    let mut cite = |id: &str, evidence: Vec<Evidence>| {
        batch.nodes.iter_mut().find(|n| n.id.0 == id).unwrap_or_else(|| panic!("no node {id}")).evidence =
            evidence;
    };
    // A line that holds, beside one whose text the commit doesn't hold: the fact keeps the first.
    cite(
        "deploy:file:src/app/main.ts",
        vec![
            line("deploy/src/app/main.ts", 1, "export const x = 1;"),
            line("deploy/src/app/main.ts", 1, "export const y = 2;"),
        ],
    );
    // A line past the file's end, a path outside the repo, and a path the commit doesn't hold
    // (an ignored file in the working tree).
    cite("deploy:file:docker-compose.yml", vec![line("deploy/docker-compose.yml", 9_999, "services:")]);
    cite("deploy:file:.gitignore", vec![line("elsewhere/.gitignore", 1, "*.log")]);
    cite("deploy:file:src/latin1.txt", vec![line("deploy/debug.log", 1, "noise")]);
    // A commit that exists with a ref that names it; then a ref and a commit that don't exist.
    cite(
        "deploy:dir:src/app",
        vec![at_ref(&commit, Some("refs/heads/main")), at_ref(&commit, Some("refs/heads/gone"))],
    );
    cite("deploy:file:src/empty.txt", vec![at_ref(&missing, None)]);

    let refs = Git::new(&view.root).refs().unwrap();
    let mut verifier =
        Verifier::new(&view.name, &view.commit, &refs, Git::new(&view.root).objects().unwrap());
    let dropped = verifier.verify(&mut batch).unwrap();
    let mut got: Vec<(&str, &str, bool)> =
        dropped.iter().map(|d| (d.fact.as_str(), d.reason.as_str(), d.fact_dropped)).collect();
    got.sort();
    let past = format!("deploy/docker-compose.yml has no line 9999 at {commit}");
    let differs = format!("line 1 of deploy/src/app/main.ts differs at {commit}");
    let not_in_commit = format!("deploy/debug.log is not in commit {commit}");
    let no_commit = format!("commit {missing} doesn't exist");
    let mut want: Vec<(&str, &str, bool)> = vec![
        ("deploy:dir:src/app", "ref refs/heads/gone doesn't exist", false),
        ("deploy:file:.gitignore", "elsewhere/.gitignore is outside repo deploy", true),
        ("deploy:file:docker-compose.yml", &past, true),
        ("deploy:file:src/app/main.ts", &differs, false),
        ("deploy:file:src/empty.txt", &no_commit, true),
        ("deploy:file:src/latin1.txt", &not_in_commit, true),
    ];
    want.sort();
    assert_eq!(got, want);
    // A fact left with no evidence goes; one with some left keeps only what holds.
    let kept: Vec<&str> = batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(
        kept,
        ["deploy:dir:src", "deploy:dir:src/app", "deploy:file:src/app/main.ts", "deploy:repo:deploy"]
    );
    let node = |id: &str| batch.nodes.iter().find(|n| n.id.0 == id).unwrap();
    assert_eq!(
        node("deploy:file:src/app/main.ts").evidence,
        [line("deploy/src/app/main.ts", 1, "export const x = 1;")]
    );
    assert_eq!(node("deploy:dir:src/app").evidence, [at_ref(&commit, Some("refs/heads/main"))]);
}

// The declarative runner: regex and tree-sitter extractors read from pack data (DR-9, DR-46, DR-47).

/// The vocabulary the declarative tests compile against: the code pack's file kind, a symbol kind,
/// a flag kind whose props are typed, and two relations.
fn vocabulary() -> Vocabulary {
    let kind = |kind: &str, prefix: &str, props: serde_json::Value| json!({"kind": kind, "level": "code", "id_prefix": prefix, "label": kind, "props": props, "tone": null});
    serde_json::from_value(json!({
        "kinds": [
            kind("code.file", "file", json!({"type": "object", "properties": {"language": {"type": "string"}}})),
            kind("code.symbol", "sym", json!({})),
            kind("flags.flag", "flag", json!({"type": "object", "properties": {
                "enabled": {"type": "boolean"}, "rollout": {"type": "integer"},
                "weight": {"type": "number"}, "label": {"type": "string"},
                "settings": {"type": "object"}, "either": {"type": ["object", "string"]},
                "color": {"type": "string", "enum": ["red", "blue"], "description": "the flag's badge"},
                "pct": {"type": "integer", "minimum": 0, "maximum": 100},
                "ids": {"type": "array", "items": {"type": "integer"}}}})),
        ],
        "relations": [
            {"kind": "code.imports", "label": "imports", "from": ["code.file"],
             "to": ["code.file", "kernel.external"], "props": {}, "inferred": false},
            {"kind": "flags.reads_flag", "label": "reads", "from": ["code.file"], "to": ["flags.flag"],
             "props": {"type": "object", "properties": {"required": {"type": "boolean"}}}, "inferred": false},
        ],
        "lenses": [],
        "suggestions": [],
    }))
    .unwrap()
}

/// The merged `[[languages]]` table: the six grammar languages, YAML, Ruby with its `=begin`
/// block, Clojure, and Swift, whose block comments nest.
fn languages() -> Vec<LanguageDecl> {
    let c_like = |id: &str, ext: &str| json!({"id": id, "extensions": [ext], "line_comment": "//", "block_comment": ["/*", "*/"]});
    serde_json::from_value(json!([
        c_like("typescript", "ts"),
        c_like("tsx", "tsx"),
        c_like("javascript", "js"),
        {"id": "python", "extensions": ["py"], "line_comment": "#"},
        c_like("go", "go"),
        c_like("rust", "rs"),
        {"id": "yaml", "extensions": ["yml", "yaml"], "line_comment": "#"},
        {"id": "ruby", "extensions": ["rb"], "line_comment": "#", "block_comment": ["=begin", "=end"]},
        {"id": "clojure", "extensions": ["clj"], "line_comment": ";"},
        c_like("swift", "swift"),
    ]))
    .unwrap()
}

/// The declared hint kinds and their attributes.
fn hints() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([(
        "code.url".to_owned(),
        ["scheme", "host", "port", "path", "method"].map(String::from).to_vec(),
    )])
}

/// `decl`, an `[[extractors]]` entry as JSON, compiled in the pack `toy` with `query` as its `.scm`.
fn compile(decl: serde_json::Value, query: Option<&str>) -> Result<Declarative, Invalid> {
    let decl: ExtractorDecl = serde_json::from_value(decl).unwrap();
    let (vocabulary, languages, hints) = (vocabulary(), languages(), hints());
    let spec = Spec {
        pack: "toy",
        decl: &decl,
        vocabulary: &vocabulary,
        languages: &languages,
        hints: &hints,
        query,
    };
    Declarative::compile(&spec)
}

/// A repo named `web` at a made-up commit, holding `files` in memory, and their sorted paths.
fn memory(files: &[(&str, &str)]) -> (RepoView, Vec<RepoPath>) {
    let blobs: BTreeMap<String, Vec<u8>> =
        files.iter().map(|(p, t)| ((*p).to_owned(), t.as_bytes().to_vec())).collect();
    let paths = blobs.keys().cloned().map(RepoPath).collect();
    let view = RepoView {
        name: "web".into(),
        root: "/nonexistent".into(),
        commit: "c".repeat(40),
        blobs: Blobs::new(blobs),
    };
    (view, paths)
}

/// A `code.url` hint for each URL a regex finds, scoped to its file.
fn url_regex(files: &[&str]) -> serde_json::Value {
    json!({"id": "urls", "files": files, "regex": r#"(https?)://([a-z0-9-]+):(\d+)(/[^'"\s]*)"#,
           "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$2",
                     "attrs": {"scheme": "$1", "host": "$2", "port": "$3", "path": "$4"}}]})
}

/// Every hint as `(file, line, its path attribute)`, each checked to cite its own source line.
fn cited(view: &RepoView, batch: &FactBatch) -> Vec<(String, u32, String)> {
    batch
        .hints
        .iter()
        .map(|h| {
            let Evidence::Line { path, line, contains: Some(contains), .. } = &h.evidence[0] else {
                panic!("a line: {h:?}")
            };
            let file = path.split_once('/').unwrap().1;
            let text = std::str::from_utf8(view.read(&RepoPath(file.into())).unwrap()).unwrap();
            assert!(verify::line_holds(text, *line, contains), "{path}:{line} holds {contains}");
            (file.to_owned(), *line, h.attrs.get("path").and_then(|p| p.as_str()).unwrap_or("").to_owned())
        })
        .collect()
}

#[test]
fn emit_props_coerce_to_the_prop_schema() {
    let flags = "# Flags, one a line.\n\
                 gift_wrap: enabled=false rollout=0 weight=0.25\n\
                 new_checkout: enabled=TRUE rollout=50 weight=1\n\
                 broken: enabled=maybe rollout=x weight=2\n";
    let (view, files) = memory(&[("flags.yaml", flags)]);
    let program = compile(
        json!({"id": "flags", "files": ["**/flags.yaml"],
        "regex": r"(?m)^(\w+): enabled=(\w+) rollout=(\w+) weight=([0-9.]+)$",
        "emit": [
            {"node": "flags.flag", "key": "$1", "parent": "@id:code.file:$file",
             "props": {"enabled": "$2", "rollout": "$3", "weight": "$4", "label": "$1",
                       "tier": 2, "source": "yaml"}},
            {"edge": "flags.reads_flag", "from": "@id:code.file:$file", "to": "@id:flags.flag:$1",
             "props": {"required": "$2"}},
        ]}),
        None,
    )
    .unwrap();
    let batch = program.run(&view, &files);
    let props = |key: &str| {
        let node = batch.nodes.iter().find(|n| n.id.0 == format!("web:flag:{key}")).unwrap();
        serde_json::to_value(&node.props).unwrap()
    };
    // Text with captures takes the schema's type; a value written plainly keeps its own.
    assert_eq!(
        props("gift_wrap"),
        json!({"enabled": false, "rollout": 0, "weight": 0.25, "label": "gift_wrap", "tier": 2, "source": "yaml"})
    );
    assert_eq!(
        (&props("new_checkout")["enabled"], &props("new_checkout")["rollout"]),
        (&json!(true), &json!(50))
    );
    assert_eq!(props("new_checkout")["weight"], 1);
    // What doesn't fit its type is left out and makes the file partial; the rest stands.
    assert_eq!(props("broken"), json!({"weight": 2, "label": "broken", "tier": 2, "source": "yaml"}));
    let edge = |key: &str| {
        let to = Ref::Id(NodeId(format!("web:flag:{key}")));
        batch.edges.iter().find(|e| e.to == to).unwrap()
    };
    assert_eq!(edge("gift_wrap").props["required"], false);
    assert_eq!(edge("new_checkout").props["required"], true);
    assert!(edge("broken").props.is_empty());
    assert_eq!(batch.partial, [PartialFile { path: "web/flags.yaml".into(), errors: 3, reason: None }]);
    let node = batch.nodes.iter().find(|n| n.id.0 == "web:flag:gift_wrap").unwrap();
    assert_eq!((node.kind.0.as_str(), node.level), ("flags.flag", Level::Code));
    assert_eq!(node.parent, Some(NodeId("web:file:flags.yaml".into())));
    assert!(matches!(&node.evidence[..], [Evidence::Line { line: 2, .. }]), "{:?}", node.evidence);
}

#[test]
fn regex_skips_comment_spans() {
    let ts = "// fetch('http://orders:8080/commented')\n\
              const a = fetch('http://orders:8080/live'); // fetch('http://x:1/trailing')\n\
              /* fetch('http://y:2/block')\n   \
              fetch('http://z:3/block') */ const b = fetch('http://web:80/after');\n\
              const s = \"// not a comment: http://api:9/in-string\";\n";
    let py = "# requests.get('http://orders:8080/commented')\n\
              URL = 'http://orders:8080/py'  # 'http://x:1/c'\n\
              s = \"#not-a-comment http://api:9/hash-in-string\"\n";
    let rb = "=begin\nget('http://orders:8080/doc')\n=end\nget('http://orders:8080/rb')\n";
    let plain = "http://notes:1/no-language # http://kept:2/kept\n";
    let (view, files) =
        memory(&[("src/api.ts", ts), ("worker/client.py", py), ("ops/ping.rb", rb), ("NOTES", plain)]);
    let batch = compile(url_regex(&["**"]), None).unwrap().run(&view, &files);
    let mut found = cited(&view, &batch);
    found.sort();
    let want = |f: &str, l: u32, p: &str| (f.to_owned(), l, p.to_owned());
    assert_eq!(
        found,
        [
            // A file no [[languages]] entry covers has no comment marks, so nothing is blanked.
            want("NOTES", 1, "/kept"),
            want("NOTES", 1, "/no-language"),
            want("ops/ping.rb", 4, "/rb"),
            want("src/api.ts", 2, "/live"),
            want("src/api.ts", 4, "/after"),
            want("src/api.ts", 5, "/in-string"),
            want("worker/client.py", 2, "/py"),
            want("worker/client.py", 3, "/hash-in-string"),
        ]
    );
    assert!(batch.partial.is_empty());
}

/// `$end` is the line a match ends on, its last byte's: a match that spans lines ends on its
/// last, and one that takes its line's newline ends on that line, not the next.
#[test]
fn end_is_the_line_a_regex_match_ends_on() {
    let decl = json!({"id": "spans", "files": ["**/*.txt"], "regex": r"(?s)begin (\w+).*?end\n?",
                      "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$1@$end",
                                "attrs": {"port": "$end", "path": "$line"}}]});
    let text = "x\nbegin one\na\nend\nbegin two end\n";
    let (view, paths) = memory(&[("a.txt", text)]);
    let batch = compile(decl, None).unwrap().run(&view, &paths);
    let got: Vec<(&str, &str, &str)> = batch
        .hints
        .iter()
        .map(|h| (h.key.as_str(), h.attrs["path"].as_str().unwrap(), h.attrs["port"].as_str().unwrap()))
        .collect();
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.contains(&("one@4", "2", "4")), "{got:?}");
    assert!(got.contains(&("two@5", "5", "5")), "{got:?}");
}

#[test]
fn a_bare_path_reference_is_pack_invalid() {
    let hint = |scope: &str| json!({"id": "urls", "files": ["**"], "regex": "(h)", "emit": [{"hint": "code.url", "scope": scope, "key": "$1"}]});
    let edge = |from: &str, to: &str| json!({"id": "imports", "files": ["**"], "regex": "(i)", "emit": [{"edge": "code.imports", "from": from, "to": to}]});
    let node = |parent: &str| {
        json!({"id": "symbols", "files": ["**"], "regex": "(f)",
               "emit": [{"node": "code.symbol", "key": "$1", "parent": parent}]})
    };
    let at = |field: &str| vec![Step::Key("emit".into()), Step::Index(0), Step::Key(field.into())];
    for (decl, reference, field) in [
        (hint("$file"), "$file", "scope"),
        (edge("$file", "@ext:$1"), "$file", "from"),
        (edge("@id:code.file:$file", "$1"), "$1", "to"),
        (edge("@id:code.file:$file", "src/lib.rs"), "src/lib.rs", "to"),
        (node("$file"), "$file", "parent"),
    ] {
        let invalid = compile(decl, None).expect_err(reference);
        assert_eq!(invalid.code(), ErrorCode::PackInvalid);
        assert_eq!(
            (invalid.reference.as_str(), &invalid.why, &invalid.at),
            (reference, &Why::BareRef, &at(field))
        );
    }
    // A file named by its kind compiles; an alias names no node, so it can't scope a hint.
    assert!(compile(hint("@id:code.file:$file"), None).is_ok());
    assert!(compile(edge("@id:code.file:$file", "@ext:$1"), None).is_ok());
    assert_eq!(compile(hint("@alias:host:$1"), None).unwrap_err().why, Why::NotAnId);
}

#[test]
fn comments_never_create_relations() {
    // A regex extractor's edges: a commented import is blanked before the pattern runs.
    let py = "import requests\n# import commented\nx = 1  # import trailing\nif x:\n    import nested\n";
    let (view, files) = memory(&[("worker/client.py", py)]);
    let imports = compile(
        json!({"id": "imports", "files": ["**/*.py"], "regex": r"(?m)^\s*import ([\w.]+)",
               "emit": [{"edge": "code.imports", "from": "@id:code.file:$file", "to": "@ext:$1", "via": "$1"}]}),
        None,
    )
    .unwrap();
    let batch = imports.run(&view, &files);
    let targets: BTreeSet<String> = batch.edges.iter().map(|e| format!("{:?}", e.to)).collect();
    let want = [Ref::Alias("ext:nested".into()), Ref::Alias("ext:requests".into())].map(|r| format!("{r:?}"));
    assert_eq!(targets, BTreeSet::from(want));

    // A query extractor's: a match that captures a comment or a docstring is dropped, though the
    // query names comments outright.
    let py = "\"\"\"Orders client. Talks to postgres://orders-db:5432/orders.\"\"\"\n\
              import requests\n\
              # DSN = \"postgresql://orders-db:5432/x\"\n\
              def get(order_id):\n    \
              \"\"\"Fetch http://orders-db:5432/doc for one order.\"\"\"\n    \
              return requests.get(\"http://orders:8080/orders\")\n";
    let ts = "/** Calls http://docs:1/jsdoc */\n// const old = 'http://docs:2/line';\nconst u = 'http://orders:8080/ts';\n";
    let (view, files) = memory(&[("worker/client.py", py), ("web/api.ts", ts)]);
    for (language, glob, strings, want) in [
        ("python", "**/*.py", "(string (string_content) @text)", "http://orders:8080/orders"),
        ("typescript", "**/*.ts", "(string (string_fragment) @text)", "http://orders:8080/ts"),
    ] {
        let query = format!("{strings}\n(comment) @note\n");
        let program = compile(
            json!({"id": "strings", "files": [glob], "language": language, "query": "queries/strings.scm",
                   "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"},
                            {"edge": "code.imports", "from": "@id:code.file:$file", "to": "@ext:@note"}]}),
            Some(&query),
        )
        .unwrap();
        let batch = program.run(&view, &select(&files, &[glob]).unwrap());
        assert!(batch.edges.is_empty(), "{language}: {:?}", batch.edges);
        let keys: Vec<&str> = batch.hints.iter().map(|h| h.key.as_str()).collect();
        assert_eq!(keys, [want], "{language}");
        assert!(batch.partial.is_empty(), "{language}: {:?}", batch.partial);
    }
}

/// Each manifest and spec the format built-ins read: its file name, a valid one, and how to nest
/// it deep: an opening, the part 600 copies of which nest, and a closing.
const FORMATS: &[(&str, &str, (&str, &str, &str))] = &[
    (
        "package.json",
        "{\n  \"name\": \"@acme/web\",\n  \"dependencies\": { \"express\": \"4.21.0\" }\n}\n",
        ("{\"name\": \"x\", \"d\": ", "[", ""),
    ),
    (
        "go.mod",
        "module example.com/api\n\nrequire github.com/x/y v1.0.0\n",
        ("module m\nrequire (\n", "(", ")"),
    ),
    ("pyproject.toml", "[project]\nname = \"acme\"\ndependencies = [\"redis==5.0.8\"]\n", ("x = ", "[", "")),
    ("Cargo.toml", "[package]\nname = \"acme\"\n\n[dependencies]\nserde = \"1\"\n", ("x = ", "[", "")),
    (
        "openapi.yaml",
        "openapi: 3.0.3\npaths:\n  /orders:\n    get: {}\n",
        ("openapi: 3.0.3\npaths: ", "[", ""),
    ),
    (
        "openapi.json",
        "{\"openapi\": \"3.0.3\", \"paths\": {\"/orders\": {\"get\": {}}}}\n",
        ("{\"openapi\": \"3\", \"paths\": ", "[", ""),
    ),
];

/// Valid code in each grammar's language.
const SOURCES: &[(&str, &str, &str)] = &[
    ("typescript", "ts", "export function get(id: string) { return fetch('http://orders:8080/o/' + id); }\n"),
    ("tsx", "tsx", "export const App = () => <a href='http://web:80/home'>{title}</a>;\n"),
    (
        "javascript",
        "js",
        "const url = 'http://api:9/x';\nmodule.exports = function call() { return url; };\n",
    ),
    ("python", "py", "def get(order_id):\n    return requests.get('http://orders:8080/orders')\n"),
    ("go", "go", "package main\n\nfunc main() {\n\thttp.Get(\"http://orders:8080/healthz\")\n}\n"),
    ("rust", "rs", "fn main() {\n    let url = \"http://orders:8080/rs\";\n    get(url);\n}\n"),
];

/// A fuzz corpus committed to the repo at `root`: per language, valid code, the same broken, an
/// empty file, one that isn't UTF-8, a deep nest, a 100 KB line, CRLF line ends and a non-ASCII
/// name.
fn corpus(root: &Path) {
    for (_, ext, valid) in SOURCES {
        let files: Vec<(String, Vec<u8>)> = vec![
            (format!("src/valid.{ext}"), valid.as_bytes().to_vec()),
            (format!("src/broken.{ext}"), format!("{valid}\n}} ((( fn def func <<\n'unclosed").into_bytes()),
            (format!("src/empty.{ext}"), Vec::new()),
            (format!("src/latin1.{ext}"), b"x = '\xe9t\xe9'\n".to_vec()),
            (format!("src/deep.{ext}"), format!("{}x{}\n", "(".repeat(600), ")".repeat(600)).into_bytes()),
            (
                format!("src/long.{ext}"),
                format!("x = '{}http://long:1/l'\n", "a".repeat(100_000)).into_bytes(),
            ),
            (format!("src/crlf.{ext}"), valid.replace('\n', "\r\n").into_bytes()),
            (format!("src/ünï.{ext}"), valid.as_bytes().to_vec()),
        ];
        for (path, bytes) in files {
            let file = root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, bytes).unwrap();
        }
    }
    // Each manifest and spec format the same ways, each in a directory of its own so its name
    // stays the one the built-in reads: valid, broken, empty, not UTF-8, nested deep, one long
    // line, CRLF line ends, and anchors a long string many aliases copy.
    for (name, valid, deep) in FORMATS {
        let bomb = format!("x: &x \"{}\"\nall: [{}]\n", "y".repeat(100_000), "*x, ".repeat(50_000));
        let variants: Vec<(&str, Vec<u8>)> = vec![
            ("valid", valid.as_bytes().to_vec()),
            ("broken", format!("{valid}\n}} ][ = : \"unclosed\n").into_bytes()),
            ("empty", Vec::new()),
            ("latin1", b"name = '\xe9t\xe9'\n".to_vec()),
            ("deep", format!("{}{}{}\n", deep.0, deep.1.repeat(600), deep.2).into_bytes()),
            ("long", format!("{valid}\n#{}\n", "a".repeat(100_000)).into_bytes()),
            ("crlf", valid.replace('\n', "\r\n").into_bytes()),
            ("bomb", bomb.into_bytes()),
        ];
        for (variant, bytes) in variants {
            let file = root.join(format!("fmt/{variant}/{name}"));
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, bytes).unwrap();
        }
    }
    git(root, &["add", "-A"]);
    git(root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "corpus"]);
}

type Run = Box<dyn Fn(&[RepoPath]) -> FactBatch>;

#[test]
fn every_extractor_is_deterministic_tolerant_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let base = repo(dir.path());
    corpus(&base.root);
    let view = view(&base.root);
    let all = walked(&view).files;
    let refs = Git::new(&view.root).refs().unwrap();

    // Every built-in, the regex runner, and a query over each grammar.
    let mut runs: Vec<(String, Run, Vec<RepoPath>)> = Vec::new();
    for name in ["files", "compose", "manifest", "openapi"] {
        let files = match name {
            "compose" => compose_files(&all),
            "manifest" => select(&all, planroom_extract::manifest::DEFAULT_FILES).unwrap(),
            "openapi" => select(&all, planroom_extract::openapi::DEFAULT_FILES).unwrap(),
            _ => all.clone(),
        };
        let v = view.clone();
        runs.push((name.into(), Box::new(move |f: &[RepoPath]| run(&v, name, f, json!({}))), files));
    }
    let mut programs = vec![compile(url_regex(&["**"]), None).unwrap()];
    for (language, ext, _) in SOURCES {
        let decl = json!({"id": format!("{language}_names"), "files": [format!("**/*.{ext}")], "language": language,
                          "query": "queries/names.scm",
                          "emit": [{"node": "code.symbol", "key": "$file#@name", "parent": "@id:code.file:$file"},
                                   {"edge": "code.imports", "from": "@id:code.file:$file", "to": "@ext:@note"}]});
        // Rust's grammar names its comments by form; the others have one comment node.
        let comments = if *language == "rust" { "[(line_comment) (block_comment)]" } else { "(comment)" };
        let query = format!("((identifier) @name)\n{comments} @note\n");
        programs.push(compile(decl, Some(&query)).unwrap_or_else(|e| panic!("{language}: {e}")));
    }
    // And every declarative extractor the code pack declares, its own queries and regexes.
    let code = code_pack::load().programs();
    let code_count = code.len();
    programs.extend(code);
    for program in programs {
        let globs: Vec<&str> = program.files().iter().map(String::as_str).collect();
        let files = select(&all, &globs).unwrap();
        let (name, v) = (program.name().to_owned(), view.clone());
        runs.push((name, Box::new(move |f: &[RepoPath]| program.run(&v, f)), files));
    }
    assert_eq!(runs.len(), 4 + 1 + SOURCES.len() + code_count);

    for (name, run, files) in &runs {
        // Deterministic: byte-identical twice, sorted by ID, each ID once.
        let first = run(files);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&run(files)).unwrap(),
            "{name}"
        );
        let node_ids: Vec<&str> = first.nodes.iter().map(|n| n.id.0.as_str()).collect();
        let edge_ids: Vec<&str> = first.edges.iter().map(|e| e.id.0.as_str()).collect();
        let hint_ids: Vec<&str> = first.hints.iter().map(|h| h.id.as_str()).collect();
        for ids in [&node_ids, &edge_ids, &hint_ids] {
            assert!(ids.windows(2).all(|w| w[0] < w[1]), "{name}: sorted, each once: {ids:?}");
        }
        // Every fact verifies against the commit.
        let mut checked = first.clone();
        let mut verifier =
            Verifier::new(&view.name, &view.commit, &refs, Git::new(&view.root).objects().unwrap());
        assert_eq!(verifier.verify(&mut checked).unwrap(), [], "{name}");
        assert_eq!(checked, first, "{name}");
        // Tolerant: what can't be read or parsed is partial, and the rest stands.
        if name.starts_with("toy/") {
            assert!(!first.hints.is_empty() || !first.nodes.is_empty(), "{name} found something");
            let partial: BTreeSet<&str> = first.partial.iter().map(|p| p.path.as_str()).collect();
            let latin1 = files.iter().filter(|f| f.0.contains("latin1")).count();
            assert_eq!(
                partial.iter().filter(|p| p.contains("latin1")).count(),
                latin1,
                "{name}: {partial:?}"
            );
        }
        if name.ends_with("_names") {
            let partial: BTreeSet<&str> = first.partial.iter().map(|p| p.path.as_str()).collect();
            assert!(partial.iter().any(|p| p.contains("/src/broken.")), "{name}: {partial:?}");
            assert!(
                !partial.iter().any(|p| p.contains("/src/valid.") || p.contains("/src/crlf.")),
                "{name}: {partial:?}"
            );
            assert!(first.nodes.iter().any(|n| n.id.0.contains(":src/valid.")), "{name}");
            assert!(
                first.nodes.iter().any(|n| n.id.0.contains(":src/broken.")),
                "{name}: a broken file keeps its facts"
            );
            assert!(first.edges.is_empty(), "{name}: no comment creates an edge");
        }
        let partial: BTreeSet<&str> = first.partial.iter().map(|p| p.path.as_str()).collect();
        if name.starts_with("code/") {
            // The pack's own extractors refuse no ordinary file, and keep the facts of each file
            // that parses.
            let refused: Vec<&PartialFile> = first
                .partial
                .iter()
                .filter(|p| {
                    p.reason.is_some()
                        && ["/valid.", "/crlf.", "/ünï.", "/broken."].iter().any(|v| p.path.contains(v))
                })
                .collect();
            assert!(refused.is_empty(), "{name}: {refused:?}");
            assert!(
                !partial.iter().any(|p| p.contains("/src/valid.") || p.contains("/src/crlf.")),
                "{name}: {partial:?}"
            );
        }
        if name == "manifest" || name == "openapi" {
            assert!(!first.nodes.is_empty(), "{name} found something");
            for file in files {
                let path = format!("{}/{}", view.name, file.0);
                let variant = file.0.split('/').nth(1).unwrap_or_default();
                // A go.mod has no nesting and no syntax to break: it gives what lines it can read.
                let refused = match variant {
                    "latin1" => true,
                    "broken" | "deep" | "bomb" => !file.0.ends_with("go.mod"),
                    _ => false,
                };
                assert_eq!(partial.contains(path.as_str()), refused, "{name}: {path}");
            }
        }
        // Bounded: given fewer files, it reads only those.
        let half: Vec<RepoPath> = files.iter().step_by(2).cloned().collect();
        let some = run(&half);
        let lines =
            some.nodes.iter().flat_map(|n| &n.evidence).chain(some.hints.iter().flat_map(|h| &h.evidence));
        for evidence in lines {
            if let Evidence::Line { path, .. } = evidence {
                let rel = path.split_once('/').unwrap().1;
                assert!(half.iter().any(|f| f.0 == rel), "{name}: {path} wasn't given");
            }
        }
    }
}

#[test]
fn bounds_cut_in_sorted_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("web");
    fs::create_dir_all(root.join("m")).unwrap();
    // Written in another order than they sort in; each is 33 bytes.
    for name in ["z.rb", "a.rb", "m/b.rb", "m/a.rb", "b.rb"] {
        let line = format!("get('http://orders:8080/{:<6}')\n", name.replace('/', "_"));
        assert_eq!(line.len(), 33, "{line}");
        fs::write(root.join(name), line).unwrap();
    }
    fs::write(root.join("big.rb"), "#".repeat(100)).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["add", "-A"]);
    git(&root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
    let repo_git = Git::new(&root);
    let commit = repo_git.head().unwrap().commit;
    let within = |max_files: usize, max_total_bytes: u64| {
        let bounds = walk::Bounds { max_file_bytes: 64, max_files, max_total_bytes };
        walk::walk_within(&repo_git, &commit, &walk::Exclude::none(), &bounds).unwrap()
    };
    let names = |paths: &[RepoPath]| paths.iter().map(|p| p.0.clone()).collect::<Vec<_>>();
    // A file bound: the first three in path order, the rest uncovered in order. The file over the
    // size bound is skipped, not uncovered.
    let walked = within(3, u64::MAX);
    assert_eq!(names(&walked.files), ["a.rb", "b.rb", "m/a.rb"]);
    assert_eq!(walked.uncovered, ["m/b.rb", "z.rb"]);
    assert_eq!(walked.skipped, 1);
    // A byte bound cuts the same way: two files fit in 98 bytes, three need 99.
    let walked = within(usize::MAX, 98);
    assert_eq!(names(&walked.files), ["a.rb", "b.rb"]);
    assert_eq!(walked.uncovered, ["m/a.rb", "m/b.rb", "z.rb"]);

    // The runner reads what the walk took, in sorted order, however the files come to it.
    let view = RepoView { name: "web".into(), root: root.clone(), commit, blobs: walked.blobs.clone() };
    let program = compile(url_regex(&["**/*.rb"]), None).unwrap();
    let mut reversed = walked.files.clone();
    reversed.reverse();
    let batch = program.run(&view, &reversed);
    assert_eq!(
        serde_json::to_string(&batch).unwrap(),
        serde_json::to_string(&program.run(&view, &walked.files)).unwrap()
    );
    let found: BTreeSet<String> = cited(&view, &batch).into_iter().map(|(f, _, _)| f).collect();
    assert_eq!(found, BTreeSet::from(["a.rb".to_owned(), "b.rb".to_owned()]));
    // A file past the cut is no input: asked for, it is partial and never read.
    let batch = program.run(&view, &[RepoPath("z.rb".into())]);
    assert!(batch.hints.is_empty());
    assert_eq!(batch.partial, [PartialFile { path: "web/z.rb".into(), errors: 1, reason: None }]);
}

/// A `code.url` hint and a `code.imports` edge to the host for every URL a regex finds, scoped to
/// its file: a pattern that must never read a comment or a docstring.
fn host_regex() -> serde_json::Value {
    json!({"id": "hosts", "files": ["**"], "regex": r"https?://([a-z0-9-]+)",
           "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$1"},
                    {"edge": "code.imports", "from": "@id:code.file:$file", "to": "@alias:host:$1"}]})
}

#[test]
fn a_comment_or_docstring_never_reaches_a_regex() {
    let files = [
        // A lifetime's quote before a trailing comment.
        (
            "src/a.rs",
            "pub fn get(key: &'static str) -> String { // calls http://rustcomment-db/x\n    \
             fetch(\"http://rustcode-db/y\")\n}\n",
        ),
        // A regex literal's quote before a trailing comment.
        ("web/b.js", "const re = /'/; // see http://jscomment-db/x\nfetch(\"http://jscode-db/y\");\n"),
        // The docstrings of a module, a class and a function.
        (
            "worker/c.py",
            "\"\"\"Module notes: http://pymodule-db/x.\"\"\"\n\
             class C:\n    \"\"\"Talks to http://pyclass-db/x.\"\"\"\n\n    \
             def f(self):\n        \"\"\"Talks to http://pydocstring-db/x.\"\"\"\n        \
             return get(\"http://pycode-db/y\")\n",
        ),
        // A triple-quoted string holding a `#` line is a string, so its URL is code.
        (
            "worker/e.py",
            "s = \"\"\"a\n# not a comment http://inpystring-db/z\n\"\"\"\n# but this is http://pycomment-db/x\n",
        ),
        // A language with no grammar: Ruby's `?'` is a character literal, and its quote doesn't
        // close on its line, so it hides no comment.
        ("ops/c.rb", "c = ?' # see http://rbcomment-db/x\nget('http://rbcode-db/y')\n"),
    ];
    let (view, paths) = memory(&files);
    let batch = compile(host_regex(), None).unwrap().run(&view, &paths);
    let hints: BTreeSet<&str> = batch.hints.iter().map(|h| h.key.as_str()).collect();
    let code = ["inpystring-db", "jscode-db", "pycode-db", "rbcode-db", "rustcode-db"];
    assert_eq!(hints, BTreeSet::from(code), "only code creates hints");
    let targets: BTreeSet<String> = batch.edges.iter().map(|e| format!("{:?}", e.to)).collect();
    let want: BTreeSet<String> =
        code.iter().map(|h| format!("{:?}", Ref::Alias(format!("host:{h}")))).collect();
    assert_eq!(targets, want, "only code creates edges");
    // Each code line keeps its own line number.
    let lines: BTreeSet<(String, u32)> = batch
        .hints
        .iter()
        .map(|h| match &h.evidence[0] {
            Evidence::Line { path, line, .. } => (path.clone(), *line),
            other => panic!("{other:?}"),
        })
        .collect();
    assert!(lines.contains(&("web/worker/c.py".to_owned(), 7)), "{lines:?}");
    assert!(lines.contains(&("web/worker/e.py".to_owned(), 2)), "{lines:?}");
    assert!(batch.partial.is_empty(), "{:?}", batch.partial);
}

#[test]
fn a_captured_prop_no_capture_can_fill_is_pack_invalid() {
    let emit = |prop: &str| {
        json!({"id": "flags", "files": ["**/flags.yaml"], "regex": r"(?m)^(\w+): (\w+)$",
               "emit": [{"node": "flags.flag", "key": "$1", "props": {prop: "$2"}}]})
    };
    let invalid = compile(emit("settings"), None).expect_err("an object prop can't come from a capture");
    assert_eq!(invalid.code(), ErrorCode::PackInvalid);
    let at = ["emit", "0", "props", "settings"]
        .map(|s| s.parse().map_or_else(|_| Step::Key(s.into()), Step::Index))
        .to_vec();
    assert_eq!(
        (invalid.reference.as_str(), &invalid.why, &invalid.at),
        (
            "$2",
            &Why::PropType { kind: "flags.flag".into(), prop: "settings".into(), types: "object".into() },
            &at
        )
    );
    // A type beside it that a capture fills, an untyped prop and a plain value all load.
    assert!(compile(emit("either"), None).is_ok());
    assert!(compile(emit("free"), None).is_ok());
    let fixed = json!({"id": "flags", "files": ["**/flags.yaml"], "regex": r"(?m)^(\w+): (\w+)$",
                       "emit": [{"node": "flags.flag", "key": "$1", "props": {"settings": {"a": 1}}}]});
    assert!(compile(fixed, None).is_ok());
}

/// A `code.imports` edge to `@ext:NAME` for every `import NAME` a regex finds.
fn import_regex() -> serde_json::Value {
    json!({"id": "imports", "files": ["**"], "regex": r"import (\w+)",
           "emit": [{"edge": "code.imports", "from": "@id:code.file:$file", "to": "@ext:$1"}]})
}

/// The hint keys and the edge targets `programs` find in `files`.
fn found(programs: &[serde_json::Value], files: &[(&str, &str)]) -> (BTreeSet<String>, BTreeSet<String>) {
    let (view, paths) = memory(files);
    let (mut hints, mut targets) = (BTreeSet::new(), BTreeSet::new());
    for program in programs {
        let batch = compile(program.clone(), None).unwrap().run(&view, &paths);
        hints.extend(batch.hints.iter().map(|h| h.key.clone()));
        targets.extend(batch.edges.iter().map(|e| format!("{:?}", e.to)));
    }
    (hints, targets)
}

fn hosts(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|h| format!("{:?}", Ref::Alias(format!("host:{h}")))).collect()
}

#[test]
fn a_quote_whose_close_comes_after_a_comment_mark_opens_no_string() {
    let files = [
        // Ruby's `?'` is a character literal; the apostrophe in "don't" would close it.
        ("ops/a.rb", "c = ?' # see http://rbapos-db/x, don't\nget('http://rbcode-db/y')\n"),
        ("ops/b.rb", "c = ?' # import rbapos, don't\n"),
        // A Clojure quote, closed by the comment's "it's".
        ("src/c.clj", "(def x 'foo) ; talks to http://cljapos-db/x, it's\n(get \"http://cljcode-db/y\")\n"),
        // A YAML `^'`, closed the same way.
        ("conf/d.yaml", "a: ^'b # http://yamlapos-db/x it's\nurl: 'http://yamlcode-db/y'\n"),
        // A URL's `//` is no comment mark, so a double-quoted URL closes its string and survives,
        // and the comment after it is still a comment.
        ("app/e.swift", "let u = \"http://swiftcode-db/y\" // see http://swiftcomment-db/x\n"),
    ];
    let (hints, targets) = found(&[host_regex(), import_regex()], &files);
    let code = ["cljcode-db", "rbcode-db", "swiftcode-db", "yamlcode-db"];
    assert_eq!(hints, code.iter().map(|h| (*h).to_owned()).collect(), "only code creates hints");
    assert_eq!(targets, hosts(&code), "only code creates edges");
}

#[test]
fn a_nested_block_comment_ends_where_its_marks_balance() {
    let swift = "/* outer /* inner */ import swiftnested */\nimport Real\n";
    let (_, targets) = found(&[import_regex()], &[("app/a.swift", swift)]);
    assert_eq!(targets, BTreeSet::from([format!("{:?}", Ref::Alias("ext:Real".into()))]));
}

#[test]
fn a_docstring_in_a_file_that_doesnt_parse_never_reaches_a_regex() {
    // The body ends unclosed and the parse can't structure any of the file: it is one error
    // region, so neither the docstring nor the code before it makes a fact.
    let py = "x = get(\"http://pyerrcode-db/y\")\ndef f():\n    \"\"\"Doc http://pyerrdoc-db/x.\"\"\"\n    return (\n";
    let program = compile(host_regex(), None).unwrap();
    assert_eq!(alone(&program, &[("web/g.py", py)]), [("web/g.py".to_owned(), vec![], true)]);
    // Where the parse confines the error to the function's last line, it names the docstring, and
    // the code before it still matches.
    let py = "x = get(\"http://pyerrcode-db/y\")\ndef f():\n    \"\"\"Doc http://pyerrdoc-db/x.\"\"\"\n    return (1\n";
    let (hints, targets) = found(&[host_regex()], &[("web/g.py", py)]);
    assert_eq!(hints, BTreeSet::from(["pyerrcode-db".to_owned()]));
    assert_eq!(targets, hosts(&["pyerrcode-db"]));
}

#[test]
fn a_missing_node_drops_the_statement_around_it() {
    // The parser makes up the missing `)` or `}`; the statement it closes makes no fact, and the
    // statements beside it keep theirs.
    let files = [
        (
            "b.go",
            "package main\n\nfunc a() { get(\"http://a-db/x\") }\nfunc b() { get(\"http://b-db/y\"\n}\n",
        ),
        ("b.js", "const a = 'http://a-db/x';\nfunction b() { return 'http://b-db/y'\n"),
        ("b.ts", "const a = get('http://a-db/x');\nconst b = get('http://b-db/y'\nconst c = 1;\n"),
    ];
    let program = compile(host_regex(), None).unwrap();
    for (path, facts, partial) in alone(&program, &files) {
        assert_eq!(facts, ["a-db", "Alias(\"host:a-db\")"], "{path}");
        assert!(partial, "{path} is partial");
    }
}

#[test]
fn a_captured_prop_that_breaks_its_schema_is_left_out() {
    let flags = "a green -5 x\nb red 50 7\nc blue 101 8\n";
    let (view, files) = memory(&[("flags.txt", flags)]);
    let program = compile(
        json!({"id": "flags", "files": ["**/flags.txt"], "regex": r"(?m)^(\w+) (\S+) (\S+) (\S+)$",
               "emit": [{"node": "flags.flag", "key": "$1",
                         "props": {"color": "$2", "pct": "$3", "ids": "$4"}}]}),
        None,
    )
    .unwrap();
    let batch = program.run(&view, &files);
    let props = |key: &str| {
        let node = batch.nodes.iter().find(|n| n.id.0 == format!("web:flag:{key}")).unwrap();
        serde_json::to_value(&node.props).unwrap()
    };
    // Outside the enum, under the minimum, and an item that isn't an integer: each is left out.
    assert_eq!(props("a"), json!({}));
    assert_eq!(props("b"), json!({"color": "red", "pct": 50, "ids": [7]}));
    // Over the maximum.
    assert_eq!(props("c"), json!({"color": "blue", "ids": [8]}));
    assert_eq!(batch.partial, [PartialFile { path: "web/flags.txt".into(), errors: 4, reason: None }]);
}

/// What a program finds in each of `files` run alone, hint keys, edge targets and node IDs, and
/// whether it marked that file partial.
fn alone(program: &Declarative, files: &[(&str, &str)]) -> Vec<(String, Vec<String>, bool)> {
    files
        .iter()
        .map(|(path, text)| {
            let (view, paths) = memory(&[(path, text)]);
            let batch = program.run(&view, &paths);
            let mut facts: Vec<String> = batch.hints.iter().map(|h| h.key.clone()).collect();
            facts.extend(batch.edges.iter().map(|e| format!("{:?}", e.to)));
            facts.extend(batch.nodes.iter().map(|n| n.id.0.clone()));
            let partial = batch.partial.iter().any(|p| p.path == format!("web/{path}"));
            ((*path).to_owned(), facts, partial)
        })
        .collect()
}

#[test]
fn no_regex_match_comes_from_a_parse_error_region() {
    // A docstring the error region holds, double- or single-quoted, in a function or a class.
    let files = [
        ("h.py", "def f():\n    \"Doc http://pysqdoc-db/x.\"\n    return (\n"),
        ("h2.py", "def f():\n    'Doc http://pysq2doc-db/x.'\n    return (\n"),
        ("h3.py", "class C:\n    \"Doc http://pyclsdoc-db/x.\"\n    x = (\n"),
    ];
    let program = compile(host_regex(), None).unwrap();
    for (path, facts, partial) in alone(&program, &files) {
        assert!(facts.is_empty(), "{path}: {facts:?}");
        assert!(partial, "{path} is partial");
    }
}

#[test]
fn no_query_capture_comes_from_a_parse_error_region() {
    let files = [
        ("g.py", "def f():\n    \"\"\"Doc http://pyerrdoc-db/x.\"\"\"\n    return (\n"),
        ("h.py", "def f():\n    \"Doc http://pysqdoc-db/x.\"\n    return (\n"),
    ];
    let program = compile(
        json!({"id": "strings", "files": ["**/*.py"], "language": "python", "query": "queries/strings.scm",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"}]}),
        Some("(string (string_content) @text)\n"),
    )
    .unwrap();
    for (path, facts, partial) in alone(&program, &files) {
        assert!(facts.is_empty(), "{path}: {facts:?}");
        assert!(partial, "{path} is partial");
    }
}

#[test]
fn a_code_string_far_from_a_parse_error_keeps_its_facts() {
    // Only the last statement is in error; the triple-quoted string at the top is code.
    let w = "Q = \"\"\"http://pysql-db/x\"\"\"\n\ndef run():\n    return Q\n\nx = (\n";
    let program = compile(host_regex(), None).unwrap();
    let found = alone(&program, &[("w.py", w)]);
    let [(_, facts, partial)] = &found[..] else { panic!("{found:?}") };
    assert_eq!(facts.first().map(String::as_str), Some("pysql-db"), "{facts:?}");
    assert!(partial, "a parse with errors makes the file partial");
}

#[test]
fn a_fixed_prop_that_breaks_its_schema_is_pack_invalid() {
    let emit = |prop: &str, value: &serde_json::Value| {
        json!({"id": "flags", "files": ["**/flags.yaml"], "regex": r"(?m)^(\w+)$",
               "emit": [{"node": "flags.flag", "key": "$1", "props": {prop: value}}]})
    };
    for (prop, value, keyword) in [
        ("color", json!("green"), "enum"),
        ("pct", json!(500), "maximum"),
        ("pct", json!(-1), "minimum"),
        ("rollout", json!("abc"), "type"),
        ("rollout", json!(1.5), "type"),
        ("ids", json!(["x"]), "items"),
    ] {
        let invalid = compile(emit(prop, &value), None).expect_err(&format!("{prop} = {value}"));
        assert_eq!(invalid.code(), ErrorCode::PackInvalid);
        let at = ["emit", "0", "props", prop]
            .map(|s| s.parse().map_or_else(|_| Step::Key(s.into()), Step::Index))
            .to_vec();
        let why = Why::PropValue { kind: "flags.flag".into(), prop: prop.into(), keyword };
        assert_eq!((&invalid.why, &invalid.at), (&why, &at), "{prop} = {value}");
    }
    // A value that meets its schema, and one on a prop the schema doesn't declare, load.
    for (prop, value) in [
        ("color", json!("red")),
        ("pct", json!(100)),
        ("rollout", json!(3)),
        ("ids", json!([1, 2])),
        ("settings", json!({"a": 1})),
        ("tier", json!("any")),
    ] {
        assert!(compile(emit(prop, &value), None).is_ok(), "{prop} = {value}");
    }
}

#[test]
fn a_double_quote_whose_close_comes_after_a_comment_mark_opens_no_string() {
    // A double quote that opens no string: Ruby's `?"`, a Clojure `\"` and a YAML plain scalar's
    // inch mark. Each would close at the comment's first quote and read the comment's URL as code.
    let files = [
        ("r30.rb", "c = ?\" # the \"http://rbdq-db/x\" quote\n"),
        ("r31.clj", "(def q \\\") ; see \"http://cljdq-db/x\"\n"),
        ("r32.yaml", "a: b\" # see \"http://yamldq-db/x\"\n"),
        // A stray double quote before a `//` comment that holds a quoted URL.
        ("r33.swift", "let s = \"oops // see \"http://straydq-db/x\"\n"),
    ];
    let program = compile(host_regex(), None).unwrap();
    for (path, facts, partial) in alone(&program, &files) {
        assert!(facts.is_empty(), "{path}: {facts:?}");
        assert!(!partial, "{path}");
    }
}

#[test]
fn a_url_in_a_string_matches_though_its_slashes_are_a_comment_mark() {
    // `//` right after a `:` is no comment mark, so the string closes and its URL is code; the
    // comment after it stays a comment.
    let files = [
        ("app/a.swift", "let u = \"http://orders-db/x\" // see http://swiftcomment-db/x\n"),
        ("app/b.swift", "let u = 'http://orders-sq-db/x' // see http://swiftcomment-db/x\n"),
    ];
    let program = compile(host_regex(), None).unwrap();
    let found = alone(&program, &files);
    assert_eq!(found[0].1, ["orders-db", "Alias(\"host:orders-db\")"], "{found:?}");
    assert_eq!(found[1].1, ["orders-sq-db", "Alias(\"host:orders-sq-db\")"], "{found:?}");
}

#[test]
fn a_colon_glued_comment_mark_after_a_non_scheme_word_is_a_comment() {
    // A Swift regex literal's double quote, then a comment glued to `k:`. A one-letter word is no
    // URL scheme, and a `//` before a space starts no URL, so the mark starts a comment, the quote
    // opens no string and the comment's URL makes no fact.
    let files = [
        ("p01.swift", "let q = /\"/; let o = [k:// see http://colonhole-db/x \"z\"]\n"),
        ("p02.swift", "let q = /\"/; let o = [ab:// see http://colonhole2-db/x \"z\"]\n"),
        ("p03.swift", "let q = /\"/; let o = [9a://x see http://colonhole3-db/x \"z\"]\n"),
    ];
    let program = compile(host_regex(), None).unwrap();
    for (path, facts, partial) in alone(&program, &files) {
        assert!(facts.is_empty(), "{path}: {facts:?}");
        assert!(!partial, "{path}");
    }
    // A scheme of two or more characters, then a host, is still a URL inside a string.
    let files = [
        ("app/c.swift", "let u = \"git+ssh://a/x http://scheme-db/x\" // see http://swiftcomment-db/x\n"),
        ("app/d.swift", "let u = \"ws://a/x http://ws-db/x\" // see http://swiftcomment-db/x\n"),
    ];
    let found = alone(&program, &files);
    assert_eq!(found[0].1, ["scheme-db", "Alias(\"host:scheme-db\")"], "{found:?}");
    assert_eq!(found[1].1, ["ws-db", "Alias(\"host:ws-db\")"], "{found:?}");
}

/// `depth` JavaScript arrays nested in one another, each holding a string, so one file has a
/// capture at every depth of its tree.
fn nested_js(depth: usize) -> String {
    format!("x = {}{};\n", "[\"a\",".repeat(depth), "]".repeat(depth))
}

/// `depth` Python functions nested in one another, each indented one more space, then a string
/// with a URL at the deepest one.
fn nested_py(depth: usize) -> String {
    let mut text = String::new();
    for d in 0..depth {
        text.push_str(&format!("{}def f():\n", " ".repeat(d)));
    }
    text.push_str(&format!("{}x = (\"http://deep-db/x\")\n", " ".repeat(depth)));
    text
}

fn strings_query(language: &str, glob: &str) -> Declarative {
    compile(
        json!({"id": "strings", "files": [glob], "language": language, "query": "queries/strings.scm",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"}]}),
        Some("(string (string_fragment) @text)\n"),
    )
    .unwrap()
}

/// What [`a_query_over_deeply_nested_code_runs_in_linear_time`] may take in a debug build: more
/// than four times the 0.15 s it took on a shared 4-CPU build machine. A runner that walked each
/// capture's ancestors took 10 s there.
const NESTED_BOUND_MS: u64 = 2_000;

#[test]
fn a_query_over_deeply_nested_code_runs_in_linear_time() {
    // Each capture's ancestors are as many as the nesting is deep, so a check that walks them
    // costs time cubic in the depth for the file.
    let text = nested_js(250);
    let names: Vec<String> = (0..8).map(|i| format!("d{i}.js")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), text.as_str())).collect();
    let (view, paths) = memory(&files);
    let program = strings_query("javascript", "**/*.js");
    let start = std::time::Instant::now();
    let batch = program.run(&view, &paths);
    let took = start.elapsed();
    assert_eq!(batch.hints.len(), 8, "one hint per file: every capture cites the one line");
    assert!(batch.partial.is_empty(), "{:?}", batch.partial);
    assert!(took < std::time::Duration::from_millis(NESTED_BOUND_MS), "{took:?}");
}

#[test]
fn a_file_nested_past_the_bound_is_partial_and_never_parsed() {
    let regex = compile(host_regex(), None).unwrap();
    let query = strings_query("javascript", "**/*.js");
    let deep_js = format!("x = {}\"http://deep-db/x\"{};\n", "[".repeat(300), "]".repeat(300));
    // Past the bound: 520 levels of indentation with an open bracket, which overflows the Python
    // scanner's state, and 300 levels of brackets.
    for (program, path, text) in [
        (&regex, "deep.py", nested_py(520)),
        (&regex, "deep.js", deep_js.clone()),
        (&query, "deep.js", deep_js.clone()),
    ] {
        let found = alone(program, &[(path, &text)]);
        let [(_, facts, partial)] = &found[..] else { panic!("{found:?}") };
        assert!(facts.is_empty(), "{path}: {facts:?}");
        assert!(partial, "{path} is partial");
        // The reason names the bound.
        let (view, paths) = memory(&[(path, &text)]);
        let batch = program.run(&view, &paths);
        let reason = Some(PartialReason::NestingLimit);
        assert_eq!(batch.partial, [PartialFile { path: format!("web/{path}"), errors: 1, reason }]);
        assert_eq!(serde_json::to_value(&batch.partial[0]).unwrap()["reason"], "nesting_limit");
    }
    // A grammar's file one level within the bound still parses, and a file in a language with no
    // grammar is never measured.
    let edge_js = format!("x = {}\"http://edge-db/x\"{};\n", "[".repeat(255), "]".repeat(255));
    let deep_swift = format!("let x = {}\"http://swift-db/x\"{}\n", "[".repeat(300), "]".repeat(300));
    let found = alone(&regex, &[("edge.js", &edge_js), ("deep.swift", &deep_swift)]);
    for (path, facts, partial) in found {
        assert!(!facts.is_empty() && !partial, "{path}: {facts:?}");
    }
    // Within it, a file 200 levels deep still parses.
    let shallow_js = format!("x = {}\"http://deep-db/x\"{};\n", "[".repeat(200), "]".repeat(200));
    for (program, path, text) in [
        (&regex, "ok.py", nested_py(200)),
        (&regex, "ok.js", shallow_js.clone()),
        (&query, "ok.js", shallow_js.clone()),
    ] {
        let found = alone(program, &[(path, &text)]);
        let [(_, facts, partial)] = &found[..] else { panic!("{found:?}") };
        assert!(!facts.is_empty(), "{path}: {facts:?}");
        assert!(!partial, "{path}");
    }
}

/// `depth` Python functions nested in one another, each indented one column more than the last,
/// with a string holding a URL at the deepest one. An indentation wider than `run` columns is
/// written as runs of at most `run` spaces joined by a backslash and a newline, which Python's
/// scanner counts as one indentation, so no physical line is wider than `run` columns.
fn continued_py(depth: usize, run: usize) -> String {
    let indent = |d: usize| {
        let mut out = String::new();
        let mut left = d;
        while left > run {
            out.push_str(&" ".repeat(run));
            out.push_str("\\\n");
            left -= run;
        }
        out.push_str(&" ".repeat(left));
        out
    };
    let mut text = String::new();
    for d in 0..depth {
        text.push_str(&format!("{}def f():\n", indent(d)));
    }
    text.push_str(&format!("{}x = (\"http://deep-db/x\")\n", indent(depth)));
    text
}

#[test]
fn indentation_continued_across_backslash_newlines_counts_against_the_nesting_bound() {
    // 520 levels, no physical line wider than 200 columns: the scanner adds the runs up and would
    // push more indentation than its saved state holds, which aborts the process.
    let text = continued_py(520, 200);
    let widest = text.lines().map(|l| l.len() - l.trim_start().len()).max();
    assert_eq!(widest, Some(200), "no physical line is indented past the bound");
    let regex = compile(host_regex(), None).unwrap();
    let query = compile(
        json!({"id": "strings", "files": ["**/*.py"], "language": "python", "query": "queries/strings.scm",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"}]}),
        Some("(string (string_content) @text)\n"),
    )
    .unwrap();
    for program in [&regex, &query] {
        let (view, paths) = memory(&[("deep.py", &text)]);
        let batch = program.run(&view, &paths);
        assert!(batch.hints.is_empty() && batch.edges.is_empty(), "{batch:?}");
        let reason = Some(PartialReason::NestingLimit);
        assert_eq!(batch.partial, [PartialFile { path: "web/deep.py".into(), errors: 1, reason }]);
    }
    // Continued lines within the bound still parse, and give their facts.
    let ok = continued_py(200, 64);
    assert!(ok.contains("\\\n"));
    for program in [&regex, &query] {
        let found = alone(program, &[("ok.py", &ok)]);
        let [(_, facts, partial)] = &found[..] else { panic!("{found:?}") };
        assert!(facts.iter().any(|f| f.contains("deep-db")), "{facts:?}");
        assert!(!partial, "{found:?}");
    }
}

/// A query extractor over `language`'s files that hints the text of each `@text` capture.
fn text_query(language: &str, glob: &str, query: &str) -> Declarative {
    compile(
        json!({"id": "texts", "files": [glob], "language": language, "query": "queries/texts.scm",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"}]}),
        Some(query),
    )
    .unwrap()
}

/// Files whose parse or query costs time quadratic in their size, each with a URL in a string
/// before the costly part, or whose captures' text grows as the square of their size, past what
/// their repo's capture budget holds, and the extractors that read them.
fn costly() -> Vec<(Declarative, &'static str, String)> {
    let opens = |n: usize| "/* ".repeat(n);
    let url = "\"http://work-db/x\"";
    let strings_js = || strings_query("javascript", "**/*.js");
    let strings_ts = || {
        compile(
            json!({"id": "strings", "files": ["**/*.ts"], "language": "typescript", "query": "queries/strings.scm",
                   "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "@text"}]}),
            Some("(string (string_fragment) @text)\n"),
        )
        .unwrap()
    };
    let go = format!("package main\n\nvar u = {url}\n\n{}", opens(10_000));
    let js = format!("var u = {url};\n{}", opens(100_000));
    let ts = format!("const u: string = {url};\n{}", opens(100_000));
    // A member chain and a left-nested sum: a tree as deep as the file is long, with no bracket.
    let member = format!("var u = {url};\nx = a{};\n", ".b".repeat(50_000));
    let binary = format!("x = {}1;\n", "\"http://work-db/x\" + ".repeat(50_000));
    // Nested member expressions over one long string: each capture's text holds the string, so the
    // text the captures copy grows as the square of the file though the tree stays shallow to the
    // cursor, and passes the capture budget of a repo that holds only this file.
    let captured = format!("x = \"{}\"{};\n", "a".repeat(100_000), ".b".repeat(500));
    vec![
        (compile(host_regex(), None).unwrap(), "f.go", go.clone()),
        (text_query("go", "**/*.go", "(interpreted_string_literal) @text\n"), "f.go", go),
        (compile(host_regex(), None).unwrap(), "f.js", js.clone()),
        (strings_js(), "f.js", js),
        (compile(host_regex(), None).unwrap(), "f.ts", ts.clone()),
        (strings_ts(), "f.ts", ts),
        (
            text_query("javascript", "**/*.js", "(member_expression (property_identifier) @text)\n"),
            "m.js",
            member,
        ),
        (
            text_query(
                "javascript",
                "**/*.js",
                "(binary_expression (binary_expression (string (string_fragment) @text)))\n",
            ),
            "b.js",
            binary,
        ),
        (text_query("javascript", "**/*.js", "(member_expression) @text\n"), "c.js", captured),
    ]
}

/// What [`a_file_past_the_work_bound_is_partial_and_gives_no_fact`] may take in a debug build, for
/// all its files: four times the 2.5 s it took on a shared 4-CPU build machine. Without the bound,
/// the Go file alone took 145 s there, and the left-nested sum 85 s with a query bounded by its
/// progress reports alone.
const WORK_BOUND_MS: u64 = 10_000;

#[test]
fn a_file_past_the_work_bound_is_partial_and_gives_no_fact() {
    let start = std::time::Instant::now();
    for (program, path, text) in costly() {
        let (view, paths) = memory(&[(path, &text)]);
        let batch = program.run(&view, &paths);
        let name = format!("{} over {path}", program.name());
        assert!(batch.hints.is_empty() && batch.edges.is_empty() && batch.nodes.is_empty(), "{name}");
        let reason = Some(PartialReason::WorkLimit);
        assert_eq!(batch.partial, [PartialFile { path: format!("web/{path}"), errors: 1, reason }], "{name}");
        assert_eq!(serde_json::to_value(&batch.partial[0]).unwrap()["reason"], "work_limit");
    }
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_millis(WORK_BOUND_MS), "{took:?}");
}

#[test]
fn a_file_as_large_as_a_refused_one_stays_within_the_work_bound() {
    // The same sizes in ordinary code: one comment each, short member chains and sums, and a URL
    // on every line, so each line gives a hint and a line of an edge's evidence, or two strings.
    let lines = |line: &str, bytes: usize| line.repeat(bytes / line.len());
    let go = format!(
        "package main\n\n{}",
        lines("// get it\nfunc f() string { return \"http://work-db/x\" + g.h.i }\n", 30_000)
    );
    let js = lines("/* get it */ const u = get('http://work-db/x').then(r => r.b.c + 'x');\n", 300_000);
    let ts = lines("/* get it */ const u: string = get(\"http://work-db/x\").b.c + \"y\";\n", 300_000);
    let member = lines("x = a.b.b.b.b.b.b.b.b.b + \"http://work-db/x\";\n", 100_000);
    let binary = lines("x = \"http://work-db/x\" + \"http://work-db/x\" + 1;\n", 900_000);
    let captured = lines(&format!("x = \"{}\".b.c;\n", "a".repeat(200)), 101_000);
    let files =
        [("f.go", go), ("f.js", js), ("f.ts", ts), ("m.js", member), ("b.js", binary), ("c.js", captured)];
    for (program, path, _) in costly() {
        let (_, text) = files.iter().find(|(p, _)| *p == path).unwrap();
        let (view, paths) = memory(&[(path, text)]);
        let batch = program.run(&view, &paths);
        let name = format!("{} over {path}", program.name());
        assert!(batch.partial.is_empty(), "{name}: {:?}", batch.partial);
        assert!(!batch.hints.is_empty(), "{name}");
    }
}

#[test]
fn dense_everyday_files_keep_their_facts() {
    // Files of some 300 KB with a fact or more on every line, in shapes ordinary code takes: a
    // URL a line under a hint and an edge, a URL with its port and path under a hint with four
    // attributes, two strings a line, and every member access of chained calls. Their facts hold
    // many times their size, and each keeps them all, within its own budget.
    let lines = |line: &str| line.repeat(300_000 / line.len());
    let members = || text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let cases: Vec<(Declarative, &str, String)> = vec![
        (
            compile(host_regex(), None).unwrap(),
            "h.py",
            lines("    r = requests.get('http://work-db/items', timeout=30)\n"),
        ),
        (
            compile(url_regex(&["**"]), None).unwrap(),
            "u.js",
            lines("const api = 'http://work-db:8080/v1/items';\n"),
        ),
        (
            strings_query("javascript", "**/*.js"),
            "s.js",
            lines("const u = get('http://work-db/x', 'items');\n"),
        ),
        (members(), "m.js", lines("const u = get('http://work-db/x').then(r => r.b.c);\n")),
        (members(), "d.js", lines("  const items = this.props.store.items.filter(i => i.active);\n")),
    ];
    for (program, path, text) in &cases {
        let files = [(*path, text.as_str())];
        let (view, paths) = memory(&files);
        let before = heap::held();
        let batch = program.run(&view, &paths);
        let (held, _) = heap_of(&batch, before);
        let name = format!("{} over {path}", program.name());
        assert!(batch.partial.is_empty(), "{name}: {:?}", batch.partial);
        assert_eq!(given(&batch), [format!("web/{path}")], "{name}");
        let lines = text.lines().count();
        assert!(batch.hints.len() >= lines, "{name}: {} hints over {lines} lines", batch.hints.len());
        assert!(held <= capture_budget(&files), "{name}: {held} bytes of heap");
    }
}

/// What [`a_regex_runs_in_linear_time_over_a_1_mib_file`] may take over its whole file in a
/// debug build: six times the 2.6 s it took on the CPU of a 4-CPU build machine, and five times
/// the 3.1 s it took there by the wall clock.
const LINEAR_BOUND_MS: u64 = 16_000;

/// How many times as long [`a_regex_runs_in_linear_time_over_a_1_mib_file`] may take over its
/// whole file as over its first half: twice, as linear time takes, with room for a shared
/// machine. On a 4-CPU build machine the runner took 2.0 to 2.2 times as long; with six busy
/// loops beside it, 1.7 to 2.7 times by the wall clock and 2.0 to 2.1 on the CPU. A runner that
/// scanned an edge's evidence for each line it adds took 3.4 to 3.7 times as long, busy or not,
/// and 11 s on the CPU over the whole file.
const DOUBLING_MAX: f64 = 2.9;

/// How many runs of each of its files [`a_regex_runs_in_linear_time_over_a_1_mib_file`] times.
const RUNS: usize = 3;

/// The time this thread has spent on a CPU, where the system tells it, as Linux's schedstat
/// does: a busy machine that keeps the thread waiting adds nothing to it.
fn on_cpu() -> Option<std::time::Duration> {
    let stat = fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    stat.split_whitespace().next()?.parse().ok().map(std::time::Duration::from_nanos)
}

/// How long `f` took: its time on the CPU where [`on_cpu`] reads it, the wall clock elsewhere.
fn timed<T>(f: impl FnOnce() -> T) -> (T, std::time::Duration) {
    let (cpu, wall) = (on_cpu(), std::time::Instant::now());
    let out = f();
    let took = match (cpu, on_cpu()) {
        (Some(start), Some(end)) => end.saturating_sub(start),
        _ => wall.elapsed(),
    };
    (out, took)
}

#[test]
fn a_regex_runs_in_linear_time_over_a_1_mib_file() {
    // Every line matches, and every match makes a hint and cites its line on one edge, so a cost
    // per match that grows with the file, or with the edge's evidence, shows as quadratic time:
    // the whole file takes more than twice as long as its first half. A file with no grammar
    // keeps a parse's time out of the count.
    let line = "y = get('http://a-db/x')\n";
    let text = line.repeat((1 << 20) / line.len());
    assert!(text.len() <= 1 << 20, "within the walk's per-file bound");
    let half = line.repeat((1 << 20) / line.len() / 2);
    let program = compile(host_regex(), None).unwrap();
    let repos = [memory(&[("big.txt", &text)]), memory(&[("big.txt", &half)])];
    let run = |(view, paths): &(RepoView, Vec<RepoPath>)| {
        let (batch, took) = timed(|| program.run(view, paths));
        let lines = std::str::from_utf8(view.read(&paths[0]).unwrap()).unwrap().lines().count();
        assert!(batch.partial.is_empty(), "{:?}", batch.partial);
        assert_eq!(batch.hints.len(), lines, "a hint per line");
        assert_eq!(batch.edges.len(), 1);
        assert_eq!(batch.edges[0].evidence.len(), lines, "the edge cites every line once");
        took
    };
    // The quickest of three runs of each, the whole file and its half taken in turn, each timed
    // on the CPU where the system tells it: a busy spell on a shared machine slows a run of each
    // alike, and the quickest of each misses it.
    let mut quickest = [std::time::Duration::MAX; 2];
    for _ in 0..RUNS {
        for (repo, best) in repos.iter().zip(quickest.iter_mut()) {
            *best = (*best).min(run(repo));
        }
    }
    let [whole, half] = quickest;
    let growth = whole.as_secs_f64() / half.as_secs_f64();
    assert!(
        growth <= DOUBLING_MAX,
        "{whole:?} over the file, {half:?} over its first half: {growth:.2} times"
    );
    assert!(whole < std::time::Duration::from_millis(LINEAR_BOUND_MS), "{whole:?}");
}

/// What [`the_evidence_of_every_line_of_a_1_mib_file_verifies_in_linear_time`] may take in a
/// debug build: more than four times the 0.35 s it took on a shared 4-CPU build machine. A verifier
/// that reads each cited line by splitting the blob took 57 s there.
const VERIFY_BOUND_MS: u64 = 2_000;

#[test]
fn the_evidence_of_every_line_of_a_1_mib_file_verifies_in_linear_time() {
    // Ingest checks every cited line against the blob at its commit, so a check that reads the
    // blob from its start for each line is quadratic in the file.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("deploy");
    fs::create_dir_all(&root).unwrap();
    let line = "y = get('http://a-db/x')\n";
    let text = line.repeat((1 << 20) / line.len());
    fs::write(root.join("big.py"), &text).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["add", "-A"]);
    git(&root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
    let view = view(&root);
    let mut batch = compile(host_regex(), None).unwrap().run(&view, &[RepoPath("big.py".into())]);
    let cited = batch.hints.len() + batch.edges[0].evidence.len();
    assert_eq!(cited, 2 * text.lines().count());
    let refs = Git::new(&root).refs().unwrap();
    let mut verifier = Verifier::new(&view.name, &view.commit, &refs, Git::new(&root).objects().unwrap());
    let start = std::time::Instant::now();
    let dropped = verifier.verify(&mut batch).unwrap();
    let took = start.elapsed();
    assert_eq!(dropped, []);
    assert_eq!(batch.hints.len() + batch.edges[0].evidence.len(), cited, "every line verifies");
    assert!(took < std::time::Duration::from_millis(VERIFY_BOUND_MS), "{took:?}");
}

/// The text a batch's hints and nodes copied out of their repo: each hint's key and value, each
/// node's ID and name, and every fact's evidence path and line.
fn copied(batch: &FactBatch) -> u64 {
    let evidence = |e: &[Evidence]| -> usize {
        e.iter()
            .map(|e| match e {
                Evidence::Line { path, contains, .. } => {
                    path.len() + contains.as_ref().map_or(0, String::len)
                }
                _ => 0,
            })
            .sum()
    };
    let hints: usize = batch
        .hints
        .iter()
        .map(|h| h.key.len() + h.value.as_ref().map_or(0, String::len) + evidence(&h.evidence))
        .sum();
    let nodes: usize = batch.nodes.iter().map(|n| n.id.0.len() + n.name.len() + evidence(&n.evidence)).sum();
    (hints + nodes) as u64
}

/// The capture budgets of `files`, each its own floor and share, summed.
fn capture_budget(files: &[(&str, &str)]) -> u64 {
    files.iter().map(|(_, t)| CAPTURE_PER_BYTE * t.len() as u64 + CAPTURE_FLOOR).sum()
}

/// The capture budgets of those of `files` that gave `batch` facts, summed: what their facts may
/// hold.
fn kept_budget(batch: &FactBatch, files: &[(&str, &str)]) -> u64 {
    let refused: BTreeSet<&str> = batch.partial.iter().map(|p| p.path.as_str()).collect();
    let kept: Vec<(&str, &str)> =
        files.iter().filter(|(p, _)| !refused.contains(format!("web/{p}").as_str())).copied().collect();
    capture_budget(&kept)
}

/// The files any of `batch`'s facts cites, in sorted order.
fn given(batch: &FactBatch) -> Vec<&str> {
    let evidence = batch
        .hints
        .iter()
        .flat_map(|h| &h.evidence)
        .chain(batch.nodes.iter().flat_map(|n| &n.evidence))
        .chain(batch.edges.iter().flat_map(|e| &e.evidence));
    let files: BTreeSet<&str> = evidence
        .map(|e| match e {
            Evidence::Line { path, .. } => path.as_str(),
            other => panic!("{other:?}"),
        })
        .collect();
    files.into_iter().collect()
}

/// The files of `batch`'s hints, in sorted order.
fn hinted(batch: &FactBatch) -> Vec<&str> {
    let files: BTreeSet<&str> = batch
        .hints
        .iter()
        .map(|h| match &h.evidence[0] {
            Evidence::Line { path, .. } => path.as_str(),
            other => panic!("{other:?}"),
        })
        .collect();
    files.into_iter().collect()
}

/// What [`what_many_small_files_hold_is_bounded_by_their_own_budgets`] may take in a debug
/// build: four times the 10 s it took on a shared 4-CPU build machine. With no capture budget,
/// its JavaScript files alone took 70 s there, and some 1.5 GB of keys.
const MANY_FILES_BOUND_MS: u64 = 40_000;

#[test]
fn what_many_small_files_hold_is_bounded_by_their_own_budgets() {
    // 1 to 2 KB files whose member chains nest up to 391 captures over one 1,000-byte string:
    // each capture's text holds the string, so the deeper chains hold many times their file and
    // pass its budget, and a few thousand such files are well within the walk.
    let nested = |i: usize| format!("x = \"{}\"{};\n", "a".repeat(1_000), ".b".repeat(1 + 10 * (i % 40)));
    // A regex whose key names its match as often as a template may, over 200 distinct matches a
    // file: densely packed matches pass their file's budget, sparse ones keep their facts.
    let run = |i: usize| {
        let gap = " ".repeat(i % 40);
        (0..200).map(|j| format!("a{j:03}{gap}")).collect::<String>() + "\n"
    };
    let regex = compile(
        json!({"id": "runs", "files": ["**/*.txt"], "regex": r"(a\d+)",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file",
                         "key": "$1".repeat(TEMPLATE_MAX_VARS)}]}),
        None,
    )
    .unwrap();
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let start = std::time::Instant::now();
    for (program, ext, count) in [(&query, "js", 2_000), (&regex, "txt", 500)] {
        let names: Vec<String> = (0..count).map(|i| format!("s/{i:04}.{ext}")).collect();
        let texts: Vec<String> = (0..count).map(|i| if ext == "js" { nested(i) } else { run(i) }).collect();
        let files: Vec<(&str, &str)> =
            names.iter().map(String::as_str).zip(texts.iter().map(String::as_str)).collect();
        let (view, paths) = memory(&files);
        // Run once first: a regex keeps the cache its matching grew for its next run, which is
        // no fact's heap.
        drop(program.run(&view, &paths));
        let before = heap::held();
        let batch = program.run(&view, &paths);
        let (held, peak) = heap_of(&batch, before);
        let name = program.name();
        let kept = kept_budget(&batch, &files);
        assert!(copied(&batch) <= kept, "{name}: {} > {kept}", copied(&batch));
        assert!(held <= kept, "{name}: the batch holds {held} bytes of heap, past {kept}");
        let budget = capture_budget(&files);
        assert!(peak <= 2 * budget, "{name}: the run held {peak} bytes of heap, past twice {budget}");
        kept_or_refused(&batch, &names);
    }
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_millis(MANY_FILES_BOUND_MS), "{took:?}");
}

/// The heap `batch`'s facts hold, all of what its run left on this thread since it held
/// `before` but the list of partial files, whose entries a run's refused files add; and the most
/// the run held while it ran, its files' matches and its parses' work included.
fn heap_of(batch: &FactBatch, before: i64) -> (u64, u64) {
    let peak = u64::try_from(heap::peak() - before).unwrap();
    let list = batch.partial.capacity() * std::mem::size_of::<PartialFile>();
    let paths: usize = batch.partial.iter().map(|p| p.path.capacity()).sum();
    let allocations = i64::try_from(batch.partial.len() + 1).unwrap() * heap::PER_ALLOCATION;
    let held = heap::held() - before - i64::try_from(list + paths).unwrap() - allocations;
    (u64::try_from(held).unwrap(), peak)
}

/// Some of `names` gave `batch` facts and the rest were refused, each with the work bound as its
/// reason; none is missing.
fn kept_or_refused(batch: &FactBatch, names: &[String]) {
    let name = &batch.extractor;
    let kept = given(batch);
    let refused: Vec<&str> = batch.partial.iter().map(|p| p.path.as_str()).collect();
    assert!(!kept.is_empty() && !refused.is_empty(), "{name}: {kept:?} {refused:?}");
    let reason = Some(PartialReason::WorkLimit);
    assert!(batch.partial.iter().all(|p| p.reason == reason && p.errors == 1), "{name}");
    let mut all: Vec<&str> = kept.into_iter().chain(refused).collect();
    all.sort_unstable();
    let expected: Vec<String> = names.iter().map(|n| format!("web/{n}")).collect();
    assert_eq!(all, expected, "{name}");
}

#[test]
fn a_file_past_its_capture_budget_is_refused_alone() {
    // One file whose captures pass its own budget is refused alone: the ordinary files before
    // and after it in sorted order keep their facts.
    let costly = format!("x = \"{}\"{};\n", "a".repeat(100_000), ".b".repeat(500));
    let ordinary = "const u = get('http://work-db/x').then(r => r.b.c);\n".repeat(200);
    let files = [("a.js", ordinary.as_str()), ("b.js", costly.as_str()), ("c.js", ordinary.as_str())];
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let (view, paths) = memory(&files);
    let batch = query.run(&view, &paths);
    let reason = Some(PartialReason::WorkLimit);
    assert_eq!(batch.partial, [PartialFile { path: "web/b.js".into(), errors: 1, reason }]);
    assert_eq!(hinted(&batch), ["web/a.js", "web/c.js"]);
    assert!(copied(&batch) <= kept_budget(&batch, &files));
}

/// Each of `files`' facts in `batch`, as JSON: its hints, nodes and edges whose evidence names
/// it, and its entry in the list of partial files.
fn facts_of(batch: &FactBatch, file: &str) -> Vec<serde_json::Value> {
    let path = format!("web/{file}");
    let names = |e: &[Evidence]| e.iter().any(|e| matches!(e, Evidence::Line { path: p, .. } if *p == path));
    let mut out: Vec<serde_json::Value> = Vec::new();
    out.extend(batch.hints.iter().filter(|h| names(&h.evidence)).map(|h| serde_json::to_value(h).unwrap()));
    out.extend(batch.nodes.iter().filter(|n| names(&n.evidence)).map(|n| serde_json::to_value(n).unwrap()));
    out.extend(batch.edges.iter().filter(|e| names(&e.evidence)).map(|e| serde_json::to_value(e).unwrap()));
    out.extend(batch.partial.iter().filter(|p| p.path == path).map(|p| serde_json::to_value(p).unwrap()));
    out
}

#[test]
fn a_files_facts_never_depend_on_the_files_before_it() {
    // Twenty files, each alone in a repo and then beside the others with a file before them all
    // in sorted order that is added, changed and removed. Half hold more per byte than their
    // budget allows, member chains that each hold one long string again, so they give no fact
    // wherever they are; the other half, dense and sparse ordinary code, give the same facts
    // wherever they are. No cut falls: the repo holds far less than its bound.
    let chained = format!("x = \"{}\"{};\n", "a".repeat(2_000), ".b".repeat(300));
    let dense = "  const items = this.props.store.items.filter(i => i.active);\n".repeat(2_000);
    let sparse = "const total = compute(items, 'http://work-db/x', options.limit) + 1;\n".repeat(1_500);
    let costly = format!("x = \"{}\"{};\n", "a".repeat(100_000), ".b".repeat(60));
    let names: Vec<String> = (0..20).map(|i| format!("z{i:02}.js")).collect();
    let zs: Vec<(&str, &str)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let text = match i % 4 {
                0 | 2 => chained.as_str(),
                1 => dense.as_str(),
                _ => sparse.as_str(),
            };
            (n.as_str(), text)
        })
        .collect();
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let alone: Vec<Vec<serde_json::Value>> = zs
        .iter()
        .map(|&(name, text)| {
            let (view, paths) = memory(&[(name, text)]);
            facts_of(&query.run(&view, &paths), name)
        })
        .collect();
    assert!(alone.iter().any(|f| f.iter().any(|v| v.get("key").is_some())), "some file gives facts");
    assert!(alone.iter().any(|f| f.iter().any(|v| v["reason"] == "work_limit")), "some file is refused");
    for before in [None, Some(costly.as_str()), Some(sparse.as_str()), Some("const a = b.c;\n")] {
        let files: Vec<(&str, &str)> =
            before.map(|t| ("a.js", t)).into_iter().chain(zs.iter().copied()).collect();
        let (view, paths) = memory(&files);
        let batch = query.run(&view, &paths);
        for (i, &(name, _)) in zs.iter().enumerate() {
            assert_eq!(facts_of(&batch, name), alone[i], "{name} beside {:?}", before.map(str::len));
        }
    }
}

/// What [`a_text_predicate_reads_within_the_work_bound`] may take in a debug build: more than
/// four times the 0.3 s it took on a shared 4-CPU build machine. A cursor that read predicate
/// text without counting it took more than 120 s there for the first predicate alone.
const PREDICATE_BOUND_MS: u64 = 2_000;

#[test]
fn a_text_predicate_reads_within_the_work_bound() {
    // Nested member expressions over one string of a million bytes: a predicate on each capture
    // reads the string again, so the text the predicates read grows as the square of the file.
    let text = format!("x = \"{}\"{};\n", "a".repeat(1_000_000), ".b".repeat(8_000));
    assert!(text.len() <= 1 << 20, "within the walk's per-file bound");
    let start = std::time::Instant::now();
    for predicate in [
        r#"(#match? @text "[A-Z_]{3,}")"#,
        r#"(#eq? @text "zzz")"#,
        r#"(#any-of? @text "a" "b")"#,
        r#"(#not-match? @text "a")"#,
    ] {
        let query =
            text_query("javascript", "**/*.js", &format!("((member_expression) @text {predicate})\n"));
        let (view, paths) = memory(&[("p.js", &text)]);
        let batch = query.run(&view, &paths);
        assert!(batch.hints.is_empty() && batch.nodes.is_empty(), "{predicate}");
        let reason = Some(PartialReason::WorkLimit);
        assert_eq!(
            batch.partial,
            [PartialFile { path: "web/p.js".into(), errors: 1, reason }],
            "{predicate}"
        );
    }
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_millis(PREDICATE_BOUND_MS), "{took:?}");
}

#[test]
fn an_ordinary_file_with_text_predicates_keeps_its_facts() {
    let line = "x = a.b.c.d + get('http://work-db/x', CONFIG_KEY).then(r => r.ok);\n";
    let text = line.repeat(300_000 / line.len());
    // Each predicate reads every node of its kind on each line, and keeps one fact a line, or
    // three for the member chains that start `a.b`: `a.b`, `a.b.c` and `a.b.c.d`.
    for (query, expected) in [
        ("((member_expression) @text (#match? @text \"^a\\\\.b\"))\n", "a.b"),
        (
            "((call_expression function: (identifier) @fn arguments: (arguments (string (string_fragment) @text))) \
             (#eq? @fn \"get\"))\n",
            "http://work-db/x",
        ),
        ("((identifier) @text (#any-of? @text \"CONFIG_KEY\" \"OTHER\"))\n", "CONFIG_KEY"),
        ("((identifier) @text (#match? @text \"^[A-Z_]{3,}$\"))\n", "CONFIG_KEY"),
    ] {
        let program = text_query("javascript", "**/*.js", query);
        let (view, paths) = memory(&[("ok.js", &text)]);
        let batch = program.run(&view, &paths);
        assert!(batch.partial.is_empty(), "{query}: {:?}", batch.partial);
        assert!(batch.hints.iter().any(|h| h.key == expected), "{query}");
        assert_eq!(hinted(&batch), ["web/ok.js"], "{query}");
    }
}

/// The heap this thread holds, counted by this test binary's allocator: the bytes it asked for,
/// plus [`heap::PER_ALLOCATION`] for each allocation, about what the system allocator adds to one.
/// Counted per thread, so tests running beside it on other threads don't change it.
mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub const PER_ALLOCATION: i64 = 16;

    thread_local! {
        static BYTES: Cell<i64> = const { Cell::new(0) };
        static ALLOCATIONS: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    fn count(bytes: i64, allocations: i64) {
        // During a thread's teardown its counters are gone; nothing is measured then.
        let _ = BYTES.try_with(|c| c.set(c.get() + bytes));
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + allocations));
        if let Ok(now) = BYTES.try_with(Cell::get) {
            let now = now + PER_ALLOCATION * ALLOCATIONS.try_with(Cell::get).unwrap_or(0);
            let _ = PEAK.try_with(|p| p.set(p.get().max(now)));
        }
    }

    fn size(layout: Layout) -> i64 {
        i64::try_from(layout.size()).unwrap_or(i64::MAX)
    }

    struct Counting;

    // SAFETY: every call goes to the system allocator with the caller's own arguments; only the
    // counts are added.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller's contract for `alloc` is the system allocator's.
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                count(size(layout), 1);
            }
            p
        }

        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            // SAFETY: `p` came from this allocator, which is the system allocator.
            unsafe { System.dealloc(p, layout) };
            count(-size(layout), -1);
        }

        unsafe fn realloc(&self, p: *mut u8, layout: Layout, new: usize) -> *mut u8 {
            // SAFETY: as for `dealloc`, and the caller's contract for `realloc`.
            let q = unsafe { System.realloc(p, layout, new) };
            if !q.is_null() {
                count(i64::try_from(new).unwrap_or(i64::MAX) - size(layout), 0);
            }
            q
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;

    /// The heap this thread holds now; from now on, the most it has held is what it holds.
    pub fn held() -> i64 {
        let now = BYTES.with(Cell::get) + PER_ALLOCATION * ALLOCATIONS.with(Cell::get);
        PEAK.with(|p| p.set(now));
        now
    }

    /// The most heap this thread has held since [`held`] last read it.
    pub fn peak() -> i64 {
        PEAK.with(Cell::get)
    }
}

#[test]
fn a_runs_facts_hold_no_more_heap_than_its_capture_budget() {
    // A member access on every four-byte line: each match copies a few bytes of text, but each
    // fact it makes, its ID, its evidence and its place in the batch, holds hundreds of bytes.
    // Files of 1 to 3,600 lines: the smaller ones keep their facts and the larger ones pass their
    // budget, so the files that keep theirs hold up to their budget.
    let texts: Vec<String> = (1..=60).map(|i| "a.b\n".repeat(i * i)).collect();
    let names: Vec<String> = (1..=60).map(|i| format!("t{i:03}.js")).collect();
    let files: Vec<(&str, &str)> =
        names.iter().map(String::as_str).zip(texts.iter().map(String::as_str)).collect();
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let regex = |emit: serde_json::Value| {
        compile(json!({"id": "dots", "files": ["**/*.js"], "regex": r"(\w+)\.(\w+)", "emit": emit}), None)
            .unwrap()
    };
    let programs = [
        ("a hint", query),
        // The architecture's example: a file node, an edge and a hint for every match.
        (
            "a node, an edge and a hint",
            regex(json!([{"node": "code.file", "key": "$file"},
                         {"edge": "code.imports", "from": "@id:code.file:$file", "to": "@alias:module:$1", "via": "$2"},
                         {"hint": "code.url", "scope": "@id:code.file:$file", "key": "$2"}])),
        ),
        (
            "a hint with one attr",
            regex(
                json!([{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$2", "attrs": {"host": "$1"}}]),
            ),
        ),
        (
            "a hint with five attrs and a value",
            regex(json!([{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$2", "value": "$1",
                          "attrs": {"scheme": "$1", "host": "$2", "port": "$1", "path": "$2", "method": "$1"}}])),
        ),
        (
            "a node with props, aliases and a parent",
            regex(
                json!([{"node": "flags.flag", "key": "$1$2$line", "name": "$2", "parent": "@id:code.file:$file",
                          "props": {"label": "$1", "weight": 1.5, "color": "red", "ids": [1, 2],
                                    "settings": {"owner": "web", "tags": ["a", "b"], "limits": {"max": 3}}},
                          "aliases": ["flag:$1", "flag:$2", "f:$1", "f:$2", "flag:$1$2"]}]),
            ),
        ),
        (
            "an edge with props",
            regex(
                json!([{"edge": "flags.reads_flag", "from": "@id:code.file:$file", "to": "@id:flags.flag:$1$2$line",
                          "props": {"required": true}}]),
            ),
        ),
    ];
    for (name, program) in &programs {
        let (view, paths) = memory(&files);
        // Run once first: a regex keeps the cache its matching grew for its next run, which is
        // no fact's heap.
        drop(program.run(&view, &paths));
        let before = heap::held();
        let batch = program.run(&view, &paths);
        let (held, peak) = heap_of(&batch, before);
        let kept = kept_budget(&batch, &files);
        assert!(held <= kept, "{name}: the batch holds {held} bytes of heap, past {kept}");
        let budget = capture_budget(&files);
        assert!(peak <= 2 * budget, "{name}: the run held {peak} bytes of heap, past twice {budget}");
        kept_or_refused(&batch, &names);
        // Each file alone: the largest that keeps its facts holds close to its own budget, so a
        // charge below what a fact holds lets it hold more.
        for file in &files {
            let (view, paths) = memory(&[*file]);
            let before = heap::held();
            let batch = program.run(&view, &paths);
            let (held, _) = heap_of(&batch, before);
            let budget = capture_budget(&[*file]);
            assert!(held <= budget, "{name} over {}: {held} bytes of heap, past {budget}", file.0);
        }
    }
}

#[test]
fn a_runs_kept_matches_count_against_its_capture_budget() {
    // One 1 MiB file of lines of a thousand numbers, under a hint whose scope is the file and
    // whose key is fixed: each line makes one fact, but each number is a distinct match the run
    // keeps, with its groups, until the file's facts are made. With more groups each match keeps
    // more, and past the file's budget the file is refused before the run holds more.
    let line = format!("{}\n", (0..1_000).map(|i| format!("{i:03}")).collect::<Vec<_>>().join(" "));
    let text = line.repeat((1 << 20) / line.len());
    let files = [("n.txt", text.as_str())];
    let budget = capture_budget(&files);
    let mut refused = false;
    for groups in [1, 5, 20, 60] {
        let regex = format!(r"(\d+){}", "()".repeat(groups - 1));
        let program = compile(
            json!({"id": "numbers", "files": ["**"], "regex": regex,
                   "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "k"}]}),
            None,
        )
        .unwrap();
        let (view, paths) = memory(&files);
        // Run once first: a regex keeps the cache its matching grew for its next run.
        drop(program.run(&view, &paths));
        let before = heap::held();
        let batch = program.run(&view, &paths);
        let (held, peak) = heap_of(&batch, before);
        assert!(held <= budget, "{groups} groups: the batch holds {held} bytes of heap, past {budget}");
        assert!(
            peak <= 2 * budget,
            "{groups} groups: the run held {peak} bytes of heap, past twice {budget}"
        );
        if batch.partial.is_empty() {
            assert_eq!(batch.hints.len(), text.lines().count(), "{groups} groups: a hint a line");
        } else {
            let reason = Some(PartialReason::WorkLimit);
            assert_eq!(batch.partial, [PartialFile { path: "web/n.txt".into(), errors: 1, reason }]);
            refused = true;
        }
    }
    assert!(refused, "the file with the most groups passes its budget");
}

#[test]
fn a_template_that_names_a_variable_many_times_holds_no_more_than_its_files_budget() {
    // A regex with an empty group under templates that name it as often as a template may: the
    // text it adds is empty, so the facts' text stays small, but the run tells one fact from
    // another by the values its ID takes, and must never hold a copy of them per time the
    // template names one. One more name than that is refused when the pack loads. Files of 40-byte lines and of 2-byte lines, a match on each, and one
    // line of 8,000 distinct matches, as a minified file has.
    let long = "a".to_owned() + &"-".repeat(38) + "\n";
    let one: Vec<String> = (0..8_000).map(|i| format!("a{i:04}")).collect();
    let files = [("l.txt", long.repeat(1_000)), ("s.txt", "a\n".repeat(20_000)), ("o.txt", one.join(" "))];
    for times in [0, 31, TEMPLATE_MAX_VARS - 1] {
        let empties = "$2".repeat(times);
        let emits = [
            (
                "a hint",
                json!([{"hint": "code.url", "scope": "@id:code.file:$file", "key": format!("$1{empties}")}]),
            ),
            (
                "an edge",
                json!([{"edge": "code.imports", "from": "@id:code.file:$file",
                        "to": format!("@alias:host:$1{empties}")}]),
            ),
        ];
        for (emitted, emit) in emits {
            let program =
                compile(json!({"id": "empties", "files": ["**"], "regex": r"(a\d*)()", "emit": emit}), None)
                    .unwrap();
            for (path, text) in &files {
                let file = [(*path, text.as_str())];
                let (view, paths) = memory(&file);
                // Run once first: a regex keeps the cache its matching grew for its next run.
                drop(program.run(&view, &paths));
                let before = heap::held();
                let batch = program.run(&view, &paths);
                let (held, peak) = heap_of(&batch, before);
                let budget = capture_budget(&file);
                let name = format!("{emitted} naming $2 {times} times over {path}");
                assert!(held <= budget, "{name}: the batch holds {held} bytes of heap, past {budget}");
                assert!(peak <= 2 * budget, "{name}: the run held {peak} bytes of heap, past twice {budget}");
                let reason = Some(PartialReason::WorkLimit);
                assert!(batch.partial.iter().all(|p| p.reason == reason), "{name}: {:?}", batch.partial);
            }
        }
    }
    let past = format!("$1{}", "$2".repeat(TEMPLATE_MAX_VARS));
    let refused = compile(
        json!({"id": "empties", "files": ["**"], "regex": r"(a\d*)()",
                       "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": past}]}),
        None,
    )
    .unwrap_err();
    assert_eq!(refused.why, Why::TemplateTooLong);
}

#[test]
fn the_same_text_at_a_longer_path_charges_more() {
    // A file's facts copy its path into their evidence and IDs, so what its matches charge
    // depends on its path as well as its text: of files of 5,000 lines of one length, some keep
    // their facts at a short path and are refused at a long one, and none the other way round.
    let program = compile(
        json!({"id": "lines", "files": ["**"], "regex": "(a)",
               "emit": [{"hint": "code.url", "scope": "@id:code.file:$file", "key": "$1"}]}),
        None,
    )
    .unwrap();
    let long = format!("{}/x.txt", "d".repeat(150));
    let mut differ = Vec::new();
    for width in 2..=40 {
        let text = format!("a{}\n", "b".repeat(width - 2)).repeat(5_000);
        let refused = |path: &str| {
            let (view, paths) = memory(&[(path, text.as_str())]);
            let batch = program.run(&view, &paths);
            assert!(batch.partial.iter().all(|p| p.reason == Some(PartialReason::WorkLimit)), "{width}");
            !batch.partial.is_empty()
        };
        let (short, longer) = (refused("x.txt"), refused(&long));
        assert!(!short || longer, "lines of {width} bytes are refused at x.txt but kept at a longer path");
        if short != longer {
            differ.push(width);
        }
    }
    assert!(!differ.is_empty(), "some line length is kept at the short path and refused at the long one");
}

#[test]
fn a_costly_file_takes_no_facts_from_the_ordinary_files_after_it() {
    // A file early in sorted order whose nested member expressions each copy its 100,000-byte
    // string, then twenty ordinary files, each holding less than its own budget.
    let costly = format!("x = \"{}\"{};\n", "a".repeat(100_000), ".b".repeat(340));
    let ordinary = "const total = compute(items, 'http://work-db/x', options.limit) + 1;\n".repeat(1_500);
    let names: Vec<String> = (0..20).map(|i| format!("z{i:02}.js")).collect();
    let files: Vec<(&str, &str)> = std::iter::once(("a.js", costly.as_str()))
        .chain(names.iter().map(|n| (n.as_str(), ordinary.as_str())))
        .collect();
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let (view, paths) = memory(&files);
    let batch = query.run(&view, &paths);
    let ordinary: Vec<String> = names.iter().map(|n| format!("web/{n}")).collect();
    let kept: Vec<String> =
        hinted(&batch).into_iter().filter(|f| *f != "web/a.js").map(String::from).collect();
    assert_eq!(kept, ordinary, "every ordinary file keeps its facts: {:?}", batch.partial);
    let reason = Some(PartialReason::WorkLimit);
    assert!(batch.partial.iter().all(|p| p.path == "web/a.js" && p.reason == reason), "{:?}", batch.partial);
    assert!(copied(&batch) <= kept_budget(&batch, &files));
}

/// The repo's bound for [`the_repo_bound_cuts_an_extractors_facts_in_sorted_path_order`]: some
/// six files' worth of what each holds, lowered from [`REPO_CAPTURE_MAX`] so the test needs no
/// repo whose facts hold 256 MiB. The runner takes a lowered bound only in a build with debug
/// assertions, so the test, and this bound, build only there.
#[cfg(debug_assertions)]
const SMALL_REPO_MAX: u64 = 6 * 1_000 * 700;

#[cfg(debug_assertions)]
#[test]
fn the_repo_bound_cuts_an_extractors_facts_in_sorted_path_order() {
    // Files that each hold far less than their own budget, and together more than the bound:
    // the facts come from a prefix of them in sorted order, and the first file past the bound
    // and every file after it give none, each reported under the repo's bound, a file that
    // would hold nothing included.
    use planroom_extract::declarative::REPO_CAPTURE_MAX;
    let line = "const total = compute(items, 'http://work-db/x', options.limit) + 1;\n";
    let ordinary = line.repeat(1_000);
    let names: Vec<String> = (0..20).map(|i| format!("f{i:02}.js")).collect();
    let mut files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), ordinary.as_str())).collect();
    files.push(("g.js", "const nothing = 1;\n"));
    let query = text_query("javascript", "**/*.js", "(member_expression) @text\n");
    let (view, paths) = memory(&files);
    let mut reversed = paths.clone();
    reversed.reverse();
    let batch = query.run_with_repo_capture_max(&view, &reversed, SMALL_REPO_MAX);
    let kept = hinted(&batch);
    assert!(kept.len() > 1 && kept.len() < names.len(), "{kept:?}");
    let prefix: Vec<String> = names[..kept.len()].iter().map(|n| format!("web/{n}")).collect();
    assert_eq!(kept, prefix, "a prefix in sorted order");
    let reason = Some(PartialReason::RepoLimit);
    let refused: Vec<PartialFile> = names[kept.len()..]
        .iter()
        .map(String::as_str)
        .chain(["g.js"])
        .map(|n| PartialFile { path: format!("web/{n}"), errors: 1, reason })
        .collect();
    assert_eq!(batch.partial, refused);
    assert_eq!(serde_json::to_value(&batch.partial[0]).unwrap()["reason"], "repo_limit");
    // What the prefix holds is within the bound, and one more file would pass it.
    let before = heap::held();
    let alone = query.run(&view, &paths[..kept.len()]);
    assert!(heap_of(&alone, before).0 <= SMALL_REPO_MAX);
    assert_eq!(serde_json::to_value(&alone.hints).unwrap(), serde_json::to_value(&batch.hints).unwrap());
    // The same cut every run, and a repo within the bound has none.
    let again = query.run_with_repo_capture_max(&view, &paths, SMALL_REPO_MAX);
    assert_eq!(serde_json::to_string(&again).unwrap(), serde_json::to_string(&batch).unwrap());
    assert!(query.run(&view, &paths).partial.is_empty(), "within REPO_CAPTURE_MAX");
    assert_eq!(REPO_CAPTURE_MAX, 256 << 20);
}

#[test]
fn an_ordinary_file_keeps_its_facts_under_a_predicate_on_nesting_nodes() {
    // A test file: two describes around 200 cases, each a promise chain. A predicate on a call or
    // an arrow function reads every call it nests in again, so it reads the file several times.
    let case = |i: usize| {
        format!(
            "    it('lists page {i}', () => {{\n      return fetch('http://work-db/items?page={i}')\n        \
             .then(r => r.json())\n        .then(body => {{\n          expect(body.items).toHaveLength({i});\n        \
             }});\n    }});\n"
        )
    };
    let cases: String = (0..200).map(case).collect();
    let js = format!("describe('api', () => {{\n  describe('items', () => {{\n{cases}  }});\n}});\n");
    // A class of 200 methods: a predicate on a block reads the class's block and each method's.
    let method = |i: usize| {
        format!(
            "    def page_{i}(self):\n        r = requests.get('http://work-db/items', params={{'page': {i}}})\n        \
             return r.json()\n\n"
        )
    };
    let methods: String = (0..200).map(method).collect();
    let py = format!("import requests\n\n\nclass Items:\n{methods}");
    for (language, file, text, query, facts) in [
        ("javascript", "t.test.js", &js, "((call_expression) @text (#match? @text \"^fetch\"))\n", true),
        ("javascript", "t.test.js", &js, "((call_expression) @text (#eq? @text \"zzz\"))\n", false),
        ("javascript", "t.test.js", &js, "((arrow_function) @text (#match? @text \"fetch\"))\n", true),
        ("python", "items.py", &py, "((block) @text (#match? @text \"requests\"))\n", true),
    ] {
        let program = text_query(language, &format!("**/*.{}", file.rsplit('.').next().unwrap()), query);
        let (view, paths) = memory(&[(file, text)]);
        let batch = program.run(&view, &paths);
        assert!(batch.partial.is_empty(), "{query}: {:?}", batch.partial);
        assert_eq!(!batch.hints.is_empty(), facts, "{query}");
    }
}

#[test]
fn git_facts_carry_ref_evidence() {
    // Two more commits that change the compose file and main.ts together, a tag and a branch.
    let dir = tempfile::tempdir().unwrap();
    let first = repo(dir.path());
    let root = first.root.clone();
    for n in 2..4 {
        fs::write(root.join("src/app/main.ts"), format!("export const x = {n};\n")).unwrap();
        fs::write(root.join("docker-compose.yml"), format!("{COMPOSE}# {n}\n")).unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Change both"]);
    }
    git(&root, &["-c", "tag.gpgsign=false", "tag", "-a", "v1.0.0", "-m", "v1.0.0"]);
    git(&root, &["branch", "feature"]);
    let view = view(&root);
    let files = walked(&view).files;
    let batch = run(&view, "git", &files, json!(null));
    assert_eq!(
        serde_json::to_string(&batch).unwrap(),
        serde_json::to_string(&run(&view, "git", &files, json!(null))).unwrap(),
        "deterministic"
    );

    // Tags and branches make no nodes: only the one author is one.
    let kinds: BTreeSet<&str> = batch.nodes.iter().map(|n| n.kind.0.as_str()).collect();
    assert_eq!(kinds, BTreeSet::from(["code.person"]));
    assert_eq!(batch.nodes.len(), 1);
    let relations: BTreeSet<&str> = batch.edges.iter().map(|e| e.kind.0.as_str()).collect();
    assert_eq!(relations, BTreeSet::from(["code.co_change", "code.owned_by"]));
    let pair = batch.edges.iter().find(|e| e.kind.0 == "code.co_change").unwrap();
    assert_eq!(pair.props["count"], 3, "the first commit and the two after it");

    // Every fact's evidence is a Ref to a commit of the history, naming no ref, and verifies.
    let history = Command::new("git").args(["rev-list", "HEAD"]).current_dir(&root).output().unwrap();
    let history: BTreeSet<String> =
        String::from_utf8_lossy(&history.stdout).lines().map(str::to_owned).collect();
    let evidence =
        batch.nodes.iter().flat_map(|n| &n.evidence).chain(batch.edges.iter().flat_map(|e| &e.evidence));
    for item in evidence {
        match item {
            Evidence::Ref { commit, ref_name: None } => assert!(history.contains(commit), "{commit}"),
            other => panic!("not a Ref with no ref name: {other:?}"),
        }
    }
    let refs = Git::new(&root).refs().unwrap();
    let mut checked = batch.clone();
    let mut verifier = Verifier::new(&view.name, &view.commit, &refs, Git::new(&root).objects().unwrap());
    assert!(verifier.verify(&mut checked).unwrap().is_empty());
    assert_eq!(checked, batch);

    // A Ref that names a tag or a branch names it in full: a short name, which a tag and a branch
    // can share, doesn't verify.
    let at = |name: &str| Evidence::Ref { commit: view.commit.clone(), ref_name: Some(name.into()) };
    let mut named = batch.clone();
    named.nodes[0].evidence =
        vec![at("refs/tags/v1.0.0"), at("refs/heads/feature"), at("v1.0.0"), at("feature")];
    let dropped = verifier.verify(&mut named).unwrap();
    let reasons: Vec<&str> = dropped.iter().map(|d| d.reason.as_str()).collect();
    assert_eq!(reasons, ["ref v1.0.0 doesn't exist", "ref feature doesn't exist"]);
    assert_eq!(named.nodes[0].evidence, [at("refs/tags/v1.0.0"), at("refs/heads/feature")]);
}

/// A compose service whose name holds a newline or a backslash, which no node ID can hold, gives
/// no hint at all: no container could be made of it. The file is partial; its other services
/// stand.
#[test]
fn a_compose_service_name_no_node_id_can_hold_gives_no_hint() {
    let text = "services:\n  \"a\\nb\":\n    image: acme/a:1\n    environment:\n      - X=1\n  'back\\slash':\n    image: acme/b:1\n  web:\n    image: acme/web:1\n";
    let (view, files) = memory(&[("docker-compose.yml", text)]);
    let batch = run(&view, "compose", &files, json!(null));
    for hint in &batch.hints {
        assert!(!hint.key.contains(['\n', '\\']), "{hint:?}");
        assert_eq!(hint.attrs.get("service").and_then(|s| s.as_str()).unwrap_or("web"), "web", "{hint:?}");
    }
    let services: Vec<&str> =
        batch.hints.iter().filter(|h| h.kind.0 == "code.compose_service").map(|h| h.key.as_str()).collect();
    assert_eq!(services, ["web"]);
    assert_eq!(
        batch.partial,
        [PartialFile { path: "web/docker-compose.yml".into(), errors: 2, reason: None }]
    );
    // The tooling container still names the file's own repo.
    let ids: Vec<&str> = batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(ids, ["web:container:web"]);
}

/// Compose's facts are bounded as the declarative runner's are: a file whose facts would hold more
/// heap than its size allows gives none and is partial with `work_limit`, and in sorted path order
/// the repo's cut falls on the first file whose facts would take the total past its bound.
#[test]
fn compose_facts_past_a_files_capture_budget_or_the_repo_bound_are_refused() {
    use planroom_extract::compose::Compose;
    use planroom_extract::declarative::CAPTURE_FLOOR;
    // An environment of a few bytes a variable on one line: each variable's hint holds far more
    // heap than its bytes.
    let vars: Vec<String> = (0..2_000).map(|i| format!("V{i}=1")).collect();
    let dense = format!("services:\n  big: {{image: acme/big:1, environment: [{}]}}\n", vars.join(", "));
    let small = "services:\n  web:\n    image: acme/web:1\n";
    let (view, files) = memory(&[("dense/docker-compose.yml", &dense), ("ok/docker-compose.yml", small)]);
    let batch = run(&view, "compose", &files, json!(null));
    let reasons: Vec<(&str, Option<PartialReason>)> =
        batch.partial.iter().map(|p| (p.path.as_str(), p.reason)).collect();
    assert_eq!(reasons, [("web/dense/docker-compose.yml", Some(PartialReason::WorkLimit))]);
    assert!(batch.hints.iter().all(|h| h.scope.0 == "web:file:ok/docker-compose.yml"), "{:?}", batch.hints);
    assert_eq!(batch.hints.len(), 1);

    // Six small files under a repo bound of half a floor, which the first file's facts, its
    // tooling container and its service, keep within: the cut is a suffix, after a kept file.
    let six: Vec<(String, &str)> = (0..6).map(|i| (format!("d{i}/docker-compose.yml"), small)).collect();
    let all: Vec<(&str, &str)> = six.iter().map(|(p, t)| (p.as_str(), *t)).collect();
    let (view, files) = memory(&all);
    let kinds = |k: &str, p: &str, l: &str| json!({"kind": k, "id_prefix": p, "level": l});
    let opts = json!({"kinds": {"service": kinds("code.service", "container", "container"),
                                "tooling": kinds("code.tooling", "container", "container"),
                                "repo": kinds("code.repo", "repo", "system"),
                                "file": kinds("code.file", "file", "code")}});
    let cut = Compose.run_with_repo_capture_max(&view, &files, &opts, CAPTURE_FLOOR / 2).unwrap();
    let reasons: Vec<(&str, Option<PartialReason>)> =
        cut.partial.iter().map(|p| (p.path.as_str(), p.reason)).collect();
    assert!(!reasons.is_empty() && reasons.len() < 6, "{reasons:?}");
    assert!(reasons.iter().all(|(_, r)| *r == Some(PartialReason::RepoLimit)), "{reasons:?}");
    let first = 6 - reasons.len();
    let suffix: Vec<String> = (first..6).map(|i| format!("web/d{i}/docker-compose.yml")).collect();
    assert_eq!(reasons.iter().map(|(p, _)| (*p).to_owned()).collect::<Vec<_>>(), suffix);
    let facts = serde_json::to_string(&(&cut.nodes, &cut.hints)).unwrap();
    for path in &suffix {
        assert!(!facts.contains(&format!("\"path\":\"{path}\"")), "{path} gave a fact");
    }
    // Under the real bound nothing is cut.
    assert!(run(&view, "compose", &files, json!(null)).partial.is_empty());
}

/// Compose charges each fact as it makes it, so a service whose long name every one of its many
/// variables copies is refused before its facts hold more heap than its file's budget, never after
/// they are all made.
#[test]
fn a_compose_files_run_holds_no_more_heap_than_its_capture_budget() {
    let name = "s".repeat(8_000);
    // An explicit key: a plain one may not run past 1,024 characters.
    let mut text = format!("services:\n  ? {name}\n  : image: acme/app:1\n    environment:\n");
    for i in 0..2_000 {
        text.push_str(&format!("      - V{i}=1\n"));
    }
    let files = [("docker-compose.yml", text.as_str())];
    let (view, paths) = memory(&files);
    let before = heap::held();
    let batch = run(&view, "compose", &paths, json!(null));
    let (_, peak) = heap_of(&batch, before);
    let budget = capture_budget(&files);
    assert_eq!(batch.partial.iter().map(|p| p.reason).collect::<Vec<_>>(), [Some(PartialReason::WorkLimit)]);
    assert!(peak <= 2 * budget, "the run held {peak} bytes of heap, past twice {budget}");
}

/// What [`a_compose_file_cites_its_lines_in_linear_time`] may take in a debug build: it takes under
/// 2 s, and reading each line by a pass over the file took over 25 s.
const COMPOSE_1_MIB_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// A compose file of nearly 1 MiB whose services sit after a long comment: every fact's evidence
/// is found through one index of the file's lines, never a pass over the text before it.
#[test]
fn a_compose_file_cites_its_lines_in_linear_time() {
    let mut text = String::new();
    while text.len() < 300_000 {
        text.push_str("# a long note before the services, as a generated file might carry it\n");
    }
    text.push_str("services:\n  app:\n    image: acme/app:1\n    environment:\n");
    let mut n = 0;
    while text.len() < 1_040_000 {
        text.push_str(&format!("      - VAR_{n}=val\n"));
        n += 1;
    }
    let (view, files) = memory(&[("docker-compose.yml", &text)]);
    let started = std::time::Instant::now();
    let batch = run(&view, "compose", &files, json!(null));
    let took = started.elapsed();
    assert!(batch.partial.is_empty(), "{:?}", batch.partial);
    assert_eq!(batch.hints.iter().filter(|h| h.kind.0 == "code.env_set").count(), n);
    assert!(took < COMPOSE_1_MIB_BUDGET, "{n} variables took {took:?}");
}

/// A declarative node emit whose captured key holds a newline or a backslash, which no node ID can
/// hold, makes no node, and its file is partial.
#[test]
fn a_captured_key_no_node_id_can_hold_gives_no_node() {
    let flags = "flag: plain;\nflag: back\\slash;\nflag: two\nlines;\n";
    let (view, files) = memory(&[("flags.txt", flags)]);
    let program = compile(
        json!({"id": "flags", "files": ["**/flags.txt"], "regex": r"flag: ([^;]+);",
               "emit": [{"node": "flags.flag", "key": "$1", "parent": "@id:code.file:$file"}]}),
        None,
    )
    .unwrap();
    let batch = program.run(&view, &files);
    let ids: Vec<&str> = batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(ids, ["web:flag:plain"]);
    assert_eq!(batch.partial, [PartialFile { path: "web/flags.txt".into(), errors: 2, reason: None }]);
}
