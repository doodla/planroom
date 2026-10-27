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
    let levels: HashMap<&Kind, Level> = vocabulary.kinds.iter().rev().map(|k| (&k.kind, k.level)).collect();
    // Each fact is copied once, into its merged node or edge.
    let sorted = facts::sorted_by_id(&facts.nodes);
    let groups: Vec<&[&Node]> = sorted.chunk_by(|a, b| a.id == b.id).collect();
    // One group per ID, sorted by ID, so a parent resolves by a binary search.
    let held = |id: &NodeId| groups.binary_search_by(|g| g[0].id.cmp(id)).is_ok();
    let nodes: Vec<Node> = groups
        .iter()
        .map(|group| {
            let mut node = match group {
                // A node one fact names has that fact's parent as its only candidate.
                [one] => {
                    let mut node = (*one).clone();
                    if node.parent.as_ref().is_some_and(|p| *p == node.id || !held(p)) {
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
            node
        })
        .collect();
    let index = Index::new(&nodes);
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    for edge in facts::merge_edges_borrowed(&facts.edges) {
        let (Some(from), Some(to)) = (index.get(&edge.from), index.get(&edge.to)) else {
            unresolved.push(edge);
            continue;
        };
        let (from, to) = (Ref::Id(from.id.clone()), Ref::Id(to.id.clone()));
        let via_ref = edge.via_ref.as_ref().map(|r| match index.get(r) {
            Some(node) => Ref::Id(node.id.clone()),
            None => r.clone(),
        });
        edges.push(Edge { from, to, via_ref, ..edge });
    }
    Resolved { nodes, edges, unresolved }
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
        Index { by_id, by_alias }
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
