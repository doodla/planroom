//! The `find` shape (architecture.md, "Lenses and questions"): fuzzy search over a model's node
//! names and IDs with `nucleo-matcher`, best first.
//!
//! The text is matched as fuzzy words, each of which must match, so no character in it has a
//! special meaning, and case never matters. A node scores the better of its name's and its ID's
//! match. A node whose name or ID is the text itself ranks before every other, so `find worker`
//! puts the blocks named `worker` first; ties keep the map's order, by level, then name, then ID.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use planroom_contract::Node;

/// One match.
#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> {
    pub node: &'a Node,
    /// The match's score: higher is better.
    pub score: u32,
    /// The name or the ID is the text itself, ignoring case.
    pub exact: bool,
}

/// Every node in `nodes` that `text` matches, best first. Blank text matches nothing.
pub fn find<'a>(nodes: &'a [Node], text: &str) -> Vec<Hit<'a>> {
    let text = text.trim();
    if text.is_empty() {
        return vec![];
    }
    let pattern = Pattern::new(text, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Vec::new();
    let mut score = |s: &str| pattern.score(Utf32Str::new(s, &mut buf), &mut matcher);
    let mut hits: Vec<Hit<'a>> = nodes
        .iter()
        .filter_map(|node| {
            let best = score(&node.name).max(score(&node.id.0))?;
            let exact = node.name.eq_ignore_ascii_case(text) || node.id.0.eq_ignore_ascii_case(text);
            Some(Hit { node, score: best, exact })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.exact.cmp(&a.exact).then(b.score.cmp(&a.score)).then_with(|| {
            (a.node.level, &a.node.name, &a.node.id).cmp(&(b.node.level, &b.node.name, &b.node.id))
        })
    });
    hits
}

#[cfg(test)]
mod tests {
    use planroom_contract::{Kind, Layer, Level, NodeId, Provenance, Tier};

    use super::*;

    fn node(id: &str, level: Level) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind("p.x".into()),
            level,
            parent: None,
            name: id.rsplit(['/', ':']).next().unwrap().into(),
            aliases: vec![],
            props: Default::default(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    fn found(nodes: &[Node], text: &str) -> Vec<String> {
        find(nodes, text).into_iter().map(|h| h.node.id.0.clone()).collect()
    }

    #[test]
    fn an_exact_name_ranks_first_then_the_best_fuzzy_match() {
        let nodes = vec![
            node("w:file:src/workers.py", Level::Code),
            node("w:x:worker", Level::Container),
            node("w:repo:worker", Level::System),
            node("s:file:src/checkout/routes.ts", Level::Code),
        ];
        let hits = found(&nodes, "worker");
        assert_eq!(hits[..2], ["w:repo:worker", "w:x:worker"]);
        assert_eq!(hits[2], "w:file:src/workers.py");
        assert!(!hits.contains(&"s:file:src/checkout/routes.ts".to_owned()));
        // By ID, and by the words of a path, in any case.
        assert_eq!(found(&nodes, "s:file:src/checkout/routes.ts")[0], "s:file:src/checkout/routes.ts");
        assert_eq!(found(&nodes, "Checkout routes")[0], "s:file:src/checkout/routes.ts");
        // A character with a meaning in nucleo's query syntax is only a character here.
        assert!(found(&nodes, "!worker").is_empty());
        assert!(found(&nodes, "   ").is_empty());
        assert!(found(&nodes, "qqqzzz").is_empty());
    }
}
