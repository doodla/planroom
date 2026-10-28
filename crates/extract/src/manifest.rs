//! The `manifest` built-in: the package each manifest declares and the packages it depends on, for
//! npm (`package.json`), Go (`go.mod`), Python (`pyproject.toml`) and Rust (`Cargo.toml`).
//!
//! - **Packages.** A manifest that names its package gives a package node keyed by that name, with
//!   the alias `pkg:<name>` a dependency joins by. It sits under the directory that holds the
//!   manifest; the repo root's sits under the repo's own container.
//! - **The repo's container.** The manifest at the repo's root, when it names a package, also gives
//!   the repo's own container, `<repo>:container:<repo>`, as the pack's library kind with role
//!   `library`. It is Universal-tier, so where a deployment runs the repo, as a compose service of
//!   the same name joined to it, the service's kind and role win and the evidence unions [DR-42].
//! - **Dependencies.** Each dependency the manifest lists is a `code.depends_on_package` edge from
//!   the package, or from the repo when the manifest names none, to the alias `pkg:<dependency>`,
//!   with the version it asks for as `uses` and every line that asks for it. It has no `via` until
//!   the `imports` linker joins it to the code that uses it.
//!
//! Every fact cites the manifest line that says it. A manifest that isn't UTF-8 or doesn't parse
//! is partial and gives nothing; a comment names nothing; a byte order mark before the text is
//! read past. A package whose name holds a newline or a backslash, which no node ID can hold,
//! gives no package node: the manifest is partial, and its dependencies hang from the repo as a
//! nameless manifest's do. JSON is read as YAML, with the YAML reader's caps on nesting and on
//! what aliases copy, and TOML by the `toml` parser, which refuses nesting past its own limit, so
//! no manifest can stall a scan. Its facts are bounded as the declarative runner's are
//! ([`crate::declarative::charge`]): a manifest whose facts would hold more heap than its size
//! allows is partial with `work_limit`, and in sorted path order the first manifest that would
//! take the repo's total past its bound, and every one after it, with `repo_limit`; each gives no
//! fact. So no manifest, nor many of them, can exhaust memory.

use std::collections::BTreeMap;

use planroom_contract::{
    Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Kind, Layer, Node, NodeId, PartialFile, PartialReason,
    Provenance, Ref, Tier,
};
use serde::Deserialize;
use serde_json::Value;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::declarative::REPO_CAPTURE_MAX;
use crate::declarative::charge::{self, Charges};
use crate::files::repo_id;
use crate::ids::{edge_id, node_id};
use crate::yaml::{self, Yaml};
use crate::{ExtractError, Extractor, KindRef, RepoPath, RepoView, lines};

pub const NAME: &str = "code/manifest";
/// The globs a pack declares this built-in with.
pub const DEFAULT_FILES: &[&str] = &["**/package.json", "**/go.mod", "**/pyproject.toml", "**/Cargo.toml"];
/// The relation each dependency is: the code pack declares it.
pub const DEPENDS_ON_PACKAGE: &str = "code.depends_on_package";

/// The kinds this built-in emits, which the pack that declares it names in `options.kinds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// The package a manifest declares.
    pub package: KindRef,
    /// The repo's own container, which its root manifest gives.
    pub library: KindRef,
    /// The repo's node, which the container sits under.
    pub repo: KindRef,
    /// The directory that holds a manifest below the root, which its package sits under.
    pub directory: KindRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    kinds: Kinds,
}

pub struct Manifest;

/// What one manifest declares: its package's name and the line that names it, and its
/// dependencies.
#[derive(Debug, Default, PartialEq)]
struct Declared {
    name: Option<(String, u32)>,
    deps: Vec<Dep>,
}

/// One dependency: the package it names, the version it asks for, and its line.
#[derive(Debug, PartialEq)]
struct Dep {
    name: String,
    uses: Option<String>,
    line: u32,
}

impl Extractor for Manifest {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
        self.run_within(repo, files, opts, REPO_CAPTURE_MAX)
    }
}

impl Manifest {
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
        let mut nodes: BTreeMap<NodeId, Node> = BTreeMap::new();
        let mut edges: BTreeMap<EdgeId, Edge> = BTreeMap::new();
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
            let (read, errors) = match read(repo, kinds, file, &full, text, &mut charges) {
                Ok(Some(read)) => read,
                // A file the built-in doesn't read.
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
            if errors > 0 {
                partial.push(PartialFile { path: full, errors, reason: None });
            }
            for node in read.nodes.into_values() {
                add_node(&mut nodes, node);
            }
            for (id, edge) in read.edges {
                match edges.get_mut(&id) {
                    // Two manifests give one edge: both lines.
                    Some(kept) => union(&mut kept.evidence, edge.evidence),
                    None => {
                        edges.insert(id, edge);
                    }
                }
            }
        }
        Ok(FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Universal,
            nodes: nodes.into_values().collect(),
            edges: edges.into_values().collect(),
            hints: vec![],
            partial,
        })
    }
}

/// One manifest's facts, by ID.
#[derive(Default)]
struct Read {
    nodes: BTreeMap<NodeId, Node>,
    edges: BTreeMap<EdgeId, Edge>,
}

/// The facts of the manifest `file`, whose text is `text`, each charged to `charges` as it is
/// made ([`charge`]), and the errors it counted. `Ok(None)` is a file this built-in doesn't read;
/// `Err(None)` one that doesn't parse, and `Err` with a reason one whose facts would charge more
/// than its budget. A package whose name no node ID can hold, with a newline or a backslash,
/// gives no package node, and the manifest's dependencies hang from the repo as a nameless
/// manifest's do; it counts one error, so the file is partial.
fn read(
    repo: &RepoView,
    kinds: &Kinds,
    file: &RepoPath,
    full: &str,
    text: &str,
    charges: &mut Charges,
) -> Result<Option<(Read, u32)>, Option<PartialReason>> {
    let (dir, base) = file.0.rsplit_once('/').unwrap_or(("", &file.0));
    // A byte order mark is no text the parsers read. It sits on line 1 alone, so lines number the
    // same with it or without; evidence quotes the blob's lines whole.
    let body = lines::strip_bom(text);
    let spans = lines::Index::new(body);
    let declared = match base {
        "package.json" => npm(body),
        "go.mod" => Ok(go_mod(body)),
        "pyproject.toml" => toml_doc(body).map(|doc| pyproject(&doc, &spans)),
        "Cargo.toml" => toml_doc(body).map(|doc| cargo(&doc, &spans)),
        // A pack may point the built-in at other files; it reads only these four.
        _ => return Ok(None),
    };
    let Ok(mut declared) = declared else { return Err(None) };
    let mut errors = 0;
    if declared.name.as_ref().is_some_and(|(name, _)| !nameable(name)) {
        declared.name = None;
        errors += 1;
    }
    let index = lines::Index::new(text);
    let evidence = |line: u32| Evidence::Line {
        path: full.to_owned(),
        line,
        contains: index.line(text, line).map(lines::cut),
        commit: Some(repo.commit.clone()),
        // A snapshot is keyed by its commit, never a branch (DR-85).
        branch: None,
    };
    let provenance = Provenance::Extracted { extractor: NAME.into(), tier: Tier::Universal };
    let mut out = Read::default();
    let from = match &declared.name {
        Some((name, line)) => {
            let parent = if dir.is_empty() {
                let id = node_id(&repo.name, &kinds.library.id_prefix, &repo.name);
                let mut props = BTreeMap::new();
                props.insert("role".to_owned(), Value::from("library"));
                let container = Node {
                    id: id.clone(),
                    kind: Kind(kinds.library.kind.clone()),
                    level: kinds.library.level,
                    parent: Some(repo_id(&repo.name, &kinds.repo)),
                    name: repo.name.clone(),
                    aliases: vec![],
                    props,
                    layer: Layer::Actual,
                    provenance: provenance.clone(),
                    evidence: vec![evidence(*line)],
                };
                charges.add(charge::node(&container))?;
                out.nodes.insert(id.clone(), container);
                id
            } else {
                node_id(&repo.name, &kinds.directory.id_prefix, dir)
            };
            let id = node_id(&repo.name, &kinds.package.id_prefix, name);
            let package = Node {
                id: id.clone(),
                kind: Kind(kinds.package.kind.clone()),
                level: kinds.package.level,
                parent: Some(parent),
                name: name.clone(),
                aliases: vec![format!("pkg:{name}")],
                props: BTreeMap::new(),
                layer: Layer::Actual,
                provenance: provenance.clone(),
                evidence: vec![evidence(*line)],
            };
            charges.add(charge::node(&package))?;
            out.nodes.insert(id.clone(), package);
            id
        }
        None => repo_id(&repo.name, &kinds.repo),
    };
    let from = Ref::Id(from);
    for dep in declared.deps {
        let kind = Kind(DEPENDS_ON_PACKAGE.into());
        let to = Ref::Alias(format!("pkg:{}", dep.name));
        let id = edge_id(&kind, &from, &to, None);
        let line = evidence(dep.line);
        match out.edges.get_mut(&id) {
            // Listed twice, as a dependency and a dev dependency: one edge, both lines.
            Some(kept) => {
                if !kept.evidence.contains(&line) {
                    charges.add(charge::further(&line))?;
                    kept.evidence.push(line);
                }
            }
            None => {
                let mut props = BTreeMap::new();
                if let Some(uses) = dep.uses {
                    props.insert("uses".to_owned(), Value::from(uses));
                }
                let edge = Edge {
                    id: id.clone(),
                    kind,
                    from: from.clone(),
                    to,
                    via: None,
                    via_ref: None,
                    props,
                    evidence: vec![line],
                    confidence: 1.0,
                    layer: Layer::Actual,
                    provenance: provenance.clone(),
                };
                charges.add(charge::edge(&edge))?;
                out.edges.insert(id, edge);
            }
        }
    }
    Ok(Some((out, errors)))
}

/// Whether a node ID can hold `key`: one with a newline or a backslash, as no path the walk takes
/// holds, can't (contracts.md, `unnamed_paths`).
fn nameable(key: &str) -> bool {
    !key.contains('\n') && !key.contains('\\')
}

/// Add `node`, or its evidence to the node of its ID that two manifests already gave.
fn add_node(nodes: &mut BTreeMap<NodeId, Node>, node: Node) {
    match nodes.get_mut(&node.id) {
        Some(kept) => union(&mut kept.evidence, node.evidence),
        None => {
            nodes.insert(node.id.clone(), node);
        }
    }
}

/// `more`'s items that `kept` doesn't hold yet, onto it.
fn union(kept: &mut Vec<Evidence>, more: Vec<Evidence>) {
    for item in more {
        if !kept.contains(&item) {
            kept.push(item);
        }
    }
}

/// `package.json`: `name`, and each of `dependencies`, `devDependencies`, `peerDependencies` and
/// `optionalDependencies`.
fn npm(text: &str) -> Result<Declared, ()> {
    // An empty file holds no document, and declares nothing.
    let Some(doc) = yaml::parse(text).map_err(drop)?.into_iter().next() else {
        return Ok(Declared::default());
    };
    if doc.entries().is_none() {
        return Err(());
    }
    let name = doc.get("name").and_then(|n| Some((n.as_str()?.to_owned(), n.line())));
    let mut deps = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        for (key, value) in doc.get(section).and_then(Yaml::entries).unwrap_or_default() {
            let Some(name) = key.as_str() else { continue };
            deps.push(Dep {
                name: name.to_owned(),
                uses: value.as_str().map(str::to_owned),
                line: key.line(),
            });
        }
    }
    Ok(Declared { name: name.filter(|(n, _)| !n.is_empty()), deps })
}

/// `go.mod`: the `module` path, and each `require`, alone or in a block. `//` starts a comment,
/// and any other directive, such as `replace`, names no dependency.
fn go_mod(text: &str) -> Declared {
    let mut declared = Declared::default();
    // The directive whose block the line is in, until its `)`.
    let mut block: Option<&str> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let code = raw.split_once("//").map_or(raw, |(code, _)| code).trim();
        if code.is_empty() {
            continue;
        }
        let spec = match block {
            Some(directive) => {
                if code == ")" {
                    block = None;
                    continue;
                }
                (directive == "require").then_some(code)
            }
            None => {
                let (directive, rest) = code.split_once(char::is_whitespace).unwrap_or((code, ""));
                let rest = rest.trim();
                if rest == "(" {
                    block = Some(directive);
                    continue;
                }
                match directive {
                    "module" if declared.name.is_none() => {
                        let name = unquote(rest);
                        if !name.is_empty() {
                            declared.name = Some((name.to_owned(), line));
                        }
                        None
                    }
                    "require" => Some(rest),
                    _ => None,
                }
            }
        };
        let mut words = spec.unwrap_or_default().split_whitespace();
        if let Some(path) = words.next().map(unquote).filter(|p| !p.is_empty()) {
            let uses = words.next().map(str::to_owned);
            declared.deps.push(Dep { name: path.to_owned(), uses, line });
        }
    }
    declared
}

/// A go.mod path without the quotes it may be written in.
fn unquote(word: &str) -> &str {
    word.trim().trim_matches(|c| c == '"' || c == '`')
}

fn toml_doc(text: &str) -> Result<Spanned<DeTable<'_>>, ()> {
    DeTable::parse(text).map_err(drop)
}

/// The value under `key` in `table`.
fn get<'t, 'i>(table: &'t DeTable<'i>, key: &str) -> Option<&'t Spanned<DeValue<'i>>> {
    table.iter().find(|(k, _)| k.get_ref().as_ref() == key).map(|(_, v)| v)
}

fn table<'t, 'i>(value: Option<&'t Spanned<DeValue<'i>>>) -> Option<&'t DeTable<'i>> {
    match value?.get_ref() {
        DeValue::Table(t) => Some(t),
        _ => None,
    }
}

fn string<'t>(value: Option<&'t Spanned<DeValue<'_>>>) -> Option<&'t str> {
    match value?.get_ref() {
        DeValue::String(s) => Some(s.as_ref()),
        _ => None,
    }
}

/// The table `path` names under `root`.
fn at<'t, 'i>(root: &'t DeTable<'i>, path: &[&str]) -> Option<&'t DeTable<'i>> {
    path.iter().try_fold(root, |t, key| table(get(t, key)))
}

/// The line a span starts on.
fn line_of(index: &lines::Index, span: std::ops::Range<usize>) -> u32 {
    index.line_of(span.start)
}

/// A string under `key` in `table`, with its line.
fn named(table: Option<&DeTable<'_>>, key: &str, index: &lines::Index) -> Option<(String, u32)> {
    let value = get(table?, key)?;
    let name = string(Some(value))?;
    (!name.is_empty()).then(|| (name.to_owned(), line_of(index, value.span())))
}

/// `pyproject.toml`: `[project]`'s `name`, `dependencies` and `optional-dependencies`, else
/// Poetry's `[tool.poetry]` `name` and its dependency tables, `python` aside.
fn pyproject(doc: &Spanned<DeTable<'_>>, index: &lines::Index) -> Declared {
    let root = doc.get_ref();
    let project = at(root, &["project"]);
    let poetry = at(root, &["tool", "poetry"]);
    let name = named(project, "name", index).or_else(|| named(poetry, "name", index));
    let mut deps = Vec::new();
    let mut requirements = |value: Option<&Spanned<DeValue<'_>>>| {
        let Some(DeValue::Array(items)) = value.map(Spanned::get_ref) else { return };
        for item in items {
            if let Some(dep) = string(Some(item)).and_then(|r| requirement(r, line_of(index, item.span()))) {
                deps.push(dep);
            }
        }
    };
    if let Some(project) = project {
        requirements(get(project, "dependencies"));
        for (_, group) in table(get(project, "optional-dependencies")).into_iter().flatten() {
            requirements(Some(group));
        }
    }
    if let Some(poetry) = poetry {
        let mut tables = vec![table(get(poetry, "dependencies")), table(get(poetry, "dev-dependencies"))];
        for (_, group) in table(get(poetry, "group")).into_iter().flatten() {
            tables.push(table(Some(group)).and_then(|g| table(get(g, "dependencies"))));
        }
        for t in tables.into_iter().flatten() {
            deps.extend(keyed(t, index).into_iter().filter(|d| d.name != "python"));
        }
    }
    Declared { name, deps }
}

/// A PEP 508 requirement, `psycopg[binary]==3.2.2; python_version > "3"`: its name, and its
/// version specifier when it has one.
fn requirement(text: &str, line: u32) -> Option<Dep> {
    let text = text.trim();
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .unwrap_or(text.len());
    let name = &text[..end];
    if name.is_empty() {
        return None;
    }
    let mut rest = text[end..].trim_start();
    if let Some(after) = rest.strip_prefix('[') {
        rest = after.split_once(']').map_or("", |(_, r)| r).trim_start();
    }
    let rest = rest.split(';').next().unwrap_or_default().trim();
    let uses = (!rest.is_empty()).then(|| rest.to_owned());
    Some(Dep { name: name.to_owned(), uses, line })
}

/// A table of dependencies by name, as Poetry and Cargo write them: a version string, or a table
/// with `version`, and for Cargo `package` naming the package a renamed one stands for.
fn keyed(table: &DeTable<'_>, index: &lines::Index) -> Vec<Dep> {
    table
        .iter()
        .map(|(key, value)| {
            let detail = match value.get_ref() {
                DeValue::Table(t) => Some(t),
                _ => None,
            };
            let name = detail.and_then(|t| string(get(t, "package"))).unwrap_or(key.get_ref().as_ref());
            let uses = string(Some(value)).or_else(|| detail.and_then(|t| string(get(t, "version"))));
            Dep { name: name.to_owned(), uses: uses.map(str::to_owned), line: line_of(index, key.span()) }
        })
        .collect()
}

/// `Cargo.toml`: `[package]`'s `name`, and its `dependencies`, `dev-dependencies` and
/// `build-dependencies`, also under each `[target.<cfg>]`.
fn cargo(doc: &Spanned<DeTable<'_>>, index: &lines::Index) -> Declared {
    const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let root = doc.get_ref();
    let name = named(at(root, &["package"]), "name", index);
    let mut deps = Vec::new();
    let mut scopes = vec![root];
    scopes.extend(table(get(root, "target")).into_iter().flatten().filter_map(|(_, t)| table(Some(t))));
    for scope in scopes {
        for section in SECTIONS {
            if let Some(t) = table(get(scope, section)) {
                deps.extend(keyed(t, index));
            }
        }
    }
    Declared { name, deps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_requirement_gives_its_name_and_version() {
        let req = |t: &str| requirement(t, 1).map(|d| (d.name, d.uses));
        assert_eq!(req("redis==5.0.8"), Some(("redis".into(), Some("==5.0.8".into()))));
        assert_eq!(req("psycopg[binary] >= 3.2"), Some(("psycopg".into(), Some(">= 3.2".into()))));
        assert_eq!(req("attrs ; python_version < '3.8'"), Some(("attrs".into(), None)));
        assert_eq!(req("zope.interface"), Some(("zope.interface".into(), None)));
        assert_eq!(req(" ==1"), None);
    }

    #[test]
    fn a_go_mod_block_of_another_directive_names_nothing() {
        let declared =
            go_mod("module \"a/b\"\nreplace (\n  x => ./y\n)\nexclude x v1\nrequire y v2 // pinned\n");
        assert_eq!(declared.name, Some(("a/b".into(), 1)));
        assert_eq!(declared.deps, [Dep { name: "y".into(), uses: Some("v2".into()), line: 6 }]);
    }
}
