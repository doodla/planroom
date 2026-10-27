//! A question's argument as a person or an agent types it: a full ID, an ID suffix such as
//! `src/cart`, or a bare name such as `worker` (architecture.md, "IDs and aliases": suffix and
//! name lookups). A suffix or a name resolves at the question's level first, then at each level
//! above it, then at each level below; several matches at the first level that has any are
//! ambiguous, never guessed.

use planroom_contract::{Level, Node};

use crate::find;

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

const LEVELS: [Level; 5] = [Level::Intent, Level::System, Level::Container, Level::Component, Level::Code];

/// The node `target` names among `nodes`, preferring `level`.
pub fn lookup<'a>(nodes: &'a [Node], target: &str, level: Level) -> Lookup<'a> {
    let target = target.trim();
    if let Some(node) = nodes.iter().find(|n| n.id.0 == target) {
        return Lookup::Found(node);
    }
    let suffix = format!(":{target}");
    let matches: Vec<&Node> = nodes
        .iter()
        .filter(|n| !target.is_empty() && (n.id.0.ends_with(&suffix) || n.name == target))
        .collect();
    for at in order(level) {
        let here: Vec<&Node> = matches.iter().copied().filter(|n| n.level == at).collect();
        match here.len() {
            0 => continue,
            1 => return Lookup::Found(here[0]),
            _ => return Lookup::Ambiguous(here),
        }
    }
    Lookup::Missing(find::find(nodes, target).into_iter().take(CLOSEST).map(|hit| hit.node).collect())
}

/// `level`, then each level above it, nearest first, then each below it.
fn order(level: Level) -> impl Iterator<Item = Level> {
    let at = LEVELS.iter().position(|l| *l == level).unwrap_or(0);
    let above = LEVELS[..at].iter().rev();
    let below = LEVELS[at + 1..].iter();
    std::iter::once(level).chain(above.chain(below).copied())
}

#[cfg(test)]
mod tests {
    use planroom_contract::{Kind, Layer, NodeId, Provenance, Tier};

    use super::*;

    fn node(id: &str, level: Level, name: &str) -> Node {
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
            node("a:box:web", Level::Container, "web"),
            node("a:part:web", Level::Component, "web"),
            node("a:sys:web", Level::System, "web"),
            node("b:part:src/cart", Level::Component, "cart"),
        ];
        let found = |t: &str, l: Level| match lookup(&nodes, t, l) {
            Lookup::Found(n) => n.id.0.clone(),
            other => panic!("{t}: {other:?}"),
        };
        assert_eq!(found("web", Level::Container), "a:box:web");
        assert_eq!(found("web", Level::Code), "a:part:web");
        assert_eq!(found("web", Level::Intent), "a:sys:web");
        assert_eq!(found("src/cart", Level::Container), "b:part:src/cart");
        assert_eq!(found("a:part:web", Level::Container), "a:part:web");
        assert!(matches!(lookup(&nodes, "nowhere", Level::Container), Lookup::Missing(_)));
        let twice = vec![node("a:box:x", Level::Container, "x"), node("b:box:x", Level::Container, "x")];
        assert!(matches!(lookup(&twice, "x", Level::Container), Lookup::Ambiguous(v) if v.len() == 2));
    }
}
