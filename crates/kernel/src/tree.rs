//! The containment tree: one parent per node, and the top of the map.

use std::collections::{BTreeMap, HashMap};

use planroom_contract::{Edge, EdgeId, Evidence, Kind, LevelId, LiftedEdge, MapNode, Node, NodeId, Ref};

use crate::keys;
use crate::ladder::Ladder;
use crate::resolve::Index;

/// The nodes at the top of the tree, those with no parent or a parent that doesn't resolve, each
/// with its descendants counted by level. Sorted by the level's rank on `ladder`, then name, then
/// ID.
pub fn top_level(nodes: &[Node], ladder: &Ladder<'_>) -> Vec<MapNode> {
    let index: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
    let is_root = |n: &Node| n.parent.as_ref().is_none_or(|p| !index.contains_key(p.0.as_str()));
    blocks(nodes, ladder, &index, is_root)
}

/// Every node at `level`, each with its descendants counted by level, sorted like `top_level`.
pub fn at_level(nodes: &[Node], ladder: &Ladder<'_>, level: &LevelId) -> Vec<MapNode> {
    let index: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
    blocks(nodes, ladder, &index, |n| n.level == *level)
}

/// The blocks a map draws at `level`, inside the subtree under `under` when it names a node:
/// every node at `level` there, and every finer node with no node at or above `level` between it
/// and the top of that subtree. So the system level holds the repos and, beside them, a container
/// no repo holds, the same blocks as the top of the map; and the blocks under a repo at the
/// component level include a directory no container holds. The intent level holds only intent
/// nodes. Each block counts its descendants by level; sorted like [`top_level`]. "Finer" is a
/// higher rank on `ladder`.
pub fn blocks_at(
    nodes: &[Node],
    ladder: &Ladder<'_>,
    level: &LevelId,
    under: Option<&NodeId>,
) -> Vec<MapNode> {
    let index: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.0.as_str(), n)).collect();
    let (apart, rank) = (ladder.is_apart(level), ladder.rank(level));
    let parent_of = |n: &Node| n.parent.as_ref().and_then(|p| index.get(p.0.as_str()).copied());
    let selected: std::collections::HashSet<&str> = nodes
        .iter()
        .filter(|n| {
            if n.level == *level {
                // At the level: anywhere in the subtree.
                let mut at = *n;
                for _ in 0..nodes.len() {
                    let Some(parent) = parent_of(at) else { return under.is_none() };
                    if under == Some(&parent.id) {
                        return true;
                    }
                    at = parent;
                }
                return false;
            }
            // Finer than the level: only the top of its own chain inside the subtree, since any
            // node above it there either holds it at the level or is finer and drawn instead.
            !apart && ladder.rank(&n.level) > rank && parent_of(n).map(|p| &p.id) == under
        })
        .map(|n| n.id.0.as_str())
        .collect();
    blocks(nodes, ladder, &index, |n| selected.contains(n.id.0.as_str()))
}

/// The nodes `is_block` selects, each counting every descendant whose nearest selected ancestor
/// it is. A parent cycle, which only bad facts can hold, counts nowhere.
fn blocks<'a>(
    nodes: &'a [Node],
    ladder: &Ladder<'_>,
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
                .entry(node.level.as_str().to_owned())
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
            level: n.level.clone(),
            parent: n.parent.clone().filter(|p| index.contains_key(p.0.as_str())),
            counts: counts.remove(n.id.0.as_str()).unwrap_or_default(),
            position: None,
            ghost: false,
            claimed_by: None,
        })
        .collect();
    let key = |n: &MapNode| ladder.rank(&n.level);
    top.sort_by(|a, b| (key(a), &a.name, &a.id).cmp(&(key(b), &b.name, &b.id)));
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
    use crate::ladder::testing::*;
    use planroom_contract::{Layer, Provenance, Tier};

    fn node(id: &str, level: LevelId, parent: Option<&str>) -> Node {
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
            node("r:repo:r", realm(), None),
            node("r:dir:src", site(), Some("r:repo:r")),
            node("r:file:src/a", room(), Some("r:dir:src")),
            node("r:container:r", region(), Some("r:repo:r")),
            node("ws:container:db", region(), None),
            node("x:container:lost", region(), Some("x:repo:missing")),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let top = top_level(&nodes, &ladder);
        let ids: Vec<&str> = top.iter().map(|n| n.id.0.as_str()).collect();
        assert_eq!(ids, ["r:repo:r", "ws:container:db", "x:container:lost"]);
        assert_eq!(
            top[0].counts,
            BTreeMap::from([("region".into(), 1), ("room".into(), 1), ("site".into(), 1)])
        );
        assert!(top[1].counts.is_empty());
        let dirs = at_level(&nodes, &ladder, &site());
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].counts, BTreeMap::from([("room".into(), 1)]));
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
        let mut aliased = node("b:file:main", room(), Some("b:repo:b"));
        aliased.aliases = vec!["host:b".into()];
        let nodes = vec![
            node("a:repo:a", realm(), None),
            node("a:file:x", room(), Some("a:repo:a")),
            node("a:file:y", room(), Some("a:repo:a")),
            node("b:repo:b", realm(), None),
            aliased,
        ];
        let id = |s: &str| Ref::Id(NodeId(s.into()));
        let edges = vec![
            edge("e:000000000002", id("a:file:x"), Ref::Alias("host:b".into())),
            edge("e:000000000001", id("a:file:y"), id("b:repo:b")),
            edge("e:000000000003", id("a:file:x"), id("a:file:y")),
            edge("e:000000000004", id("a:file:x"), Ref::Alias("host:nowhere".into())),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let blocks = top_level(&nodes, &ladder);
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
    fn a_level_keeps_the_finer_nodes_nothing_at_it_holds() {
        let nodes = vec![
            node("r:repo:r", realm(), None),
            node("r:container:r", region(), Some("r:repo:r")),
            node("r:dir:app", site(), Some("r:container:r")),
            node("r:dir:docs", site(), Some("r:repo:r")),
            node("r:file:docs/a", room(), Some("r:dir:docs")),
            node("ws:container:db", region(), None),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let ids = |blocks: Vec<MapNode>| -> Vec<String> { blocks.into_iter().map(|b| b.id.0).collect() };
        // The top level is the top of the map: the realm node, and the region node none holds.
        assert_eq!(ids(blocks_at(&nodes, &ladder, &realm(), None)), ids(top_level(&nodes, &ladder)));
        assert_eq!(ids(blocks_at(&nodes, &ladder, &realm(), None)), ["r:repo:r", "ws:container:db"]);
        // Under the realm node at the region level: its region, and the site no region holds.
        let repo = NodeId("r:repo:r".into());
        let under = blocks_at(&nodes, &ladder, &region(), Some(&repo));
        assert_eq!(ids(under.clone()), ["r:container:r", "r:dir:docs"]);
        assert_eq!(under[1].counts, BTreeMap::from([("room".into(), 1)]));
        assert_eq!(ids(blocks_at(&nodes, &ladder, &site(), Some(&repo))), ["r:dir:app", "r:dir:docs"]);
        // Nothing under a leaf, and only intent nodes at the intent level.
        assert!(blocks_at(&nodes, &ladder, &room(), Some(&NodeId("r:file:docs/a".into()))).is_empty());
        assert!(blocks_at(&nodes, &ladder, &intent(), None).is_empty());
    }

    #[test]
    fn a_parent_cycle_counts_nowhere() {
        let nodes = vec![
            node("r:x:a", room(), Some("r:x:b")),
            node("r:x:b", room(), Some("r:x:a")),
            node("r:repo:r", realm(), None),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let top = top_level(&nodes, &ladder);
        assert_eq!(top.len(), 1);
        assert!(top[0].counts.is_empty());
    }
}
