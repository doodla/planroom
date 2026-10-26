//! Where the workspace lives (architecture.md#where-the-workspace-lives): its root, its repos and
//! its `.planroom/` directory.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use planroom_contract::{Warning, WarningCode};
use planroom_extract::git::{Git, is_repo};
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

pub const DIR: &str = ".planroom";
pub const STORE: &str = "store.db";

/// One repo of the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The ID namespace: the directory's path from the root, or the root's own name.
    pub name: String,
    pub path: PathBuf,
}

/// A workspace root and its repos, sorted by name.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub repos: Vec<Repo>,
    pub warnings: Vec<Warning>,
}

impl Workspace {
    pub fn store_path(&self) -> PathBuf {
        store_path(&self.root)
    }
}

pub fn store_path(root: &Path) -> PathBuf {
    root.join(DIR).join(STORE)
}

/// The nearest ancestor of `start`, itself included, that holds `.planroom/`.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|dir| dir.join(DIR).is_dir()).map(Path::to_path_buf)
}

/// The workspace `dir` belongs to: the nearest ancestor with `.planroom/`, else `dir` itself.
/// Its repos are the root when it's a git repo, plus its initialized submodules; otherwise its
/// child directories that are git repos, or, when none are, its grandchildren that are (DR-71).
pub fn discover(dir: &Path) -> Result<Workspace, AppError> {
    let dir = fs::canonicalize(dir).map_err(|_| AppError::workspace_not_found(dir))?;
    let root = find_root(&dir).unwrap_or(dir);
    let mut repos = Vec::new();
    let mut warnings = Vec::new();
    if is_repo(&root) {
        let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("root").to_owned();
        repos.push(Repo { name, path: root.clone() });
        // A root repo git can't read leaves the repo set itself unknown, so this one fails whole.
        let subs = Git::new(&root).submodules().map_err(|e| AppError::git(&repos[0].name, &root, &e))?;
        for sub in subs {
            let path = root.join(&sub);
            if is_repo(&path) {
                repos.push(Repo { name: sub, path });
            }
        }
    } else {
        for depth in [1, 2] {
            for path in dirs_at(&root, depth) {
                if path.join(DIR).is_dir() {
                    let other = path.display().to_string();
                    warnings.push(Warning {
                        code: WarningCode::WorkspaceNested,
                        message: text::get("warning.workspace_nested", &[("other_root", &other)]),
                        details: json!({"other_root": other}),
                    });
                    continue;
                }
                if is_repo(&path) {
                    let rel = path.strip_prefix(&root).unwrap_or(&path);
                    repos.push(Repo { name: rel.to_string_lossy().into_owned(), path });
                }
            }
            if !repos.is_empty() {
                break;
            }
        }
    }
    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { root, repos, warnings })
}

/// The repos of the workspace `dir` belongs to, as a scan would find them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scannable {
    /// Repos git can read: with commits, or none yet (those map nothing until their first commit).
    pub readable: usize,
    /// The names of the repos git refuses, which a scan skips, sorted.
    pub unreadable: Vec<String>,
}

/// Which of `dir`'s workspace repos a scan can read: one `git rev-parse` each.
pub fn scannable(dir: &Path) -> Scannable {
    let Ok(ws) = discover(dir) else { return Scannable::default() };
    let mut out = Scannable::default();
    for repo in ws.repos {
        let git = Git::new(&repo.path);
        if git.head().is_ok() || git.unborn_branch().is_some() {
            out.readable += 1;
        } else {
            out.unreadable.push(repo.name);
        }
    }
    out
}

/// Directories exactly `depth` levels below `root`, sorted, skipping hidden ones.
fn dirs_at(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut level = vec![root.to_path_buf()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let hidden = entry.file_name().to_string_lossy().starts_with('.');
                if !hidden && entry.file_type().is_ok_and(|t| t.is_dir()) {
                    next.push(entry.path());
                }
            }
        }
        level = next;
    }
    level.sort();
    level
}

/// Create `.planroom/` (mode 0700) with a `.gitignore` of `*` (mode 0600) if it's missing.
pub fn ensure_dir(root: &Path) -> Result<PathBuf, AppError> {
    let dir = root.join(DIR);
    if !dir.is_dir() {
        DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| AppError::internal(format!("{}: {e}", dir.display())))?;
    }
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&ignore)
            .map_err(|e| AppError::internal(format!("{}: {e}", ignore.display())))?;
        file.write_all(b"*\n").map_err(|e| AppError::internal(e.to_string()))?;
    }
    Ok(dir)
}
