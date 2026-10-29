//! A question's argument as a person or an agent types it: a full ID, an ID suffix such as
//! `src/cart`, or a bare name such as `worker` (architecture.md, "IDs and aliases": suffix and
//! name lookups). A suffix or a name resolves at the question's level first, then at each level
//! above it, then at each level below; several matches at the first level that has any are
//! ambiguous, never guessed. Above and below are the ladder's order.

use planroom_contract::{LevelId, Node};

use crate::find;
use crate::ladder::Ladder;

/// What one argument names.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<'a> {
    Found(&'a Node),
    /// Nothing matches: the closest nodes by fuzzy match, best first, at most [`CLOSEST`].
    Missing(Vec<&'a Node>),
    /// Several match at the first level that has any, in the model's order.
    Ambiguous(Vec<&'a Node>),
}

/// The most close matches a missing argument names.
pub const CLOSEST: usize = 5;

/// The node `target` names among `nodes`, preferring `level`, then the levels `ladder` puts
/// nearest above it, then those below ([`Ladder::order_from`]).
pub fn lookup<'a>(nodes: &'a [Node], target: &str, level: &LevelId, ladder: &Ladder<'_>) -> Lookup<'a> {
    let target = target.trim();
    if let Some(node) = nodes.iter().find(|n| n.id.0 == target) {
        return Lookup::Found(node);
    }
    let suffix = format!(":{target}");
    let matches: Vec<&Node> = nodes
        .iter()
        .filter(|n| !target.is_empty() && (n.id.0.ends_with(&suffix) || n.name == target))
        .collect();
    for at in ladder.order_from(level) {
        let here: Vec<&Node> = matches.iter().copied().filter(|n| n.level == *at).collect();
        match here.len() {
            0 => continue,
            1 => return Lookup::Found(here[0]),
            _ => return Lookup::Ambiguous(here),
        }
    }
    Lookup::Missing(find::find(nodes, ladder, target).into_iter().take(CLOSEST).map(|hit| hit.node).collect())
}

#[cfg(test)]
mod tests {
    use crate::ladder::testing::*;
    use planroom_contract::{Kind, Layer, NodeId, Provenance, Tier};

    use super::*;

    fn node(id: &str, level: LevelId, name: &str) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("t.x".into()),
            level,
            parent: None,
            name: name.into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "t/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    #[test]
    fn a_name_resolves_at_the_level_first_then_above_then_below() {
        let nodes = vec![
            node("a:box:web", region(), "web"),
            node("a:part:web", site(), "web"),
            node("a:sys:web", realm(), "web"),
            node("b:part:src/cart", site(), "cart"),
        ];
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let found = |t: &str, l: LevelId| match lookup(&nodes, t, &l, &ladder) {
            Lookup::Found(n) => n.id.0.clone(),
            other => panic!("{t}: {other:?}"),
        };
        assert_eq!(found("web", region()), "a:box:web");
        assert_eq!(found("web", room()), "a:part:web");
        assert_eq!(found("web", intent()), "a:sys:web");
        assert_eq!(found("src/cart", region()), "b:part:src/cart");
        assert_eq!(found("a:part:web", region()), "a:part:web");
        assert!(matches!(lookup(&nodes, "nowhere", &region(), &ladder), Lookup::Missing(_)));
        let twice = vec![node("a:box:x", region(), "x"), node("b:box:x", region(), "x")];
        assert!(matches!(lookup(&twice, "x", &region(), &ladder), Lookup::Ambiguous(v) if v.len() == 2));
    }
}
