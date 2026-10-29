//! The `runtime_wiring` linker: compose services as containers, served routes as endpoints, and
//! the calls, queue operations and database opens of a container joined to the container they
//! reach (DR-12, DR-86).
//!
//! - **Containers.** Each `code.compose_service` hint is a container, its ID
//!   `<ns>:<service prefix>:<service>`. `ns` is the active repo its `build` context's last segment
//!   names, or else the one its image names (`acme/orders:1.4.0` to `orders`), or else `ws`. It
//!   takes the hint's `kind` and `role`, which compose resolved from the image roles, its image,
//!   the aliases `host:` and `service:`, the repo's node as its parent, and the hint's evidence. So
//!   the join of a service to a repo is a function of the active set, never of one repo's scan.
//! - **Test code.** No hint from a file the loaded languages' `tests` globs name is read
//!   ([`TestFiles`]), nor one on a line a `code.test_scope` hint of its file holds, such as a Rust
//!   item `#[cfg(test)]` or `#[test]` marks, nor one in a file a test module a file declares at
//!   its top level is read from ([`TestScopes`](super::TestScopes)): test code never anchors a
//!   join, wires one, serves an endpoint, binds a wrapper, gives a constant or is cited as a
//!   caller. Its definitions, imports and calls are still facts of
//!   its repo's snapshot, and a test scope is a hint the link reads and never shows.
//! - **A hint's container.** A hint's container is the one a member fact places its file in, above
//!   the universal tier (`WorkspaceFacts::file_containers`), such as a script a pack gives a
//!   container of its own. Otherwise its repo is its scope's namespace, and its container is that
//!   repo's one Container-level node, minted here or extracted. When exactly one compose service
//!   joins the repo, its container is the repo's, and a container only the universal tier gives
//!   (a root manifest's) yields to it. A join hint (`route_call`, `url`, `queue_op`, `db_open`)
//!   in a repo with two or more containers otherwise is [`Unresolved`] and joins nothing; with
//!   none it joins nothing and warns of nothing.
//! - **Hosts.** A host resolves through the `host:<name>` alias of the containers that carry it.
//!   Candidates are counted after the relation's `to` kinds filter them, one per target: one
//!   makes an edge, two or more are [`Unresolved`].
//! - **Endpoints.** One `code.endpoint` per container, method and path its `code.route_serve`
//!   hints serve, an OpenAPI spec's included: its key `<METHOD> <path>`, under the container, with
//!   the evidence of every hint that serves it. A route served for every method is `ANY`.
//! - **`http`.** A `code.route_call` whose base is a literal URL, or a `code.url`, in container A,
//!   whose host resolves to container B; [`bind`] reads a call's base, path and method. A call
//!   whose base isn't a literal takes its host as a database open does, from the call's file: an
//!   `env_read` there of a name compose sets on A; then one whose default compose doesn't
//!   override, or a URL literal there; then any `env_read` in A's repo of a name compose sets on
//!   A, counting only the targets that serve an endpoint the call matches. Paths compare with
//!   each interpolated or parameter segment as `{}`. `via` is the matched endpoint's served
//!   `METHOD path`, and `via_ref` points at it; with no single match, `via` is the call's own
//!   method and normalized path, and the edge has no `via_ref`.
//! - **Wrappers.** A call whose method or path (after its base) is a bare name of a parameter of
//!   its function binds at each of that function's first [`BIND_SITES`] call sites in A that pass
//!   a literal or a template, or leave it out with a literal default: one route call each, citing
//!   the call site, then the wrapped call, and the call site's callers. A name no call site binds
//!   never joins. Binding goes one hop.
//! - **Queues.** A `code.queue_op` in A, with an `env_set` on A's service or a `code.url` in A
//!   whose host is a container the relation accepts: `queue_publish` or `queue_consume` by its
//!   `op`, `via` its key, resolved through a `code.const` of that name in its file, or else in a
//!   module of its repo that one of its file's first [`IMPORT_READS`] imports names (a dotted
//!   one by at most [`MODULE_SEGMENTS`] segments), when they agree on one value, or else its own
//!   text; a key that names a parameter of its function and
//!   no constant resolves joins nothing.
//! - **`db`.** A `code.db_open` in A. The first source whose host resolves to a container wins:
//!   an `env_read` in its file of a name compose sets on A; then an `env_read` in its file whose
//!   literal or constant default isn't overridden, or a URL literal in its file; then every
//!   `env_read` in A of a name compose sets on A. The `code.sql` hints of the open's file add
//!   their lines.
//!
//! An edge's evidence is, in order: its anchor hints, the constants that resolved them, the
//! target's wiring, `code.sql` lines, the endpoint's evidence, the `env_read` lines, and the call
//! sites of the anchor's function ([`Callers`]), each once, the first kept. Linked facts copy
//! evidence verified when their hints were extracted.
//!
//! **Bounded by its input.** Every pass reads an index keyed by file, container, name, host or
//! path: none compares every pair of hints. The containers a host reaches for a relation are
//! worked out once per link, and every join that reaches them shares that one set instead of
//! walking or copying it. What the sets of a variable compose sets on a container reach is worked
//! out once per container, variable and relation, keeping its first [`WIRES`] sets;
//! what a source of wiring resolves to, once per container, file and relation; and a literal host,
//! once per container and relation. A source reads only a variable's first [`WIRES`] reads, the
//! only ones a join can cite, so many reads of a variable compose sets many times, in one file or
//! in many, are never paired with each set. A call finds the endpoints of its method at its path
//! by method, never by walking every endpoint the path holds. Each join hint cites at most
//! [`WIRES`] wiring items for its target, the first by path and line, so an edge that several
//! hints join cites at most that many for each. An edge takes the `code.sql` lines of a file, and
//! its endpoint's evidence, once, however many of its hints share them. A line's many calls of
//! one function are one caller, indexed once, so a join never walks them. So what the link holds,
//! and the work it does, grow with the hints it reads, never with their product: a file's thousand
//! calls and thousand environment reads make at most a thousand edges, each citing a few lines.
//!
//! The one exception: a join whose wiring names hosts that reach different sets of containers
//! counts its candidates in the union of those sets, built once per link for each distinct
//! combination of sets. A union shares the largest host's set in place and reads each other
//! host's set it combines, so a combination costs the sizes of its hosts' sets but the largest.
//! Many containers that each combine a host they share with a host of their own cost a few
//! lookups each. Joins that combine two or more large sets in different combinations, each with
//! a host of its own, read every large set but the largest once per combination: that work grows
//! with the number of combinations times the sizes of their smaller sets. A call whose host comes
//! from its container's reads anywhere in its repo finds the targets that serve an endpoint it
//! matches once per set of targets, path and method, however many containers share the set,
//! reading the smaller of the set and the containers that serve its path; the wires that reach
//! one target come from its container's wires by host, built once per container. A wrapped call
//! reads at most [`BIND_SITES`] call sites, each call site's arguments split once, and an
//! imported constant at most [`IMPORT_READS`] imports of its file, each one lookup per module key.
//! A file's module keys, at most `2 * (MODULE_SEGMENTS + 1)` however deep it sits, are worked out
//! once, however many constants it holds.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use planroom_contract::{
    Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Hint, HintRef, Kind, Layer, Level, Node, NodeId,
    Provenance, Ref, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use super::bind::{self, BIND_SITES, Bound, Method, Target};
use super::imports::IMPORT_HINT;
use super::{
    CANDIDATE_SAMPLE, Callers, Linked, Linker, RelationRef, TestFiles, Unresolved, WorkspaceFacts, at, attr,
    extractor, image_repo, key, last_segment, nameable, namespace, normalize_path, rank, relative, tier_rank,
    url_host,
};
use crate::ids::{edge_id, node_id};
use crate::{ExtractError, KindRef};

pub const NAME: &str = "code/runtime_wiring";

pub const COMPOSE_SERVICE_HINT: &str = "code.compose_service";
pub const ENV_SET_HINT: &str = "code.env_set";
pub const ENV_READ_HINT: &str = "code.env_read";
pub const URL_HINT: &str = "code.url";
pub const ROUTE_SERVE_HINT: &str = "code.route_serve";
pub const ROUTE_CALL_HINT: &str = "code.route_call";
pub const QUEUE_OP_HINT: &str = "code.queue_op";
pub const DB_OPEN_HINT: &str = "code.db_open";
pub const SQL_HINT: &str = "code.sql";
pub const CONST_HINT: &str = "code.const";

/// Every hint kind this linker reads.
pub const CONSUMES: &[&str] = &[
    COMPOSE_SERVICE_HINT,
    ENV_SET_HINT,
    ENV_READ_HINT,
    URL_HINT,
    ROUTE_SERVE_HINT,
    ROUTE_CALL_HINT,
    QUEUE_OP_HINT,
    DB_OPEN_HINT,
    SQL_HINT,
    CONST_HINT,
    super::DEF_HINT,
    super::CALL_HINT,
    super::TEST_SCOPE_HINT,
    IMPORT_HINT,
];

/// The wiring items an edge cites at most for its target, the first by path and line.
pub const WIRES: usize = 3;
/// The call sites that bind one edge's route calls and add their lines, the first by path and
/// line.
pub const BOUND_ANCHORS: usize = 3;
/// The imports of a file an imported constant is looked up through, the first by line.
pub const IMPORT_READS: usize = 64;
/// The trailing segments of a module's path that a dotted import names at most: a dotted import
/// of more segments resolves no constant.
pub const MODULE_SEGMENTS: usize = 32;
/// The method key of a route served for every method.
const ANY_METHOD: &str = "ANY";

/// The kinds this linker emits, which the pack that declares it names in `options.kinds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// A compose service no image role classifies; every container takes its level and prefix.
    pub service: KindRef,
    /// A repo's node, which a joined service sits under.
    pub repo: KindRef,
    /// A route a container serves.
    pub endpoint: KindRef,
}

/// The relations this linker emits, which the pack names in `options.relations`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relations {
    pub http: RelationRef,
    pub queue_publish: RelationRef,
    pub queue_consume: RelationRef,
    pub db: RelationRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    kinds: Kinds,
    relations: Relations,
    #[serde(default)]
    languages: Vec<planroom_contract::LanguageDecl>,
}

pub struct RuntimeWiring;

impl Linker for RuntimeWiring {
    fn name(&self) -> &'static str {
        NAME
    }

    fn link(&self, ws: &WorkspaceFacts, opts: &Value) -> Result<Linked, ExtractError> {
        let opts: Options = serde_json::from_value(opts.clone())
            .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?;
        let tests = TestFiles::new(&opts.languages)?;
        Ok(Run::new(ws, &opts, &tests).link())
    }
}

/// A container as joins read it: the kind the merge gives it, and its aliases from every fact.
struct Container {
    kind: String,
    rank: u8,
    aliases: BTreeSet<String>,
}

/// A repo's containers, as a join hint in it reads them.
enum Holds {
    One(NodeId),
    /// Two or more: their first [`CANDIDATE_SAMPLE`] by ID, and how many.
    Many(Vec<NodeId>, usize),
}

/// A served route.
struct Endpoint {
    node: Node,
    seen: HashSet<Evidence>,
}

/// A container's endpoints at one normalized path, by the method they serve, so a call finds the
/// ones of its method without walking the rest.
#[derive(Default)]
struct AtPath {
    by_method: HashMap<String, Vec<NodeId>>,
    count: usize,
}

impl AtPath {
    /// The one endpoint a call of `method` reaches here: of that method or served for every
    /// method, or with no method named, any. Two or more that fit are no single match.
    fn single(&self, method: Option<&str>) -> Option<&NodeId> {
        let none: &[NodeId] = &[];
        let of = |m: &str| self.by_method.get(m).map_or(none, Vec::as_slice);
        match method {
            Some(m) if m != ANY_METHOD => match (of(m), of(ANY_METHOD)) {
                ([one], []) | ([], [one]) => Some(one),
                _ => None,
            },
            _ if self.count == 1 => self.by_method.values().flatten().next(),
            _ => None,
        }
    }

    /// Whether some endpoint here serves a call of `method`: one of that method or served for
    /// every method, or with no method named, any.
    fn fits(&self, method: Option<&str>) -> bool {
        match method {
            Some(m) => [m, ANY_METHOD].iter().any(|k| self.by_method.get(*k).is_some_and(|v| !v.is_empty())),
            None => self.count > 0,
        }
    }
}

/// The evidence slots of an edge, in the order the edge lists them.
const ANCHOR: usize = 0;
const CONSTANT: usize = 1;
const WIRING: usize = 2;
const SQL: usize = 3;
const VIA: usize = 4;
const ENV_READ: usize = 5;
const CALLER: usize = 6;
const SLOTS: usize = 7;

/// An edge being built: its ends, the hints it joined, and its evidence by slot, each item once
/// in its slot.
struct Draft {
    kind: String,
    from: NodeId,
    to: NodeId,
    via: Option<String>,
    via_ref: Option<NodeId>,
    hints: BTreeSet<(String, String)>,
    evidence: [Vec<Evidence>; SLOTS],
    seen: [HashSet<Evidence>; SLOTS],
    /// The files whose `code.sql` lines it cites, each taken once however many opens it holds.
    sql_files: HashSet<String>,
    /// Whether it cites its `via_ref` endpoint's evidence yet, taken once however many calls
    /// reach it.
    via_cited: bool,
    /// How many binding call sites it cites, at most [`BOUND_ANCHORS`].
    bound: usize,
}

impl Draft {
    fn cite(&mut self, slot: usize, evidence: &Evidence) {
        if self.seen[slot].insert(evidence.clone()) {
            self.evidence[slot].push(evidence.clone());
        }
    }

    /// Add `hint`: its evidence under `slot`, and its ID to the edge's provenance.
    fn add(&mut self, slot: usize, hint: &Hint) {
        for e in &hint.evidence {
            self.cite(slot, e);
        }
        self.hints.insert((hint.id.clone(), extractor(hint)));
    }

    /// Add each wire's constant, wiring and environment read.
    fn wires(&mut self, wires: &[Wire<'_>]) {
        for wire in wires {
            if let Some(c) = wire.constant {
                self.add(CONSTANT, c);
            }
            if let Some(w) = wire.wiring {
                self.add(WIRING, w);
            }
            if let Some(r) = wire.env_read {
                self.add(ENV_READ, r);
            }
        }
    }
}

/// One wiring item a join found: the host it names, and the hints that say so.
#[derive(Clone, Default)]
struct Wire<'a> {
    host: String,
    constant: Option<&'a Hint>,
    wiring: Option<&'a Hint>,
    env_read: Option<&'a Hint>,
}

impl Wire<'_> {
    /// Where it is cited: its wiring's place, or else its read's or its constant's.
    fn place(&self) -> (&str, u32, &str) {
        self.wiring.or(self.env_read).or(self.constant).map_or(("", 0, ""), place)
    }
}

/// The containers a host, or several, can reach for one relation, in ID order: worked out once per
/// link and shared by every join that reads them. A union keeps its largest host's set in place,
/// shared, and holds only what the others add to it, so a set many joins combine with one of their
/// own is never copied for each.
struct Targets {
    base: Rc<BTreeSet<NodeId>>,
    /// The containers the other hosts add, none of them in `base`.
    rest: BTreeSet<NodeId>,
}

impl Targets {
    fn of(set: BTreeSet<NodeId>) -> Targets {
        Targets { base: Rc::new(set), rest: BTreeSet::new() }
    }

    fn len(&self) -> usize {
        self.base.len() + self.rest.len()
    }

    fn contains(&self, id: &NodeId) -> bool {
        self.base.contains(id) || self.rest.contains(id)
    }

    /// In ID order: the two sorted halves, merged as they are read.
    fn iter(&self) -> impl Iterator<Item = &NodeId> {
        let mut base = self.base.iter().peekable();
        let mut rest = self.rest.iter().peekable();
        std::iter::from_fn(move || match (base.peek(), rest.peek()) {
            (Some(b), Some(r)) if r < b => rest.next(),
            (Some(_), _) => base.next(),
            (None, _) => rest.next(),
        })
    }
}

/// The candidates of one join: a shared set of targets, the joining container aside, read
/// without walking or copying the set.
#[derive(Clone)]
struct Candidates {
    targets: Rc<Targets>,
    from: NodeId,
}

impl Candidates {
    fn count(&self) -> usize {
        self.targets.len() - usize::from(self.targets.contains(&self.from))
    }

    /// In ID order.
    fn iter(&self) -> impl Iterator<Item = &NodeId> {
        self.targets.iter().filter(|b| **b != self.from)
    }
}

/// What a source of wiring resolves to for one relation: no target it accepts, one with the
/// first [`WIRES`] wires that reach it, or two or more.
#[derive(Clone)]
enum Resolution<'a> {
    None,
    One(NodeId, Vec<Wire<'a>>),
    Many(Candidates),
}

/// What compose sets one variable to on one container reaches for one relation: whether any of
/// its values names a container, the targets its values reach (the container aside), and its
/// first [`WIRES`] sets, by path and line, whose host reaches one.
struct Variable<'a> {
    reaches: bool,
    targets: Option<Rc<Targets>>,
    first: Vec<(&'a Hint, String)>,
}

/// The constants of one name at one module key: the first by path and line, and whether every
/// one gives its value.
struct ConstsAt<'a> {
    first: &'a Hint,
    agree: bool,
}

/// A container's wires by the host they name.
type ByHost<'a> = HashMap<String, Vec<Wire<'a>>>;

/// A source of wiring, for its memo: which join reads it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Source {
    /// The variables a call's file reads that compose sets on its container.
    CallReads,
    /// What compose sets on a container, and the URL literals in its repo.
    Queue,
    /// The three sources of a database open, in order.
    DbSet,
    DbLiteral,
    DbAnywhere,
}

/// One run over one active set.
struct Run<'a> {
    opts: &'a Options,
    repos: &'a BTreeSet<String>,
    /// Every hint, once, sorted by ID.
    hints: Vec<&'a Hint>,
    minted: BTreeMap<NodeId, Node>,
    minted_seen: HashMap<NodeId, HashSet<Evidence>>,
    /// Each compose file's services, by (file, service name), to the container minted for it.
    services: BTreeMap<(&'a str, &'a str), NodeId>,
    containers: BTreeMap<NodeId, Container>,
    holds: BTreeMap<String, Holds>,
    /// The containers that carry each `host:` alias.
    by_host: HashMap<String, Vec<NodeId>>,
    /// What each host reaches for each relation, by host and relation, worked out once.
    targets: HashMap<(String, String), Rc<Targets>>,
    /// The union of two or more distinct target sets, by the sets' addresses, worked out once.
    unions: HashMap<Vec<usize>, Rc<Targets>>,
    /// What each variable compose sets reaches, by container, variable and relation.
    variables: HashMap<(NodeId, &'a str, String), Rc<Variable<'a>>>,
    /// By file: constants by name, environment reads, URL literals and SQL, each in line order.
    consts: BTreeMap<(&'a str, &'a str), &'a Hint>,
    reads_in_file: BTreeMap<&'a str, Vec<&'a Hint>>,
    urls_in_file: BTreeMap<&'a str, Vec<&'a Hint>>,
    sql_in_file: BTreeMap<&'a str, Vec<&'a Hint>>,
    /// By repo: environment reads and URL literals, in (path, line) order.
    reads_in_repo: BTreeMap<&'a str, Vec<&'a Hint>>,
    urls_in_repo: BTreeMap<&'a str, Vec<&'a Hint>>,
    /// By container: what compose sets on it, all of it and by name.
    sets: BTreeMap<NodeId, Vec<&'a Hint>>,
    sets_by_name: BTreeMap<(NodeId, &'a str), Vec<&'a Hint>>,
    endpoints: BTreeMap<NodeId, Endpoint>,
    /// Each container's endpoints by normalized path, then by method.
    endpoints_at: BTreeMap<(NodeId, String), AtPath>,
    /// What each source resolves to, by source, container, file (empty for a whole repo's) and
    /// relation, and whether any of its wires named a container at all.
    memo: HashMap<(Source, NodeId, &'a str, String), (bool, Resolution<'a>)>,
    /// What a literal host resolves to, by container, host and relation.
    hosts: HashMap<(NodeId, String, String), Resolution<'a>>,
    /// The container a pack's facts place each file in, beside its repo's.
    file_containers: &'a BTreeMap<NodeId, NodeId>,
    /// The containers that serve some endpoint at each normalized path, in ID order.
    served: HashMap<String, Vec<NodeId>>,
    /// The containers of a set of targets that serve a call, by the set's address, normalized
    /// path and method.
    serving: HashMap<(usize, String, Option<String>), Rc<Targets>>,
    /// The wires of a container's repo by host, by container and relation.
    repo_wires: HashMap<(NodeId, String), Rc<ByHost<'a>>>,
    /// Each call site's arguments, split once however many hints bind through it.
    args: HashMap<&'a str, Rc<bind::Args>>,
    /// Each file's first [`IMPORT_READS`] imports, by line.
    imports_in_file: BTreeMap<&'a str, Vec<&'a Hint>>,
    /// Each module key of a file that holds a constant, by repo and key, as a number: worked out
    /// once per file, however many constants it holds. A key is `=` and the module's path without
    /// its extension, or the directory of an `index`, `__init__` or `mod` file, or `~` and any of
    /// their last [`MODULE_SEGMENTS`] runs of segments, which a dotted import's path ends with.
    modules: HashMap<(&'a str, String), usize>,
    /// Constants by their file's module: by module key's number and name.
    module_consts: HashMap<(usize, &'a str), ConstsAt<'a>>,
    /// What each file's imports resolve a constant name to, worked out once per file and name.
    imported: HashMap<(&'a str, &'a str), Option<&'a Hint>>,
    drafts: BTreeMap<EdgeId, Draft>,
    unresolved: BTreeMap<String, Unresolved>,
}

/// A hint's place, for sorting: its file and line, then its ID.
fn place(hint: &Hint) -> (&str, u32, &str) {
    let (path, line) = at(hint).unwrap_or_default();
    (path, line, hint.id.as_str())
}

fn of_kind<'a>(hints: &[&'a Hint], kind: &str) -> Vec<&'a Hint> {
    let mut out: Vec<&Hint> = hints.iter().copied().filter(|h| h.kind.0 == kind).collect();
    out.sort_by(|a, b| place(a).cmp(&place(b)));
    out
}

/// Whether `targets` hold a container other than `a`.
fn reach(targets: &Targets, a: &NodeId) -> bool {
    targets.len() > usize::from(targets.contains(a))
}

/// The name an `env_read` or `env_set` hint names.
fn variable(hint: &Hint) -> &str {
    attr(hint, "name").unwrap_or(&hint.key)
}

/// The first [`WIRES`] reads of each variable among `reads`, in their order. Every read of one
/// variable meets the same sets, and a join keeps its first [`WIRES`] wires by the set's place,
/// then in read order, so a later read is never cited: leaving it out keeps a file's many reads
/// of a variable compose sets many times from pairing each read with each set.
fn first_reads<'a, 'h>(reads: &'a [&'h Hint]) -> impl Iterator<Item = &'h Hint> + 'a {
    let mut seen: HashMap<&'h str, usize> = HashMap::new();
    reads.iter().copied().filter(move |r| {
        let n = seen.entry(variable(r)).or_default();
        *n += 1;
        *n <= WIRES
    })
}

impl<'a> Run<'a> {
    fn new(ws: &'a WorkspaceFacts, opts: &'a Options, tests: &TestFiles) -> Run<'a> {
        // A test file's hints are test code's, and so are a test module's file's and those on a
        // line a test scope of their file holds: none anchors or cites a join.
        let hints = super::runtime_code(&ws.hints, tests);
        let mut run = Run {
            opts,
            repos: &ws.repos,
            hints,
            minted: BTreeMap::new(),
            minted_seen: HashMap::new(),
            services: BTreeMap::new(),
            containers: BTreeMap::new(),
            holds: BTreeMap::new(),
            by_host: HashMap::new(),
            targets: HashMap::new(),
            unions: HashMap::new(),
            variables: HashMap::new(),
            consts: BTreeMap::new(),
            reads_in_file: BTreeMap::new(),
            urls_in_file: BTreeMap::new(),
            sql_in_file: BTreeMap::new(),
            reads_in_repo: BTreeMap::new(),
            urls_in_repo: BTreeMap::new(),
            sets: BTreeMap::new(),
            sets_by_name: BTreeMap::new(),
            endpoints: BTreeMap::new(),
            endpoints_at: BTreeMap::new(),
            memo: HashMap::new(),
            hosts: HashMap::new(),
            file_containers: &ws.file_containers,
            served: HashMap::new(),
            serving: HashMap::new(),
            repo_wires: HashMap::new(),
            args: HashMap::new(),
            imports_in_file: BTreeMap::new(),
            modules: HashMap::new(),
            module_consts: HashMap::new(),
            imported: HashMap::new(),
            drafts: BTreeMap::new(),
            unresolved: BTreeMap::new(),
        };
        run.mint();
        run.index_containers(&ws.nodes);
        run.index_hints();
        run
    }

    /// A container for each compose service.
    fn mint(&mut self) {
        let kinds = &self.opts.kinds;
        for hint in of_kind(&self.hints, COMPOSE_SERVICE_HINT) {
            let name = hint.key.as_str();
            if !nameable(name) {
                continue;
            }
            let build = attr(hint, "build").filter(|b| self.repos.contains(*b));
            let image = attr(hint, "image");
            let joined = build.or_else(|| image.map(image_repo).filter(|i| self.repos.contains(*i)));
            let ns = joined.unwrap_or("ws");
            let id = node_id(ns, &kinds.service.id_prefix, name);
            self.services.insert((hint.scope.0.as_str(), name), id.clone());
            let seen = self.minted_seen.entry(id.clone()).or_default();
            if let Some(kept) = self.minted.get_mut(&id) {
                // Two compose files that run one service: one container, citing both.
                for e in &hint.evidence {
                    if seen.insert(e.clone()) {
                        kept.evidence.push(e.clone());
                    }
                }
                if let Provenance::Linked { hints, .. } = &mut kept.provenance {
                    hints.push(HintRef { id: hint.id.clone(), extractor: extractor(hint) });
                }
                continue;
            }
            seen.extend(hint.evidence.iter().cloned());
            let mut props = BTreeMap::new();
            props.insert("role".to_owned(), Value::from(attr(hint, "role").unwrap_or("service")));
            if let Some(image) = image {
                props.insert("image".to_owned(), Value::from(image));
            }
            let node = Node {
                id: id.clone(),
                kind: Kind(attr(hint, "kind").unwrap_or(&kinds.service.kind).to_owned()),
                level: kinds.service.level,
                parent: joined.map(|r| node_id(r, &kinds.repo.id_prefix, r)),
                name: name.to_owned(),
                aliases: vec![format!("host:{name}"), format!("service:{name}")],
                props,
                layer: Layer::Actual,
                provenance: linked(&[hint]),
                evidence: hint.evidence.clone(),
            };
            self.minted.insert(id, node);
        }
    }

    /// Every Container-level node, minted or extracted, with the kind the merge gives it: the most
    /// specific fact's, and of two equally specific the least by name, whatever order they come in.
    fn index_containers(&mut self, nodes: &'a [Node]) {
        let all = nodes.iter().filter(|n| n.level == Level::Container).chain(self.minted.values());
        let mut containers: BTreeMap<NodeId, Container> = BTreeMap::new();
        for node in all {
            let r = rank(&node.provenance);
            let entry = containers.entry(node.id.clone()).or_insert_with(|| Container {
                kind: node.kind.0.clone(),
                rank: r,
                aliases: BTreeSet::new(),
            });
            if (r, std::cmp::Reverse(&node.kind.0)) > (entry.rank, std::cmp::Reverse(&entry.kind)) {
                entry.kind = node.kind.0.clone();
                entry.rank = r;
            }
            entry.aliases.extend(node.aliases.iter().cloned());
        }
        let mut by_repo: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
        for (id, container) in &containers {
            by_repo.entry(namespace(id).to_owned()).or_default().push(id.clone());
            for alias in &container.aliases {
                if let Some(host) = alias.strip_prefix("host:") {
                    // Each container's aliases are a set, and containers come in ID order.
                    self.by_host.entry(host.to_owned()).or_default().push(id.clone());
                }
            }
        }
        // A repo that exactly one compose service joins: that service's container is the repo's
        // for joins, and a container only the universal tier gives (a root manifest's) yields.
        let universal = tier_rank(Tier::Universal);
        self.holds = by_repo
            .into_iter()
            .map(|(repo, ids)| {
                let joined: Vec<&NodeId> = ids.iter().filter(|id| self.minted.contains_key(*id)).collect();
                let others = |one: &NodeId| {
                    ids.iter().filter(|id| *id != one && containers[*id].rank > universal).count()
                };
                let holds = match (&ids[..], &joined[..]) {
                    ([one], _) => Holds::One(one.clone()),
                    (_, [one]) if others(one) == 0 => Holds::One((*one).clone()),
                    _ => Holds::Many(ids.iter().take(CANDIDATE_SAMPLE).cloned().collect(), ids.len()),
                };
                (repo, holds)
            })
            .collect();
        self.containers = containers;
    }

    fn index_hints(&mut self) {
        let hints = self.hints.clone();
        // Constants come by path and line, so a file's come together: its module keys are worked
        // out when its first one comes.
        let mut file: Option<(&str, Vec<usize>)> = None;
        for hint in of_kind(&hints, CONST_HINT) {
            let scope = hint.scope.0.as_str();
            self.consts.entry((scope, hint.key.as_str())).or_insert(hint);
            if file.as_ref().is_none_or(|(s, _)| *s != scope) {
                let repo = namespace(&hint.scope);
                let mut ids = Vec::new();
                for module in module_keys(key(&hint.scope)) {
                    let next = self.modules.len();
                    ids.push(*self.modules.entry((repo, module)).or_insert(next));
                }
                file = Some((scope, ids));
            }
            let (_, ids) = file.as_ref().expect("just set");
            for &id in ids {
                // The first is kept, and a later one of another value makes the key give none.
                let at = self
                    .module_consts
                    .entry((id, hint.key.as_str()))
                    .or_insert(ConstsAt { first: hint, agree: true });
                at.agree &= at.first.value == hint.value;
            }
        }
        for hint in of_kind(&hints, IMPORT_HINT) {
            let file = self.imports_in_file.entry(hint.scope.0.as_str()).or_default();
            if file.len() < IMPORT_READS {
                file.push(hint);
            }
        }
        for hint in of_kind(&hints, ENV_READ_HINT) {
            self.reads_in_file.entry(hint.scope.0.as_str()).or_default().push(hint);
            self.reads_in_repo.entry(namespace(&hint.scope)).or_default().push(hint);
        }
        for hint in of_kind(&hints, URL_HINT) {
            self.urls_in_file.entry(hint.scope.0.as_str()).or_default().push(hint);
            self.urls_in_repo.entry(namespace(&hint.scope)).or_default().push(hint);
        }
        for hint in of_kind(&hints, SQL_HINT) {
            self.sql_in_file.entry(hint.scope.0.as_str()).or_default().push(hint);
        }
        for hint in of_kind(&hints, ENV_SET_HINT) {
            let Some(service) = attr(hint, "service") else { continue };
            let Some(container) = self.services.get(&(hint.scope.0.as_str(), service)).cloned() else {
                continue;
            };
            self.sets.entry(container.clone()).or_default().push(hint);
            self.sets_by_name.entry((container, variable(hint))).or_default().push(hint);
        }
    }

    /// The container the facts place the file `scope` in, when one does above the universal
    /// tier: a container only a root manifest gives yields to the repo's joined service, as it
    /// does for the repo's own files.
    fn placed(&self, scope: &NodeId) -> Option<&NodeId> {
        let universal = tier_rank(Tier::Universal);
        self.file_containers
            .get(scope)
            .filter(|c| self.containers.get(*c).is_some_and(|k| k.rank > universal))
    }

    /// The one container the file `scope` sits in: the one the facts place it in, or else its
    /// repo's, if it has exactly one.
    fn sole(&self, scope: &NodeId) -> Option<NodeId> {
        if let Some(placed) = self.placed(scope) {
            return Some(placed.clone());
        }
        match self.holds.get(namespace(scope)) {
            Some(Holds::One(id)) => Some(id.clone()),
            _ => None,
        }
    }

    /// The container a join hint sits in, or `None` when it joins nothing; a hint in a repo with
    /// two or more containers, none of which the facts place its file in, is left unresolved.
    fn anchor_container(&mut self, hint: &Hint, relation: &RelationRef) -> Option<NodeId> {
        if let Some(placed) = self.placed(&hint.scope) {
            return self.accepts(placed, &relation.from).then(|| placed.clone());
        }
        match self.holds.get(namespace(&hint.scope)) {
            Some(Holds::One(a)) if self.accepts(a, &relation.from) => Some(a.clone()),
            Some(Holds::Many(sample, count)) => {
                let unresolved = Unresolved {
                    linker: NAME.to_owned(),
                    hint: hint.id.clone(),
                    candidates: sample.clone(),
                    candidate_count: *count,
                };
                self.unresolved.entry(hint.id.clone()).or_insert(unresolved);
                None
            }
            _ => None,
        }
    }

    fn accepts(&self, id: &NodeId, kinds: &[String]) -> bool {
        self.containers.get(id).is_some_and(|c| kinds.contains(&c.kind))
    }

    /// A constant's or a literal's value: an identifier with a `code.const` of its name in the
    /// file `scope` takes the constant's value, and anything else is its own text.
    fn resolve(&self, scope: &str, text: &'a str) -> (String, Option<&'a Hint>) {
        let ident = !text.is_empty()
            && text.chars().all(|c| c.is_alphanumeric() || c == '_')
            && !text.starts_with(|c: char| c.is_ascii_digit());
        match self.consts.get(&(scope, text)).filter(|_| ident) {
            Some(c) => (c.value.clone().unwrap_or_default(), Some(*c)),
            None => (text.to_owned(), None),
        }
    }

    /// The wires a source that reads no variable gives for container `a` and the file `scope`.
    fn wires_of(&self, source: Source, a: &NodeId, scope: &'a str) -> Vec<Wire<'a>> {
        match source {
            Source::DbLiteral => {
                let reads = self.reads_in_file.get(scope).map(Vec::as_slice).unwrap_or_default();
                let mut wires = Vec::new();
                for read in reads {
                    if self.sets_by_name.contains_key(&(a.clone(), variable(read))) {
                        continue;
                    }
                    let Some(default) = attr(read, "default") else { continue };
                    let (value, constant) = self.resolve(scope, default);
                    if let Some(host) = url_host(&value) {
                        // A literal's own line is the wiring; a constant's line comes before it.
                        let (wiring, env_read) =
                            if constant.is_some() { (None, Some(*read)) } else { (Some(*read), None) };
                        wires.push(Wire { host: host.to_owned(), constant, wiring, env_read });
                    }
                }
                for url in self.urls_in_file.get(scope).into_iter().flatten() {
                    if let Some(host) = attr(url, "host") {
                        wires.push(Wire { host: host.to_owned(), wiring: Some(url), ..Wire::default() });
                    }
                }
                wires
            }
            Source::Queue => {
                let sets = self.sets.get(a).into_iter().flatten().filter_map(|set| {
                    let host = url_host(set.value.as_deref()?)?;
                    Some(Wire { host: host.to_owned(), wiring: Some(set), ..Wire::default() })
                });
                let urls = self.urls_in_repo.get(namespace(a)).into_iter().flatten().filter_map(|url| {
                    Some(Wire { host: attr(url, "host")?.to_owned(), wiring: Some(url), ..Wire::default() })
                });
                sets.chain(urls).collect()
            }
            Source::CallReads | Source::DbSet | Source::DbAnywhere => Vec::new(),
        }
    }

    /// What `host` reaches for `relation`: the containers carrying its alias that the relation's
    /// `to` kinds accept, worked out once per host and relation however many joins read it.
    fn targets(&mut self, host: &str, relation: &RelationRef) -> Rc<Targets> {
        let key = (host.to_owned(), relation.kind.clone());
        if let Some(found) = self.targets.get(&key) {
            return Rc::clone(found);
        }
        let found: BTreeSet<NodeId> = self
            .by_host
            .get(host)
            .into_iter()
            .flatten()
            .filter(|b| self.accepts(b, &relation.to))
            .cloned()
            .collect();
        let found = Rc::new(Targets::of(found));
        self.targets.insert(key, Rc::clone(&found));
        found
    }

    /// The union of `sets`: one distinct set as it is, shared, and two or more built once per
    /// link for each distinct combination of sets. The union shares the largest host's set of
    /// those it combines and reads only the others, so its work is the size of every host's set
    /// it combines but the largest.
    fn union(&mut self, mut sets: Vec<Rc<Targets>>) -> Option<Rc<Targets>> {
        sets.sort_by_key(|s| Rc::as_ptr(s).addr());
        sets.dedup_by(|x, y| Rc::ptr_eq(x, y));
        if sets.len() < 2 {
            return sets.pop();
        }
        // Every set lives in this run's maps until it ends, so no two share an address.
        let key: Vec<usize> = sets.iter().map(|s| Rc::as_ptr(s).addr()).collect();
        if let Some(found) = self.unions.get(&key) {
            return Some(Rc::clone(found));
        }
        // The set with the largest host's set; which of two equal ones changes no member.
        let largest = sets
            .iter()
            .enumerate()
            .max_by_key(|(i, s)| (s.base.len(), s.len(), std::cmp::Reverse(*i)))
            .map(|(i, _)| i)
            .expect("two or more sets");
        let kept = sets.swap_remove(largest);
        let mut rest = kept.rest.clone();
        for set in &sets {
            let base = (!Rc::ptr_eq(&set.base, &kept.base)).then_some(set.base.iter());
            for id in set.rest.iter().chain(base.into_iter().flatten()) {
                if !kept.base.contains(id) && !rest.contains(id) {
                    rest.insert(id.clone());
                }
            }
        }
        let union = Rc::new(Targets { base: Rc::clone(&kept.base), rest });
        self.unions.insert(key, Rc::clone(&union));
        Some(union)
    }

    /// What the targets `sets` give container `a` resolve to: none, one with the first [`WIRES`]
    /// of `wires` by path and line, or two or more, which share the sets.
    fn decide(&mut self, a: &NodeId, sets: Vec<Rc<Targets>>, mut wires: Vec<Wire<'a>>) -> Resolution<'a> {
        let Some(targets) = self.union(sets) else { return Resolution::None };
        let candidates = Candidates { targets, from: a.clone() };
        match candidates.count() {
            0 => Resolution::None,
            1 => {
                let b = candidates.iter().next().cloned().expect("one target");
                // Its first wires by path and line, whatever order the source lists them in.
                wires.sort_by(|x, y| x.place().cmp(&y.place()));
                wires.truncate(WIRES);
                Resolution::One(b, wires)
            }
            _ => Resolution::Many(candidates),
        }
    }

    /// What `wires` resolve to for `relation` from container `a`, and whether any wire named a
    /// container at all. Each distinct host is looked up once.
    fn resolution(
        &mut self,
        a: &NodeId,
        wires: Vec<Wire<'a>>,
        relation: &RelationRef,
    ) -> (bool, Resolution<'a>) {
        let mut reaches = false;
        // Whether each host reaches a target, `a` aside.
        let mut gives: HashMap<String, bool> = HashMap::new();
        let mut sets = Vec::new();
        for wire in &wires {
            if gives.contains_key(&wire.host) {
                continue;
            }
            reaches |= self.by_host.contains_key(&wire.host);
            let targets = self.targets(&wire.host, relation);
            let g = reach(&targets, a);
            if g {
                sets.push(targets);
            }
            gives.insert(wire.host.clone(), g);
        }
        let kept: Vec<Wire<'a>> = wires.into_iter().filter(|w| gives[&w.host]).collect();
        (reaches, self.decide(a, sets, kept))
    }

    /// What compose sets the variable `name` to on container `a` reaches for `relation`, worked
    /// out once per container, variable and relation however many files read it.
    fn variable(&mut self, a: &NodeId, name: &'a str, relation: &RelationRef) -> Rc<Variable<'a>> {
        let key = (a.clone(), name, relation.kind.clone());
        if let Some(found) = self.variables.get(&key) {
            return Rc::clone(found);
        }
        let sets: Vec<&'a Hint> = self.sets_by_name.get(&(a.clone(), name)).cloned().unwrap_or_default();
        let mut reaches = false;
        let mut gives: HashMap<&'a str, bool> = HashMap::new();
        let mut targets = Vec::new();
        let mut first = Vec::new();
        // The sets come by path and line.
        for set in sets {
            let Some(host) = set.value.as_deref().and_then(url_host) else { continue };
            let g = match gives.get(host) {
                Some(g) => *g,
                None => {
                    reaches |= self.by_host.contains_key(host);
                    let t = self.targets(host, relation);
                    let g = reach(&t, a);
                    if g {
                        targets.push(t);
                    }
                    gives.insert(host, g);
                    g
                }
            };
            if g && first.len() < WIRES {
                first.push((set, host.to_owned()));
            }
        }
        let found = Rc::new(Variable { reaches, targets: self.union(targets), first });
        self.variables.insert(key, Rc::clone(&found));
        found
    }

    /// What the variables `reads` read resolve to for `relation` on container `a`: each read
    /// among its variable's first [`WIRES`], with each of the variable's first [`WIRES`] sets
    /// that reach a target. A join keeps its first [`WIRES`] wires by the set's place, then in
    /// read order, so no later set or read could be cited.
    fn reads(&mut self, a: &NodeId, reads: &[&'a Hint], relation: &RelationRef) -> (bool, Resolution<'a>) {
        let mut reaches = false;
        let mut sets = Vec::new();
        let mut wires = Vec::new();
        for read in first_reads(reads) {
            let v = self.variable(a, variable(read), relation);
            reaches |= v.reaches;
            if let Some(t) = &v.targets {
                sets.push(Rc::clone(t));
            }
            for (set, host) in &v.first {
                wires.push(Wire {
                    host: host.clone(),
                    wiring: Some(*set),
                    env_read: Some(read),
                    constant: None,
                });
            }
        }
        (reaches, self.decide(a, sets, wires))
    }

    /// What a source resolves to, worked out once per source, container, file and relation.
    fn memo(
        &mut self,
        source: Source,
        a: &NodeId,
        scope: &'a str,
        relation: &RelationRef,
    ) -> (bool, Resolution<'a>) {
        let key = (source, a.clone(), scope, relation.kind.clone());
        if let Some(found) = self.memo.get(&key) {
            return found.clone();
        }
        let found = match source {
            Source::CallReads | Source::DbSet => {
                let reads = self.reads_in_file.get(scope).cloned().unwrap_or_default();
                self.reads(a, &reads, relation)
            }
            Source::DbAnywhere => {
                let reads = self.reads_in_repo.get(namespace(a)).cloned().unwrap_or_default();
                self.reads(a, &reads, relation)
            }
            Source::DbLiteral | Source::Queue => {
                let wires = self.wires_of(source, a, scope);
                self.resolution(a, wires, relation)
            }
        };
        self.memo.insert(key, found.clone());
        found
    }

    /// What the literal host `host` resolves to for `relation` from container `a`, worked out once
    /// per container, host and relation.
    fn host(&mut self, a: &NodeId, host: &str, relation: &RelationRef) -> Resolution<'a> {
        let key = (a.clone(), host.to_owned(), relation.kind.clone());
        if let Some(found) = self.hosts.get(&key) {
            return found.clone();
        }
        let wire = Wire { host: host.to_owned(), ..Wire::default() };
        let (_, found) = self.resolution(a, vec![wire], relation);
        self.hosts.insert(key, found.clone());
        found
    }

    /// The one target of a join, or `None` after recording two or more as unresolved.
    fn single(&mut self, hint: &Hint, resolution: Resolution<'a>) -> Option<(NodeId, Vec<Wire<'a>>)> {
        match resolution {
            Resolution::None => None,
            Resolution::One(b, wires) => Some((b, wires)),
            Resolution::Many(candidates) => {
                self.unresolved.entry(hint.id.clone()).or_insert_with(|| {
                    Unresolved::new(NAME, &hint.id, candidates.iter(), candidates.count())
                });
                None
            }
        }
    }

    fn link(mut self) -> Linked {
        let hints = self.hints.clone();
        let callers = Callers::new(hints.iter().copied(), |scope| self.sole(scope));
        self.endpoints(&hints);
        self.http(&hints, &callers);
        self.queues(&hints, &callers);
        self.dbs(&hints, &callers);
        self.finish()
    }

    /// One endpoint per container, method and path served.
    fn endpoints(&mut self, hints: &[&'a Hint]) {
        let kind = self.opts.kinds.endpoint.clone();
        for hint in of_kind(hints, ROUTE_SERVE_HINT) {
            let Some(container) = self.sole(&hint.scope) else { continue };
            let path = attr(hint, "path").unwrap_or(&hint.key);
            let method = attr(hint, "method").map_or_else(|| ANY_METHOD.to_owned(), str::to_ascii_uppercase);
            if !nameable(path) || !nameable(&method) {
                continue;
            }
            let key = format!("{method} {path}");
            let id = node_id(namespace(&container), &kind.id_prefix, &key);
            if !self.endpoints.contains_key(&id) {
                let mut props = BTreeMap::new();
                props.insert("method".to_owned(), Value::from(method.as_str()));
                props.insert("path".to_owned(), Value::from(path));
                let node = Node {
                    id: id.clone(),
                    kind: Kind(kind.kind.clone()),
                    level: kind.level,
                    parent: Some(container.clone()),
                    name: key,
                    aliases: vec![],
                    props,
                    layer: Layer::Actual,
                    provenance: linked(&[]),
                    evidence: vec![],
                };
                self.endpoints.insert(id.clone(), Endpoint { node, seen: HashSet::new() });
                let normalized = normalize_path(path);
                let at = self.endpoints_at.entry((container.clone(), normalized.clone())).or_default();
                if at.count == 0 {
                    self.served.entry(normalized).or_default().push(container);
                }
                at.by_method.entry(method).or_default().push(id.clone());
                at.count += 1;
            }
            let endpoint = self.endpoints.get_mut(&id).expect("just made");
            for e in &hint.evidence {
                if endpoint.seen.insert(e.clone()) {
                    endpoint.node.evidence.push(e.clone());
                }
            }
            if let Provenance::Linked { hints, .. } = &mut endpoint.node.provenance {
                hints.push(HintRef { id: hint.id.clone(), extractor: extractor(hint) });
            }
        }
        for servers in self.served.values_mut() {
            servers.sort_unstable();
        }
    }

    /// The endpoint of `b` a call of `method` to `path` reaches, when exactly one does.
    fn endpoint(&self, b: &NodeId, method: Option<&str>, path: &str) -> Option<&Endpoint> {
        let id = self.endpoints_at.get(&(b.clone(), path.to_owned()))?.single(method)?;
        self.endpoints.get(id)
    }

    fn http(&mut self, hints: &[&'a Hint], callers: &Callers<'a>) {
        let relation = self.opts.relations.http.clone();
        let mut anchors: Vec<&Hint> =
            hints.iter().copied().filter(|h| h.kind.0 == ROUTE_CALL_HINT || h.kind.0 == URL_HINT).collect();
        anchors.sort_by(|a, b| place(a).cmp(&place(b)));
        // The calls wrappers' call sites make, joined after the rest by their call site's path and
        // line, so the call sites an edge cites are its first by path and line.
        let mut bound: Vec<(NodeId, RouteCall<'a>)> = Vec::new();
        for hint in anchors {
            if hint.kind.0 == URL_HINT {
                let Some(host) = attr(hint, "host") else { continue };
                let path = normalize_path(attr(hint, "path").unwrap_or("/"));
                let method = attr(hint, "method").and_then(bind::verb);
                let Some(a) = self.anchor_container(hint, &relation) else { continue };
                let call = RouteCall { wrapped: hint, site: None, method, host: Some(host.to_owned()), path };
                self.call(&relation, &a, call, callers);
                continue;
            }
            let method = bind::method(attr(hint, "method"));
            let Some(target) = bind::target(attr(hint, "path").unwrap_or(&hint.key), attr(hint, "base"))
            else {
                continue;
            };
            let Some(a) = self.anchor_container(hint, &relation) else { continue };
            match (method, target) {
                (Method::Verb(m), Target::Path { host, path, .. }) => {
                    let call = RouteCall { wrapped: hint, site: None, method: Some(m), host, path };
                    self.call(&relation, &a, call, callers);
                }
                (Method::Unknown, _) => {}
                (method, target) => {
                    let calls = self.bound(hint, &a, &method, &target, callers);
                    bound.extend(calls.into_iter().map(|call| (a.clone(), call)));
                }
            }
        }
        bound.sort_by(|(_, x), (_, y)| {
            (x.site.map(place), place(x.wrapped)).cmp(&(y.site.map(place), place(y.wrapped)))
        });
        for (a, call) in bound {
            self.call(&relation, &a, call, callers);
        }
    }

    /// The route calls a wrapper's call sites make of `wrapped`, whose method or path is a bare
    /// name: each of the first [`BIND_SITES`] calls of its enclosing function in `a`, by path and
    /// line, that passes a literal or a template for each name, or leaves one out whose default
    /// is a literal. A name that is no parameter of the enclosing function, or an argument that is
    /// itself a name, binds nothing: binding goes one hop.
    fn bound(
        &mut self,
        wrapped: &'a Hint,
        a: &NodeId,
        method: &Method,
        target: &Target,
        callers: &Callers<'a>,
    ) -> Vec<RouteCall<'a>> {
        let Some((_, line)) = at(wrapped) else { return vec![] };
        let Some(def) = callers.enclosing(wrapped.scope.0.as_str(), line) else { return vec![] };
        let params = bind::params(def.value.as_deref().unwrap_or_default());
        let named = |n: &str| params.iter().any(|p| p.name == n);
        if matches!(method, Method::Name(m) if !named(m))
            || matches!(target, Target::Name { name, .. } if !named(name))
        {
            return vec![];
        }
        let mut out = Vec::new();
        for site in callers.calls_named(a, last_segment(&def.key)).take(BIND_SITES) {
            let args = self.args_of(site);
            let passed = |name: &str| match bind::bind(&params, &args, name) {
                Bound::Arg(text) | Bound::Default(text) => Some(text.to_owned()),
                Bound::Unknown => None,
            };
            let method = match method {
                Method::Verb(m) => m.clone(),
                Method::Name(n) => match passed(n).map(|t| bind::method(Some(&t))) {
                    Some(Method::Verb(m)) => m,
                    _ => continue,
                },
                Method::Unknown => continue,
            };
            let (host, path) = match target {
                Target::Path { host, path, .. } => (host.clone(), path.clone()),
                // After a base, the argument is the path: it starts with `/` or binds nothing.
                Target::Name { host, name, based: true } => {
                    let Some(path) = passed(name).and_then(|t| bind::canonical(&bind::pieces(&t))) else {
                        continue;
                    };
                    (host.clone(), path)
                }
                Target::Name { name, based: false, .. } => {
                    match passed(name).and_then(|t| bind::target(&t, None)) {
                        Some(Target::Path { host, path, .. }) => (host, path),
                        _ => continue,
                    }
                }
            };
            out.push(RouteCall { wrapped, site: Some(site), method: Some(method), host, path });
        }
        out
    }

    /// A call site's arguments, split once.
    fn args_of(&mut self, site: &'a Hint) -> Rc<bind::Args> {
        Rc::clone(
            self.args
                .entry(site.id.as_str())
                .or_insert_with(|| Rc::new(bind::args(site.value.as_deref().unwrap_or_default()))),
        )
    }

    /// Join one route call from container `a`: an `http` edge to the one container its host
    /// reaches, with `via` the endpoint it matches there, or else its own method and path.
    fn call(&mut self, relation: &RelationRef, a: &NodeId, call: RouteCall<'a>, callers: &Callers<'a>) {
        let path = normalize_path(&call.path);
        let method = call.method.as_deref();
        // A join the call site makes is the call site's to report.
        let report = call.site.unwrap_or(call.wrapped);
        let resolution = match &call.host {
            Some(host) => self.host(a, host, relation),
            None => self.call_host(a, call.wrapped.scope.0.as_str(), method, &path, relation),
        };
        let Some((b, wires)) = self.single(report, resolution) else { return };
        let endpoint = self.endpoint(&b, method, &path);
        let (via, via_ref) = match endpoint {
            Some(e) => (e.node.name.clone(), Some(e.node.id.clone())),
            None => (method.map_or_else(|| path.clone(), |m| format!("{m} {path}")), None),
        };
        let id = self.draft(&relation.kind, a, &b, Some(via), via_ref.clone());
        let draft = self.drafts.get_mut(&id).expect("just made");
        match call.site {
            // The call site, then the wrapped call; its callers are the call site's.
            Some(site) => {
                if draft.bound < BOUND_ANCHORS {
                    draft.bound += 1;
                    draft.add(ANCHOR, site);
                    for e in callers.sites(a, site) {
                        draft.cite(CALLER, e);
                    }
                }
                draft.add(ANCHOR, call.wrapped);
            }
            None => {
                draft.add(ANCHOR, call.wrapped);
                for e in callers.sites(a, call.wrapped) {
                    draft.cite(CALLER, e);
                }
            }
        }
        // The endpoint's evidence once an edge, however many calls reach it.
        if let Some(e) = via_ref.and_then(|v| self.endpoints.get(&v)).filter(|_| !draft.via_cited) {
            draft.via_cited = true;
            for item in &e.node.evidence {
                draft.cite(VIA, item);
            }
        }
        draft.wires(&wires);
    }

    /// The host of a call from `a` in the file `scope` whose base isn't a literal, as a database
    /// open finds its own, the first source whose wiring names a container: an environment read in
    /// the file of a variable compose sets on `a`; an environment read in the file whose literal or
    /// constant default compose doesn't override, or a URL literal in the file; then any read in
    /// `a`'s repo of a variable compose sets on `a`, among the containers that serve an endpoint
    /// the call matches, so a variable that reaches some other container never joins by
    /// elimination.
    fn call_host(
        &mut self,
        a: &NodeId,
        scope: &'a str,
        method: Option<&str>,
        path: &str,
        relation: &RelationRef,
    ) -> Resolution<'a> {
        for source in [Source::CallReads, Source::DbLiteral] {
            let (reaches, resolution) = self.memo(source, a, scope, relation);
            if reaches {
                return resolution;
            }
        }
        let (reaches, resolution) = self.memo(Source::DbAnywhere, a, "", relation);
        if !reaches {
            return Resolution::None;
        }
        match resolution {
            Resolution::None => Resolution::None,
            Resolution::One(b, wires) => {
                if self.serves(&b, method, path) {
                    Resolution::One(b, wires)
                } else {
                    Resolution::None
                }
            }
            Resolution::Many(candidates) => {
                let fit = self.serving(&candidates.targets, method, path);
                let fit = Candidates { targets: fit, from: a.clone() };
                match fit.count() {
                    0 => Resolution::None,
                    1 => {
                        let b = fit.iter().next().cloned().expect("one target");
                        let wires = self.reach_wires(a, &b, relation);
                        Resolution::One(b, wires)
                    }
                    _ => Resolution::Many(fit),
                }
            }
        }
    }

    /// The containers of `targets` that serve an endpoint a call of `method` to `path` matches:
    /// worked out once per set of targets, path and method, however many containers share the
    /// set, reading the smaller of the set and the containers that serve the path.
    fn serving(&mut self, targets: &Rc<Targets>, method: Option<&str>, path: &str) -> Rc<Targets> {
        // Every set lives in this run's maps until it ends, so no two share an address.
        let key = (Rc::as_ptr(targets).addr(), path.to_owned(), method.map(str::to_owned));
        if let Some(found) = self.serving.get(&key) {
            return Rc::clone(found);
        }
        let servers = self.served.get(path).map(Vec::as_slice).unwrap_or_default();
        let fit: BTreeSet<NodeId> = if servers.len() <= targets.len() {
            servers.iter().filter(|b| targets.contains(b) && self.serves(b, method, path)).cloned().collect()
        } else {
            targets.iter().filter(|b| self.serves(b, method, path)).cloned().collect()
        };
        let fit = Rc::new(Targets::of(fit));
        self.serving.insert(key, Rc::clone(&fit));
        fit
    }

    /// Whether `b` serves an endpoint a call of `method` to `path` matches.
    fn serves(&self, b: &NodeId, method: Option<&str>, path: &str) -> bool {
        self.endpoints_at.get(&(b.clone(), path.to_owned())).is_some_and(|at| at.fits(method))
    }

    /// The first [`WIRES`] wires by which the reads in `a`'s repo reach `b`, by path and line:
    /// those of the hosts `b` carries, from the wires of `a`'s repo by host, worked out once per
    /// container.
    fn reach_wires(&mut self, a: &NodeId, b: &NodeId, relation: &RelationRef) -> Vec<Wire<'a>> {
        let by_host = self.repo_wires(a, relation);
        let hosts = self.containers.get(b).map(|c| &c.aliases).into_iter().flatten();
        let mut wires: Vec<Wire<'a>> = hosts
            .filter_map(|alias| alias.strip_prefix("host:"))
            .filter_map(|host| by_host.get(host))
            .flatten()
            .cloned()
            .collect();
        wires.sort_by(|x, y| x.place().cmp(&y.place()));
        wires.truncate(WIRES);
        wires
    }

    /// The wires of `a`'s repo by the host they name: each read among its variable's first
    /// [`WIRES`], with each of that variable's first [`WIRES`] sets on `a` that reach a target,
    /// keeping each host's first [`WIRES`] by path and line. Worked out once per container and
    /// relation.
    fn repo_wires(&mut self, a: &NodeId, relation: &RelationRef) -> Rc<ByHost<'a>> {
        let key = (a.clone(), relation.kind.clone());
        if let Some(found) = self.repo_wires.get(&key) {
            return Rc::clone(found);
        }
        let reads = self.reads_in_repo.get(namespace(a)).cloned().unwrap_or_default();
        let mut by_host: ByHost<'a> = HashMap::new();
        for read in first_reads(&reads) {
            let v = self.variable(a, variable(read), relation);
            for (set, host) in &v.first {
                let wire =
                    Wire { host: host.clone(), wiring: Some(*set), env_read: Some(read), constant: None };
                by_host.entry(host.clone()).or_default().push(wire);
            }
        }
        for wires in by_host.values_mut() {
            wires.sort_by(|x, y| x.place().cmp(&y.place()));
            wires.truncate(WIRES);
        }
        let found = Rc::new(by_host);
        self.repo_wires.insert(key, Rc::clone(&found));
        found
    }

    /// The constant a file's imports resolve the name `text` to: one named its last segment in a
    /// module of the same repo that one of the file's first [`IMPORT_READS`] imports names, when
    /// all such constants agree on one value. An import resolves by a relative path from the file,
    /// or a dotted one the module's path ends with; one constant is read, never what it names, so
    /// no import cycle is followed.
    fn imported(&mut self, scope: &'a str, text: &'a str) -> Option<&'a Hint> {
        let name = last_segment(text);
        if !bind::ident(name) || !bind::name(text) {
            return None;
        }
        if let Some(found) = self.imported.get(&(scope, name)) {
            return *found;
        }
        let file = key_of_scope(scope);
        let repo = scope.split(':').next().unwrap_or_default();
        // Each module key is one lookup, whatever number of constants it holds.
        let mut first: Option<&'a Hint> = None;
        let mut agree = true;
        for import in self.imports_in_file.get(scope).into_iter().flatten() {
            for module in import_keys(import, file) {
                let Some(&id) = self.modules.get(&(repo, module)) else { continue };
                let Some(at) = self.module_consts.get(&(id, name)) else { continue };
                agree &= at.agree && first.is_none_or(|f| f.value == at.first.value);
                if first.is_none_or(|f| place(at.first) < place(f)) {
                    first = Some(at.first);
                }
            }
        }
        let one = first.filter(|c| agree && c.value.as_deref().is_some_and(|v| !v.is_empty()));
        self.imported.insert((scope, name), one);
        one
    }

    fn queues(&mut self, hints: &[&'a Hint], callers: &Callers<'a>) {
        for hint in of_kind(hints, QUEUE_OP_HINT) {
            let relation = match attr(hint, "op") {
                Some("publish") => self.opts.relations.queue_publish.clone(),
                Some("consume") => self.opts.relations.queue_consume.clone(),
                _ => continue,
            };
            let Some(a) = self.anchor_container(hint, &relation) else { continue };
            let scope = hint.scope.0.as_str();
            let raw = attr(hint, "key").unwrap_or(&hint.key);
            let (mut key, mut constant) = self.resolve(scope, raw);
            if constant.is_none() {
                if let Some(c) = self.imported(scope, raw) {
                    key = c.value.clone().unwrap_or_default();
                    constant = Some(c);
                } else if bind::ident(raw) && is_parameter(callers, hint, raw) {
                    // A parameter no constant resolves: binding a queue key waits.
                    continue;
                }
            }
            if key.is_empty() {
                continue;
            }
            let (_, resolution) = self.memo(Source::Queue, &a, "", &relation);
            let Some((b, wires)) = self.single(hint, resolution) else { continue };
            let id = self.draft(&relation.kind, &a, &b, Some(key), None);
            let draft = self.drafts.get_mut(&id).expect("just made");
            draft.add(ANCHOR, hint);
            if let Some(c) = constant {
                draft.add(CONSTANT, c);
            }
            draft.wires(&wires);
            for e in callers.sites(&a, hint) {
                draft.cite(CALLER, e);
            }
        }
    }

    fn dbs(&mut self, hints: &[&'a Hint], callers: &Callers<'a>) {
        let relation = self.opts.relations.db.clone();
        for hint in of_kind(hints, DB_OPEN_HINT) {
            let Some(a) = self.anchor_container(hint, &relation) else { continue };
            let scope = hint.scope.0.as_str();
            // The first source whose wires name a container wins.
            let mut chosen = Resolution::None;
            for (source, file) in
                [(Source::DbSet, scope), (Source::DbLiteral, scope), (Source::DbAnywhere, "")]
            {
                let (reaches, resolution) = self.memo(source, &a, file, &relation);
                if reaches {
                    chosen = resolution;
                    break;
                }
            }
            let Some((b, wires)) = self.single(hint, chosen) else { continue };
            let id = self.draft(&relation.kind, &a, &b, None, None);
            let draft = self.drafts.get_mut(&id).expect("just made");
            draft.add(ANCHOR, hint);
            draft.wires(&wires);
            // The file's SQL once an edge, however many opens it holds.
            if draft.sql_files.insert(scope.to_owned()) {
                for s in self.sql_in_file.get(scope).into_iter().flatten() {
                    draft.add(SQL, s);
                }
            }
            for e in callers.sites(&a, hint) {
                draft.cite(CALLER, e);
            }
        }
    }

    fn draft(
        &mut self,
        kind: &str,
        from: &NodeId,
        to: &NodeId,
        via: Option<String>,
        via_ref: Option<NodeId>,
    ) -> EdgeId {
        let id =
            edge_id(&Kind(kind.to_owned()), &Ref::Id(from.clone()), &Ref::Id(to.clone()), via.as_deref());
        self.drafts.entry(id.clone()).or_insert_with(|| Draft {
            kind: kind.to_owned(),
            from: from.clone(),
            to: to.clone(),
            via,
            via_ref,
            hints: BTreeSet::new(),
            evidence: Default::default(),
            seen: Default::default(),
            sql_files: HashSet::new(),
            via_cited: false,
            bound: 0,
        });
        id
    }

    fn finish(self) -> Linked {
        let mut nodes: Vec<Node> = self.minted.into_values().collect();
        nodes.extend(self.endpoints.into_values().map(|e| e.node));
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        let edges: Vec<Edge> = self
            .drafts
            .into_iter()
            .map(|(id, draft)| {
                let mut seen: HashSet<&Evidence> = HashSet::new();
                let evidence: Vec<Evidence> =
                    draft.evidence.iter().flatten().filter(|e| seen.insert(*e)).cloned().collect();
                Edge {
                    id,
                    kind: Kind(draft.kind),
                    from: Ref::Id(draft.from),
                    to: Ref::Id(draft.to),
                    via: draft.via,
                    via_ref: draft.via_ref.map(Ref::Id),
                    props: BTreeMap::new(),
                    evidence,
                    confidence: 1.0,
                    layer: Layer::Actual,
                    provenance: Provenance::Linked {
                        linker: NAME.into(),
                        hints: draft
                            .hints
                            .into_iter()
                            .map(|(id, extractor)| HintRef { id, extractor })
                            .collect(),
                    },
                }
            })
            .collect();
        Linked {
            batch: FactBatch {
                format: FACTS_FORMAT.into(),
                extractor: NAME.into(),
                tier: Tier::Linked,
                nodes,
                edges,
                hints: vec![],
                partial: vec![],
            },
            unresolved: self.unresolved.into_values().collect(),
        }
    }
}

/// `Linked` provenance naming `hints`.
fn linked(hints: &[&Hint]) -> Provenance {
    Provenance::Linked {
        linker: NAME.into(),
        hints: hints.iter().map(|h| HintRef { id: h.id.clone(), extractor: extractor(h) }).collect(),
    }
}

/// One route call to join: the hint that makes it, the call site that bound it, if any, and its
/// method, literal host and canonical path.
struct RouteCall<'a> {
    wrapped: &'a Hint,
    site: Option<&'a Hint>,
    method: Option<String>,
    host: Option<String>,
    path: String,
}

/// Whether `name` is a parameter of the function that holds `hint`.
fn is_parameter(callers: &Callers<'_>, hint: &Hint, name: &str) -> bool {
    let Some((_, line)) = at(hint) else { return false };
    callers.enclosing(hint.scope.0.as_str(), line).is_some_and(|def| {
        bind::params(def.value.as_deref().unwrap_or_default()).iter().any(|p| p.name == name)
    })
}

/// The repo-relative path a file's ID names.
fn key_of_scope(scope: &str) -> &str {
    scope.splitn(3, ':').nth(2).unwrap_or_default()
}

/// `path` without its last segment's extension.
fn stem(path: &str) -> &str {
    let name_at = path.rfind('/').map_or(0, |i| i + 1);
    match path[name_at..].rfind('.') {
        Some(dot) if dot > 0 => &path[..name_at + dot],
        _ => path,
    }
}

/// The module keys a constant in the file at `path` is imported by: `=` and the module's path
/// without its extension, and for an `index`, `__init__` or `mod` file its directory's; and `~`
/// and each of their trailing runs of at most [`MODULE_SEGMENTS`] segments, which a dotted
/// import's path ends with. So a file has at most `2 * (MODULE_SEGMENTS + 1)` keys, however deep
/// it sits.
fn module_keys(path: &str) -> Vec<String> {
    let module = stem(path);
    let mut modules = vec![module];
    let (dir, name) = module.rsplit_once('/').unwrap_or(("", module));
    if matches!(name, "index" | "__init__" | "mod") && !dir.is_empty() {
        modules.push(dir);
    }
    let mut keys = Vec::new();
    for module in modules {
        keys.push(format!("={module}"));
        let mut rest = module;
        let mut segments = module.matches('/').count() + 1;
        while segments > MODULE_SEGMENTS {
            rest = rest.split_once('/').map_or(rest, |(_, tail)| tail);
            segments -= 1;
        }
        loop {
            keys.push(format!("~{rest}"));
            match rest.split_once('/') {
                Some((_, tail)) => rest = tail,
                None => break,
            }
        }
    }
    keys
}

/// The module keys an import in the file at `file` names: a relative specifier resolved from the
/// file's directory (`./keys.js`, Python's `.keys`), or a dotted one (`worker.keys`) as the end of
/// a module's path. Any other names no module of its repo in M1.
fn import_keys(import: &Hint, file: &str) -> Vec<String> {
    let spec = import.key.as_str();
    let dir = file.rsplit_once('/').map_or("", |(d, _)| d);
    match attr(import, "separator").filter(|s| !s.is_empty()) {
        Some(separator) => {
            let dots = spec.chars().take_while(|c| *c == '.').count();
            let rest = spec[dots..].replace(separator, "/");
            if dots == 0 {
                let first = rest.split('/').next().unwrap_or_default();
                let deep = rest.matches('/').count() >= MODULE_SEGMENTS;
                return if rest.is_empty() || deep || matches!(first, "crate" | "self" | "super") {
                    vec![]
                } else {
                    vec![format!("~{rest}")]
                };
            }
            let up = "../".repeat(dots - 1);
            let joined =
                if rest.is_empty() { up.trim_end_matches('/').to_owned() } else { format!("{up}{rest}") };
            let target = if joined.is_empty() { Some(dir.to_owned()) } else { relative(dir, &joined) };
            target.into_iter().filter(|t| !t.is_empty()).map(|t| format!("={t}")).collect()
        }
        None if spec.starts_with("./") || spec.starts_with("../") => {
            relative(dir, spec).map(|t| vec![format!("={}", stem(&t))]).unwrap_or_default()
        }
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn hint(kind: &str, scope: &str, line: u32, key: &str, attrs: &[(&str, &str)]) -> Hint {
        let path = format!("{}/{}", namespace(&NodeId(scope.into())), key_of_scope(scope));
        Hint {
            id: format!("{:0>12}", format!("{line}{}", key.len())),
            kind: Kind(kind.into()),
            scope: NodeId(scope.into()),
            key: key.into(),
            value: None,
            attrs: attrs.iter().map(|(k, v)| ((*k).to_owned(), Value::from(*v))).collect(),
            evidence: vec![Evidence::Line { path, line, contains: None, commit: None, branch: None }],
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Pattern },
        }
    }

    fn container(id: &str, kind: &str, tier: Tier) -> Node {
        let name = id.rsplit(':').next().unwrap();
        Node {
            id: NodeId(id.into()),
            kind: Kind(kind.into()),
            level: Level::Container,
            parent: None,
            name: name.into(),
            aliases: vec![format!("host:{name}")],
            props: BTreeMap::new(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier },
            evidence: vec![],
        }
    }

    /// A script a pack places in a container of its own, in a repo that holds two: its URL hints
    /// join from that container, as the file's facts place it, and another file of the repo,
    /// which no fact places, stays unresolved between the two.
    #[test]
    fn a_url_hint_joins_from_the_container_its_file_sits_in() {
        let kind = |kind: &str, prefix: &str, level: &str| json!({"kind": kind, "id_prefix": prefix, "level": level});
        let from = json!(["code.service", "code.library", "code.tooling"]);
        let opts = json!({
            "kinds": {"service": kind("code.service", "container", "container"),
                      "repo": kind("code.repo", "repo", "system"),
                      "endpoint": kind("code.endpoint", "endpoint", "code")},
            "relations": {"http": {"kind": "code.http", "from": from, "to": ["code.service"]},
                          "queue_publish": {"kind": "code.queue_publish", "from": from, "to": ["code.queue"]},
                          "queue_consume": {"kind": "code.queue_consume", "from": from, "to": ["code.queue"]},
                          "db": {"kind": "code.db", "from": from, "to": ["code.datastore"]}},
        });
        let script = "platform:file:tools/healthcheck.rb";
        let url = |line: u32, host: &str| {
            let attrs = [("scheme", "http"), ("host", host), ("path", "/healthz"), ("method", "GET")];
            hint(URL_HINT, script, line, host, &attrs)
        };
        let other = [("host", "orders"), ("path", "/x")];
        let ws = WorkspaceFacts {
            repos: ["platform", "orders", "storefront"].map(str::to_owned).into(),
            nodes: vec![
                container("platform:container:platform", "code.tooling", Tier::Config),
                container("platform:container:healthcheck", "code.tooling", Tier::Pattern),
                container("orders:container:orders", "code.service", Tier::Config),
                container("storefront:container:storefront", "code.service", Tier::Config),
            ],
            edges: vec![],
            hints: vec![
                url(15, "orders"),
                url(16, "storefront"),
                hint(URL_HINT, "platform:file:tools/other.rb", 3, "orders", &other),
            ],
            file_containers: [(NodeId(script.into()), NodeId("platform:container:healthcheck".into()))]
                .into(),
        };
        let linked = RuntimeWiring.link(&ws, &opts).unwrap();
        let id = |r: &Ref| match r {
            Ref::Id(id) => id.0.clone(),
            Ref::Alias(a) => a.clone(),
        };
        let joined: BTreeSet<(String, String, Option<String>)> =
            linked.batch.edges.iter().map(|e| (id(&e.from), id(&e.to), e.via.clone())).collect();
        let want = |to: &str| {
            ("platform:container:healthcheck".to_owned(), to.to_owned(), Some("GET /healthz".to_owned()))
        };
        assert_eq!(
            joined,
            BTreeSet::from([want("orders:container:orders"), want("storefront:container:storefront")])
        );
        let [unresolved] = &linked.unresolved[..] else { panic!("{:?}", linked.unresolved) };
        assert_eq!(unresolved.candidate_count, 2, "the file no fact places");
    }

    #[test]
    fn imports_name_the_modules_of_their_repo() {
        let import = |spec: &str, separator: Option<&str>| {
            let attrs: Vec<(&str, &str)> = separator.map(|s| ("separator", s)).into_iter().collect();
            hint(IMPORT_HINT, "w:file:pkg/app/main.py", 1, spec, &attrs)
        };
        let file = "pkg/app/main.py";
        let none: Vec<String> = vec![];
        assert_eq!(import_keys(&import(".keys", Some(".")), file), ["=pkg/app/keys"]);
        assert_eq!(import_keys(&import("..shared.keys", Some(".")), file), ["=pkg/shared/keys"]);
        assert_eq!(import_keys(&import(".", Some(".")), file), ["=pkg/app"]);
        assert_eq!(import_keys(&import("worker.keys", Some(".")), file), ["~worker/keys"]);
        assert_eq!(import_keys(&import("crate::keys", Some("::")), file), none);
        assert_eq!(import_keys(&import("./keys.js", None), file), ["=pkg/app/keys"]);
        assert_eq!(import_keys(&import("../../../out", None), file), none, "leaves the repo");
        assert_eq!(import_keys(&import("express", None), file), none);
        assert_eq!(
            module_keys("worker/worker/keys.py"),
            ["=worker/worker/keys", "~worker/worker/keys", "~worker/keys", "~keys"]
        );
        assert!(module_keys("src/keys/index.ts").contains(&"=src/keys".to_owned()));
        // However deep a module sits, a dotted import reads at most its last segments.
        let deep = format!("{}keys.py", "a/".repeat(100));
        let keys = module_keys(&deep);
        assert_eq!(keys.len(), MODULE_SEGMENTS + 1);
        assert_eq!(keys[1], format!("~{}keys", "a/".repeat(MODULE_SEGMENTS - 1)));
        let dotted = format!("{}keys", "a.".repeat(MODULE_SEGMENTS));
        assert_eq!(import_keys(&import(&dotted, Some(".")), file), none);
        let dotted = format!("{}keys", "a.".repeat(MODULE_SEGMENTS - 1));
        assert_eq!(import_keys(&import(&dotted, Some(".")), file).len(), 1);
    }
}
