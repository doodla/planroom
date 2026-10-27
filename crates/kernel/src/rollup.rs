//! Roll-up (architecture.md, "Levels and roll-ups"; DR-42): a model's edges lifted to a level.
//!
//! For each edge whose kind the lens lifts:
//! 1. Let `a` and `b` be each end's nearest ancestor at or above the level (itself included), or,
//!    when no node on its chain is, the top of its chain.
//! 2. If `a` and `b` are one node, the edge counts in that node's `internal`.
//! 3. Otherwise, when `a` is the other end's original node or one of its ancestors, `a` steps down
//!    to its child on the path to its own original end; the same goes for `b`. An end that is
//!    already its original node can't step down, so the edge is that node's `internal`. Nothing
//!    else lifts an end below the level.
//!
//! So no lifted edge joins a node to itself or to its own ancestor, and an edge whose two ends
//! share an ancestor at the level is that ancestor's internal count. Lifted edges with the same
//! kind and ends merge, with their IDs as children and a sample of their evidence. Every edge the
//! lens lifts lands exactly once: in one lifted edge's children, in one node's `internal`, or, when
//! an end resolves to no node, in `unresolved`. So lifting then expanding gives back the edge set.

use std::collections::BTreeMap;

use planroom_contract::{EdgeId, Evidence, Kind, LensVocab, Level, LiftedEdge, Node, NodeId};

use crate::model::Model;
use crate::resolve::Index;
use crate::tree::EVIDENCE_SAMPLE;

/// A model's edges at one level.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rollup {
    /// Sorted by kind, then `from`, then `to`; each one's children sorted.
    pub edges: Vec<LiftedEdge>,
    /// The edges inside each node, by the node they lie inside; each list sorted.
    pub internal: BTreeMap<NodeId, Vec<EdgeId>>,
    /// The edges with an end that resolves to no node, or to several; sorted.
    pub unresolved: Vec<EdgeId>,
}

impl Rollup {
    /// The number of edges inside `node`.
    pub fn internal_count(&self, node: &NodeId) -> u32 {
        self.internal.get(node).map_or(0, |edges| edges.len() as u32)
    }
}

/// One lifted edge being gathered: its children's IDs and its evidence sample.
type Lifting = (Vec<EdgeId>, Vec<Evidence>);

/// `model`'s edges lifted to `level`, those whose kind `lens` lifts, or every edge with no lens.
pub fn lift(model: &Model, level: Level, lens: Option<&LensVocab>) -> Rollup {
    let index = Index::new(&model.nodes);
    let mut lifted: BTreeMap<(&Kind, &NodeId, &NodeId), Lifting> = BTreeMap::new();
    let mut internal: BTreeMap<NodeId, Vec<EdgeId>> = BTreeMap::new();
    let mut unresolved: Vec<EdgeId> = Vec::new();
    for edge in &model.edges {
        if lens.is_some_and(|lens| !lens.relations.contains(&edge.kind)) {
            continue;
        }
        let (Some(from), Some(to)) = (index.get(&edge.from), index.get(&edge.to)) else {
            unresolved.push(edge.id.clone());
            continue;
        };
        match ends(&index, from, to, level) {
            Ends::Internal(node) => internal.entry(node.id.clone()).or_default().push(edge.id.clone()),
            Ends::Lifted(a, b) => {
                let (children, sample) = lifted.entry((&edge.kind, &a.id, &b.id)).or_default();
                children.push(edge.id.clone());
                for evidence in &edge.evidence {
                    if sample.len() < EVIDENCE_SAMPLE && !sample.contains(evidence) {
                        sample.push(evidence.clone());
                    }
                }
            }
        }
    }
    let edges = lifted
        .into_iter()
        .map(|((kind, from, to), (mut children, evidence_sample))| {
            children.sort();
            children.dedup();
            LiftedEdge {
                kind: kind.clone(),
                from: from.clone(),
                to: to.clone(),
                count: children.len() as u32,
                evidence_sample,
                children,
            }
        })
        .collect();
    for edges in internal.values_mut() {
        edges.sort();
        edges.dedup();
    }
    unresolved.sort();
    unresolved.dedup();
    Rollup { edges, internal, unresolved }
}

/// Where one edge lands at a level.
enum Ends<'a> {
    Internal(&'a Node),
    Lifted(&'a Node, &'a Node),
}

/// The lifted ends of an edge from `from` to `to` at `level`, by the module's three steps.
fn ends<'a>(index: &Index<'a>, from: &'a Node, to: &'a Node, level: Level) -> Ends<'a> {
    let (up_a, up_b) = (index.chain(from), index.chain(to));
    let (mut a, mut b) = (at_level(&up_a, level), at_level(&up_b, level));
    loop {
        if up_a[a].id == up_b[b].id {
            return Ends::Internal(up_a[a]);
        }
        let a_holds_b = up_b.iter().any(|n| n.id == up_a[a].id);
        let b_holds_a = up_a.iter().any(|n| n.id == up_b[b].id);
        if a_holds_b {
            if a == 0 {
                return Ends::Internal(up_a[a]);
            }
            a -= 1;
        } else if b_holds_a {
            if b == 0 {
                return Ends::Internal(up_b[b]);
            }
            b -= 1;
        } else {
            return Ends::Lifted(up_a[a], up_b[b]);
        }
    }
}

/// The position in `chain` (a node and its ancestors, nearest first) of the nearest node at or
/// above `level`, or of the chain's top when none is.
fn at_level(chain: &[&Node], level: Level) -> usize {
    chain.iter().position(|n| n.level <= level).unwrap_or(chain.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Edge, Layer, Provenance, Ref, Tier};

    fn node(id: &str, level: Level, parent: Option<&str>) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("k.x".into()),
            level,
            parent: parent.map(|p| NodeId(p.into())),
            name: id.into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    fn edge(id: &str, kind: &str, from: &str, to: &str) -> Edge {
        Edge {
            id: EdgeId(id.into()),
            kind: Kind(kind.into()),
            from: Ref::Id(NodeId(from.into())),
            to: Ref::Id(NodeId(to.into())),
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
    fn a_lens_lifts_only_its_relations() {
        let model = Model {
            at: 0,
            nodes: vec![node("r:x:a", Level::System, None), node("r:x:b", Level::System, None)],
            edges: vec![
                edge("e:000000000001", "k.y", "r:x:a", "r:x:b"),
                edge("e:000000000002", "k.z", "r:x:a", "r:x:b"),
            ],
            ..Model::default()
        };
        let lens = LensVocab {
            id: "p.l".into(),
            label: "L".into(),
            relations: vec![Kind("k.y".into())],
            flow: Default::default(),
            kinds: vec![],
            level: Level::System,
            default: false,
        };
        let rollup = lift(&model, Level::System, Some(&lens));
        assert_eq!(rollup.edges.len(), 1);
        assert_eq!(rollup.edges[0].kind, Kind("k.y".into()));
        assert_eq!(lift(&model, Level::System, None).edges.len(), 2);
    }

    #[test]
    fn a_parent_cycle_lifts_without_looping() {
        let model = Model {
            at: 0,
            nodes: vec![
                node("r:x:a", Level::Code, Some("r:x:b")),
                node("r:x:b", Level::Code, Some("r:x:a")),
                node("r:x:c", Level::Code, None),
            ],
            edges: vec![
                edge("e:000000000001", "k.y", "r:x:a", "r:x:c"),
                edge("e:000000000002", "k.y", "r:x:a", "r:x:b"),
            ],
            ..Model::default()
        };
        for level in [Level::Intent, Level::System, Level::Container, Level::Component, Level::Code] {
            let rollup = lift(&model, level, None);
            let landed = rollup.edges.iter().map(|e| e.children.len()).sum::<usize>()
                + rollup.internal.values().map(Vec::len).sum::<usize>();
            assert_eq!(landed, 2, "{level:?}: {rollup:?}");
        }
    }
}
