//! Fact merging (DR-42): when extractors emit one node ID, evidence unions and scalar fields take
//! the more specific tier's value.

use planroom_contract::{Node, Provenance, Tier};

/// How specific a tier is: precise, then syntactic, then config, then pattern, then universal.
/// Linked and inferred facts rank between pattern and universal.
pub fn specificity(tier: Tier) -> u8 {
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

fn node_specificity(node: &Node) -> u8 {
    match &node.provenance {
        Provenance::Extracted { tier, .. } => specificity(*tier),
        Provenance::Linked { .. } => specificity(Tier::Linked),
        Provenance::Inferred { .. } => specificity(Tier::Inference),
        Provenance::Authored { .. } => u8::MAX,
    }
}

/// One node per ID, sorted by ID. For a shared ID the most specific node's fields win (the
/// earliest on a tie), its props fill in from the others, and aliases and evidence union.
pub fn merge_nodes(mut nodes: Vec<Node>) -> Vec<Node> {
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    let mut group: Vec<Node> = Vec::new();
    for node in nodes {
        if group.first().is_some_and(|g| g.id != node.id) {
            out.push(merge_group(std::mem::take(&mut group)));
        }
        group.push(node);
    }
    if !group.is_empty() {
        out.push(merge_group(group));
    }
    out
}

fn merge_group(mut group: Vec<Node>) -> Node {
    if group.len() == 1 {
        return group.pop().expect("one node");
    }
    let best = (0..group.len())
        .max_by_key(|&i| (node_specificity(&group[i]), std::cmp::Reverse(i)))
        .expect("a group");
    let mut winner = group.remove(best);
    for other in group {
        for (key, value) in other.props {
            winner.props.entry(key).or_insert(value);
        }
        for alias in other.aliases {
            if !winner.aliases.contains(&alias) {
                winner.aliases.push(alias);
            }
        }
        for evidence in other.evidence {
            if !winner.evidence.contains(&evidence) {
                winner.evidence.push(evidence);
            }
        }
    }
    winner.aliases.sort();
    winner
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Evidence, Kind, Layer, Level, NodeId};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn node(id: &str, tier: Tier, name: &str, line: u32) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("k.x".into()),
            level: Level::Code,
            parent: None,
            name: name.into(),
            aliases: vec![format!("a:{name}")],
            props: BTreeMap::from([(name.to_owned(), json!(1))]),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier },
            evidence: vec![Evidence::Line {
                path: "r/f".into(),
                line,
                contains: None,
                commit: None,
                branch: None,
            }],
        }
    }

    #[test]
    fn the_more_specific_tier_wins_and_evidence_unions() {
        let merged = merge_nodes(vec![
            node("r:x:b", Tier::Universal, "plain", 1),
            node("r:x:a", Tier::Universal, "only", 1),
            node("r:x:b", Tier::Config, "config", 2),
        ]);
        assert_eq!(merged.iter().map(|n| n.id.0.as_str()).collect::<Vec<_>>(), ["r:x:a", "r:x:b"]);
        let b = &merged[1];
        assert_eq!(b.name, "config");
        assert_eq!(b.evidence.len(), 2);
        assert_eq!(b.aliases, ["a:config", "a:plain"]);
        assert_eq!(b.props.len(), 2);
    }
}
