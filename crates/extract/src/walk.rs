//! The file walk: `.gitignore`-aware, no symlinks followed, confined to the repo, and bounded the
//! same way on every machine (DR-47).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ignore::WalkBuilder;

use crate::{ExtractError, RepoPath};

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
}

/// Walk `root`: every file git doesn't ignore, hidden files included, except `.git` and any
/// nested repo, which is a repo of its own.
pub fn walk(root: &Path) -> Result<Walked, ExtractError> {
    let mut found: Vec<(String, u64)> = Vec::new();
    let mut skipped = 0;
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .ignore(false)
        .parents(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            entry.file_name() != ".git"
                && !(is_dir && entry.depth() > 0 && entry.path().join(".git").exists())
        })
        .build();
    for entry in walker {
        let entry = entry.map_err(|e| ExtractError::Io(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue; // directories, and symlinks, which are never followed
        }
        let rel = entry.path().strip_prefix(root).map_err(|e| ExtractError::Io(e.to_string()))?;
        let Some(rel) = rel.to_str().filter(|r| !r.contains('\n') && !r.contains('\\')) else {
            skipped += 1;
            continue;
        };
        let bytes = entry.metadata().map_err(|e| ExtractError::Io(e.to_string()))?.len();
        if bytes > MAX_FILE_BYTES || is_binary(entry.path()) {
            skipped += 1;
            continue;
        }
        found.push((rel.to_owned(), bytes));
    }
    found.sort();
    let mut walked = Walked { skipped, ..Walked::default() };
    let mut total = 0u64;
    for (path, bytes) in found {
        if walked.files.len() >= MAX_FILES || total + bytes > MAX_TOTAL_BYTES || !walked.uncovered.is_empty()
        {
            walked.uncovered.push(path);
            continue;
        }
        total += bytes;
        walked.files.push(RepoPath(path));
    }
    Ok(walked)
}

fn is_binary(path: &Path) -> bool {
    let mut head = [0u8; BINARY_SNIFF];
    let Ok(mut file) = File::open(path) else { return false };
    let mut n = 0;
    while n < head.len() {
        match file.read(&mut head[n..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => n += read,
        }
    }
    head[..n].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_walk_honors_gitignore_skips_binaries_and_sorts_by_full_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "x").unwrap();
        fs::write(root.join(".gitignore"), "ignored/\n*.log\n").unwrap();
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::create_dir_all(root.join("ignored")).unwrap();
        fs::write(root.join("a/b/c.txt"), "c").unwrap();
        fs::write(root.join("a.txt"), "a").unwrap();
        fs::write(root.join("x.log"), "log").unwrap();
        fs::write(root.join("ignored/y"), "y").unwrap();
        fs::write(root.join("bin.dat"), [1u8, 0, 2]).unwrap();
        fs::create_dir_all(root.join("nested/.git")).unwrap();
        fs::write(root.join("nested/n.txt"), "n").unwrap();
        std::os::unix::fs::symlink(root.join("a.txt"), root.join("link.txt")).unwrap();
        let walked = walk(root).unwrap();
        let paths: Vec<&str> = walked.files.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(paths, [".gitignore", "a.txt", "a/b/c.txt"]);
        assert_eq!(walked.skipped, 1);
        assert!(walked.uncovered.is_empty());
    }
}
