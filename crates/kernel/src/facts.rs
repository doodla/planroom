//! Fact merging (DR-42): when extractors emit one node ID, evidence unions and scalar fields take
//! the more specific tier's value. Edges of one ID merge the same way.

use std::collections::HashMap;

use planroom_contract::{Edge, EdgeId, Node, NodeId, Provenance, Tier};

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

fn provenance_specificity(provenance: &Provenance) -> u8 {
    match provenance {
        Provenance::Extracted { tier, .. } => specificity(*tier),
        Provenance::Linked { .. } => specificity(Tier::Linked),
        Provenance::Inferred { .. } => specificity(Tier::Inference),
        Provenance::Authored { .. } => u8::MAX,
    }
}

/// One node per ID, sorted by ID. For a shared ID the most specific node's fields win (the
/// earliest on a tie), its props fill in from the others, and aliases and evidence union.
pub fn merge_nodes(nodes: Vec<Node>) -> Vec<Node> {
    merge(nodes).into_iter().map(|merged| merged.node).collect()
}

/// A node merged from every fact that names its ID, and the parents those facts name.
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    /// The merged node; its `parent` is the most specific fact's.
    pub node: Node,
    /// Every distinct parent the facts name, the most specific fact's first (the earliest on a
    /// tie), so resolution can fall back to the next one when a parent doesn't resolve.
    pub parents: Vec<NodeId>,
}

/// [`merge_nodes`], keeping each node's candidate parents in tier order.
pub fn merge(mut nodes: Vec<Node>) -> Vec<Merged> {
    // A stable sort keeps the facts of one ID in their emitted order, which breaks tier ties.
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out: Vec<Merged> = Vec::with_capacity(nodes.len());
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

/// The facts in the order [`merge`] groups them, borrowed: sorted by ID, the facts of one ID in
/// their emitted order. A caller that keeps its facts merges each run of one ID with
/// [`merge_borrowed`], so no fact is moved or copied but into its merged node.
pub fn sorted_by_id(nodes: &[Node]) -> Vec<&Node> {
    let mut sorted: Vec<&Node> = nodes.iter().collect();
    // A stable sort keeps the facts of one ID in their emitted order, which breaks tier ties.
    if !sorted.is_sorted_by(|a, b| a.id <= b.id) {
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
    }
    sorted
}

/// The node [`merge`] makes of the facts of one ID, given in their emitted order.
pub fn merge_borrowed(group: &[&Node]) -> Merged {
    merge_group(group.iter().map(|n| (*n).clone()).collect())
}

fn merge_group(mut group: Vec<Node>) -> Merged {
    // Most specific first, the earliest on a tie.
    let mut order: Vec<usize> = (0..group.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(provenance_specificity(&group[i].provenance)), i));
    let mut parents: Vec<NodeId> = Vec::new();
    for &i in &order {
        if let Some(parent) = &group[i].parent
            && !parents.contains(parent)
        {
            parents.push(parent.clone());
        }
    }
    if group.len() == 1 {
        return Merged { node: group.pop().expect("one node"), parents };
    }
    let mut winner = group.remove(order[0]);
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
    Merged { node: winner, parents }
}

/// One edge per ID, in the order each ID first appears. An edge's ID hashes its kind, ends and
/// `via`, so two facts of one ID are one relation: the most specific fact's fields win (the
/// earliest on a tie), its props fill in from the others, and evidence unions.
pub fn merge_edges(edges: Vec<Edge>) -> Vec<Edge> {
    let mut slot: HashMap<EdgeId, usize> = HashMap::new();
    let mut groups: Vec<Vec<Edge>> = Vec::new();
    for edge in edges {
        match slot.get(&edge.id) {
            Some(&i) => groups[i].push(edge),
            None => {
                slot.insert(edge.id.clone(), groups.len());
                groups.push(vec![edge]);
            }
        }
    }
    groups.into_iter().map(merge_edge_group).collect()
}

/// [`merge_edges`] over edges it borrows, copying each edge once, into its merged edge.
pub fn merge_edges_borrowed(edges: &[Edge]) -> Vec<Edge> {
    let mut slot: HashMap<&EdgeId, usize> = HashMap::with_capacity(edges.len());
    let mut groups: Vec<Vec<&Edge>> = Vec::new();
    for edge in edges {
        match slot.get(&edge.id) {
            Some(&i) => groups[i].push(edge),
            None => {
                slot.insert(&edge.id, groups.len());
                groups.push(vec![edge]);
            }
        }
    }
    groups
        .into_iter()
        .map(|group| match group.as_slice() {
            [one] => (*one).clone(),
            _ => merge_edge_group(group.into_iter().cloned().collect()),
        })
        .collect()
}

fn merge_edge_group(mut group: Vec<Edge>) -> Edge {
    if group.len() == 1 {
        return group.pop().expect("one edge");
    }
    let best = (0..group.len())
        .max_by_key(|&i| (provenance_specificity(&group[i].provenance), std::cmp::Reverse(i)))
        .expect("a group");
    let mut winner = group.remove(best);
    for other in group {
        for (key, value) in other.props {
            winner.props.entry(key).or_insert(value);
        }
        for evidence in other.evidence {
            if !winner.evidence.contains(&evidence) {
                winner.evidence.push(evidence);
            }
        }
    }
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

    #[test]
    fn a_merged_node_keeps_its_candidate_parents_most_specific_first() {
        let mut plain = node("r:x:b", Tier::Universal, "plain", 1);
        plain.parent = Some(NodeId("r:x:low".into()));
        let mut config = node("r:x:b", Tier::Config, "config", 2);
        config.parent = Some(NodeId("r:x:high".into()));
        let mut again = node("r:x:b", Tier::Universal, "again", 3);
        again.parent = Some(NodeId("r:x:low".into()));
        let merged = merge(vec![plain, config, again]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].node.parent, Some(NodeId("r:x:high".into())));
        assert_eq!(merged[0].parents, [NodeId("r:x:high".into()), NodeId("r:x:low".into())]);
    }
}
