//! Extraction: the git CLI, the file walk, formats, tree-sitter, the declarative runner and linkers.
//!
//! Every extractor obeys the extractor contract (docs/contributing/architecture.md#the-extractor-contract):
//! deterministic output sorted by ID, tolerant of files it can't read, bounded the same way on
//! every machine, and evidence on every fact, verified at ingest by [`verify`].

pub mod compose;
pub mod files;
pub mod git;
mod ids;
mod lines;
pub mod verify;
pub mod walk;
mod yaml;

use std::fmt;
use std::path::PathBuf;

use planroom_contract::FactBatch;
use serde_json::Value;

pub use ids::{hex, node_id};

/// One repo at one commit, as an extractor sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoView {
    pub name: String,
    /// Where the repo's git runs. An extractor never reads a file from here: the working tree
    /// isn't the commit. It reads contents with [`RepoView::read`].
    pub root: PathBuf,
    pub commit: String,
    /// Empty when `HEAD` is detached.
    pub branch: String,
    /// The contents of the commit's files the walk took ([`walk::Walked::blobs`]).
    pub blobs: walk::Blobs,
}

impl RepoView {
    /// The contents of `file` at this view's commit, or `None` when the walk didn't take it.
    pub fn read(&self, file: &RepoPath) -> Option<&[u8]> {
        self.blobs.get(&file.0)
    }
}

/// A path relative to the repo root, with `/` separators.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(pub String);

/// Why an extractor couldn't run at all. A file it can't read is a `partial` entry instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractError {
    /// The pack's options for this extractor don't fit its schema.
    InvalidOptions(String),
    /// The repo itself couldn't be walked.
    Io(String),
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::InvalidOptions(e) | ExtractError::Io(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for ExtractError {}

/// A built-in extractor: files of one repo at one commit in, a `FactBatch` out.
pub trait Extractor {
    /// Pack-qualified, such as `code/compose`.
    fn name(&self) -> &'static str;
    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError>;
}

/// The files an extractor declared with `files` globs takes, in their sorted order. A glob
/// matches at any depth when it starts with `**/`.
pub fn select(files: &[RepoPath], globs: &[&str]) -> Result<Vec<RepoPath>, ExtractError> {
    let mut set = globset::GlobSetBuilder::new();
    for glob in globs {
        let glob = globset::GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .map_err(|e| ExtractError::InvalidOptions(format!("files glob `{glob}`: {e}")))?;
        set.add(glob);
    }
    let set = set.build().map_err(|e| ExtractError::InvalidOptions(e.to_string()))?;
    Ok(files.iter().filter(|f| set.is_match(&f.0)).cloned().collect())
}

/// The built-in extractor a pack names with `builtin = NAME`.
pub fn builtin(name: &str) -> Option<&'static dyn Extractor> {
    match name {
        "files" => Some(&files::Files),
        "compose" => Some(&compose::Compose),
        _ => None,
    }
}
