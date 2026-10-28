//! Dev-only test helpers: the fixture wrapper, the truth comparator and golden helpers.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub mod truth;

/// The repository root, found from this crate's manifest directory.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/testkit sits two levels down")
        .to_path_buf()
}

/// The fixture's five repos.
pub const REPOS: [&str; 5] = ["kit", "orders", "platform", "storefront", "worker"];

/// What a fixture build made.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The workspace root: the five repos, and no `.planroom/` yet: the engine's first scan
    /// creates the state directory and records it as trusted in the test's data root (DR-48).
    pub root: PathBuf,
    /// `build-fixture`'s output: each repo's branch heads.
    pub heads: Value,
}

/// Copy the fixture's five repos into `out`. `out` may be absent or an empty directory.
///
/// It writes no `.planroom/`: a state directory the engine didn't make is untrusted, and its
/// config stays unloaded until a person accepts the trust card (DR-48). So the engine's first
/// scan creates it and records it in the data root the test runs with, and a test that needs a
/// loaded workspace config scans first, then calls [`write_config`].
///
/// `examples/build-fixture` runs once per set of fixture sources, not once per test: the first
/// call builds into the target directory beside the test binaries and every later call, in any
/// test process, copies that build. Each test still gets its own copy, so its commits and branch switches stay
/// its own. Building per test wrote thousands of git objects at once under nextest, which failed on
/// macOS runners with `unable to create temporary file: Invalid argument`.
pub fn fixture(out: &Path) -> Fixture {
    let root = repo_root();
    let (built, _shared) = built_fixture(&root);
    copy_tree(&built.join("tree"), out);
    let heads = fs::read(built.join("heads.json")).expect("the built fixture's heads.json reads");
    let heads = serde_json::from_slice(&heads).expect("build-fixture prints its heads as JSON");
    // The engine reports physical paths, and on macOS a temporary directory sits behind a link
    // (/var is /private/var), so the root a test compares with is the physical one (DR-83).
    let root = out.canonicalize().unwrap_or_else(|_| out.to_path_buf());
    Fixture { root, heads }
}

/// The built fixture for the current sources, `CACHE/planroom-fixture-<sha256>/` with `tree/` (the
/// five repos), `heads.json` (build-fixture's output) and `files.txt` (every path in `tree/`).
/// It comes with a shared lock on `CACHE/planroom-fixture-<sha256>.lock`, held while the caller
/// copies, so no other process rebuilds the directory under it.
///
/// `CACHE` is `target/<profile>/fixture-cache/`, beside the test binaries: a path only this
/// checkout's user writes, so no other user of a shared machine can plant a build whose git config
/// runs a command, and `cargo clean` removes it.
fn built_fixture(root: &Path) -> (PathBuf, File) {
    let hash = sources_hash(root);
    let tmp = cache_dir();
    let dir = tmp.join(format!("planroom-fixture-{hash}"));
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(tmp.join(format!("planroom-fixture-{hash}.lock")))
        .expect("the fixture lock file opens");
    lock.lock_shared().expect("the fixture lock is taken");
    if !complete(&dir) {
        // One process builds while the others wait, so a cold run builds once.
        lock.unlock().expect("the fixture lock is released");
        lock.lock().expect("the fixture lock is taken");
        if !complete(&dir) {
            build_into_place(root, &tmp, &dir);
        }
        lock.unlock().expect("the fixture lock is released");
        lock.lock_shared().expect("the fixture lock is taken");
    }
    (dir, lock)
}

/// `target/<profile>/fixture-cache/`, found from the running test binary at
/// `target/<profile>/deps/<binary>`.
fn cache_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary has a path");
    let profile = exe.ancestors().nth(2).expect("a test binary sits in target/<profile>/deps/");
    let dir = profile.join("fixture-cache");
    fs::create_dir_all(&dir).expect("the fixture cache directory is created");
    dir
}

/// A sha256 over `examples/build-fixture` and every file under `examples/fixture/`: each one's path
/// from the repo root and its contents, each length-prefixed, in path order (component by
/// component). `python3`'s hashlib does the hashing: the fixture build needs `python3` anyway, and
/// testkit keeps to the dependencies its area contract lists.
fn sources_hash(root: &Path) -> String {
    let mut files = vec![root.join("examples/build-fixture")];
    walk(&root.join("examples/fixture"), &mut files);
    files.sort();
    let mut input = Vec::new();
    for file in &files {
        let path = file.strip_prefix(root).expect("a source sits under the repo root");
        let contents = fs::read(file).expect("a fixture source reads");
        for part in [path.as_os_str().as_bytes(), &contents] {
            input.extend((part.len() as u64).to_le_bytes());
            input.extend(part);
        }
    }
    let mut python = Command::new("python3")
        .args(["-c", "import hashlib, sys; print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python3 runs");
    python
        .stdin
        .take()
        .expect("python3's stdin is piped")
        .write_all(&input)
        .expect("python3 reads the sources");
    let out = python.wait_with_output().expect("python3 finishes");
    assert!(out.status.success(), "python3 couldn't hash the fixture sources");
    String::from_utf8(out.stdout).expect("a hex digest is UTF-8").trim().to_owned()
}

/// Every file under `dir`, recursively, following no links.
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a fixture directory reads") {
        let path = entry.expect("a directory entry reads").path();
        if fs::symlink_metadata(&path).expect("a fixture path stats").is_dir() {
            walk(&path, files);
        } else {
            files.push(path);
        }
    }
}

/// Whether `dir` is a whole build: it has its heads and every path its file list names. A build is
/// renamed into place only when finished, so this fails only when something pruned it, such as a
/// temporary-file cleaner.
fn complete(dir: &Path) -> bool {
    dir.join("heads.json").is_file()
        && fs::read_to_string(dir.join("files.txt")).is_ok_and(|list| {
            list.lines().all(|path| fs::symlink_metadata(dir.join("tree").join(path)).is_ok())
        })
}

/// Run `examples/build-fixture` into a unique sibling of `dir`, then rename it to `dir`. When
/// another process got there first, keep its build and remove this one. The caller holds the
/// exclusive lock, so any other sibling is a build whose process died: it's removed first.
fn build_into_place(root: &Path, tmp: &Path, dir: &Path) {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = dir.file_name().expect("the fixture directory has a name").to_string_lossy();
    let prefix = format!("{name}.");
    for entry in fs::read_dir(tmp).expect("the fixture cache directory reads").flatten() {
        let stale =
            entry.file_name().to_str().is_some_and(|n| n.starts_with(&prefix) && !n.ends_with(".lock"));
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    let staging = tmp.join(format!("{name}.{}-{nanos}", std::process::id()));
    fs::create_dir(&staging).expect("the staging directory is created");
    let built = Command::new("python3")
        .arg(root.join("examples/build-fixture"))
        .arg(staging.join("tree"))
        .output()
        .expect("python3 runs examples/build-fixture");
    if !built.status.success() {
        let _ = fs::remove_dir_all(&staging);
        panic!("build-fixture failed: {}", String::from_utf8_lossy(&built.stderr));
    }
    fs::write(staging.join("heads.json"), &built.stdout).expect("heads.json is written");
    let tree = staging.join("tree");
    let mut paths = Vec::new();
    walk(&tree, &mut paths);
    let list: String = paths
        .iter()
        .map(|p| format!("{}\n", p.strip_prefix(&tree).expect("a path sits in the tree").display()))
        .collect();
    fs::write(staging.join("files.txt"), list).expect("files.txt is written");
    if dir.exists() && !complete(dir) {
        // A pruned build: the lock keeps every reader out while it goes.
        fs::remove_dir_all(dir).expect("a pruned fixture build is removed");
    }
    if fs::rename(&staging, dir).is_err() {
        assert!(complete(dir), "the built fixture can't be moved to {}", dir.display());
        let _ = fs::remove_dir_all(&staging);
    }
}

/// Copy the contents of `from` into `to` with `cp -pR`, which keeps file modes, links and the
/// `.git` directories, and behaves the same on macOS and Linux; one process per copy, with no
/// directory walk of our own to keep right.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the fixture's directory is created");
    let copied = Command::new("cp").arg("-pR").arg(from.join(".")).arg(to).output().expect("cp runs");
    assert!(
        copied.status.success(),
        "copying the fixture failed: {}",
        String::from_utf8_lossy(&copied.stderr)
    );
}

/// `OUT/.planroom/config.toml` naming one pack, with the modes the engine uses (0700, 0600). It
/// loads only once the engine trusts `OUT/.planroom/`, as after the first scan created it.
pub fn write_config(out: &Path, pack: &Path) {
    let dir = out.join(".planroom");
    fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).expect(".planroom is created");
    let config = dir.join("config.toml");
    let text = format!("packs = [{}]\n", toml_string(&pack.display().to_string()));
    fs::write(&config, text).expect("the config is written");
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).expect("the config's mode is set");
}

fn toml_string(s: &str) -> String {
    serde_json::to_string(s).expect("a string serializes")
}

/// A git repo at `dir` holding `files` (path, contents) in one commit, with a fixed author and
/// date: a synthetic repo for tests the fixture can't cover, such as a large or a hostile one.
pub fn git_repo(dir: &Path, files: &[(String, Vec<u8>)]) {
    for (path, contents) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a file has a parent")).expect("the directory is created");
        fs::write(&path, contents).expect("the file is written");
    }
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
}

/// `examples/TRUTH.json`, frozen.
pub fn truth() -> Value {
    let text = fs::read_to_string(repo_root().join("examples/TRUTH.json")).expect("TRUTH.json reads");
    serde_json::from_str(&text).expect("TRUTH.json parses")
}

/// Whether `actual`, an evidence item a scan reported, satisfies `expected`, one from the truth:
/// the same path and line, and the truth's `contains` a substring of the reported line [TRUTH about].
pub fn evidence_matches(expected: &Value, actual: &Value) -> bool {
    actual["path"] == expected["path"]
        && actual["line"] == expected["line"]
        && match (expected["contains"].as_str(), actual["contains"].as_str()) {
            (Some(want), Some(got)) => got.contains(want),
            (None, _) => true,
            (Some(_), None) => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truth_evidence_matches_by_path_line_and_substring() {
        let want = json!({"path": "platform/docker-compose.yml", "line": 6, "contains": "image: acme/storefront:2.3.0"});
        let got = json!({"type": "line", "path": "platform/docker-compose.yml", "line": 6, "contains": "    image: acme/storefront:2.3.0"});
        assert!(evidence_matches(&want, &got));
        assert!(!evidence_matches(
            &want,
            &json!({"path": "platform/docker-compose.yml", "line": 7, "contains": "    image: acme/storefront:2.3.0"})
        ));
    }

    #[test]
    fn the_truth_loads() {
        assert_eq!(truth()["format"], "planroom.truth/v1");
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).expect("git prints UTF-8")
    }

    /// Each repo's branches and their commits, as `build-fixture` prints them.
    fn branch_heads(root: &Path) -> Value {
        let mut repos = serde_json::Map::new();
        for repo in REPOS {
            let refs = git(
                &root.join(repo),
                &["for-each-ref", "--format=%(refname:short) %(objectname)", "refs/heads"],
            );
            let branches: serde_json::Map<String, Value> = refs
                .lines()
                .map(|line| {
                    let (branch, sha) = line.split_once(' ').expect("a ref line has a name and a commit");
                    (branch.to_owned(), json!(sha))
                })
                .collect();
            repos.insert(repo.to_owned(), Value::Object(branches));
        }
        Value::Object(repos)
    }

    #[test]
    fn two_fixtures_have_equal_heads_and_independent_trees() {
        let tmp = tempfile::tempdir().unwrap();
        let a = fixture(&tmp.path().join("a"));
        let b_dir = tmp.path().join("b");
        fs::create_dir(&b_dir).unwrap();
        let b = fixture(&b_dir);
        assert_eq!(a.heads, b.heads);
        assert_ne!(a.root, b.root);

        let kit = a.root.join("kit");
        fs::write(kit.join("ONLY_IN_A.txt"), "a\n").unwrap();
        git(&kit, &["add", "ONLY_IN_A.txt"]);
        git(&kit, &["commit", "-q", "-m", "only in a"]);
        git(&a.root.join("orders"), &["switch", "-q", "-c", "only-in-a"]);

        assert!(!b.root.join("kit/ONLY_IN_A.txt").exists());
        assert_eq!(branch_heads(&b.root), b.heads, "b's branches are as built");
        assert_ne!(branch_heads(&a.root), a.heads, "a's commit and branch are a's own");
        assert_eq!(git(&b.root.join("orders"), &["branch", "--show-current"]).trim(), "main");
    }

    #[test]
    fn a_copys_repos_are_clean_and_at_their_built_heads() {
        let tmp = tempfile::tempdir().unwrap();
        let fix = fixture(&tmp.path().join("shop"));
        assert_eq!(branch_heads(&fix.root), fix.heads);
        for repo in REPOS {
            let dir = fix.root.join(repo);
            assert_eq!(git(&dir, &["status", "--porcelain"]), "", "{repo} is clean");
            let branch = git(&dir, &["branch", "--show-current"]);
            assert_eq!(
                git(&dir, &["rev-parse", "HEAD"]).trim(),
                fix.heads[repo][branch.trim()],
                "{repo}'s HEAD"
            );
            git(&dir, &["fsck", "--no-dangling", "--no-progress"]);
        }
        assert!(!fix.root.join(".planroom").exists(), "the engine's first scan makes the state directory");
    }

    #[test]
    fn a_cached_build_matches_a_fresh_build_fixture_run() {
        let tmp = tempfile::tempdir().unwrap();
        let fresh = Command::new("python3")
            .arg(repo_root().join("examples/build-fixture"))
            .arg(tmp.path().join("fresh"))
            .output()
            .expect("python3 runs examples/build-fixture");
        assert!(fresh.status.success(), "{}", String::from_utf8_lossy(&fresh.stderr));
        let fresh: Value = serde_json::from_slice(&fresh.stdout).unwrap();
        assert_eq!(fixture(&tmp.path().join("copy")).heads, fresh);
    }
}
