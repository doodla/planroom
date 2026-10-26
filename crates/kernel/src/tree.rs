//! The containment tree: one parent per node, and the top of the map.

use std::collections::{BTreeMap, HashMap};

use planroom_contract::{Level, MapNode, Node};

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
    let mut counts: HashMap<&str, BTreeMap<String, u32>> = HashMap::new();
    for node in nodes.iter().filter(|n| !is_root(n)) {
        // Walk up to the root; a parent cycle, which only bad facts can hold, counts nowhere.
        let mut at = node;
        let mut steps = 0;
        while let Some(parent) = at.parent.as_ref().and_then(|p| index.get(p.0.as_str())) {
            at = parent;
            steps += 1;
            if steps > nodes.len() {
                break;
            }
        }
        if steps <= nodes.len() && is_root(at) {
            *counts
                .entry(at.id.0.as_str())
                .or_default()
                .entry(level_name(node.level).to_owned())
                .or_default() += 1;
        }
    }
    let mut top: Vec<MapNode> = nodes
        .iter()
        .filter(|n| is_root(n))
        .map(|n| MapNode {
            id: n.id.clone(),
            kind: n.kind.clone(),
            name: n.name.clone(),
            level: n.level,
            parent: None,
            counts: counts.remove(n.id.0.as_str()).unwrap_or_default(),
            position: None,
            ghost: false,
            claimed_by: None,
        })
        .collect();
    top.sort_by(|a, b| (a.level, &a.name, &a.id).cmp(&(b.level, &b.name, &b.id)));
    top
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Kind, Layer, NodeId, Provenance, Tier};

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
    }
}
