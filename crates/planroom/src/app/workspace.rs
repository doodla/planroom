//! Where the workspace lives (architecture.md#where-the-workspace-lives): its root, its repos and
//! its state directory.
//!
//! The state directory has two forms (DR-74). A directory of repos keeps `.planroom/` at its root.
//! A workspace whose root is itself a git repo, alone or with its submodules, keeps `planroom/`
//! inside that repo's git directory, as `git rev-parse --absolute-git-dir` names it, so
//! `git clean -x`, `git stash -u` and a copy of the working tree that leaves out `.git` never
//! reach it. A `.planroom/` at a repo's root is never that repo's state directory, so one a
//! clone brings changes nothing.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use planroom_contract::{Warning, WarningCode};
use planroom_extract::git::{Git, is_repo};
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

/// The state directory beside a directory of repos.
pub const DIR: &str = ".planroom";
/// The state directory's name inside a single repo's git directory.
pub const GIT_DIR_NAME: &str = "planroom";
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
    pub fn state_dir(&self) -> PathBuf {
        state_dir(&self.root)
    }

    pub fn store_path(&self) -> PathBuf {
        store_path(&self.root)
    }
}

/// The state directory of the workspace rooted at `root`, whether or not it exists yet:
/// `planroom/` in the git directory when `root` is a repo, else `.planroom/` at `root`.
pub fn state_dir(root: &Path) -> PathBuf {
    if is_repo(root) {
        return git_dir(root).unwrap_or_else(|| root.join(".git")).join(GIT_DIR_NAME);
    }
    root.join(DIR)
}

pub fn store_path(root: &Path) -> PathBuf {
    state_dir(root).join(STORE)
}

/// The git directory of the repo at `repo`: its `.git` directory, or, for the `.git` file of a
/// submodule or a linked worktree, the directory git names for it. `None` when git can't say.
fn git_dir(repo: &Path) -> Option<PathBuf> {
    let dot_git = repo.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    Git::new(repo).absolute_git_dir().ok()
}

/// Whether `dir` is a workspace root: a repo whose git directory holds `planroom/`, or a
/// directory that holds `.planroom/` and isn't a repo.
pub fn is_root(dir: &Path) -> bool {
    if is_repo(dir) {
        return git_dir(dir).is_some_and(|g| g.join(GIT_DIR_NAME).is_dir());
    }
    dir.join(DIR).is_dir()
}

/// The nearest ancestor of `start`, itself included, that is a workspace root.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|dir| is_root(dir)).map(Path::to_path_buf)
}

/// The workspace `dir` belongs to: the nearest workspace root above it, else `dir` itself.
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
                if is_root(&path) {
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
    /// Repos a scan maps: git can read them and they have a commit.
    pub readable: usize,
    /// The names of the repos with no commit yet, which map nothing until their first, sorted.
    pub unborn: Vec<String>,
    /// The names of the repos git refuses, which a scan skips, sorted.
    pub unreadable: Vec<String>,
}

/// Which of `dir`'s workspace repos a scan can map: one `git rev-parse` each.
pub fn scannable(dir: &Path) -> Scannable {
    let Ok(ws) = discover(dir) else { return Scannable::default() };
    let mut out = Scannable::default();
    for repo in ws.repos {
        let git = Git::new(&repo.path);
        if git.head().is_ok() {
            out.readable += 1;
        } else if git.unborn_branch().is_some() {
            out.unborn.push(repo.name);
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

/// Create the state directory (mode 0700) if it's missing. As `.planroom/` it gets a `.gitignore`
/// of `*` (mode 0600); inside a git directory nothing tracks it, so it needs none.
pub fn ensure_dir(root: &Path) -> Result<PathBuf, AppError> {
    let dir = state_dir(root);
    if !dir.is_dir() {
        DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| AppError::internal(format!("{}: {e}", dir.display())))?;
    }
    if dir != root.join(DIR) {
        return Ok(dir);
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
