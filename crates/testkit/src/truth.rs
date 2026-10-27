//! The truth comparator: what a scanned fixture's map shows, held against `examples/TRUTH.json`.
//!
//! [`read`] takes the map through the CLI, `map --level L --lens all --detail graph --json` at
//! every level, with `PLANROOM_RAW=1` so evidence lines aren't masked, following each `meta.next`
//! with `--cursor` until it is null, so the comparison reads the whole map (DR-86). [`compare`]
//! then checks each truth item by the rules in `examples/README.md` and architecture.md's
//! [Testing strategy]:
//!
//! - **Containers** by name and repo, the truth's `kind` against the kind's own name (`service`
//!   for `code.service`), and every evidence item.
//! - **Components** by path, at any level of the hierarchy, then the container above them and
//!   their kind.
//! - **Relations** by `(from, to, kind)` between container blocks, and every evidence item.
//! - **Evidence** by `path`, `line` and `contains` as a substring of the line. An item is read on
//!   `main` unless it names a `branch`; evidence names its commit and no branch, so the branch is
//!   the one the scan event that read that commit names (DR-86).
//!
//! A truth item passes only with every check it carries: a relation missing one evidence item
//! fails. The scope is the loaded `Vocabulary` less [`DEFERRED`], DR-89's list of what M1 leaves
//! to M3. Every item outside it is listed in [`Report::out_of_scope`] with the reason, never
//! dropped, so the checks and the listed items together are every truth item.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use planroom_contract::{Evidence, Level, Vocabulary};
use serde_json::Value;

use crate::evidence_matches;

/// What a truth item that names no branch is read on [TRUTH about].
pub const MAIN: &str = "main";

/// DR-89's out-of-scope list: what M1's comparison leaves to M3, which brings the observability
/// pack and the cross-repo joins.
pub const DEFERRED: Deferred = Deferred {
    relations: &["deploys", "scrapes", "exports_telemetry", "queries", "depends_on_package"],
    container_kinds: &[
        ("otel-collector", "observability"),
        ("prometheus", "observability"),
        ("grafana", "observability"),
    ],
    container_evidence: &[
        ("otel-collector", "platform/otel/collector.yaml"),
        ("prometheus", "platform/prometheus/prometheus.yml"),
    ],
};

/// A list of truth items the comparison leaves out, by what they are.
#[derive(Debug, Clone, Copy)]
pub struct Deferred {
    /// Relation kinds, as the truth spells them.
    pub relations: &'static [&'static str],
    /// A container's kind: (container, kind).
    pub container_kinds: &'static [(&'static str, &'static str)],
    /// A container's evidence items in one file: (container, path).
    pub container_evidence: &'static [(&'static str, &'static str)],
}

/// What the comparison covers: the kinds and relations the loaded packs declare, by the names
/// the truth spells them in (the part after `<pack>.`), less a deferred list.
#[derive(Debug, Clone)]
pub struct Scope {
    kinds: BTreeSet<String>,
    relations: BTreeSet<String>,
    deferred: Deferred,
}

impl Scope {
    /// The loaded vocabulary, less DR-89's [`DEFERRED`] list.
    pub fn loaded(vocabulary: &Vocabulary) -> Scope {
        Scope::new(
            vocabulary.kinds.iter().map(|k| k.kind.0.as_str()),
            vocabulary.relations.iter().map(|r| r.kind.0.as_str()),
            DEFERRED,
        )
    }

    /// Qualified kinds and relations, such as `code.service` and `code.http`, less `deferred`.
    pub fn new<'a>(
        kinds: impl IntoIterator<Item = &'a str>,
        relations: impl IntoIterator<Item = &'a str>,
        deferred: Deferred,
    ) -> Scope {
        Scope {
            kinds: kinds.into_iter().map(|k| short(k).to_owned()).collect(),
            relations: relations.into_iter().map(|r| short(r).to_owned()).collect(),
            deferred,
        }
    }

    fn relation(&self, kind: &str) -> Result<(), String> {
        if self.deferred.relations.contains(&kind) {
            Err(format!("DR-89 leaves the {kind} relations to M3"))
        } else if !self.relations.contains(kind) {
            Err(format!("no loaded pack declares the relation kind {kind}"))
        } else {
            Ok(())
        }
    }

    fn kind(&self, container: Option<&str>, kind: &str) -> Result<(), String> {
        if container.is_some_and(|c| self.deferred.container_kinds.contains(&(c, kind))) {
            Err(format!("DR-89 leaves the {kind} kind to M3"))
        } else if !self.kinds.contains(kind) {
            Err(format!("no loaded pack declares the kind {kind}"))
        } else {
            Ok(())
        }
    }

    fn container_evidence(&self, container: &str, path: &str) -> Result<(), String> {
        if self.deferred.container_evidence.contains(&(container, path)) {
            Err(format!("DR-89 leaves {path}'s evidence to M3, since only the observability pack reads it"))
        } else {
            Ok(())
        }
    }
}

/// A block the map shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub level: Level,
    pub parent: Option<String>,
    /// The block's own evidence; empty while no read leaf shows it.
    pub evidence: Vec<Evidence>,
}

/// A relation the map shows, between two blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedEdge {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub evidence: Vec<Evidence>,
}

/// A scan event: the commit it read in a repo, and the branch HEAD named then (DR-86).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRead {
    pub repo: String,
    pub branch: String,
    pub commit: String,
}

/// Everything the comparison reads: the map's blocks and relations, and the scan events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observed {
    pub nodes: Vec<ObservedNode>,
    pub edges: Vec<ObservedEdge>,
    pub scans: Vec<ScanRead>,
}

/// One check of a truth item, keyed so a recall floor can name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub key: String,
    pub matched: bool,
}

/// One truth item in scope: its first check is that it exists, the rest what it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub key: String,
    pub checks: Vec<Check>,
}

impl Item {
    /// Whether every check holds: the item exists with its kind, its place and every evidence item.
    pub fn passes(&self) -> bool {
        self.checks.iter().all(|c| c.matched)
    }

    /// The checks that don't hold.
    pub fn missing(&self) -> Vec<&str> {
        self.checks.iter().filter(|c| !c.matched).map(|c| c.key.as_str()).collect()
    }
}

/// A truth check the comparison leaves out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutOfScope {
    pub key: String,
    pub why: String,
}

/// What [`compare`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Every truth container, component and relation in scope, in the truth's order.
    pub items: Vec<Item>,
    /// Every check left out of scope, with its reason.
    pub out_of_scope: Vec<OutOfScope>,
}

impl Report {
    /// The item with this key.
    pub fn item(&self, key: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.key == key)
    }

    /// The keys of every check that holds: what a recall floor records.
    pub fn recall(&self) -> BTreeSet<String> {
        self.items.iter().flat_map(|i| &i.checks).filter(|c| c.matched).map(|c| c.key.clone()).collect()
    }

    /// How many checks there are, in scope and out.
    pub fn total(&self) -> usize {
        self.items.iter().map(|i| i.checks.len()).sum::<usize>() + self.out_of_scope.len()
    }

    /// The items that don't pass.
    pub fn failures(&self) -> Vec<&Item> {
        self.items.iter().filter(|i| !i.passes()).collect()
    }

    /// A few lines for a failing test's message: the counts, each failing item's missing checks,
    /// and each out-of-scope check with its reason.
    pub fn describe(&self) -> String {
        let checks: usize = self.items.iter().map(|i| i.checks.len()).sum();
        let mut out = format!(
            "{} of {} items pass; {} of {checks} checks hold; {} checks out of scope\n",
            self.items.iter().filter(|i| i.passes()).count(),
            self.items.len(),
            self.recall().len(),
            self.out_of_scope.len()
        );
        for item in self.failures() {
            out.push_str(&format!("  fails: {}: missing {:?}\n", item.key, item.missing()));
        }
        for left in &self.out_of_scope {
            out.push_str(&format!("  out of scope: {}: {}\n", left.key, left.why));
        }
        out
    }
}

/// Compare what a scan shows with the truth, within `scope`.
pub fn compare(truth: &Value, observed: &Observed, scope: &Scope) -> Report {
    let map = Map::new(observed);
    let mut report = Report::default();
    for container in list(truth, "containers") {
        containers(&mut report, &map, scope, container);
    }
    for component in list(truth, "components") {
        components(&mut report, &map, scope, component);
    }
    for relation in list(truth, "relations") {
        relations(&mut report, &map, scope, relation);
    }
    report
}

fn list<'a>(truth: &'a Value, key: &str) -> &'a [Value] {
    truth[key].as_array().map_or(&[], Vec::as_slice)
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_else(|| panic!("a truth item has a string {key}: {value}"))
}

/// The part of a qualified kind after its pack: `service` for `code.service`.
fn short(kind: &str) -> &str {
    kind.split_once('.').map_or(kind, |(_, name)| name)
}

fn containers(report: &mut Report, map: &Map, scope: &Scope, truth: &Value) {
    let name = text(truth, "name");
    let repo = truth["repo"].as_str();
    let key = format!("container {name}");
    let node = map
        .nodes
        .values()
        .find(|n| n.level == Level::Container && n.name == name && map.repo(n).as_deref() == repo);
    let mut item =
        Item { key: key.clone(), checks: vec![Check { key: key.clone(), matched: node.is_some() }] };
    let kind = text(truth, "kind");
    let kind_key = format!("{key} kind {kind}");
    match scope.kind(Some(name), kind) {
        Ok(()) => {
            item.checks.push(Check { key: kind_key, matched: node.is_some_and(|n| short(&n.kind) == kind) })
        }
        Err(why) => report.out_of_scope.push(OutOfScope { key: kind_key, why }),
    }
    for want in list(truth, "evidence") {
        let evidence_key = format!("{key} evidence {}", where_(want));
        match scope.container_evidence(name, text(want, "path")) {
            Ok(()) => item.checks.push(Check {
                key: evidence_key,
                matched: node.is_some_and(|n| found(want, &n.evidence, map.scans)),
            }),
            Err(why) => report.out_of_scope.push(OutOfScope { key: evidence_key, why }),
        }
    }
    report.items.push(item);
}

fn components(report: &mut Report, map: &Map, scope: &Scope, truth: &Value) {
    let path = text(truth, "path");
    let key = format!("component {path}");
    let node = map.nodes.values().find(|n| n.level != Level::System && map.path(n).as_deref() == Some(path));
    let mut item =
        Item { key: key.clone(), checks: vec![Check { key: key.clone(), matched: node.is_some() }] };
    let container = text(truth, "container");
    item.checks.push(Check {
        key: format!("{key} in {container}"),
        matched: node.and_then(|n| map.container(n)).is_some_and(|c| c.name == container),
    });
    let kind = text(truth, "kind");
    let kind_key = format!("{key} kind {kind}");
    match scope.kind(None, kind) {
        Ok(()) => {
            item.checks.push(Check { key: kind_key, matched: node.is_some_and(|n| short(&n.kind) == kind) })
        }
        Err(why) => report.out_of_scope.push(OutOfScope { key: kind_key, why }),
    }
    report.items.push(item);
}

fn relations(report: &mut Report, map: &Map, scope: &Scope, truth: &Value) {
    let (from, to, kind) = (text(truth, "from"), text(truth, "to"), text(truth, "kind"));
    let key = format!("relation {from} -> {to} {kind}");
    let evidence: Vec<String> =
        list(truth, "evidence").iter().map(|e| format!("{key} evidence {}", where_(e))).collect();
    if let Err(why) = scope.relation(kind) {
        for check in std::iter::once(key).chain(evidence) {
            report.out_of_scope.push(OutOfScope { key: check, why: why.clone() });
        }
        return;
    }
    let named = |id: &str, name: &str| {
        map.nodes.get(id).is_some_and(|n| n.level == Level::Container && n.name == name)
    };
    let edges: Vec<&ObservedEdge> = map
        .edges
        .iter()
        .filter(|e| short(&e.kind) == kind && named(&e.from, from) && named(&e.to, to))
        .collect();
    let seen: Vec<Evidence> = edges.iter().flat_map(|e| e.evidence.iter().cloned()).collect();
    let mut checks = vec![Check { key: key.clone(), matched: !edges.is_empty() }];
    for (want, check) in list(truth, "evidence").iter().zip(evidence) {
        checks.push(Check { key: check, matched: found(want, &seen, map.scans) });
    }
    report.items.push(Item { key, checks });
}

/// `path:line`, and `@branch` when the item names one: a check key's tail.
fn where_(want: &Value) -> String {
    let mut at = format!("{}:{}", text(want, "path"), want["line"]);
    if let Some(branch) = want["branch"].as_str() {
        at.push('@');
        at.push_str(branch);
    }
    at
}

/// Whether some evidence item in `got` satisfies `want` on `want`'s branch: the same path and
/// line, `contains` within the line, and read on that branch. An item's branch is the one a scan
/// event of its repo names for the commit it names (DR-86), never a branch the item itself
/// carries. An item no scan event accounts for counts as read on `main` only when `want` names no
/// branch either.
fn found(want: &Value, got: &[Evidence], scans: &[ScanRead]) -> bool {
    let branch = want["branch"].as_str().unwrap_or(MAIN);
    got.iter().any(|evidence| {
        let Evidence::Line { path, commit, .. } = evidence else {
            return false;
        };
        let actual = serde_json::to_value(evidence).expect("evidence serializes");
        if !evidence_matches(want, &actual) {
            return false;
        }
        let repo = path.split('/').next().unwrap_or_default();
        let read: Vec<&ScanRead> = commit
            .iter()
            .flat_map(|c| scans.iter().filter(move |s| s.repo == repo && &s.commit == c))
            .collect();
        if read.is_empty() { want["branch"].is_null() } else { read.iter().any(|s| s.branch == branch) }
    })
}

/// The observed map, indexed.
struct Map<'a> {
    nodes: BTreeMap<&'a str, &'a ObservedNode>,
    edges: &'a [ObservedEdge],
    scans: &'a [ScanRead],
}

impl<'a> Map<'a> {
    fn new(observed: &'a Observed) -> Map<'a> {
        Map {
            nodes: observed.nodes.iter().map(|n| (n.id.as_str(), n)).collect(),
            edges: &observed.edges,
            scans: &observed.scans,
        }
    }

    /// The node and its ancestors, nearest first, stopping at a parent the map doesn't show or
    /// at a loop.
    fn ancestry(&self, node: &'a ObservedNode) -> Vec<&'a ObservedNode> {
        let mut out = vec![node];
        let mut next = node.parent.as_deref();
        while let Some(id) = next {
            match self.nodes.get(id) {
                Some(parent) if !out.iter().any(|n| n.id == parent.id) => {
                    out.push(parent);
                    next = parent.parent.as_deref();
                }
                _ => break,
            }
        }
        out
    }

    /// The repo the node sits in: the name of its nearest `repo` ancestor, by the ID's kind-short.
    fn repo(&self, node: &ObservedNode) -> Option<String> {
        self.ancestry(node).into_iter().find(|n| kind_short(&n.id) == Some("repo")).map(|n| n.name.clone())
    }

    /// `<repo>/<key>`: the repo-relative path an ID's key holds, under its repo's name.
    fn path(&self, node: &ObservedNode) -> Option<String> {
        let key = node.id.splitn(3, ':').nth(2)?;
        Some(format!("{}/{key}", self.repo(node)?))
    }

    /// The nearest container-level block above the node.
    fn container(&self, node: &'a ObservedNode) -> Option<&'a ObservedNode> {
        self.ancestry(node).into_iter().skip(1).find(|n| n.level == Level::Container)
    }
}

/// The kind-short of `<namespace>:<kind-short>:<key>`.
fn kind_short(id: &str) -> Option<&str> {
    id.split(':').nth(1)
}

/// How to run the engine for a read: the binary, the workspace it runs in and the data root.
#[derive(Debug, Clone, Copy)]
pub struct Cli<'a> {
    pub bin: &'a Path,
    pub cwd: &'a Path,
    pub data: &'a Path,
}

impl Cli<'_> {
    /// `planroom ARGS --json` with `PLANROOM_RAW=1`, as the truth comparator alone runs a read:
    /// the one envelope, which must be `ok`.
    pub fn json(&self, args: &[&str]) -> Value {
        let out = Command::new(self.bin)
            .args(args)
            .arg("--json")
            .current_dir(self.cwd)
            .env("PLANROOM_DATA", self.data)
            .env("PLANROOM_RAW", "1")
            .env_remove("PLANROOM_PERSON_TOKEN")
            .env_remove("PLANROOM_ACTOR")
            .output()
            .expect("the engine runs");
        let stdout = String::from_utf8(out.stdout).expect("the envelope is UTF-8");
        let env: Value = serde_json::from_str(stdout.trim_end())
            .unwrap_or_else(|e| panic!("{args:?}: one JSON envelope ({e}): {stdout}"));
        assert!(
            out.status.success() && env["ok"] == true,
            "{args:?}: {env}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        env
    }

    /// Every page of `ARGS`, following `meta.next` with `--cursor` until it is null: each page's
    /// envelope, in order.
    pub fn pages(&self, args: &[&str]) -> Vec<Value> {
        let mut pages: Vec<Value> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut argv = args.to_vec();
            if let Some(cursor) = &cursor {
                argv.extend(["--cursor", cursor]);
            }
            let env = self.json(&argv);
            let next = env["meta"]["next"].as_str().map(str::to_owned);
            pages.push(env);
            match next {
                None => return pages,
                Some(next) => {
                    assert!(cursor.as_ref() != Some(&next), "{args:?}: the cursor {next} moves on");
                    assert!(pages.len() < 10_000, "{args:?}: the pages end");
                    cursor = Some(next);
                }
            }
        }
    }
}

/// The levels the map shows blocks at, top down; `intent` holds plans, not the system.
pub const LEVELS: [&str; 4] = ["system", "container", "component", "code"];

/// The whole map at every level, `--lens all --detail graph`, through every page, with the scan
/// events given: each block once, and each relation once per level it shows at.
pub fn read(cli: Cli, scans: Vec<ScanRead>) -> Observed {
    let mut observed = Observed { scans, ..Observed::default() };
    let mut ids = BTreeSet::new();
    for level in LEVELS {
        for page in cli.pages(&["map", "--level", level, "--lens", "all", "--detail", "graph"]) {
            let result = &page["result"];
            for node in result["nodes"].as_array().expect("a map page has nodes") {
                let node = observed_node(node);
                if ids.insert(node.id.clone()) {
                    observed.nodes.push(node);
                }
            }
            for edge in result["edges"].as_array().expect("a map page has edges") {
                observed.edges.push(ObservedEdge {
                    kind: text(edge, "kind").to_owned(),
                    from: text(edge, "from").to_owned(),
                    to: text(edge, "to").to_owned(),
                    evidence: serde_json::from_value(edge["evidence_sample"].clone())
                        .expect("a lifted edge's evidence is evidence"),
                });
            }
        }
    }
    observed
}

fn observed_node(node: &Value) -> ObservedNode {
    ObservedNode {
        id: text(node, "id").to_owned(),
        kind: text(node, "kind").to_owned(),
        name: text(node, "name").to_owned(),
        level: serde_json::from_value(node["level"].clone()).expect("a block has a level"),
        parent: node["parent"].as_str().map(str::to_owned),
        evidence: match node.get("evidence") {
            Some(evidence) => {
                serde_json::from_value(evidence.clone()).expect("a block's evidence is evidence")
            }
            None => Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_the_repo_name_and_the_ids_key() {
        let node = |id: &str, name: &str, level, parent: Option<&str>| ObservedNode {
            id: id.into(),
            kind: "code.directory".into(),
            name: name.into(),
            level,
            parent: parent.map(Into::into),
            evidence: vec![],
        };
        let observed = Observed {
            nodes: vec![
                node("orders:repo:orders", "orders", Level::System, None),
                node("orders:container:orders", "orders", Level::Container, Some("orders:repo:orders")),
                node("orders:dir:internal", "internal", Level::Component, Some("orders:container:orders")),
                node("orders:dir:internal/http", "http", Level::Component, Some("orders:dir:internal")),
            ],
            ..Observed::default()
        };
        let map = Map::new(&observed);
        let http = map.nodes["orders:dir:internal/http"];
        assert_eq!(map.path(http).as_deref(), Some("orders/internal/http"));
        assert_eq!(map.container(http).map(|c| c.id.as_str()), Some("orders:container:orders"));
        assert_eq!(map.repo(map.nodes["orders:container:orders"]).as_deref(), Some("orders"));
    }
}
