//! Resolution, the fold's second step (architecture.md, "The fold"; "IDs and aliases"; "Levels
//! and roll-ups"): one node per ID, merged by tier (DR-42), its level taken from the loaded packs'
//! vocabulary, its parent attached, and every edge end resolved by ID or by a unique alias.
//!
//! - A node's parent is the most specific fact's parent that resolves to a node. When it doesn't,
//!   the node falls back to the parent the next fact names, in tier order; with none that
//!   resolves, the node sits at the top.
//! - An edge end resolves by its ID, or by an alias exactly one node holds. An ambiguous or
//!   unknown alias resolves to nothing and is never guessed: the edge goes to `unresolved`, for
//!   the caller to report as `unresolved_ref`.
//! - Edges of one ID merge by tier, as nodes do.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, BuildHasherDefault, Hasher};

use planroom_contract::{Edge, Kind, Level, Node, NodeId, Ref, Vocabulary};

use crate::facts::{self, Merged};
use crate::model::Facts;

/// The facts, resolved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolved {
    /// One per ID, sorted by ID; every `parent` names one of them.
    pub nodes: Vec<Node>,
    /// One per ID, in the order each ID first appears, with both ends (and a `via_ref` that
    /// resolves) as node IDs.
    pub edges: Vec<Edge>,
    /// The edges, as extracted, with an end that resolves to no node or to several, in the order
    /// each first appears.
    pub unresolved: Vec<Edge>,
}

/// Resolve `facts` against `vocabulary`.
pub fn resolve(facts: &Facts, vocabulary: &Vocabulary) -> Resolved {
    resolve_placed(facts, vocabulary).0
}

/// [`resolve`], with where each resolved node sits by ID, which the fold keeps.
pub(crate) fn resolve_placed(facts: &Facts, vocabulary: &Vocabulary) -> (Resolved, Places) {
    let levels: HashMap<&Kind, Level> = vocabulary.kinds.iter().rev().map(|k| (&k.kind, k.level)).collect();
    // Each fact is copied once, into its merged node or edge.
    let sorted = facts::sorted_by_id(&facts.nodes);
    let groups: Vec<&[&Node]> = sorted.chunk_by(|a, b| a.id == b.id).collect();
    // One group per ID, sorted by ID, so a parent resolves by a binary search. Siblings sit side
    // by side in ID order, so the parent the last node looked up answers most nodes again.
    let mut last: Option<&NodeId> = None;
    let mut held = |id: &NodeId| -> bool {
        match last {
            Some(parent) if parent == id => true,
            _ => {
                match groups.binary_search_by(|g| g[0].id.cmp(id)) {
                    Ok(i) => {
                        // The ID as the facts hold it, so the cache borrows nothing else.
                        last = Some(&groups[i][0].id);
                        true
                    }
                    Err(_) => false,
                }
            }
        }
    };
    let mut nodes: Vec<Node> = Vec::with_capacity(groups.len());
    for group in &groups {
        let mut node = match group {
            // A node one fact names has that fact's parent as its only candidate.
            [one] => {
                let mut node = (*one).clone();
                if one.parent.as_ref().is_some_and(|p| *p == one.id || !held(p)) {
                    node.parent = None;
                }
                node
            }
            _ => {
                let Merged { mut node, parents } = facts::merge_borrowed(group);
                node.parent = parents.into_iter().find(|p| *p != node.id && held(p));
                node
            }
        };
        if let Some(&level) = levels.get(&node.kind) {
            node.level = level;
        }
        nodes.push(node);
    }
    let places = Places::of(&nodes);
    let aliases = by_alias(&nodes);
    // An end by ID finds its node by its place, one by alias as [`Index::get`] does.
    let index = |end: &Ref| -> Option<&Node> {
        match end {
            Ref::Id(id) => places.find(&nodes, id).map(|i| &nodes[i]),
            Ref::Alias(alias) => aliases.get(alias.as_str()).copied().flatten(),
        }
    };
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    for edge in facts::merge_edges_borrowed(&facts.edges) {
        let (Some(from), Some(to)) = (index(&edge.from), index(&edge.to)) else {
            unresolved.push(edge);
            continue;
        };
        // An end that names its node by ID already is kept as it is.
        let end = |end: Ref, node: &Node| match end {
            Ref::Id(id) if id == node.id => Ref::Id(id),
            _ => Ref::Id(node.id.clone()),
        };
        let (from, to) = (end(edge.from, from), end(edge.to, to));
        let via_ref = edge.via_ref.map(|r| match index(&r) {
            Some(node) => end(r, node),
            None => r,
        });
        edges.push(Edge { from, to, via_ref, ..edge });
    }
    (Resolved { nodes, edges, unresolved }, places)
}

/// Where each node of a slice sorted by ID sits, by the hash of its ID. The hash is std's
/// SipHash with keys drawn at random for each table, so no ID a repo chooses can make lookups
/// collide; an ID whose hash another one's already took is found by a binary search instead.
#[derive(Clone)]
pub(crate) struct Places {
    keys: RandomState,
    by_hash: HashMap<u64, usize, BuildHasherDefault<Keyed>>,
}

impl Places {
    /// The places of `nodes`, which are sorted by ID.
    pub(crate) fn of(nodes: &[Node]) -> Places {
        let keys = RandomState::new();
        let mut by_hash = HashMap::with_capacity_and_hasher(nodes.len(), BuildHasherDefault::default());
        for (i, node) in nodes.iter().enumerate() {
            by_hash.entry(hash(&keys, &node.id)).or_insert(i);
        }
        Places { keys, by_hash }
    }

    /// The place in `nodes`, the slice these are the places of, of the node with ID `id`.
    pub(crate) fn find(&self, nodes: &[Node], id: &NodeId) -> Option<usize> {
        let at = *self.by_hash.get(&hash(&self.keys, id))?;
        if nodes[at].id == *id {
            Some(at)
        } else {
            // Another ID's hash is this one's: rare, and found all the same.
            nodes.binary_search_by(|n| n.id.cmp(id)).ok()
        }
    }
}

/// The hasher of [`Places`]' table. Its keys are SipHash values under keys drawn at random, which
/// no ID a repo chooses can steer, so it passes each one through rather than hash it again.
#[derive(Default)]
pub(crate) struct Keyed(u64);

impl Hasher for Keyed {
    fn write(&mut self, bytes: &[u8]) {
        // The table's keys arrive through `write_u64`; this keeps any other input whole.
        for &b in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(b);
        }
    }

    fn write_u64(&mut self, hash: u64) {
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// The hash of `id` under `keys`: its bytes, in one write.
fn hash(keys: &RandomState, id: &NodeId) -> u64 {
    let mut hasher = keys.build_hasher();
    hasher.write(id.0.as_bytes());
    hasher.finish()
}

impl std::fmt::Debug for Places {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Places").finish_non_exhaustive()
    }
}

/// The nodes by alias: an alias two nodes hold is ambiguous and finds nothing.
fn by_alias(nodes: &[Node]) -> HashMap<&str, Option<&Node>> {
    let mut by_alias: HashMap<&str, Option<&Node>> = HashMap::new();
    for node in nodes {
        for alias in &node.aliases {
            by_alias
                .entry(alias.as_str())
                .and_modify(|held| {
                    if held.is_some_and(|h| h.id != node.id) {
                        *held = None;
                    }
                })
                .or_insert(Some(node));
        }
    }
    by_alias
}

/// The nodes by ID and by alias. An alias two nodes hold is ambiguous and finds nothing.
#[derive(Debug, Clone)]
pub struct Index<'a> {
    by_id: HashMap<&'a str, &'a Node>,
    by_alias: HashMap<&'a str, Option<&'a Node>>,
}

impl<'a> Index<'a> {
    pub fn new(nodes: &'a [Node]) -> Self {
        let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
        Index { by_id, by_alias: by_alias(nodes) }
    }

    /// The node with this ID.
    pub fn node(&self, id: &str) -> Option<&'a Node> {
        self.by_id.get(id).copied()
    }

    /// The node a reference names: by ID, or by an alias exactly one node holds.
    pub fn get(&self, end: &Ref) -> Option<&'a Node> {
        match end {
            Ref::Id(id) => self.node(&id.0),
            Ref::Alias(alias) => self.by_alias.get(alias.as_str()).copied().flatten(),
        }
    }

    /// The node and its ancestors, nearest first, up to the first parent that doesn't resolve.
    /// A parent cycle, which only bad facts or an older store can hold, ends the chain before the
    /// first node it would repeat.
    pub fn chain(&self, node: &'a Node) -> Vec<&'a Node> {
        let mut chain = vec![node];
        let mut at = node;
        while let Some(parent) = at.parent.as_ref().and_then(|p| self.node(&p.0)) {
            if chain.iter().any(|n| n.id == parent.id) {
                break;
            }
            chain.push(parent);
            at = parent;
        }
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{EdgeId, KindVocab, Layer, Provenance, Tier};

    fn node(id: &str, tier: Tier, name: &str) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("k.x".into()),
            level: Level::Code,
            parent: None,
            name: name.into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier },
            evidence: vec![],
        }
    }

    fn edge(id: &str, from: Ref, to: Ref) -> Edge {
        Edge {
            id: EdgeId(id.into()),
            kind: Kind("k.y".into()),
            from,
            to,
            via: None,
            via_ref: None,
            props: Default::default(),
            evidence: vec![],
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        }
    }

    #[test]
    fn resolution_merges_nodes_by_tier_as_fact_merging_does() {
        let nodes = vec![node("r:x:b", Tier::Universal, "plain"), node("r:x:b", Tier::Config, "config")];
        let facts = Facts { nodes: nodes.clone(), ..Facts::default() };
        let resolved = resolve(&facts, &Vocabulary::default());
        assert_eq!(resolved.nodes, facts::merge_nodes(nodes));
        assert_eq!(resolved.nodes[0].name, "config");
    }

    #[test]
    fn a_kind_the_vocabulary_declares_takes_its_level() {
        let facts = Facts { nodes: vec![node("r:x:a", Tier::Universal, "a")], ..Facts::default() };
        let vocabulary = Vocabulary {
            kinds: vec![KindVocab {
                kind: Kind("k.x".into()),
                level: Level::Component,
                id_prefix: "x".into(),
                label: "X".into(),
                props: serde_json::Value::Null,
                tone: None,
            }],
            ..Vocabulary::default()
        };
        assert_eq!(resolve(&facts, &vocabulary).nodes[0].level, Level::Component);
        assert_eq!(resolve(&facts, &Vocabulary::default()).nodes[0].level, Level::Code);
    }

    #[test]
    fn an_end_resolves_by_id_or_a_unique_alias_and_never_by_an_ambiguous_one() {
        let mut a = node("r:x:a", Tier::Universal, "a");
        a.aliases = vec!["host:a".into(), "shared".into()];
        let mut b = node("r:x:b", Tier::Universal, "b");
        b.aliases = vec!["shared".into()];
        let id = |s: &str| Ref::Id(NodeId(s.into()));
        let edges = vec![
            edge("e:000000000001", id("r:x:b"), Ref::Alias("host:a".into())),
            edge("e:000000000002", id("r:x:b"), Ref::Alias("shared".into())),
            edge("e:000000000003", id("r:x:b"), id("r:x:gone")),
        ];
        let resolved = resolve(&Facts { nodes: vec![a, b], edges, hints: vec![] }, &Vocabulary::default());
        assert_eq!(resolved.edges.len(), 1);
        assert_eq!(resolved.edges[0].to, id("r:x:a"));
        let unresolved: Vec<&str> = resolved.unresolved.iter().map(|e| e.id.0.as_str()).collect();
        assert_eq!(unresolved, ["e:000000000002", "e:000000000003"]);
    }

    #[test]
    fn a_chain_stops_before_a_parent_cycle_repeats() {
        let mut a = node("r:x:a", Tier::Universal, "a");
        a.parent = Some(NodeId("r:x:b".into()));
        let mut b = node("r:x:b", Tier::Universal, "b");
        b.parent = Some(NodeId("r:x:a".into()));
        let nodes = vec![a, b];
        let index = Index::new(&nodes);
        let chain: Vec<&str> = index.chain(&nodes[0]).iter().map(|n| n.id.0.as_str()).collect();
        assert_eq!(chain, ["r:x:a", "r:x:b"]);
    }

    #[test]
    fn a_node_whose_hash_another_took_is_found_all_the_same() {
        let nodes: Vec<Node> = (0..40).map(|i| node(&format!("r:x:{i:02}"), Tier::Universal, "n")).collect();
        let mut places = Places::of(&nodes);
        for i in 0..nodes.len() {
            assert_eq!(places.find(&nodes, &nodes[i].id), Some(i));
        }
        // Every hash taken by the first node, as if each ID shared its hash.
        for place in places.by_hash.values_mut() {
            *place = 0;
        }
        for i in 0..nodes.len() {
            assert_eq!(places.find(&nodes, &nodes[i].id), Some(i));
        }
        assert_eq!(places.find(&nodes, &NodeId("r:x:none".into())), None);
        // Two tables draw their own keys, so no one ID list collides in both.
        let other = Places::of(&nodes);
        assert_ne!(hash(&places.keys, &nodes[0].id), hash(&other.keys, &nodes[0].id));
    }

    use proptest::prelude::*;
    use proptest::test_runner::Config;

    const TIERS: [Tier; 3] = [Tier::Universal, Tier::Pattern, Tier::Syntactic];

    /// A fact for `(key, tier, parent, alias)`, named after its place in the input.
    fn fact(i: usize, (key, tier, parent, alias): (u8, usize, Option<u8>, Option<u8>)) -> Node {
        let mut n = node(&format!("r:x:{key}"), TIERS[tier], &format!("f{i}"));
        n.parent = parent.map(|p| NodeId(format!("r:x:{p}")));
        n.aliases = alias.map(|a| format!("al{a}")).into_iter().collect();
        n.props = [(format!("p{i}"), serde_json::json!(i))].into();
        n
    }

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        /// Resolving borrowed facts gives what merging a copy of them gives, each node under the
        /// first of its candidate parents that is a node other than itself, and each edge as
        /// merging a copy of the edges gives it.
        #[test]
        fn resolving_borrowed_facts_merges_them_as_a_copy_would(
            facts in prop::collection::vec(
                (0u8..10, 0usize..TIERS.len(), prop::option::of(0u8..12), prop::option::of(0u8..3)),
                0..24,
            ),
            ends in prop::collection::vec((0u8..6, 0u8..12, 0u8..12), 0..10),
        ) {
            let nodes: Vec<Node> = facts.into_iter().enumerate().map(|(i, f)| fact(i, f)).collect();
            let id = |k: u8| Ref::Id(NodeId(format!("r:x:{k}")));
            let edges: Vec<Edge> = ends
                .iter()
                .enumerate()
                .map(|(i, &(e, from, to))| {
                    let mut edge = edge(&format!("e:{e:012x}"), id(from), id(to));
                    edge.props = [(format!("p{i}"), serde_json::json!(i))].into();
                    edge
                })
                .collect();
            let resolved = resolve(&Facts { nodes: nodes.clone(), edges: edges.clone(), hints: vec![] }, &Vocabulary::default());

            let merged = facts::merge(nodes);
            let ids: Vec<&NodeId> = merged.iter().map(|m| &m.node.id).collect();
            let want: Vec<Node> = merged
                .iter()
                .map(|m| {
                    let mut node = m.node.clone();
                    node.parent = m.parents.iter().find(|p| **p != node.id && ids.contains(p)).cloned();
                    node
                })
                .collect();
            prop_assert_eq!(&resolved.nodes, &want);
            let mut all = resolved.edges.iter().chain(&resolved.unresolved).map(|e| e.id.clone()).collect::<Vec<_>>();
            all.sort();
            let mut merged_edges: Vec<EdgeId> = facts::merge_edges(edges.clone()).into_iter().map(|e| e.id).collect();
            merged_edges.sort();
            prop_assert_eq!(all, merged_edges);
            prop_assert_eq!(facts::merge_edges_borrowed(&edges), facts::merge_edges(edges));
        }
    }
}
