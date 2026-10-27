//! The containment tree: one parent per node, and the top of the map.

use std::collections::{BTreeMap, HashMap};

use planroom_contract::{Edge, EdgeId, Evidence, Kind, Level, LiftedEdge, MapNode, Node, NodeId, Ref};

use crate::keys;
use crate::resolve::Index;

/// The level's name as the contract spells it, the key of `MapNode.counts`.
pub fn level_name(level: Level) -> &'static str {
    match level {
        Level::Intent => "intent",
        Level::System => "system",
        Level::Container => "container",
        Level::Component => "component",
        Level::Code => "code",
    }
}

/// The nodes at the top of the tree, those with no parent or a parent that doesn't resolve, each
/// with its descendants counted by level. Sorted by level, then name, then ID.
pub fn top_level(nodes: &[Node]) -> Vec<MapNode> {
    let index: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
    let is_root = |n: &Node| n.parent.as_ref().is_none_or(|p| !index.contains_key(p.0.as_str()));
    blocks(nodes, &index, is_root)
}

/// Every node at `level`, each with its descendants counted by level, sorted like `top_level`.
pub fn at_level(nodes: &[Node], level: Level) -> Vec<MapNode> {
    let index: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
    blocks(nodes, &index, |n| n.level == level)
}

/// The nodes `is_block` selects, each counting every descendant whose nearest selected ancestor
/// it is. A parent cycle, which only bad facts can hold, counts nowhere.
fn blocks<'a>(
    nodes: &'a [Node],
    index: &HashMap<&str, &'a Node>,
    is_block: impl Fn(&Node) -> bool,
) -> Vec<MapNode> {
    let mut counts: HashMap<&str, BTreeMap<String, u32>> = HashMap::new();
    for node in nodes {
        let mut at = node;
        let mut steps = 0;
        while let Some(parent) = at.parent.as_ref().and_then(|p| index.get(p.0.as_str())) {
            at = parent;
            steps += 1;
            if steps > nodes.len() || is_block(at) {
                break;
            }
        }
        if steps > 0 && steps <= nodes.len() && is_block(at) {
            *counts
                .entry(at.id.0.as_str())
                .or_default()
                .entry(level_name(node.level).to_owned())
                .or_default() += 1;
        }
    }
    let mut top: Vec<MapNode> = nodes
        .iter()
        .filter(|n| is_block(n))
        .map(|n| MapNode {
            id: n.id.clone(),
            kind: n.kind.clone(),
            tone: None,
            name: n.name.clone(),
            level: n.level,
            parent: n.parent.clone().filter(|p| index.contains_key(p.0.as_str())),
            counts: counts.remove(n.id.0.as_str()).unwrap_or_default(),
            position: None,
            ghost: false,
            claimed_by: None,
        })
        .collect();
    top.sort_by(|a, b| (a.level, &a.name, &a.id).cmp(&(b.level, &b.name, &b.id)));
    top
}

/// One lifted edge being gathered: its children's IDs and its evidence sample.
type Lifting = (Vec<EdgeId>, Vec<Evidence>);

/// Evidence items a lifted edge carries at most.
pub const EVIDENCE_SAMPLE: usize = 3;

/// `edges` lifted to `blocks`: each end resolved by ID or by a unique alias, then walked up to its
/// nearest ancestor among the blocks. Edges between two blocks merge by kind and direction, with
/// their IDs as children and a sample of their evidence; an edge inside one block, or with an end
/// that doesn't resolve, lifts nowhere. Sorted by kind, then ends. This is the map's lift to the
/// blocks it draws; `rollup::lift` is the roll-up to a level, with its internal counts.
pub fn lift_edges(nodes: &[Node], edges: &[Edge], blocks: &[MapNode]) -> Vec<LiftedEdge> {
    let index = Index::new(nodes);
    let is_block: HashMap<&str, ()> = blocks.iter().map(|b| (b.id.0.as_str(), ())).collect();
    let block_of = |end: &Ref| -> Option<&str> {
        let mut at = index.get(end)?;
        for _ in 0..=nodes.len() {
            if is_block.contains_key(at.id.0.as_str()) {
                return Some(at.id.0.as_str());
            }
            at = index.node(&at.parent.as_ref()?.0)?;
        }
        None
    };
    let mut lifted: BTreeMap<(&Kind, &str, &str), Lifting> = BTreeMap::new();
    for edge in edges {
        let (Some(from), Some(to)) = (block_of(&edge.from), block_of(&edge.to)) else { continue };
        if from == to {
            continue;
        }
        let (children, sample) = lifted.entry((&edge.kind, from, to)).or_default();
        children.push(edge.id.clone());
        for evidence in &edge.evidence {
            if sample.len() < EVIDENCE_SAMPLE {
                sample.push(evidence.clone());
            }
        }
    }
    lifted
        .into_iter()
        .map(|((kind, from, to), (mut children, evidence_sample))| {
            children.sort();
            children.dedup();
            LiftedEdge {
                kind: kind.clone(),
                from: NodeId(from.to_owned()),
                to: NodeId(to.to_owned()),
                count: children.len() as u32,
                evidence_sample,
                children,
            }
        })
        .collect()
}

/// The hash of a view's structure, which a cached layout is keyed by: its block IDs and its
/// lifted edges' kinds and ends, sorted.
pub fn structure_hash(blocks: &[MapNode], edges: &[LiftedEdge]) -> String {
    let mut parts: Vec<String> = blocks.iter().map(|b| format!("n {}", b.id.0)).collect();
    parts.extend(edges.iter().map(|e| format!("e {} {} {}", e.kind.0, e.from.0, e.to.0)));
    parts.sort();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    keys::sha256_hex(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Layer, Provenance, Tier};

    fn node(id: &str, level: Level, parent: Option<&str>) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("k.x".into()),
            level,
            parent: parent.map(|p| NodeId(p.into())),
            name: id.rsplit(':').next().unwrap().into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    #[test]
    fn roots_count_their_descendants_by_level() {
        let nodes = vec![
            node("r:repo:r", Level::System, None),
            node("r:dir:src", Level::Component, Some("r:repo:r")),
            node("r:file:src/a", Level::Code, Some("r:dir:src")),
            node("r:container:r", Level::Container, Some("r:repo:r")),
            node("ws:container:db", Level::Container, None),
            node("x:container:lost", Level::Container, Some("x:repo:missing")),
        ];
        let top = top_level(&nodes);
        let ids: Vec<&str> = top.iter().map(|n| n.id.0.as_str()).collect();
        assert_eq!(ids, ["r:repo:r", "ws:container:db", "x:container:lost"]);
        assert_eq!(
            top[0].counts,
            BTreeMap::from([("code".into(), 1), ("component".into(), 1), ("container".into(), 1)])
        );
        assert!(top[1].counts.is_empty());
        let dirs = at_level(&nodes, Level::Component);
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].counts, BTreeMap::from([("code".into(), 1)]));
    }

    fn edge(id: &str, from: Ref, to: Ref) -> Edge {
        Edge {
            id: EdgeId(id.into()),
            kind: Kind("k.calls".into()),
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
    fn edges_lift_to_their_nearest_blocks_and_merge() {
        let mut aliased = node("b:file:main", Level::Code, Some("b:repo:b"));
        aliased.aliases = vec!["host:b".into()];
        let nodes = vec![
            node("a:repo:a", Level::System, None),
            node("a:file:x", Level::Code, Some("a:repo:a")),
            node("a:file:y", Level::Code, Some("a:repo:a")),
            node("b:repo:b", Level::System, None),
            aliased,
        ];
        let id = |s: &str| Ref::Id(NodeId(s.into()));
        let edges = vec![
            edge("e:000000000002", id("a:file:x"), Ref::Alias("host:b".into())),
            edge("e:000000000001", id("a:file:y"), id("b:repo:b")),
            edge("e:000000000003", id("a:file:x"), id("a:file:y")),
            edge("e:000000000004", id("a:file:x"), Ref::Alias("host:nowhere".into())),
        ];
        let blocks = top_level(&nodes);
        let lifted = lift_edges(&nodes, &edges, &blocks);
        assert_eq!(lifted.len(), 1, "{lifted:?}");
        assert_eq!(
            (lifted[0].from.0.as_str(), lifted[0].to.0.as_str(), lifted[0].count),
            ("a:repo:a", "b:repo:b", 2)
        );
        assert_eq!(lifted[0].children, [EdgeId("e:000000000001".into()), EdgeId("e:000000000002".into())]);
        let hash = structure_hash(&blocks, &lifted);
        assert_eq!(hash, structure_hash(&blocks, &lifted));
        assert_ne!(hash, structure_hash(&blocks, &[]));
    }

    #[test]
    fn a_parent_cycle_counts_nowhere() {
        let nodes = vec![
            node("r:x:a", Level::Code, Some("r:x:b")),
            node("r:x:b", Level::Code, Some("r:x:a")),
            node("r:repo:r", Level::System, None),
        ];
        let top = top_level(&nodes);
        assert_eq!(top.len(), 1);
        assert!(top[0].counts.is_empty());
    }
}
