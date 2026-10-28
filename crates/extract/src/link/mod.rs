//! Built-in linkers: pure functions that join the hints of a workspace's active snapshots into
//! the facts of its link snapshot (DR-12).
//!
//! A linker reads [`WorkspaceFacts`], the active set's repo names with its member snapshots'
//! System- and Container-level nodes and every hint, and returns [`Linked`]: the facts it joined,
//! sorted by ID, and the joins it couldn't make because two or more targets fit. It never reads a
//! file, a store or the order the repos were scanned in, so one active set links to one set of
//! facts. The scan that changes the active set stores them under the link snapshot's ID, inside
//! its own transaction.
//!
//! This module also holds what every linker shares: the index of definitions and calls that
//! evidence follows to callers ([`Callers`]), a URL's host, an image's repo and a route's
//! normalized path.

pub mod runtime;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use planroom_contract::{Evidence, FactBatch, Hint, LanguageDecl, Node, NodeId, Provenance, Tier};
use serde::Deserialize;
use serde_json::Value;

use crate::ExtractError;

/// The hint kind of a function definition: its key is the name.
pub const DEF_HINT: &str = "code.def";
/// The hint kind of a call: its key is the name it calls.
pub const CALL_HINT: &str = "code.call";
/// How many call sites one hint's evidence follows to, across both hops.
pub const CALL_SITES: usize = 3;
/// How many hops evidence follows callers: a hint's function's callers, then theirs.
pub const HOPS: usize = 2;
/// How many candidate IDs an [`Unresolved`] keeps, the first by ID.
pub const CANDIDATE_SAMPLE: usize = 3;

/// A built-in linker: the workspace's hints and nodes in, linked facts out.
pub trait Linker {
    /// Pack-qualified, such as `code/runtime_wiring`.
    fn name(&self) -> &'static str;
    fn link(&self, ws: &WorkspaceFacts, opts: &Value) -> Result<Linked, ExtractError>;
}

/// What a linker reads: one active set's facts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkspaceFacts {
    /// The active set's repo names, which are its namespaces.
    pub repos: BTreeSet<String>,
    /// The member snapshots' System- and Container-level nodes.
    pub nodes: Vec<Node>,
    /// Every hint of the member snapshots.
    pub hints: Vec<Hint>,
}

/// A join a linker didn't make because two or more targets fit: it never guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// Pack-qualified.
    pub linker: String,
    /// The hint's ID.
    pub hint: String,
    /// The first [`CANDIDATE_SAMPLE`] candidates by ID.
    pub candidates: Vec<NodeId>,
    /// How many candidates fit.
    pub candidate_count: usize,
}

impl Unresolved {
    /// The join of `hint` that `count` candidates, two or more, all fit, of which `candidates`
    /// iterates them in ID order; only the first [`CANDIDATE_SAMPLE`] are read.
    pub fn new<'c>(
        linker: &str,
        hint: &str,
        candidates: impl IntoIterator<Item = &'c NodeId>,
        count: usize,
    ) -> Unresolved {
        Unresolved {
            linker: linker.to_owned(),
            hint: hint.to_owned(),
            candidates: candidates.into_iter().take(CANDIDATE_SAMPLE).cloned().collect(),
            candidate_count: count,
        }
    }
}

/// A linker's output: its facts, as a batch of tier `Linked`, and the joins it left unresolved,
/// sorted by hint ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Linked {
    pub batch: FactBatch,
    pub unresolved: Vec<Unresolved>,
}

/// A relation a linker emits, as its options name it: the engine fills in the kinds its ends may
/// be from the loaded vocabulary, so no linker spells a relation's ends of its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationRef {
    /// Qualified, `<pack>.<name>`.
    pub kind: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
}

/// The built-in linker a pack names with `builtin = NAME`.
pub fn builtin(name: &str) -> Option<&'static dyn Linker> {
    match name {
        "runtime_wiring" => Some(&runtime::RuntimeWiring),
        _ => None,
    }
}

/// Whether `key` can stand in a node ID: one with a newline or a backslash can't, as no path the
/// walk maps can.
pub fn nameable(key: &str) -> bool {
    !key.is_empty() && !key.contains(['\n', '\\'])
}

/// A hint's first line of evidence: its file and line.
pub fn at(hint: &Hint) -> Option<(&str, u32)> {
    hint.evidence.iter().find_map(|e| match e {
        Evidence::Line { path, line, .. } => Some((path.as_str(), *line)),
        Evidence::Ref { .. } => None,
    })
}

/// The namespace a node ID names: what comes before its first `:`.
pub fn namespace(id: &NodeId) -> &str {
    id.0.split(':').next().unwrap_or_default()
}

/// The key a node ID names: what follows its namespace and its kind's prefix. A file's is its
/// repo-relative path.
pub fn key(id: &NodeId) -> &str {
    id.0.splitn(3, ':').nth(2).unwrap_or_default()
}

/// The test files of the loaded languages: a file is one when its extension gives a language and
/// one of that language's `tests` globs matches its repo-relative path. A test exercises the
/// runtime; it isn't the runtime's wiring, so no runtime join anchors in one or cites it.
#[derive(Debug, Clone, Default)]
pub struct TestFiles {
    by_extension: BTreeMap<String, globset::GlobSet>,
}

impl TestFiles {
    /// The test files `languages` declare; a glob that doesn't parse names itself in the error,
    /// though the pack loader refuses one first.
    pub fn new(languages: &[LanguageDecl]) -> Result<TestFiles, ExtractError> {
        let mut by_extension = BTreeMap::new();
        for language in languages.iter().filter(|l| !l.tests.is_empty()) {
            let mut set = globset::GlobSetBuilder::new();
            for glob in &language.tests {
                let built = globset::GlobBuilder::new(glob).literal_separator(true).build().map_err(|e| {
                    ExtractError::InvalidOptions(format!("{}: test glob `{glob}`: {e}", language.id))
                })?;
                set.add(built);
            }
            let set = set.build().map_err(|e| ExtractError::InvalidOptions(e.to_string()))?;
            for ext in &language.extensions {
                by_extension.insert(ext.trim_start_matches('.').to_owned(), set.clone());
            }
        }
        Ok(TestFiles { by_extension })
    }

    /// Whether the file at the repo-relative `path` is a test file.
    pub fn matches(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        name.rsplit_once('.')
            .and_then(|(_, ext)| self.by_extension.get(ext))
            .is_some_and(|globs| globs.is_match(path))
    }
}

/// The hint kind of a span of test code inside a file, such as a Rust item `#[cfg(test)]` marks:
/// its line is the span's first, and its `end` attribute the span's last. One that a module's
/// declaration gives also names the module, in `module`, and the file its `#[path]` names, in
/// `path` ([`TestScopes`]).
pub const TEST_SCOPE_HINT: &str = "code.test_scope";

/// The spans of test code inside files, from their [`TEST_SCOPE_HINT`] hints: a hint on a line
/// one of its file's spans holds is test code, as a test file's are. Each file's spans are merged
/// into disjoint ones in line order, so a hint finds its file's by a lookup and its line by a
/// binary search. A span whose `end` is no line number, or comes before its first line, is none.
///
/// A test scope that names a `module` is a module its file declares at its top level, and the
/// files Rust reads that module from are test code in whole ([`TestScopes::hold_file`]). With a
/// `path`, that is the file the path names, relative to the declaring file's directory. Without
/// one, it is `<module>.rs` and every file below `<module>/`, in the declaring file's directory
/// when that file is a `mod.rs`, `lib.rs`, `main.rs` or `build.rs` or a binary in `src/bin/` or
/// `examples/`, and otherwise in the directory named for its stem. A module declared inline (its
/// scope's `inline` attribute) has no file of its own, so only the files below `<module>/`, its
/// own out-of-line modules', are. A declaration one of whose scopes names a `path` is read from
/// its paths alone. A path that is absolute, or leaves its repo, names no file.
#[derive(Debug, Clone, Default)]
pub struct TestScopes<'a> {
    by_file: HashMap<&'a str, Vec<(u32, u32)>>,
    /// The scopes of the files a test module is read from.
    module_files: HashSet<String>,
    /// The scope of each directory below which every file is a test module's.
    module_dirs: HashSet<String>,
}

impl<'a> TestScopes<'a> {
    /// The spans `hints`' test scopes give.
    pub fn new(hints: impl IntoIterator<Item = &'a Hint>) -> TestScopes<'a> {
        let mut by_file: HashMap<&'a str, Vec<(u32, u32)>> = HashMap::new();
        // Each module declaration, by its file, its last line and its name, with the paths its
        // scopes name and whether it is declared inline.
        let mut modules: BTreeMap<(&'a str, u32, &'a str), (BTreeSet<&'a str>, bool)> = BTreeMap::new();
        for hint in hints.into_iter().filter(|h| h.kind.0 == TEST_SCOPE_HINT) {
            let Some((_, first)) = at(hint) else { continue };
            let Some(last) = attr(hint, "end").and_then(|e| e.parse::<u32>().ok()).filter(|l| *l >= first)
            else {
                continue;
            };
            by_file.entry(hint.scope.0.as_str()).or_default().push((first, last));
            if let Some(module) = attr(hint, "module") {
                let (paths, inline) = modules.entry((hint.scope.0.as_str(), last, module)).or_default();
                paths.extend(attr(hint, "path"));
                *inline |= attr(hint, "inline").is_some();
            }
        }
        for spans in by_file.values_mut() {
            spans.sort_unstable();
            let mut merged: Vec<(u32, u32)> = Vec::with_capacity(spans.len());
            for &(first, last) in spans.iter() {
                match merged.last_mut() {
                    Some(open) if first <= open.1.saturating_add(1) => open.1 = open.1.max(last),
                    _ => merged.push((first, last)),
                }
            }
            *spans = merged;
        }
        let mut scopes = TestScopes { by_file, module_files: HashSet::new(), module_dirs: HashSet::new() };
        for ((scope, _, module), (paths, inline)) in modules {
            scopes.add_module(scope, module, &paths, inline);
        }
        scopes
    }

    /// Add the files Rust reads `module` from, as the file `scope` names declares it at its top
    /// level: for a module declared `inline`, only the directory its own modules are read from.
    fn add_module(&mut self, scope: &str, module: &str, paths: &BTreeSet<&str>, inline: bool) {
        let file = key_of(scope);
        let Some(prefix) = scope.strip_suffix(file) else { return };
        let (dir, name) = file.rsplit_once('/').unwrap_or(("", file));
        let Some(stem) = name.strip_suffix(".rs") else { return };
        if !paths.is_empty() {
            for path in paths {
                if let Some(target) = relative(dir, path) {
                    self.module_files.insert(format!("{prefix}{target}"));
                }
            }
            return;
        }
        let module = module.strip_prefix("r#").unwrap_or(module);
        if module.is_empty() || !module.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return;
        }
        let parent = dir.rsplit('/').next().unwrap_or(dir);
        let owns_dir = matches!(name, "mod.rs" | "lib.rs" | "main.rs" | "build.rs")
            || dir == "src/bin"
            || dir.ends_with("/src/bin")
            || parent == "examples";
        let base = match (owns_dir, dir.is_empty()) {
            (true, true) => module.to_owned(),
            (true, false) => format!("{dir}/{module}"),
            (false, true) => format!("{stem}/{module}"),
            (false, false) => format!("{dir}/{stem}/{module}"),
        };
        if !inline {
            self.module_files.insert(format!("{prefix}{base}.rs"));
        }
        self.module_dirs.insert(format!("{prefix}{base}"));
    }

    /// Whether the file `scope` names is one a test module is read from: every hint of such a
    /// file is test code, as a test file's is.
    pub fn hold_file(&self, scope: &NodeId) -> bool {
        if self.module_files.is_empty() && self.module_dirs.is_empty() {
            return false;
        }
        let scope = scope.0.as_str();
        if self.module_files.contains(scope) {
            return true;
        }
        let file = key_of(scope);
        let Some(prefix) = scope.strip_suffix(file) else { return false };
        let mut dir = file;
        while let Some((parent, _)) = dir.rsplit_once('/') {
            if self.module_dirs.contains(&format!("{prefix}{parent}")) {
                return true;
            }
            dir = parent;
        }
        false
    }

    /// Whether `hint` sits on a line a test scope of its file holds: a test scope's own hint
    /// always does.
    pub fn hold(&self, hint: &Hint) -> bool {
        let Some(spans) = self.by_file.get(hint.scope.0.as_str()) else { return false };
        let Some((_, line)) = at(hint) else { return false };
        let i = spans.partition_point(|(first, _)| *first <= line);
        i.checked_sub(1).is_some_and(|i| line <= spans[i].1)
    }
}

/// What a node ID's text names after its namespace and its kind's prefix.
fn key_of(id: &str) -> &str {
    id.splitn(3, ':').nth(2).unwrap_or_default()
}

/// `path` relative to the repo directory `dir`, each `.` and `..` resolved: `None` when it is
/// absolute, names a directory or leaves the repo.
fn relative(dir: &str, path: &str) -> Option<String> {
    if path.starts_with('/') || path.ends_with('/') || path.contains('\\') {
        return None;
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The pack-qualified extractor a hint names in its provenance.
pub fn extractor(hint: &Hint) -> String {
    match &hint.provenance {
        Provenance::Extracted { extractor, .. } => extractor.clone(),
        Provenance::Linked { linker, .. } => linker.clone(),
        _ => String::new(),
    }
}

/// A string attribute of a hint.
pub fn attr<'h>(hint: &'h Hint, name: &str) -> Option<&'h str> {
    hint.attrs.get(name).and_then(Value::as_str)
}

/// How a tier ranks in [`rank`].
pub fn tier_rank(tier: Tier) -> u8 {
    match tier {
        Tier::Precise => 7,
        Tier::Syntactic => 6,
        Tier::Config => 5,
        Tier::Pattern => 4,
        Tier::Linked => 3,
        Tier::Inference => 2,
        Tier::Universal => 1,
    }
}

/// How a fact's provenance ranks when several give one node ID, as the kernel's merge ranks it
/// (`kernel::facts::specificity`): precise, syntactic, config, pattern, linked, inferred, then
/// universal. A linker reads the kind the merge will give a node it didn't make itself.
pub fn rank(provenance: &Provenance) -> u8 {
    let tier = tier_rank;
    match provenance {
        Provenance::Extracted { tier: t, .. } => tier(*t),
        Provenance::Linked { .. } => tier(Tier::Linked),
        Provenance::Inferred { .. } => tier(Tier::Inference),
        Provenance::Authored { .. } => u8::MAX,
    }
}

/// `acme/orders:1.4.0` to `orders`; `registry:5000/x/y@sha256:..` to `y`.
pub fn image_repo(image: &str) -> &str {
    let image = image.split('@').next().unwrap_or(image);
    let last = image.rsplit('/').next().unwrap_or(image);
    last.split(':').next().unwrap_or(last)
}

/// The host of `scheme://[userinfo@]host[:port][/path]`, or `None` when `text` isn't one.
/// Hand-rolled: the crate may depend on no URL parser, and this one form is all a join reads.
pub fn url_host(text: &str) -> Option<&str> {
    let (scheme, rest) = text.split_once("://")?;
    let scheme_ok = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !scheme_ok {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = match host_port.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => host_port.split(':').next().unwrap_or_default(),
    };
    (!host.is_empty()).then_some(host)
}

/// The path of `scheme://authority/path`, from its first `/` after the authority; `/` when it
/// has none.
pub fn url_path(text: &str) -> &str {
    let rest = text.split_once("://").map_or(text, |(_, r)| r);
    rest.find('/').map_or("/", |i| &rest[i..])
}

/// A route's path as joins compare it: each interpolated or parameter segment (`${id}`, `{id}`,
/// `{quote(id)}`, `:id`) becomes `{}`, and a query string or fragment is left out.
pub fn normalize_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        let opens = c == '{' || (c == '$' && chars.peek() == Some(&'{'));
        if opens {
            if c == '$' {
                chars.next();
            }
            let mut depth = 1usize;
            for inner in chars.by_ref() {
                match inner {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push_str("{}");
            continue;
        }
        if c == '?' || c == '#' {
            break;
        }
        out.push(c);
    }
    out.split('/')
        .map(|s| if s.len() > 1 && s.starts_with(':') { "{}" } else { s })
        .collect::<Vec<_>>()
        .join("/")
}

/// The name a call or a definition is known by: its last segment after `.`, `::` or `->`.
pub fn last_segment(name: &str) -> &str {
    let mut last = name;
    for sep in [".", "::", "->"] {
        if let Some((_, tail)) = last.rsplit_once(sep) {
            last = tail;
        }
    }
    last
}

/// One call site: the file its hint is scoped to, its line, and its evidence.
#[derive(Debug, Clone, Copy)]
struct Site<'a> {
    scope: &'a str,
    path: &'a str,
    line: u32,
    evidence: &'a Evidence,
}

/// Definitions and calls, indexed so evidence follows a hint to its callers without comparing
/// every pair of hints: each file's definitions by line, and each container's calls by the name
/// they call.
///
/// A hint's enclosing function is the nearest `code.def` at or above its line in the same file,
/// since an extractor gives each capture only its start line. Its callers are the `code.call`
/// hints in the same container whose name's last segment is the definition's name. The search
/// goes [`HOPS`] hops and keeps at most [`CALL_SITES`] call sites across both: the first hop's
/// first, each hop in (path, line) order.
pub struct Callers<'a> {
    defs: BTreeMap<&'a str, Vec<(u32, &'a str)>>,
    calls: BTreeMap<(NodeId, &'a str), Vec<Site<'a>>>,
}

impl<'a> Callers<'a> {
    /// The index of `hints`, each call placed in the container `container` gives its scope; a
    /// call in no container is left out.
    pub fn new(
        hints: impl IntoIterator<Item = &'a Hint>,
        container: impl Fn(&NodeId) -> Option<NodeId>,
    ) -> Callers<'a> {
        let mut defs: BTreeMap<&str, Vec<(u32, &str)>> = BTreeMap::new();
        let mut calls: BTreeMap<(NodeId, &str), Vec<Site<'_>>> = BTreeMap::new();
        for hint in hints {
            let Some((path, line)) = at(hint) else { continue };
            if hint.kind.0 == DEF_HINT {
                defs.entry(hint.scope.0.as_str()).or_default().push((line, last_segment(&hint.key)));
            } else if hint.kind.0 == CALL_HINT
                && let Some(container) = container(&hint.scope)
            {
                let evidence = hint.evidence.first().expect("a hint with a line has evidence");
                let site = Site { scope: hint.scope.0.as_str(), path, line, evidence };
                calls.entry((container, last_segment(&hint.key))).or_default().push(site);
            }
        }
        for list in defs.values_mut() {
            list.sort_unstable();
        }
        for list in calls.values_mut() {
            list.sort_by(|a, b| (a.path, a.line).cmp(&(b.path, b.line)));
            list.dedup_by(|a, b| (a.path, a.line) == (b.path, b.line));
        }
        Callers { defs, calls }
    }

    /// The name of the function that holds line `line` of the file `scope`.
    fn enclosing(&self, scope: &str, line: u32) -> Option<&'a str> {
        let defs = self.defs.get(scope)?;
        let i = defs.partition_point(|(l, _)| *l <= line);
        i.checked_sub(1).map(|i| defs[i].1)
    }

    /// The call sites of the function holding `hint`, in `container`, two hops at most and
    /// [`CALL_SITES`] in all: the first hop's in (path, line) order, then the second's. A call on
    /// the hint's own line is the hint's own call, not a caller.
    pub fn sites(&self, container: &NodeId, hint: &Hint) -> Vec<&'a Evidence> {
        let Some((path, line)) = at(hint) else { return vec![] };
        let own = (path, line);
        let mut taken: Vec<Site<'a>> = Vec::new();
        let mut frontier: Vec<(&str, u32)> = vec![(hint.scope.0.as_str(), line)];
        for _ in 0..HOPS {
            let room = CALL_SITES - taken.len();
            if room == 0 || frontier.is_empty() {
                break;
            }
            let mut hop: Vec<Site<'a>> = Vec::new();
            for (scope, line) in &frontier {
                let Some(name) = self.enclosing(scope, *line) else { continue };
                let Some(list) = self.calls.get(&(container.clone(), name)) else { continue };
                // At most `room` new sites from each list: the rest could never be kept.
                let new = list
                    .iter()
                    .filter(|s| (s.path, s.line) != own)
                    .filter(|s| !taken.iter().any(|t| (t.path, t.line) == (s.path, s.line)))
                    .take(room);
                hop.extend(new);
            }
            hop.sort_by(|a, b| (a.path, a.line).cmp(&(b.path, b.line)));
            hop.dedup_by(|a, b| (a.path, a.line) == (b.path, b.line));
            hop.truncate(room);
            frontier = hop.iter().map(|s| (s.scope, s.line)).collect();
            taken.extend(hop);
        }
        taken.into_iter().map(|s| s.evidence).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_gives_its_host_past_userinfo_and_port() {
        assert_eq!(
            url_host("postgres://orders:orders@orders-db:5432/orders?sslmode=disable"),
            Some("orders-db")
        );
        assert_eq!(url_host("redis://redis:6379/0"), Some("redis"));
        assert_eq!(url_host("http://orders:8080"), Some("orders"));
        assert_eq!(url_host("http://[::1]:80/x"), Some("::1"));
        assert_eq!(url_host("git+ssh://u@h/x"), Some("h"));
        assert_eq!(url_host("a:b@c:5"), None, "no scheme");
        assert_eq!(url_host("://h"), None);
        assert_eq!(url_host("1x://h"), None);
        assert_eq!(url_host("http://"), None);
        assert_eq!(url_host("http://u@:80"), None);
        assert_eq!(url_path("http://orders:8080/orders/1?x=1"), "/orders/1?x=1");
        assert_eq!(url_path("http://orders:8080"), "/");
    }

    #[test]
    fn a_route_path_normalizes_its_parameters() {
        assert_eq!(normalize_path("/orders/${encodeURIComponent(id)}"), "/orders/{}");
        assert_eq!(normalize_path("/orders/{id}/events"), "/orders/{}/events");
        assert_eq!(normalize_path("/orders/{urllib.parse.quote(order_id)}/events"), "/orders/{}/events");
        assert_eq!(normalize_path("/items/:sku"), "/items/{}");
        assert_eq!(normalize_path("/a/{b{c}}/d?x=1#f"), "/a/{}/d");
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn names_and_images_take_their_last_segment() {
        assert_eq!(last_segment("h.queue.Publish"), "Publish");
        assert_eq!(last_segment("std::fs::read"), "read");
        assert_eq!(last_segment("$this->save"), "save");
        assert_eq!(image_repo("acme/orders:1.4.0"), "orders");
        assert_eq!(image_repo("redis:7.4"), "redis");
        assert_eq!(image_repo("registry:5000/x/y@sha256:abc"), "y");
    }
}
