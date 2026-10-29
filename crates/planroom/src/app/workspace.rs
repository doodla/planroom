//! Where the workspace lives (architecture.md#where-the-workspace-lives): its root, its repos and
//! its state directory.
//!
//! The state directory has two forms (DR-74). A directory of repos keeps `.planroom/` at its root.
//! A workspace whose root is itself a git repo, alone or with its submodules, keeps `planroom/`
//! inside that repo's git directory, as `git rev-parse --absolute-git-dir` names it, so
//! `git clean -x`, `git stash -u` and a copy of the working tree that leaves out `.git` never
//! reach it. A `.planroom/` at a repo's root is never that repo's state directory, so one a
//! clone brings changes nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use planroom_contract::{Warning, WarningCode};
use planroom_extract::git::{self, Git, GitError, OwnedPlace, Refusal, is_repo};
use planroom_store::{META_REPOS, Store};
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
    /// The ID namespace: its directory's name, escaped, and suffixed when another repo of the
    /// workspace holds that name (DR-74); see [`assign`].
    pub name: String,
    pub path: PathBuf,
    /// Its path from the workspace root, escaped as a namespace is, and empty for the root
    /// itself: the key under which `meta.repos` fixes its namespace.
    pub rel: String,
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
/// `planroom/` in the git directory when `root` is a repo, else `.planroom/` at `root`. A root
/// whose `.git` is a file naming a git directory git can't name, as when the owner check refuses
/// the repo, has none: the path then sits under that file, where no store is found and none can be
/// made, so the scan skips the root like any other repo it can't read.
pub fn state_dir(root: &Path) -> PathBuf {
    if is_repo(root) {
        return git_dir(root).unwrap_or_else(|| root.join(".git")).join(GIT_DIR_NAME);
    }
    root.join(DIR)
}

pub fn store_path(root: &Path) -> PathBuf {
    state_dir(root).join(STORE)
}

/// The workspace root whose state directory is `dir`, [`state_dir`] read backwards: `.planroom`
/// gives its parent; `<x>/.git/planroom` gives `<x>`; `planroom/` in a linked worktree's git
/// directory, whose `gitdir` file names the worktree's `.git`, gives that worktree. Anything else,
/// such as `planroom/` in a submodule's git directory, gives `None`.
pub fn root_of_state_dir(dir: &Path) -> Option<PathBuf> {
    let parent = dir.parent()?;
    match dir.file_name()?.as_bytes() {
        name if name == DIR.as_bytes() => Some(parent.to_path_buf()),
        name if name == GIT_DIR_NAME.as_bytes() => {
            if parent.file_name()? == ".git" {
                return parent.parent().map(Path::to_path_buf);
            }
            let named = fs::read(parent.join("gitdir")).ok()?;
            let named = Path::new(std::ffi::OsStr::from_bytes(named.trim_ascii_end()));
            parent.join(named).parent().map(Path::to_path_buf)
        }
        _ => None,
    }
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
/// Each repo's namespace is the one the store's `meta.repos` fixed for its root at its first
/// scan, or else the one [`assign`] gives it (DR-74).
pub fn discover(dir: &Path) -> Result<Workspace, AppError> {
    let dir = fs::canonicalize(dir).map_err(|_| AppError::workspace_not_found(dir))?;
    let root = find_root(&dir).unwrap_or(dir);
    let mut found: Vec<PathBuf> = Vec::new();
    let mut warnings = Vec::new();
    if is_repo(&root) {
        found.push(root.clone());
        // The listing runs the owner check on the root first, before it reads anything there,
        // and runs no git in a submodule: each is a repo of its own, whose own check runs before
        // any git call in it. A root another user owns, or one that borrows another user's object
        // store, lists none and is skipped as other_owner like any other repo. A root repo git
        // can't read otherwise leaves the repo set itself unknown, so that one fails whole.
        let subs = match Git::new(&root).submodules() {
            Ok(subs) => subs,
            Err(e) if e.is_other_owner() => Vec::new(),
            Err(e) => {
                let name = root.file_name().map_or_else(|| "root".to_owned(), |n| escape(n.as_bytes()));
                return Err(AppError::git(&name, &root, &e));
            }
        };
        for sub in subs {
            let path = root.join(&sub);
            if is_repo(&path) {
                found.push(path);
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
                    found.push(path);
                }
            }
            if !found.is_empty() {
                break;
            }
        }
    }
    let stored = stored_namespaces(&root)?;
    let mut repos = assign(&root, found, &stored);
    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { root, repos, warnings })
}

/// The namespaces the workspace's store fixed, by [`Repo::rel`]; none before its first scan.
fn stored_namespaces(root: &Path) -> Result<BTreeMap<String, String>, AppError> {
    let Some(store) = Store::open_existing(&store_path(root))? else { return Ok(BTreeMap::new()) };
    match store.meta(META_REPOS)? {
        Some(text) => serde_json::from_str(&text)
            .map_err(|e| AppError::internal(format!("meta.{META_REPOS} doesn't parse: {e}"))),
        None => Ok(BTreeMap::new()),
    }
}

/// Namespaces Planroom's own IDs use, which no repo takes: `ws` for workspace-level things and
/// `plan` for plan items (architecture.md, IDs and aliases).
pub const RESERVED: [&str; 2] = ["ws", "plan"];

/// Each repo's namespace (DR-74). A root that `stored` names keeps the namespace fixed there.
/// Every other repo, in path order, takes its directory's name, [`escape`]d. When that name is
/// taken, by a stored namespace, an earlier repo or a [`RESERVED`] one, it adds `-` and 4 hex
/// characters of the sha256 of its path from the workspace root, and 4 more of that hash while
/// the name is still taken, so no two repos ever share a namespace.
pub fn assign(root: &Path, mut found: Vec<PathBuf>, stored: &BTreeMap<String, String>) -> Vec<Repo> {
    found.sort_by(|a, b| rel_bytes(root, a).cmp(rel_bytes(root, b)));
    found.dedup();
    let mut taken: BTreeSet<&str> = RESERVED.into_iter().collect();
    taken.extend(stored.values().map(String::as_str));
    let mut given: BTreeSet<String> = BTreeSet::new();
    let mut repos = Vec::with_capacity(found.len());
    for path in found {
        let rel = escape_bytes(rel_bytes(root, &path));
        let name = match stored.get(&rel) {
            Some(ns) => ns.clone(),
            None => {
                let base = path.file_name().map_or_else(|| "root".to_owned(), |n| escape(n.as_bytes()));
                let free = |name: &str| !taken.contains(name) && !given.contains(name);
                let mut name = base.clone();
                if !free(&name) {
                    let hash = planroom_kernel::keys::sha256_plain_hex(rel_bytes(root, &path));
                    let mut len = 4;
                    name = format!("{base}-{}", &hash[..len]);
                    while !free(&name) && len < hash.len() {
                        len += 4;
                        name = format!("{base}-{}", &hash[..len]);
                    }
                }
                given.insert(name.clone());
                name
            }
        };
        repos.push(Repo { name, path, rel });
    }
    repos
}

/// `path`'s bytes relative to `root`; empty for `root` itself.
fn rel_bytes<'a>(root: &Path, path: &'a Path) -> &'a [u8] {
    path.strip_prefix(root).unwrap_or(path).as_os_str().as_bytes()
}

/// A directory name as a namespace: every whitespace character, `:`, `%` and control character,
/// and every byte that isn't part of a UTF-8 character, percent-encoded as `%` and the uppercase
/// hex of each byte (`my app` is `my%20app`, `svc:v2` is `svc%3Av2`). So a namespace is never
/// empty, holds no `:` and no whitespace, as `NodeId` requires, and [`unescape`] gives the name
/// back.
pub fn escape(name: &[u8]) -> String {
    let out = escape_bytes(name);
    // A directory always has a name; this keeps the rule total.
    if out.is_empty() { "%00".to_owned() } else { out }
}

fn escape_bytes(name: &[u8]) -> String {
    fn encode(out: &mut String, bytes: &[u8]) {
        for b in bytes {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    let mut out = String::with_capacity(name.len());
    for chunk in name.utf8_chunks() {
        for c in chunk.valid().chars() {
            // U+FEFF isn't whitespace to Unicode, but is to the JSON Schema pattern's `\s`.
            if c.is_whitespace() || c.is_control() || c == ':' || c == '%' || c == '\u{FEFF}' {
                let mut buf = [0u8; 4];
                encode(&mut out, c.encode_utf8(&mut buf).as_bytes());
            } else {
                out.push(c);
            }
        }
        encode(&mut out, chunk.invalid());
    }
    out
}

/// The name [`escape`] encoded as `ns`; `None` for text it never writes.
pub fn unescape(ns: &str) -> Option<Vec<u8>> {
    let bytes = ns.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            if !hex.iter().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b)) {
                return None;
            }
            out.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
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
    /// The names of the repos another user owns, or whose git directory or common directory
    /// another user owns, which a scan skips before git runs there, sorted.
    pub others: Vec<String>,
    /// The names of the repos whose own places are the person's but which read objects from a
    /// directory another user owns, which a scan skips before git runs there too, sorted.
    pub borrowers: Vec<String>,
}

/// Why a scan left a repo out, as its `repo_skipped` warning says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    /// `other_owner` or `git_failed`.
    pub reason: &'static str,
    /// The warning's line, which ends with the fix when there is one.
    pub message: String,
    /// For `git_failed`, one line with an exact command that needs no root, each path quoted for
    /// the shell (DR-86). For `other_owner`, `None`: Planroom never runs git in another user's
    /// repo, and names no command to run there either.
    pub fix: Option<String>,
}

/// Why git can't read the repo `name` at `path`, and its fix (DR-86). When another user owns the
/// repo or a place git uses for it, the line says who, that Planroom never runs git in another
/// user's repo, and who can scan it: its owner, or the person, a copy they own; for an object
/// directory a repo reads, only a copy that borrows none of its objects, since the repo itself
/// may be the person's. There's no fix to run. A place git needs as a directory or a regular
/// file that is neither, such as a FIFO, is shown with `ls -l`, which never opens it, where git
/// would block on it.
pub fn skip(name: &str, path: &Path, e: &GitError) -> Skip {
    let word = |p: &Path| text::shell_word(&p.display().to_string());
    let line = |key: &str, fix: String| {
        let message = text::get(&format!("warning.repo_skipped_{key}"), &[("repo", &name), ("fix", &fix)]);
        (message, Some(fix))
    };
    let (reason, (message, fix)) = match &e.refusal {
        _ if e.is_other_owner() => {
            // git refused it itself (`safe.directory`), as in a repo it looked into on its own: the
            // engine's own check names the place, as it would have.
            let refusal = e.refusal.clone().or_else(|| git::owner_refusal(path));
            let (place, uid, what) = match refusal {
                Some(Refusal::Owned { place, uid, what }) => (place, Some(uid), what),
                _ => (path.to_path_buf(), owner_of(path), OwnedPlace::Tree),
            };
            let who = match uid {
                Some(uid) => text::get("warning.repo_skipped_user", &[("uid", &uid)]),
                None => text::get("warning.repo_skipped_someone", &[]),
            };
            let key = match what {
                OwnedPlace::Tree if place == path => "other_owner",
                OwnedPlace::Tree | OwnedPlace::GitDir => "other_owner_git_dir",
                OwnedPlace::Objects => "other_owner_objects",
            };
            let shown = place.display().to_string();
            let vars: [(&str, &dyn std::fmt::Display); 3] =
                [("who", &who), ("repo", &name), ("place", &shown)];
            ("other_owner", (text::get(&format!("warning.repo_skipped_{key}"), &vars), None))
        }
        Some(Refusal::NotAFile(place)) => (
            "git_failed",
            line("git_failed", text::get("fix.git_failed_not_a_file", &[("path", &word(place))])),
        ),
        _ => ("git_failed", line("git_failed", text::get("fix.git_failed", &[("path", &word(path))]))),
    };
    Skip { reason, message, fix }
}

/// The user who owns `path`, when it can be read.
fn owner_of(path: &Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).ok().map(|m| m.uid())
}

/// Which of `dir`'s workspace repos a scan can map: one `git rev-parse` each.
pub fn scannable(dir: &Path) -> Scannable {
    let Ok(ws) = discover(dir) else { return Scannable::default() };
    let mut out = Scannable::default();
    for repo in ws.repos {
        let git = Git::new(&repo.path);
        match git.head() {
            Ok(_) => out.readable += 1,
            Err(GitError { refusal: Some(Refusal::Owned { what: OwnedPlace::Objects, .. }), .. }) => {
                out.borrowers.push(repo.name)
            }
            Err(e) if e.is_other_owner() => out.others.push(repo.name),
            Err(_) if git.unborn_branch().is_some() => out.unborn.push(repo.name),
            Err(_) => out.unreadable.push(repo.name),
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

/// Create the state directory (mode 0700) if it's missing, and record it as trusted by the engine
/// that made it (DR-48). With no data root to record it in, the workspace goes on untrusted. As
/// `.planroom/` it gets a `.gitignore` of `*` (mode 0600); inside a git directory nothing tracks
/// it, so it needs none.
pub fn ensure_dir(root: &Path) -> Result<PathBuf, AppError> {
    let dir = state_dir(root);
    if !dir.is_dir() {
        DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| AppError::internal(format!("{}: {e}", dir.display())))?;
        if let Err(e) = crate::app::trust::record(root, crate::app::data::TrustedBy::Engine)
            && crate::app::data::root().is_ok()
        {
            return Err(e);
        }
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
