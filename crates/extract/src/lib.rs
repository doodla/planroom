//! Extraction: the git CLI, the file walk, formats, tree-sitter, the declarative runner and linkers.
//!
//! Every extractor obeys the extractor contract (docs/contributing/architecture.md#the-extractor-contract):
//! deterministic output sorted by ID, tolerant of files it can't read, bounded the same way on
//! every machine, and evidence on every fact, verified at ingest by [`verify`].

pub mod compose;
pub mod declarative;
pub mod files;
pub mod git;
pub mod history;
mod ids;
mod lines;
pub mod link;
pub mod manifest;
pub mod openapi;
pub mod registry;
pub mod verify;
pub mod walk;
mod yaml;

use std::fmt;
use std::path::PathBuf;

use planroom_contract::{FactBatch, LevelId};
use serde::Deserialize;
use serde_json::Value;

pub use ids::{edge_id, hex, node_id};

/// One repo at one commit, as an extractor sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoView {
    pub name: String,
    /// Where the repo's git runs. An extractor never reads a file from here: the working tree
    /// isn't the commit. It reads contents with [`RepoView::read`].
    pub root: PathBuf,
    /// The commit the facts are read at. No branch: a snapshot's facts are a function of its key,
    /// and a branch can move or be deleted while the commit stays, so it isn't an input (DR-85).
    pub commit: String,
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
    /// git couldn't read what the extractor needs, such as the history: the scan leaves the repo
    /// out, as it does one git can't walk.
    Git(git::GitError),
}

impl fmt::Display for ExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractError::InvalidOptions(e) | ExtractError::Io(e) => f.write_str(e),
            ExtractError::Git(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ExtractError {}

/// A built-in extractor: files of one repo at one commit in, a `FactBatch` out.
pub trait Extractor {
    /// Pack-qualified, such as `code/compose`.
    fn name(&self) -> &'static str;
    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError>;

    /// What this extractor leaves out of `repo` past a bound of its own, beyond the walk's cut,
    /// as `partial_scan` lists it, such as the `git` built-in's `history before <sha>` (DR-47).
    /// Empty for an extractor that reads only the walk's files.
    fn uncovered(&self, _repo: &RepoView) -> Result<Vec<String>, ExtractError> {
        Ok(Vec::new())
    }

    /// Whether it reads the repo's history rather than the files of one commit: a pack's
    /// conformance inputs are files, not a repo, so `doctor --pack` runs only extractors that
    /// don't.
    fn reads_history(&self) -> bool {
        false
    }
}

/// A kind a built-in emits, as its options name it: the pack's `options.kinds` names the kind, and
/// the engine fills in the level and ID prefix the loaded vocabulary gives it. So no built-in
/// spells a kind, a level or a prefix of its own: the pack that declares it supplies all three.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindRef {
    /// Qualified, `<pack>.<name>`.
    pub kind: String,
    /// The ID's kind-short: `<namespace>:<id_prefix>:<key>`.
    pub id_prefix: String,
    pub level: LevelId,
}

/// Whether `glob` is a `files` or image glob this build can match, with the reason it isn't.
pub fn check_glob(glob: &str) -> Result<(), String> {
    globset::GlobBuilder::new(glob).literal_separator(true).build().map(drop).map_err(|e| e.to_string())
}

/// A declarative extractor's `regex`, compiled as the declarative runner compiles it: the one
/// place both the pack loader's check and the runner build it, so a pattern that loads runs.
pub fn compile_regex(pattern: &str) -> Result<regex::Regex, String> {
    regex::Regex::new(pattern).map_err(|e| {
        // A syntax error spans several lines, the pattern and a caret; its last line says why.
        let text = e.to_string();
        let why = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(&text);
        why.trim().trim_start_matches("error: ").to_owned()
    })
}

/// Whether `pattern` is a `regex` this build can run, with the reason it isn't, in one line.
pub fn check_regex(pattern: &str) -> Result<(), String> {
    compile_regex(pattern).map(drop)
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
        "manifest" => Some(&manifest::Manifest),
        "openapi" => Some(&openapi::OpenApi),
        "git" => Some(&history::History),
        _ => None,
    }
}
