//! Discovery and what a person learns about the repos it finds (DR-71, DR-85): a root repo's
//! submodules and repos two levels down are repos, a workspace inside another warns, the repos
//! with uncommitted work are named on `scan`, `focus` and `map` with no rescan, a scan its bounds
//! cut says so in `focus`, and `scan`'s text prints its warnings.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use planroom_testkit::git_repo;
use serde_json::Value;

fn planroom_env(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args).current_dir(cwd).env("PLANROOM_DATA", data(cwd)).env_remove("PLANROOM_RAW");
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().expect("the binary runs")
}

fn planroom(cwd: &Path, args: &[&str]) -> Output {
    planroom_env(cwd, args, &[])
}

/// The test's own data root, beside its workspace, so nothing lands in the repos.
fn data(cwd: &Path) -> PathBuf {
    cwd.ancestors().map(|dir| dir.join("data-root")).find(|d| d.is_dir()).expect("a test data root")
}

/// A `--json` reply's envelope: one line on stdout, ok.
fn reply(cwd: &Path, args: &[&str]) -> Value {
    reply_env(cwd, args, &[])
}

fn reply_env(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let out = planroom_env(cwd, args, env);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "one JSON line: {stdout}\nstderr: {}", String::from_utf8_lossy(&out.stderr));
    let env: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(env["ok"], true, "{env}");
    env
}

/// A text-mode reply's stdout.
fn text(cwd: &Path, args: &[&str]) -> String {
    let out = planroom(cwd, args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// [`git`] with arguments that need not be UTF-8, such as a Latin-1 path.
fn git_os(dir: &Path, args: &[&OsStr]) {
    let out = Command::new("git").args(args).current_dir(dir).stdin(Stdio::null()).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A test directory with a data root of its own, and the workspace directory inside it.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("data-root")).unwrap();
    let ws = dir.path().join("ws");
    fs::create_dir(&ws).unwrap();
    (dir, ws.canonicalize().unwrap())
}

/// A committed repo at `dir` holding one file named after it.
fn repo(dir: &Path) {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    git_repo(dir, &[(format!("{name}.txt"), format!("{name}\n").into_bytes())]);
}

/// The repos a reply's map shows at the top, by name.
fn top_names(env: &Value) -> Vec<String> {
    let map = if env["command"] == "scan" { &env["result"]["map"] } else { &env["result"] };
    let mut names: Vec<String> =
        map["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap().to_owned()).collect();
    names.sort();
    names
}

fn warnings<'a>(env: &'a Value, code: &str) -> Vec<&'a Value> {
    env["meta"]["warnings"].as_array().map_or(vec![], |w| w.iter().filter(|w| w["code"] == code).collect())
}

#[test]
fn submodules_are_repos() {
    let (_dir, ws) = workspace();
    let sources = ws.parent().unwrap().join("sources");
    for name in ["core", "old"] {
        repo(&sources.join(name));
    }
    let root = ws.join("meta");
    repo(&root);
    for (name, path) in [("core", "libs/core"), ("old", "libs/old")] {
        let url = sources.join(name).display().to_string();
        git(&root, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, path]);
    }
    git(&root, &["commit", "-q", "-m", "Add the submodules"]);
    // An uninitialized submodule has no working tree, so it isn't a repo.
    git(&root, &["submodule", "deinit", "-q", "-f", "libs/old"]);

    let env = reply(&root, &["scan", ".", "--json"]);
    assert!(env["summary"].as_str().unwrap().starts_with("Mapped 2 repos"), "{env}");
    assert_eq!(top_names(&env), ["libs/core", "meta"], "the root repo and its initialized submodule");
    assert!(warnings(&env, "repo_skipped").is_empty(), "{env}");
    // Read from the submodule's own directory, it is the same workspace.
    let map = reply(&root.join("libs/core"), &["map", "--json"]);
    assert_eq!(top_names(&map), ["libs/core", "meta"]);
}

#[test]
fn repos_two_levels_down_are_found() {
    let (_dir, ws) = workspace();
    for path in ["acme/api", "acme/web", "tools/cli", ".cache/hidden"] {
        repo(&ws.join(path));
    }
    fs::create_dir_all(ws.join("notes")).unwrap();
    let env = reply(&ws, &["scan", ".", "--json"]);
    assert_eq!(top_names(&env), ["acme/api", "acme/web", "tools/cli"], "a hidden directory isn't searched");

    // With a repo at the first level, the second isn't searched.
    let (_dir, flat) = workspace();
    repo(&flat.join("top"));
    repo(&flat.join("group/deep"));
    let env = reply(&flat, &["scan", ".", "--json"]);
    assert_eq!(top_names(&env), ["top"]);
}

#[test]
fn a_nested_workspace_warns() {
    let (_dir, ws) = workspace();
    repo(&ws.join("app"));
    let inner = ws.join("inner");
    repo(&inner.join("lib"));
    let inner_env = reply(&inner, &["scan", ".", "--json"]);
    assert_eq!(top_names(&inner_env), ["lib"]);

    let env = reply(&ws, &["scan", ".", "--json"]);
    let nested = warnings(&env, "workspace_nested");
    assert_eq!(nested.len(), 1, "{env}");
    assert_eq!(nested[0]["details"]["other_root"], inner.display().to_string());
    assert!(nested[0]["message"].as_str().unwrap().contains("inner"), "{}", nested[0]);
    assert_eq!(top_names(&env), ["app"], "the inner workspace's repos stay its own");
    // Both keep their maps.
    assert_eq!(top_names(&reply(&inner, &["map", "--json"])), ["lib"]);
    assert_eq!(top_names(&reply(&ws, &["map", "--json"])), ["app"]);
    // A person scanning at a terminal sees it too.
    assert!(text(&ws, &["scan", "."]).contains("Another workspace sits inside this one"));
}

#[test]
fn focus_lists_dirty_repos() {
    let (_dir, ws) = workspace();
    for name in ["clean", "orders", "web"] {
        repo(&ws.join(name));
    }
    fs::write(ws.join("web/.gitignore"), "*.log\n").unwrap();
    git(&ws.join("web"), &["add", ".gitignore"]);
    git(&ws.join("web"), &["commit", "-q", "-m", "Ignore logs"]);
    reply(&ws, &["scan", ".", "--json"]);
    let before = reply(&ws, &["focus", "--json"]);
    assert!(before["meta"].get("dirty_repos").is_none(), "a clean workspace names none: {before}");

    // A changed file and an untracked one in orders; one untracked file and an ignored one in web.
    fs::write(ws.join("orders/orders.txt"), "changed\n").unwrap();
    fs::write(ws.join("orders/notes.md"), "draft\n").unwrap();
    fs::write(ws.join("web/new.ts"), "export {};\n").unwrap();
    fs::write(ws.join("web/debug.log"), "noise\n").unwrap();
    let env = reply(&ws, &["focus", "--json"]);
    assert_eq!(
        env["meta"]["dirty_repos"],
        serde_json::json!([{"repo": "orders", "files": 2}, {"repo": "web", "files": 1}]),
        "{env}"
    );
    assert_eq!(env["meta"]["seq"], before["meta"]["seq"], "no rescan");
    assert_eq!(env["result"], before["result"], "the pane's map is the committed one");
    let lines = text(&ws, &["focus"]);
    assert!(
        lines
            .lines()
            .any(|l| l == "2 repos have uncommitted files that aren't on the map: orders (2), web (1)."),
        "{lines}"
    );

    // One repo: the line the pane shows.
    fs::remove_file(ws.join("web/new.ts")).unwrap();
    let lines = text(&ws, &["focus"]);
    assert!(lines.lines().any(|l| l == "orders: 2 uncommitted files aren't on the map."), "{lines}");
}

#[test]
fn meta_carries_dirty_repos() {
    let (_dir, ws) = workspace();
    // Seven repos, one named like an AWS key: repo text, which the reply masks.
    let secret = "AKIAIOSFODNN7EXAMPLE";
    let names = ["r1", "r2", "r3", "r4", "r5", "r6", secret];
    for name in names {
        repo(&ws.join(name));
    }
    reply(&ws, &["scan", ".", "--json"]);
    for name in names {
        fs::write(ws.join(name).join("untracked.txt"), "u\n").unwrap();
    }
    let scan = reply(&ws, &["scan", ".", "--json"]);
    let map = reply(&ws, &["map", "--json"]);
    let focus = reply(&ws, &["focus", "--json"]);
    for env in [&scan, &map, &focus] {
        let meta = &env["meta"];
        let listed = meta["dirty_repos"].as_array().unwrap_or_else(|| panic!("{env}"));
        assert_eq!(listed.len(), 5, "a few listed: {meta}");
        assert_eq!(meta["more_dirty_repos"], 2, "and the rest counted: {meta}");
        assert!(listed.iter().all(|r| r["files"] == 1));
        // The first five by name: the key's name sorts first, masked.
        assert_eq!(listed[0]["repo"], "[redacted]", "{meta}");
        assert_eq!(listed[1]["repo"], "r1");
        assert!(!meta.to_string().contains(secret), "{meta}");
    }
    // Only the truth comparator's raw read sees the name.
    let raw = reply_env(&ws, &["focus", "--json"], &[("PLANROOM_RAW", "1")]);
    assert_eq!(raw["meta"]["dirty_repos"][0]["repo"], secret);
    // Each reply keeps to its leaf's budget.
    for env in [&scan, &map, &focus] {
        assert!(env["meta"].get("truncated").is_none(), "{env}");
    }
}

#[test]
fn a_cut_scan_says_so_in_focus() {
    let (_dir, ws) = workspace();
    let big = ws.join("big");
    fs::create_dir_all(&big).unwrap();
    git(&big, &["init", "-q", "-b", "main"]);
    // 50,001 committed files, one past the walk's file bound, written straight into the index:
    // one blob, many paths.
    let blob = git(&big, &["hash-object", "-w", "--stdin"]);
    let mut index = String::new();
    for i in 0..50_001 {
        index.push_str(&format!("100644 {blob}\td{:03}/f{i:05}.txt\n", i / 1000));
    }
    let mut child = Command::new("git")
        .args(["update-index", "--index-info"])
        .current_dir(&big)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), index.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
    git(&big, &["commit", "-q", "-m", "Many files"]);
    git(&big, &["reset", "-q", "--hard"]);
    repo(&ws.join("small"));

    let scan = reply(&ws, &["scan", ".", "--json"]);
    let cut = warnings(&scan, "partial_scan");
    assert_eq!(cut.len(), 1, "{}", scan["meta"]);
    assert_eq!((&cut[0]["details"]["repo"], &cut[0]["details"]["count"]), (&"big".into(), &1.into()));

    // Every later read of the pane says the repo is partly mapped, rescan or not.
    reply(&ws, &["scan", ".", "--json"]);
    let focus = reply(&ws, &["focus", "--json"]);
    let cut = warnings(&focus, "partial_scan");
    assert_eq!(cut.len(), 1, "{}", focus["meta"]);
    assert_eq!(cut[0]["details"]["repo"], "big");
    assert_eq!(cut[0]["details"]["count"], 1);
    assert_eq!(cut[0]["details"]["extractor"], "code/files");
    let lines = text(&ws, &["focus"]);
    let said =
        "1 files in big weren't scanned by code/files: past its bound, or missing from a partial clone.";
    assert!(lines.lines().any(|l| l == said), "{lines}");
    // The repo that wasn't cut says nothing.
    assert!(!lines.contains("in small"), "{lines}");
}

#[test]
fn scan_text_prints_its_warnings() {
    let (_dir, ws) = workspace();
    repo(&ws.join("api"));
    // A repo with no commit yet, and one with a path that isn't UTF-8.
    let fresh = ws.join("fresh");
    fs::create_dir_all(&fresh).unwrap();
    git(&fresh, &["init", "-q", "-b", "main"]);
    let legacy = ws.join("legacy");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("ok.py"), "ok = 1\n").unwrap();
    git(&legacy, &["init", "-q", "-b", "main"]);
    git(&legacy, &["add", "-A"]);
    // Latin-1 `café.py`, as a repo committed elsewhere holds it: into the index directly, since a
    // filesystem may refuse the name (APFS does) while a git tree never does, and kept out of the
    // working tree's status so the repo stays clean.
    let blob = git(&legacy, &["hash-object", "-w", "ok.py"]);
    let cafe = OsStr::from_bytes(b"caf\xe9.py");
    let info = [b"100644,".as_slice(), blob.trim().as_bytes(), b",", cafe.as_bytes()].concat();
    git_os(
        &legacy,
        &[
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            OsStr::from_bytes(&info),
        ],
    );
    git(&legacy, &["commit", "-q", "-m", "init"]);
    git_os(&legacy, &[OsStr::new("update-index"), OsStr::new("--skip-worktree"), cafe]);
    fs::write(ws.join("api/draft.md"), "draft\n").unwrap();

    let env = reply(&ws, &["scan", ".", "--json"]);
    let unnamed = warnings(&env, "unnamed_paths");
    assert_eq!(unnamed.len(), 1, "{}", env["meta"]);
    assert_eq!(unnamed[0]["details"]["repo"], "legacy");
    assert_eq!(unnamed[0]["details"]["paths"], serde_json::json!(["caf\\xe9.py"]));
    let lines = text(&ws, &["scan", "."]);
    for line in [
        "fresh has no commits yet; it maps after its first commit.",
        "1 files in legacy have paths that aren't UTF-8, or hold a newline or a backslash, so they aren't on the map.",
        "api: 1 uncommitted file isn't on the map.",
    ] {
        assert!(lines.lines().any(|l| l == line), "missing {line:?} in:\n{lines}");
    }
    assert!(lines.lines().next().unwrap().starts_with("Mapped 2 repos"), "the summary comes first: {lines}");
}
