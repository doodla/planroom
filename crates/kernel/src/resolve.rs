//! Resolution, the fold's second step (architecture.md, "The fold"): one node per ID, merged by
//! tier (DR-42), with its references resolved and its kind and props checked against the loaded
//! packs' vocabulary.
//!
//! Today it merges nodes by tier and passes edges through as extracted; alias resolution,
//! kind and prop checks and parents follow behind this signature.

use planroom_contract::{Edge, Node, Vocabulary};

use crate::facts;
use crate::model::Facts;

/// The facts, resolved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolved {
    /// One per ID, sorted by ID.
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Resolve `facts` against `vocabulary`.
pub fn resolve(facts: &Facts, vocabulary: &Vocabulary) -> Resolved {
    let _ = vocabulary;
    Resolved { nodes: facts::merge_nodes(facts.nodes.clone()), edges: facts.edges.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Kind, Layer, Level, NodeId, Provenance, Tier};

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

    #[test]
    fn resolution_merges_nodes_by_tier_as_fact_merging_does() {
        let nodes = vec![node("r:x:b", Tier::Universal, "plain"), node("r:x:b", Tier::Config, "config")];
        let facts = Facts { nodes: nodes.clone(), ..Facts::default() };
        let resolved = resolve(&facts, &Vocabulary::default());
        assert_eq!(resolved.nodes, facts::merge_nodes(nodes));
        assert_eq!(resolved.nodes[0].name, "config");
    }
}
