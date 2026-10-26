//! Where a workspace keeps its state (architecture.md, Where the workspace lives; DR-74): a
//! directory of repos keeps `.planroom/` at its root, and a workspace whose root is itself a repo
//! keeps `planroom/` inside that repo's git directory, where `git clean -x`, `git stash -u` and a
//! copy of the working tree never reach it. A `.planroom/` a repo commits is never its state.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use planroom_testkit::git_repo;
use serde_json::Value;

fn planroom(cwd: &Path, data: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", data)
        .env("XDG_CONFIG_HOME", data.join("config"))
        .output()
        .expect("the binary runs");
    let env: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("{args:?}: {}", String::from_utf8_lossy(&out.stderr)));
    assert!(out.status.success() && env["ok"] == true, "{env}");
    env
}

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8(out.stdout).unwrap()
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// A repo at `dir` with a compose service and a source directory, in one commit.
fn repo(dir: &Path) -> PathBuf {
    git_repo(
        dir,
        &[
            ("docker-compose.yml".into(), b"services:\n  web:\n    image: nginx:1\n".to_vec()),
            ("src/main.rs".into(), b"fn main() {}\n".to_vec()),
        ],
    );
    dir.canonicalize().unwrap()
}

fn repo_ids(cwd: &Path, data: &Path) -> Vec<String> {
    let map = planroom(cwd, data, &["map", "--json"]);
    map["result"]["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn a_single_repo_workspace_keeps_its_state_in_the_git_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let app = repo(&tmp.path().join("app"));
    let state = app.join(".git/planroom");

    // A read creates nothing, in either place.
    let focus = planroom(&app, &data, &["focus", "--json"]);
    assert_eq!(focus["result"]["next"]["command"], "scan", "{focus}");
    assert!(!state.exists() && !app.join(".planroom").exists(), "a read created a state directory");

    planroom(&app, &data, &["scan", "--json"]);
    assert_eq!(mode(&state), 0o700);
    assert_eq!(mode(&state.join("store.db")), 0o600);
    assert!(!state.join(".gitignore").exists(), "nothing in a git directory needs ignoring");
    assert!(!app.join(".planroom").exists(), "nothing in the working tree");
    assert_eq!(git(&app, &["status", "--porcelain", "--ignored"]), "", "the working tree is as committed");

    // Found from below the root, and the map is the repo's.
    let ids = repo_ids(&app.join("src"), &data);
    assert!(ids.iter().any(|id| id == "app:repo:app"), "{ids:?}");
    let focus = planroom(&app.join("src"), &data, &["focus", "--json"]);
    assert!(focus["result"]["empty"].is_null(), "{focus}");

    // git clean -x and git stash -u, which take every ignored file, leave it alone.
    fs::write(app.join("scratch.log"), "x").unwrap();
    git(&app, &["clean", "-xdf"]);
    fs::write(app.join("scratch.log"), "x").unwrap();
    git(&app, &["stash", "-u"]);
    assert!(!app.join("scratch.log").exists(), "the stash took the untracked file");
    assert!(state.join("store.db").is_file());
    let rescan = planroom(&app, &data, &["scan", "--json"]);
    assert!(rescan["summary"].as_str().unwrap().contains("0 of them moved"), "{rescan}");
}

#[test]
fn a_planroom_directory_a_repo_commits_is_never_its_state() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let app = tmp.path().join("app");
    git_repo(
        &app,
        &[
            ("README.md".into(), b"# app\n".to_vec()),
            (".planroom/config.toml".into(), b"models = \"off\"\n".to_vec()),
            (".planroom/store.db".into(), b"not a store\n".to_vec()),
        ],
    );
    let app = app.canonicalize().unwrap();
    // The committed directory isn't a workspace: a read offers the scan.
    let focus = planroom(&app, &data, &["focus", "--json"]);
    assert_eq!(focus["result"]["empty"]["reason"], "not_scanned", "{focus}");

    planroom(&app, &data, &["scan", "--json"]);
    assert!(app.join(".git/planroom/store.db").is_file());
    assert_eq!(
        fs::read(app.join(".planroom/store.db")).unwrap(),
        b"not a store\n",
        "the clone's file is untouched"
    );
    assert_eq!(git(&app, &["status", "--porcelain", "--ignored"]), "");
}

#[test]
fn a_linked_worktree_keeps_its_state_in_its_own_git_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let app = repo(&tmp.path().join("app"));
    git(&app, &["worktree", "add", "-q", "-b", "topic", "../topic"]);
    let topic = tmp.path().join("topic").canonicalize().unwrap();
    let git_dir = PathBuf::from(git(&topic, &["rev-parse", "--absolute-git-dir"]).trim());
    assert_ne!(git_dir, app.join(".git"), "a linked worktree has its own git directory");

    planroom(&topic, &data, &["scan", "--json"]);
    assert!(git_dir.join("planroom/store.db").is_file());
    assert!(!app.join(".git/planroom").exists(), "the main working tree's workspace is another one");
    assert!(!topic.join(".planroom").exists());
    let ids = repo_ids(&topic, &data);
    assert!(ids.iter().any(|id| id == "topic:repo:topic"), "{ids:?}");
}

#[test]
fn a_single_repo_workspace_inside_a_directory_of_repos_stays_apart() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let ws = tmp.path().join("ws");
    let inner = repo(&ws.join("inner"));
    repo(&ws.join("outer"));
    planroom(&inner, &data, &["scan", "--json"]);

    let scanned = planroom(&ws, &data, &["scan", "--json"]);
    assert!(ws.join(".planroom/store.db").is_file(), "a directory of repos keeps .planroom/ at its root");
    let nested: Vec<&Value> = scanned["meta"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["code"] == "workspace_nested")
        .collect();
    assert_eq!(nested.len(), 1, "{scanned}");
    assert_eq!(nested[0]["details"]["other_root"], inner.display().to_string());
    let ids = repo_ids(&ws, &data);
    assert!(ids.iter().any(|id| id == "outer:repo:outer"), "{ids:?}");
    assert!(!ids.iter().any(|id| id.starts_with("inner:")), "{ids:?}");
    // A session inside the inner repo keeps to its own workspace.
    let ids = repo_ids(&inner, &data);
    assert!(ids.iter().any(|id| id == "inner:repo:inner"), "{ids:?}");
}
