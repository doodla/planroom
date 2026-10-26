//! The file walk: the files of one commit's tree, never the working tree, so a snapshot's facts
//! are a function of its commit alone. One `git ls-tree` lists the tree and one `cat-file --batch`
//! reads the blobs. The tree's committed `.gitignore` files apply, symlinks and submodules are
//! never followed, and the walk is bounded the same way on every machine (DR-47).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::RepoPath;
use crate::git::{Git, GitError, Objects};

/// Files larger than this are skipped.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// At most this many files per repo, in sorted path order.
pub const MAX_FILES: usize = 50_000;
/// At most this many bytes per repo, in sorted path order.
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// A NUL byte in this many leading bytes marks a file as binary, as git decides.
const BINARY_SNIFF: usize = 8000;

/// What one walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walked {
    /// Sorted by path; each a text file within the bounds.
    pub files: Vec<RepoPath>,
    /// Files past the file or byte bound, in sorted order: the `partial_scan` warning's list.
    pub uncovered: Vec<String>,
    /// Files skipped as too large, binary, or with a path that isn't plain UTF-8.
    pub skipped: u32,
    /// The contents of `files`, read from the commit's blobs.
    pub blobs: Blobs,
}

/// File contents by repo-relative path: what an extractor reads instead of the filesystem.
/// Cloning shares them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Blobs(Arc<BTreeMap<String, Vec<u8>>>);

impl Blobs {
    pub fn new(files: BTreeMap<String, Vec<u8>>) -> Blobs {
        Blobs(Arc::new(files))
    }

    /// The contents of `path`, or `None` when the walk didn't read it.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

impl fmt::Debug for Blobs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blobs({} files)", self.0.len())
    }
}

/// One regular file of the tree.
struct Entry {
    path: Vec<u8>,
    object: String,
    size: u64,
}

/// Walk `commit`'s tree in the repo `git` runs in: every regular file a committed `.gitignore`
/// doesn't ignore, hidden files included. A submodule is a repo of its own, and a symlink is
/// never followed.
pub fn walk(git: &Git, commit: &str) -> Result<Walked, GitError> {
    let listing = git.ls_tree(commit)?;
    let entries = parse(&listing).map_err(|stderr| GitError { argv: vec!["ls-tree".into()], stderr })?;
    let mut objects = git.objects()?;
    let ignores = Ignores::read(&entries, &mut objects)?;

    let mut skipped = 0;
    let mut found: Vec<(String, &Entry)> = Vec::new();
    for entry in &entries {
        if ignores.excludes(&entry.path) {
            continue;
        }
        let Some(rel) =
            std::str::from_utf8(&entry.path).ok().filter(|r| !r.contains('\n') && !r.contains('\\'))
        else {
            skipped += 1;
            continue;
        };
        if entry.size > MAX_FILE_BYTES {
            skipped += 1;
            continue;
        }
        found.push((rel.to_owned(), entry));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));

    let mut walked = Walked { skipped, ..Walked::default() };
    let mut blobs = BTreeMap::new();
    let mut total = 0u64;
    for (path, entry) in found {
        // Every file is read, even past the cut, so the cut's list holds no binary file.
        let Some(bytes) = objects.get(&entry.object)? else {
            walked.skipped += 1;
            continue;
        };
        if bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0) {
            walked.skipped += 1;
            continue;
        }
        let size = bytes.len() as u64;
        if walked.files.len() >= MAX_FILES || total + size > MAX_TOTAL_BYTES || !walked.uncovered.is_empty() {
            walked.uncovered.push(path);
            continue;
        }
        total += size;
        walked.files.push(RepoPath(path.clone()));
        blobs.insert(path, bytes);
    }
    walked.blobs = Blobs::new(blobs);
    Ok(walked)
}

/// The tree's regular files, from `ls-tree -r -z -l` output. Symlinks (mode 120000) and
/// submodules (type `commit`) are left out.
fn parse(listing: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for record in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let bad = || format!("unexpected ls-tree record: {}", String::from_utf8_lossy(record));
        let tab = record.iter().position(|b| *b == b'\t').ok_or_else(bad)?;
        let head = std::str::from_utf8(&record[..tab]).map_err(|_| bad())?;
        let [mode, kind, object, size] = head.split_whitespace().collect::<Vec<_>>()[..] else {
            return Err(bad());
        };
        if kind != "blob" || !mode.starts_with("100") {
            continue;
        }
        let path = record[tab + 1..].to_vec();
        if path.split(|b| *b == b'/').any(|part| part == b".git") {
            continue;
        }
        let size = size.parse().map_err(|_| bad())?;
        entries.push(Entry { path, object: object.to_owned(), size });
    }
    Ok(entries)
}

/// The tree's `.gitignore` files, each matching paths under its own directory.
struct Ignores(BTreeMap<Vec<u8>, Gitignore>);

impl Ignores {
    fn read(entries: &[Entry], objects: &mut Objects) -> Result<Ignores, GitError> {
        let mut by_dir = BTreeMap::new();
        for entry in entries {
            let (dir, name) = split(&entry.path);
            if name != b".gitignore" {
                continue;
            }
            let Some(bytes) = objects.get(&entry.object)? else { continue };
            // Paths are matched relative to the file's directory, so its root is `.`.
            let mut builder = GitignoreBuilder::new(".");
            for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
                // As the working-tree walk read it: lines up to the first one that isn't UTF-8.
                let Ok(line) = std::str::from_utf8(line) else { break };
                let line = line.strip_suffix('\r').unwrap_or(line);
                let line = if i == 0 { line.trim_start_matches('\u{feff}') } else { line };
                let _ = builder.add_line(None, line); // a bad line is skipped, as git does
            }
            if let Ok(gitignore) = builder.build() {
                by_dir.insert(dir.to_vec(), gitignore);
            }
        }
        Ok(Ignores(by_dir))
    }

    /// Whether `path`, or a directory above it, is ignored.
    fn excludes(&self, path: &[u8]) -> bool {
        if self.0.is_empty() {
            return false;
        }
        let mut dirs = Vec::new();
        let mut at = path;
        while let Some(slash) = at.iter().rposition(|b| *b == b'/') {
            at = &at[..slash];
            dirs.push(at);
        }
        dirs.iter().rev().any(|dir| self.ignored(dir, true)) || self.ignored(path, false)
    }

    /// The deepest `.gitignore` above `path` that has a say decides, as git does.
    fn ignored(&self, path: &[u8], is_dir: bool) -> bool {
        let mut dir = split(path).0;
        loop {
            if let Some(gitignore) = self.0.get(dir) {
                let rel = if dir.is_empty() { path } else { &path[dir.len() + 1..] };
                match gitignore.matched(Path::new(OsStr::from_bytes(rel)), is_dir) {
                    Match::Ignore(_) => return true,
                    Match::Whitelist(_) => return false,
                    Match::None => {}
                }
            }
            if dir.is_empty() {
                return false;
            }
            dir = split(dir).0;
        }
    }
}

/// `path`'s directory (empty at the top) and its last part.
fn split(path: &[u8]) -> (&[u8], &[u8]) {
    match path.iter().rposition(|b| *b == b'/') {
        Some(slash) => (&path[..slash], &path[slash + 1..]),
        None => (&[], path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
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
    }

    #[test]
    fn the_walk_reads_the_commit_honors_gitignore_skips_binaries_and_sorts_by_full_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        fs::write(root.join(".gitignore"), "ignored/\n*.log\n").unwrap();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::create_dir_all(root.join("ignored")).unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/.gitignore"), "*.tmp\n!keep.log\n").unwrap();
        fs::write(root.join("sub/x.tmp"), "x").unwrap();
        fs::write(root.join("sub/keep.log"), "k").unwrap();
        fs::write(root.join("a/b/c.txt"), "c").unwrap();
        fs::write(root.join("a.txt"), "a").unwrap();
        fs::write(root.join("x.log"), "log").unwrap();
        fs::write(root.join("ignored/y"), "y").unwrap();
        fs::write(root.join("bin.dat"), [1u8, 0, 2]).unwrap();
        std::os::unix::fs::symlink(root.join("a.txt"), root.join("link.txt")).unwrap();
        // Tracked despite the ignore rules, as `git add -f` leaves them.
        git(root, &["add", "-A"]);
        git(root, &["add", "-f", "x.log", "ignored/y", "sub/x.tmp"]);
        git(root, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "init"]);
        // The working tree moves on; the walk doesn't see it.
        fs::write(root.join("a.txt"), "changed").unwrap();
        fs::write(root.join("untracked.txt"), "u").unwrap();
        fs::remove_file(root.join("a/b/c.txt")).unwrap();

        let git = Git::new(root);
        let commit = git.head().unwrap().commit;
        let walked = walk(&git, &commit).unwrap();
        let paths: Vec<&str> = walked.files.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(paths, [".gitignore", "a.txt", "a/b/c.txt", "sub/.gitignore", "sub/keep.log"]);
        assert_eq!(walked.skipped, 1, "the binary");
        assert!(walked.uncovered.is_empty());
        assert_eq!(walked.blobs.get("a.txt"), Some(&b"a"[..]), "the commit's contents");
        assert_eq!(walked.blobs.get("a/b/c.txt"), Some(&b"c"[..]));
        assert_eq!(walked.blobs.get("untracked.txt"), None);
    }
}
