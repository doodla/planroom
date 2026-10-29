//! The `imports` linker: each `code.import` hint joined to the workspace package it names, and the
//! manifest's dependency on that package given the import lines that use it (DR-12).
//!
//! - **Which package a spec names.** One index of package nodes by the name their `pkg:` alias
//!   gives. A spec with no `separator` looks up each `/`-prefix of itself, longest first, so an npm
//!   subpath (`@acme/kit/money`) and a Go package under its module path both find theirs. A spec
//!   with a `separator` (Python's `.`, Rust's `::`) looks up only its first segment, against names
//!   lowercased with `-` read as `_`: `acme_kit.money` finds `acme-kit`, and JavaScript's
//!   `socket.io` never finds `socket`. A spec that starts with `.` or `/`, or whose first segment
//!   is `crate`, `self` or `super`, names a file or a module of its own package: it matches
//!   nothing until module and file joins come.
//! - **Two repos holding a package of that name:** the join is [`Unresolved`], and nothing is
//!   emitted. It never guesses.
//! - **An import inside its own package** emits nothing: the package whose manifest covers the
//!   importing file, the one in the nearest directory above it, is the one the spec names.
//! - **`imports`.** Every other matched import gives an edge from its file to the package node, by
//!   ID, whether or not a manifest declares the dependency. Its `via` is the package's name, never
//!   the spec, and its evidence every line of the file that imports the package: one file's
//!   imports of one package make one edge.
//! - **`depends_on_package`.** When the importing file's repo declares the dependency, the
//!   manifest's edge is emitted again under its own ID, so the merge unions its evidence onto the
//!   manifest's fact (DR-42): the input edge to `pkg:<name>` whose manifest sits in the nearest
//!   directory above the file. Its `from`, `to` and props are the manifest's, and it has no `via`
//!   (DR-89). Its evidence is the manifest's lines, then the package node's, then at most
//!   [`IMPORT_LINES`] import lines across the repo, the first by path and line; its provenance
//!   names those import hints. A linker joins what exists: it never invents a dependency.
//! - **Test code** imports nothing here: no hint from a test file, a test module's file or a test
//!   scope is read, as for `runtime_wiring`.
//!
//! Every lookup reads an index keyed by name or directory: a spec costs at most one lookup per
//! segment, and a file one per directory above it, so the work grows with the hints and the
//! length of their keys and paths, never with a product of two counts. A name many repos hold is
//! read from the index's list in place: an import of it compares its file's own packages with
//! that list by search, and an unresolved one samples its first candidates by ID.

use std::collections::{BTreeMap, HashMap, HashSet};

use planroom_contract::{
    Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Hint, HintRef, Kind, LanguageDecl, Layer, Node, NodeId,
    Provenance, Ref, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    Linked, Linker, Reads, RelationRef, TestFiles, Unresolved, WorkspaceFacts, at, attr, extractor, rank,
};
use crate::ids::edge_id;
use crate::{ExtractError, KindRef};

pub const NAME: &str = "code/imports";

/// The hint kind of an import: its key is the specifier as the source writes it, and its
/// `separator` the language's path separator when the specifier's first segment names a package.
pub const IMPORT_HINT: &str = "code.import";

/// Every hint kind this linker reads.
pub const CONSUMES: &[&str] = &[IMPORT_HINT, super::TEST_SCOPE_HINT];

/// The import lines a dependency cites at most across its repo, the first by path and line.
pub const IMPORT_LINES: usize = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Kinds {
    /// A manifest's package, which an import names.
    package: KindRef,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Relations {
    /// What it emits from a file to a package.
    imports: RelationRef,
    /// The manifests' dependencies, which it reads and gives their import lines.
    depends_on_package: RelationRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    kinds: Kinds,
    relations: Relations,
    #[serde(default)]
    languages: Vec<LanguageDecl>,
}

pub struct Imports;

impl Linker for Imports {
    fn name(&self) -> &'static str {
        NAME
    }

    fn reads(&self) -> Reads {
        Reads { kinds: &["package"], relations: &["depends_on_package"] }
    }

    fn link(&self, ws: &WorkspaceFacts, opts: &Value) -> Result<Linked, ExtractError> {
        let opts: Options = serde_json::from_value(opts.clone())
            .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?;
        let tests = TestFiles::new(&opts.languages)?;
        Ok(link(ws, &opts, &tests))
    }
}

/// A package node as imports read it.
struct Package<'a> {
    node: &'a Node,
    name: &'a str,
}

/// The directory of an evidence path, `<repo>/<dir>/<file>`; the repo's own is `<repo>`.
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// The directories above the file at the evidence path `path`, nearest first, down to its repo's.
fn dirs_above(path: &str) -> impl Iterator<Item = &str> {
    let mut dir = Some(dir_of(path)).filter(|d| !d.is_empty());
    std::iter::from_fn(move || {
        let this = dir?;
        dir = this.rsplit_once('/').map(|(up, _)| up);
        Some(this)
    })
}

/// A package name as a spec with a separator compares it: lowercased, `-` read as `_`.
fn folded(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

/// The first evidence path of a fact.
fn first_path(evidence: &[Evidence]) -> Option<&str> {
    evidence.iter().find_map(|e| match e {
        Evidence::Line { path, .. } => Some(path.as_str()),
        Evidence::Ref { .. } => None,
    })
}

fn link(ws: &WorkspaceFacts, opts: &Options, tests: &TestFiles) -> Linked {
    let package_kind = opts.kinds.package.kind.as_str();
    let imports = &opts.relations.imports;
    let depends = &opts.relations.depends_on_package;

    // One node per package ID: the most specific fact's, and of two equally specific the least
    // by its JSON, whatever order they come in.
    let mut by_id: BTreeMap<&NodeId, &Node> = BTreeMap::new();
    for node in ws.nodes.iter().filter(|n| n.kind.0 == package_kind) {
        let better = |kept: &&Node| {
            let key = |n: &Node| {
                (std::cmp::Reverse(rank(&n.provenance)), serde_json::to_string(n).unwrap_or_default())
            };
            key(node) < key(kept)
        };
        match by_id.get(&node.id) {
            Some(kept) if !better(kept) => {}
            _ => {
                by_id.insert(&node.id, node);
            }
        }
    }
    let packages: Vec<Package<'_>> = by_id
        .into_values()
        .filter_map(|node| {
            let name = node.aliases.iter().filter_map(|a| a.strip_prefix("pkg:")).min()?;
            Some(Package { node, name })
        })
        .collect();
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_folded: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_dir: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, package) in packages.iter().enumerate() {
        by_name.entry(package.name).or_default().push(i);
        by_folded.entry(folded(package.name)).or_default().push(i);
        if let Some(path) = first_path(&package.node.evidence) {
            by_dir.entry(dir_of(path)).or_default().push(i);
        }
    }

    // The manifests' dependencies, once each by ID, by the manifest's directory and the name
    // they ask for.
    let mut edges: BTreeMap<&EdgeId, &Edge> = BTreeMap::new();
    for edge in ws.edges.iter().filter(|e| e.kind.0 == depends.kind) {
        edges.entry(&edge.id).or_insert(edge);
    }
    let mut deps: HashMap<(&str, &str), Vec<&Edge>> = HashMap::new();
    for edge in edges.into_values() {
        let Ref::Alias(to) = &edge.to else { continue };
        let Some(name) = to.strip_prefix("pkg:") else { continue };
        let Some(path) = first_path(&edge.evidence) else { continue };
        deps.entry((dir_of(path), name)).or_default().push(edge);
    }

    let accepts = imports.to.iter().any(|k| k == package_kind);
    let mut hints: Vec<&Hint> =
        super::runtime_code(&ws.hints, tests).into_iter().filter(|h| h.kind.0 == IMPORT_HINT).collect();
    hints.sort_by(|a, b| (at(a), &a.id).cmp(&(at(b), &b.id)));

    // A file's covering packages, worked out once per file.
    let mut covering: HashMap<&str, &[usize]> = HashMap::new();
    let mut imported: BTreeMap<(NodeId, usize), Vec<&Hint>> = BTreeMap::new();
    let mut used: BTreeMap<&EdgeId, (&Edge, usize, Vec<&Hint>)> = BTreeMap::new();
    let mut unresolved: BTreeMap<String, Unresolved> = BTreeMap::new();
    for hint in hints {
        let Some((path, _)) = at(hint) else { continue };
        let matched = matches(hint, &by_name, &by_folded);
        if matched.is_empty() {
            continue;
        }
        let own: &[usize] = covering
            .entry(path)
            .or_insert_with(|| dirs_above(path).find_map(|dir| by_dir.get(dir)).map_or(&[], Vec::as_slice));
        // An import of the file's own package waits for module and file joins.
        if overlap(own, matched) {
            continue;
        }
        if let [_, _, ..] = matched {
            // Package indices come in ID order, each once: the sample is the first by ID.
            let ids = matched.iter().map(|&i| &packages[i].node.id);
            let entry = Unresolved::new(NAME, &hint.id, ids, matched.len());
            unresolved.entry(hint.id.clone()).or_insert(entry);
            continue;
        }
        let target = matched[0];
        if accepts {
            imported.entry((hint.scope.clone(), target)).or_default().push(hint);
        }
        // The dependency of the manifest nearest above the file that declares it.
        let name = packages[target].name;
        let declared = dirs_above(path).find_map(|dir| deps.get(&(dir, name)));
        for edge in declared.into_iter().flatten() {
            used.entry(&edge.id).or_insert_with(|| (*edge, target, Vec::new())).2.push(hint);
        }
    }

    let mut out: Vec<Edge> = Vec::new();
    for ((file, target), hints) in imported {
        let package = &packages[target];
        let kind = Kind(imports.kind.clone());
        let (from, to) = (Ref::Id(file), Ref::Id(package.node.id.clone()));
        let evidence = once_each(hints.iter().flat_map(|h| &h.evidence));
        out.push(Edge {
            id: edge_id(&kind, &from, &to, Some(package.name)),
            kind,
            from,
            to,
            via: Some(package.name.to_owned()),
            via_ref: None,
            props: BTreeMap::new(),
            evidence,
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: linked(&hints),
        });
    }
    for (_, (edge, target, mut hints)) in used {
        hints.sort_by(|a, b| (at(a), &a.id).cmp(&(at(b), &b.id)));
        hints.dedup_by(|a, b| a.id == b.id);
        hints.truncate(IMPORT_LINES);
        let lines = hints.iter().filter_map(|h| h.evidence.first());
        let evidence = once_each(edge.evidence.iter().chain(&packages[target].node.evidence).chain(lines));
        out.push(Edge {
            id: edge.id.clone(),
            kind: edge.kind.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            via: None,
            via_ref: None,
            props: edge.props.clone(),
            evidence,
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: linked(&hints),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Linked {
        batch: FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Linked,
            nodes: vec![],
            edges: out,
            hints: vec![],
            partial: vec![],
        },
        unresolved: unresolved.into_values().collect(),
    }
}

/// The packages an import's spec names, by index, in ID order: the index's own list, never a
/// copy, so however many packages share a name an import costs one lookup per segment.
fn matches<'i>(
    hint: &Hint,
    by_name: &'i HashMap<&str, Vec<usize>>,
    by_folded: &'i HashMap<String, Vec<usize>>,
) -> &'i [usize] {
    let spec = hint.key.as_str();
    if spec.is_empty() || spec.starts_with(['.', '/']) {
        return &[];
    }
    match attr(hint, "separator").filter(|s| !s.is_empty()) {
        Some(separator) => {
            let first = spec.split(separator).next().unwrap_or_default();
            if first.is_empty() || matches!(first, "crate" | "self" | "super") {
                return &[];
            }
            by_folded.get(&folded(first)).map_or(&[], Vec::as_slice)
        }
        None => {
            let mut end = spec.len();
            loop {
                if let Some(found) = by_name.get(&spec[..end]) {
                    return found;
                }
                match spec[..end].rfind('/') {
                    Some(i) if i > 0 => end = i,
                    _ => return &[],
                }
            }
        }
    }
}

/// Whether two ascending lists of package indices share one: each item of the shorter searched
/// in the longer, so the work is the shorter's length times a logarithm.
fn overlap(a: &[usize], b: &[usize]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.iter().any(|i| long.binary_search(i).is_ok())
}

/// `items` in order, each once, the first kept.
fn once_each<'e>(items: impl Iterator<Item = &'e Evidence>) -> Vec<Evidence> {
    let mut seen: HashSet<&Evidence> = HashSet::new();
    items.filter(|e| seen.insert(*e)).cloned().collect()
}

/// `Linked` provenance naming `hints`.
fn linked(hints: &[&Hint]) -> Provenance {
    Provenance::Linked {
        linker: NAME.into(),
        hints: hints.iter().map(|h| HintRef { id: h.id.clone(), extractor: extractor(h) }).collect(),
    }
}
