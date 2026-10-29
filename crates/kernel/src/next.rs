//! The one next action (architecture.md, "Pane" and "Packs and extractors"; DR-54): the loaded
//! packs' `[[suggestions]]` are its candidates, and the one with the highest `priority` whose
//! `when` holds wins, the first in load order on a tie.
//!
//! A `when` predicate holds when some node or edge of the model satisfies every key it sets, read
//! through the predicate table the loader checked it against (`PREDICATE_PARAMS`): the subject's
//! kind, ID and parent, its `status` and `confidence`, its props, and the edges it has or lacks. An
//! empty `when` always holds. Nothing here names a pack's kind or relation; they arrive as data.

use planroom_contract::{Edge, Node, Predicate, Ref, SuggestionDecl};
use serde_json::Value;

use crate::model::Model;
use crate::resolve::Index;

/// The suggestion the pane shows as its next action, if any holds on `model`.
pub fn next<'a>(model: &Model, suggestions: &'a [SuggestionDecl]) -> Option<&'a SuggestionDecl> {
    let index = Index::new(&model.nodes);
    let mut best: Option<&SuggestionDecl> = None;
    for suggestion in suggestions {
        if best.is_some_and(|b| b.priority >= suggestion.priority) {
            continue;
        }
        if holds(model, &index, &suggestion.when) {
            best = Some(suggestion);
        }
    }
    best
}

/// Whether some node or edge of `model` satisfies `when`.
pub fn holds_on(model: &Model, when: &Predicate) -> bool {
    holds(model, &Index::new(&model.nodes), when)
}

fn holds(model: &Model, index: &Index<'_>, when: &Predicate) -> bool {
    if when.is_empty() {
        return true;
    }
    model.nodes.iter().any(|n| node_satisfies(model, index, n, when))
        || model.edges.iter().any(|e| edge_satisfies(e, when))
}

fn node_satisfies(model: &Model, index: &Index<'_>, node: &Node, when: &Predicate) -> bool {
    when.iter().all(|(key, test)| match key.as_str() {
        "kind" | "kinds" => names(test, &node.kind.0),
        "id" => test.as_str() == Some(node.id.0.as_str()),
        "parent" => node.parent.as_ref().is_some_and(|p| test.as_str() == Some(p.0.as_str())),
        "status" => field(node.props.get("status"), test),
        "confidence" => false,
        "with_edge" => has_edge(model, index, node, test, |_| true),
        "without_edge" => !has_edge(model, index, node, test, |_| true),
        key => match key.strip_prefix("props.") {
            Some(prop) => field(node.props.get(prop), test),
            None => false,
        },
    })
}

fn edge_satisfies(edge: &Edge, when: &Predicate) -> bool {
    when.iter().all(|(key, test)| match key.as_str() {
        "kind" | "kinds" => names(test, &edge.kind.0),
        "id" => test.as_str() == Some(edge.id.0.as_str()),
        "status" => field(edge.props.get("status"), test),
        "confidence" => field(Some(&Value::from(f64::from(edge.confidence))), test),
        // An edge has no parent and no edges of its own.
        "parent" | "with_edge" => false,
        "without_edge" => true,
        key => match key.strip_prefix("props.") {
            Some(prop) => field(edge.props.get(prop), test),
            None => false,
        },
    })
}

/// A kind test: one kind, or a list of them.
fn names(test: &Value, kind: &str) -> bool {
    match test {
        Value::String(s) => s == kind,
        Value::Array(items) => items.iter().any(|i| i.as_str() == Some(kind)),
        _ => false,
    }
}

/// A field test: a value it equals, or `{ lt = N }`, `{ gt = N }` or `{ in = [...] }`. A field the
/// subject lacks satisfies no test.
pub(crate) fn field(value: Option<&Value>, test: &Value) -> bool {
    let Some(value) = value else { return false };
    if let Value::Object(op) = test
        && op.len() == 1
    {
        let (name, operand) = op.iter().next().expect("one entry");
        let number = |v: &Value| v.as_f64();
        return match name.as_str() {
            "lt" => number(value).zip(number(operand)).is_some_and(|(v, n)| v < n),
            "gt" => number(value).zip(number(operand)).is_some_and(|(v, n)| v > n),
            "in" => operand.as_array().is_some_and(|items| items.iter().any(|i| same(i, value))),
            _ => false,
        };
    }
    same(test, value)
}

/// Equal values, numbers compared as numbers.
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Whether `node` has an edge the table `test` describes: its `relation`, its `direction` (`in`
/// to the node, `out` from it, either when absent) and, when given, its `layer`, whose far end
/// `far` keeps: the node that end resolves to, or `None` for an end that resolves to none.
pub(crate) fn has_edge(
    model: &Model,
    index: &Index<'_>,
    node: &Node,
    test: &Value,
    far: impl Fn(Option<&Node>) -> bool,
) -> bool {
    let Some(test) = test.as_object() else { return false };
    let relation = test.get("relation").and_then(Value::as_str);
    let direction = test.get("direction").and_then(Value::as_str);
    let layer = test.get("layer");
    let is_node = |end: &Ref| index.get(end).is_some_and(|n| n.id == node.id);
    let from = |e: &Edge| is_node(&e.from) && far(index.get(&e.to));
    let to = |e: &Edge| is_node(&e.to) && far(index.get(&e.from));
    model.edges.iter().any(|e| {
        relation.is_none_or(|r| r == e.kind.0)
            && layer.is_none_or(|l| serde_json::to_value(e.layer).is_ok_and(|v| &v == l))
            && match direction {
                Some("in") => to(e),
                Some("out") => from(e),
                _ => from(e) || to(e),
            }
    })
}

#[cfg(test)]
mod tests {
    use crate::ladder::testing::*;
    use std::collections::BTreeMap;

    use planroom_contract::{ActionDecl, CommandName, EdgeId, Kind, Layer, NodeId, Provenance, Tier};
    use serde_json::json;

    use super::*;

    fn node(id: &str, kind: &str, props: Value) -> Node {
        Node {
            id: NodeId(id.into()),
            kind: Kind(kind.into()),
            level: region(),
            parent: Some(NodeId("w:x:top".into())),
            name: id.rsplit(':').next().unwrap().into(),
            aliases: vec![],
            props: serde_json::from_value(props).unwrap(),
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
            evidence: vec![],
        }
    }

    fn model() -> Model {
        let edge = Edge {
            id: EdgeId("e:000000000001".into()),
            kind: Kind("p.call".into()),
            from: Ref::Id(NodeId("w:x:api".into())),
            to: Ref::Id(NodeId("w:x:vault".into())),
            via: None,
            via_ref: None,
            props: BTreeMap::new(),
            evidence: vec![],
            confidence: 0.6,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        };
        Model {
            at: 3,
            nodes: vec![
                node("w:x:api", "p.app", json!({"role": "api", "replicas": 3})),
                node("w:x:vault", "p.vault", json!({"role": "vault"})),
            ],
            edges: vec![edge],
            unresolved: vec![],
        }
    }

    fn suggestion(when: Value, text: &str, priority: u32) -> SuggestionDecl {
        SuggestionDecl {
            when: serde_json::from_value(when).unwrap(),
            text: text.into(),
            action: ActionDecl { command: CommandName("map".into()), args: vec![] },
            priority,
        }
    }

    fn when(value: Value) -> Predicate {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn the_highest_priority_suggestion_that_holds_wins() {
        let model = model();
        let suggestions = vec![
            suggestion(json!({}), "anything", 1),
            suggestion(json!({"kind": "p.nowhere"}), "absent", 50),
            suggestion(json!({"kinds": ["p.vault", "p.nowhere"]}), "vault", 20),
            suggestion(json!({"kind": "p.app"}), "app, later", 20),
        ];
        assert_eq!(next(&model, &suggestions).map(|s| s.text.as_str()), Some("vault"));
        assert_eq!(next(&model, &suggestions[..2]).map(|s| s.text.as_str()), Some("anything"));
        assert!(next(&model, &suggestions[1..2]).is_none());
        assert!(next(&model, &[]).is_none());
    }

    #[test]
    fn a_when_reads_props_edges_and_confidence() {
        let model = model();
        assert!(holds_on(&model, &when(json!({"props.role": {"in": ["vault", "ledger"]}}))));
        assert!(holds_on(&model, &when(json!({"props.replicas": {"gt": 2}}))));
        assert!(!holds_on(&model, &when(json!({"props.replicas": {"lt": 2}}))));
        assert!(holds_on(
            &model,
            &when(json!({"kind": "p.app", "with_edge": {"relation": "p.call", "direction": "out"}}))
        ));
        assert!(!holds_on(
            &model,
            &when(json!({"kind": "p.app", "with_edge": {"relation": "p.call", "direction": "in"}}))
        ));
        assert!(holds_on(
            &model,
            &when(json!({"kind": "p.vault", "without_edge": {"relation": "p.call", "direction": "out"}}))
        ));
        assert!(holds_on(&model, &when(json!({"kind": "p.call", "confidence": {"lt": 0.8}}))));
        assert!(!holds_on(&model, &when(json!({"kind": "p.call", "confidence": {"gt": 0.8}}))));
        assert!(holds_on(&model, &when(json!({"parent": "w:x:top", "id": "w:x:api"}))));
        assert!(!holds_on(&model, &when(json!({"status": "live"}))));
        assert!(holds_on(&model, &when(json!({}))));
        assert!(holds_on(&Model::default(), &when(json!({}))));
        assert!(!holds_on(&Model::default(), &when(json!({"kind": "p.app"}))));
    }
}
