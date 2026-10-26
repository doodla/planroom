//! The extractor contract for `files` and `compose`: deterministic, sorted, tolerant, bounded, and
//! every fact's evidence verifies against the commit.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use planroom_contract::{Evidence, FactBatch, Level};
use planroom_extract::git::{Git, is_repo};
use planroom_extract::verify::Verifier;
use planroom_extract::{RepoPath, RepoView, builtin, walk};
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
    let head = Git::new(&root).head().unwrap();
    RepoView { name: "deploy".into(), root, commit: head.commit, branch: head.branch.unwrap() }
}

fn run(view: &RepoView, name: &str, files: &[RepoPath], opts: serde_json::Value) -> FactBatch {
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
    let walked = walk::walk(&view.root).unwrap();
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

#[test]
fn compose_emits_containers_aliases_and_hints_on_their_lines() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let files = compose_files(&walk::walk(&view.root).unwrap().files);
    let opts = json!({"repos": ["deploy", "api", "web"],
                      "image_roles": [{"image": "postgres*", "role": "datastore", "kind": "code.datastore"}]});
    let batch = run(&view, "compose", &files, opts);
    let ids: Vec<&str> = batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(ids, ["api:container:api", "deploy:container:deploy", "web:container:web", "ws:container:db"]);
    let node = |id: &str| batch.nodes.iter().find(|n| n.id.0 == id).unwrap();
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
    let env: Vec<(&str, &str, Option<&str>)> = batch
        .hints
        .iter()
        .filter(|h| h.kind.0 == "code.env_set")
        .map(|h| (h.scope.0.as_str(), h.key.as_str(), h.value.as_deref()))
        .collect();
    assert_eq!(env.len(), 2);
    assert!(env.contains(&("web:container:web", "API_URL", Some("http://api:8080"))));
    assert!(env.contains(&("api:container:api", "DATABASE_URL", Some("postgres://db:5432/app"))));
    let deps: BTreeSet<(&str, &str)> = batch
        .hints
        .iter()
        .filter(|h| h.kind.0 == "code.depends_on")
        .map(|h| (h.scope.0.as_str(), h.key.as_str()))
        .collect();
    assert_eq!(deps, BTreeSet::from([("web:container:web", "api"), ("api:container:api", "db")]));
    let hint_ids: Vec<&String> = batch.hints.iter().map(|h| &h.id).collect();
    let mut sorted = hint_ids.clone();
    sorted.sort();
    assert_eq!(hint_ids, sorted, "hints are sorted by ID");
}

#[test]
fn extraction_is_deterministic_and_every_fact_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    let files = walk::walk(&view.root).unwrap().files;
    let refs = Git::new(&view.root).refs().unwrap();
    for (name, files, opts) in
        [("files", files.clone(), json!(null)), ("compose", compose_files(&files), json!({"repos": ["web"]}))]
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
    let view = repo(dir.path());
    // The working tree moves on; the commit doesn't.
    fs::write(view.root.join("src/app/main.ts"), "export const y = 2;\n").unwrap();
    let files = walk::walk(&view.root).unwrap().files;
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

#[test]
fn a_broken_compose_file_is_partial_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let view = repo(dir.path());
    fs::write(view.root.join("docker-compose.yml"), "services: [unclosed\n").unwrap();
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
