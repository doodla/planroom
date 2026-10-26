//! The fold (architecture.md, "The fold"): the model at `at`, from the active snapshots' facts and
//! the events, pure and deterministic, so `fold(at = N)` is reproducible.
//!
//! Today it resolves the facts and applies no event, which is what every reader shows before
//! edits, reviews and undos land; overrides, ghosts, the pre-pass and scan drift follow behind
//! this signature.

use planroom_contract::{Event, Seq, Vocabulary};

use crate::model::{Facts, Model};
use crate::resolve::resolve;

/// The model at `at`, from `facts`, the `events` up to `at`, and the loaded packs' `vocabulary`.
pub fn fold(facts: &Facts, events: &[Event], vocabulary: &Vocabulary, at: Seq) -> Model {
    let _ = events;
    let resolved = resolve(facts, vocabulary);
    Model { at, nodes: resolved.nodes, edges: resolved.edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planroom_contract::{Kind, Layer, Level, Node, NodeId, Provenance, Tier};
    use proptest::prelude::*;
    use proptest::test_runner::Config;

    const TIERS: [Tier; 7] = [
        Tier::Precise,
        Tier::Syntactic,
        Tier::Config,
        Tier::Pattern,
        Tier::Linked,
        Tier::Inference,
        Tier::Universal,
    ];

    fn node(key: u8, tier: usize) -> Node {
        Node {
            id: NodeId(format!("r:x:{key}")),
            kind: Kind("k.x".into()),
            level: Level::Code,
            parent: None,
            name: format!("n{key}"),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: TIERS[tier] },
            evidence: vec![],
        }
    }

    proptest! {
        // No I/O in the kernel's tests: a failing case is printed, never written to a file.
        #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

        #[test]
        fn a_folded_model_holds_each_fact_node_once_in_id_order(
            facts in prop::collection::vec((0u8..6, 0usize..TIERS.len()), 0..24),
            at in 0u64..1_000,
        ) {
            let nodes: Vec<Node> = facts.iter().map(|&(key, tier)| node(key, tier)).collect();
            let model = fold(&Facts { nodes: nodes.clone(), ..Facts::default() }, &[], &Vocabulary::default(), at);
            let ids: Vec<&str> = model.nodes.iter().map(|n| n.id.0.as_str()).collect();
            let mut want: Vec<&str> = nodes.iter().map(|n| n.id.0.as_str()).collect();
            want.sort_unstable();
            want.dedup();
            prop_assert_eq!(ids, want);
            prop_assert_eq!(model.at, at);
        }
    }

    #[test]
    fn a_fold_with_no_events_is_its_resolved_facts_at_its_seq() {
        let facts = Facts::default();
        let vocabulary = Vocabulary::default();
        let model = fold(&facts, &[], &vocabulary, 7);
        let resolved = resolve(&facts, &vocabulary);
        assert_eq!(model, Model { at: 7, nodes: resolved.nodes, edges: resolved.edges });
    }
}
