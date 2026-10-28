//! The `openapi` built-in: an OpenAPI 3 or Swagger 2 document, YAML or JSON, as the API spec node
//! it is and a `code.route_serve` hint for each endpoint it describes.
//!
//! - **The spec.** A node of the pack's spec kind keyed by the file's path, under the directory
//!   that holds it, citing the line that names its version (`openapi:` or `swagger:`). A YAML or
//!   JSON file that names neither is no spec and gives nothing.
//! - **Endpoints.** Each operation under `paths`, one per HTTP method of a path item, is a
//!   `code.route_serve` hint scoped to the spec, keyed by its path, with `method` in capitals and
//!   `path`, citing the path's line and then the method's. Its ID holds its method, so every
//!   operation is its own hint, however many share a line, as a one-line JSON spec's do. Swagger
//!   2's `basePath` comes before each path. A path item that only refers elsewhere with `$ref` serves nothing here: the
//!   built-in follows no reference, so a reference loop can't stall it.
//!
//! A file that isn't UTF-8 or doesn't parse is partial and gives nothing; a comment names nothing.
//! A byte order mark before the document is read past. It is read with the YAML reader's caps on
//! nesting and on what aliases copy, and its facts are bounded as the declarative runner's are
//! ([`crate::declarative::charge`]): a spec whose facts would hold more heap than its size allows
//! is partial with `work_limit`, and in sorted path order the first spec that would take the
//! repo's total past its bound, and every one after it, with `repo_limit`; each gives no fact.

use std::collections::BTreeMap;

use planroom_contract::{
    Evidence, FACTS_FORMAT, FactBatch, Hint, Kind, Layer, Node, PartialFile, PartialReason, Provenance, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use crate::declarative::REPO_CAPTURE_MAX;
use crate::declarative::charge::{self, Charges};
use crate::files::repo_id;
use crate::ids::{node_id, operation_id};
use crate::yaml::{self, Yaml};
use crate::{ExtractError, Extractor, KindRef, RepoPath, RepoView, lines};

pub const NAME: &str = "code/openapi";
/// The globs a pack declares this built-in with.
pub const DEFAULT_FILES: &[&str] = &[
    "**/openapi.yaml",
    "**/openapi.yml",
    "**/openapi.json",
    "**/swagger.yaml",
    "**/swagger.yml",
    "**/swagger.json",
];
/// The hint kind each endpoint is: the code pack declares it, and its linkers join it.
pub const ROUTE_SERVE_HINT: &str = "code.route_serve";

/// The methods a path item may describe an operation for.
const METHODS: [&str; 8] = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];

/// The kinds this built-in emits, which the pack that declares it names in `options.kinds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// The spec document.
    pub spec: KindRef,
    /// The repo's node, which a spec at the root sits under.
    pub repo: KindRef,
    /// The directory that holds a spec below the root.
    pub directory: KindRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    kinds: Kinds,
}

pub struct OpenApi;

impl Extractor for OpenApi {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
        self.run_within(repo, files, opts, REPO_CAPTURE_MAX)
    }
}

impl OpenApi {
    /// [`Extractor::run`] with the repo's bound at `max` in place of [`REPO_CAPTURE_MAX`], so a
    /// test can reach the cut without a repo whose facts hold 256 MiB. Builds without debug
    /// assertions leave it out, so no release can lower the bound.
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn run_with_repo_capture_max(
        &self,
        repo: &RepoView,
        files: &[RepoPath],
        opts: &Value,
        max: u64,
    ) -> Result<FactBatch, ExtractError> {
        self.run_within(repo, files, opts, max)
    }

    fn run_within(
        &self,
        repo: &RepoView,
        files: &[RepoPath],
        opts: &Value,
        max: u64,
    ) -> Result<FactBatch, ExtractError> {
        let opts: Options = serde_json::from_value(opts.clone())
            .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?;
        let kinds = &opts.kinds;
        let mut files: Vec<&RepoPath> = files.iter().collect();
        files.sort();
        files.dedup();
        let mut nodes = Vec::new();
        let mut hints: BTreeMap<String, Hint> = BTreeMap::new();
        let mut partial = Vec::new();
        let mut charges = Charges::new(max);
        for file in files {
            let full = format!("{}/{}", repo.name, file.0);
            let bytes = repo.read(file);
            if let Err(reason) = charges.start(bytes.map_or(0, <[u8]>::len)) {
                // Past the repo's cut: not read at all.
                partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) });
                continue;
            }
            let Some(text) = bytes.and_then(|bytes| std::str::from_utf8(bytes).ok()) else {
                partial.push(PartialFile { path: full, errors: 1, reason: None });
                continue;
            };
            let (node, read) = match read(repo, kinds, file, &full, text, &mut charges) {
                Ok(Some(read)) => read,
                // An empty file, or one that names no spec version: no spec.
                Ok(None) => continue,
                Err(reason) => {
                    partial.push(PartialFile { path: full, errors: 1, reason });
                    continue;
                }
            };
            if let Err(reason) = charges.finish() {
                partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) });
                continue;
            }
            nodes.push(node);
            hints.extend(read);
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Config,
            nodes,
            edges: vec![],
            hints: hints.into_values().collect(),
            partial,
        })
    }
}

/// The spec `file`, whose text is `text`, and its endpoints by ID, each charged to `charges` as
/// it is made ([`charge`]). `Ok(None)` is a file that holds no document or names no spec
/// version; `Err(None)` one that doesn't parse, and `Err` with a reason one whose facts would
/// charge more than its budget.
#[allow(clippy::type_complexity)]
fn read(
    repo: &RepoView,
    kinds: &Kinds,
    file: &RepoPath,
    full: &str,
    text: &str,
    charges: &mut Charges,
) -> Result<Option<(Node, BTreeMap<String, Hint>)>, Option<PartialReason>> {
    // A byte order mark is no text the YAML reader reads. It sits on line 1 alone, so lines
    // number the same with it or without; evidence quotes the blob's lines whole.
    let doc = match yaml::parse(lines::strip_bom(text)) {
        Ok(mut docs) if !docs.is_empty() => docs.swap_remove(0),
        // An empty file holds no document, and describes nothing.
        Ok(_) => return Ok(None),
        Err(_) => return Err(None),
    };
    let Some(version) = doc.get("openapi").or_else(|| doc.get("swagger")) else { return Ok(None) };
    let index = lines::Index::new(text);
    let evidence = |line: u32| Evidence::Line {
        path: full.to_owned(),
        line,
        contains: index.line(text, line).map(lines::cut),
        commit: Some(repo.commit.clone()),
        // A snapshot is keyed by its commit, never a branch (DR-85).
        branch: None,
    };
    let provenance = Provenance::Extracted { extractor: NAME.into(), tier: Tier::Config };
    let parent = match file.0.rsplit_once('/') {
        Some((dir, _)) => node_id(&repo.name, &kinds.directory.id_prefix, dir),
        None => repo_id(&repo.name, &kinds.repo),
    };
    let id = node_id(&repo.name, &kinds.spec.id_prefix, &file.0);
    let node = Node {
        id: id.clone(),
        kind: Kind(kinds.spec.kind.clone()),
        level: kinds.spec.level,
        parent: Some(parent),
        name: file.0.rsplit('/').next().unwrap_or(&file.0).to_owned(),
        aliases: vec![],
        props: BTreeMap::new(),
        layer: Layer::Actual,
        provenance: provenance.clone(),
        evidence: vec![evidence(version.line())],
    };
    charges.add(charge::node(&node))?;
    let mut hints: BTreeMap<String, Hint> = BTreeMap::new();
    // Swagger 2 serves every path under its base path; OpenAPI 3 names servers instead.
    let base = doc
        .get("basePath")
        .and_then(Yaml::as_str)
        .map(|b| b.trim_end_matches('/'))
        .filter(|b| b.starts_with('/'))
        .unwrap_or("");
    for (key, item) in doc.get("paths").and_then(Yaml::entries).unwrap_or_default() {
        let Some(path) = key.as_str().filter(|p| p.starts_with('/')) else { continue };
        let path = format!("{base}{path}");
        for (method_key, _) in item.entries().unwrap_or_default() {
            let Some(method) = method_key.as_str().filter(|m| METHODS.contains(m)) else { continue };
            let method = method.to_ascii_uppercase();
            let kind = Kind(ROUTE_SERVE_HINT.into());
            let cited = vec![evidence(key.line()), evidence(method_key.line())];
            // The method is in the ID: a path item's operations on one line are each a hint.
            let hint_id = operation_id(&kind, &id, &path, &method, &cited);
            if hints.contains_key(&hint_id) {
                // A path item that names one method twice: one operation.
                continue;
            }
            let mut attrs = BTreeMap::new();
            attrs.insert("method".to_owned(), Value::from(method));
            attrs.insert("path".to_owned(), Value::from(path.as_str()));
            let hint = Hint {
                id: hint_id.clone(),
                kind,
                scope: id.clone(),
                key: path.clone(),
                value: None,
                attrs,
                evidence: cited,
                provenance: provenance.clone(),
            };
            charges.add(charge::hint(&hint))?;
            hints.insert(hint_id, hint);
        }
    }
    Ok(Some((node, hints)))
}
