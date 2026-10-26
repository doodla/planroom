//! Dev-only test helpers: the fixture wrapper, the truth comparator and golden helpers.

use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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
    /// The workspace root: the five repos and `.planroom/config.toml`.
    pub root: PathBuf,
    /// `build-fixture`'s output: each repo's branch heads.
    pub heads: Value,
}

/// Build the fixture into `out` with `examples/build-fixture`, then write the workspace config
/// that loads the Ruby pack, as `cargo xtask fixture` does (DR-13).
pub fn fixture(out: &Path) -> Fixture {
    let root = repo_root();
    let built = Command::new("python3")
        .arg(root.join("examples/build-fixture"))
        .arg(out)
        .output()
        .expect("python3 runs examples/build-fixture");
    assert!(built.status.success(), "build-fixture failed: {}", String::from_utf8_lossy(&built.stderr));
    let heads = serde_json::from_slice(&built.stdout).expect("build-fixture prints its heads as JSON");
    write_config(out, &root.join("examples/packs/ruby"));
    // The engine reports physical paths, and on macOS a temporary directory sits behind a link
    // (/var is /private/var), so the root a test compares with is the physical one (DR-83).
    let root = out.canonicalize().unwrap_or_else(|_| out.to_path_buf());
    Fixture { root, heads }
}

/// `OUT/.planroom/config.toml` naming one pack, with the modes the engine uses (0700, 0600).
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
}
