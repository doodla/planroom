//! Structural lint (architecture.md, "What a pack is" and "Planning: stages, ghosts and lint";
//! DR-23, DR-45): each pack `[[rules]]` entry over the model, as its rule type reads it.
//!
//! A rule's subjects are the nodes of its `subject` kind that the caller's scope keeps and whose
//! `props.<name>` tests hold, read as `next`'s field tests read them. It reads only the model, so a
//! ghost, which stays in the overlay until a person accepts it, is never linted. The rule types:
//!
//! - `requires_prop`: the prop is missing, null, an empty string or an empty list.
//! - `requires_edge`: no edge of `relation`, in `direction` (either way when absent), whose far end
//!   resolves to a node of the model, of `kinds` when given. An end that resolves to nothing, such
//!   as a target a rescan removed, doesn't count.
//! - `min_children`: fewer than `min` children of `kinds`.
//! - `covers`: the subject's `prop` values that no counted child holds. A child counts when it is
//!   of `kinds` and passes the rule's own prop tests, so a subject with no counted child passes:
//!   `min_children` is the rule that reports a missing breakdown.
//! - `disjoint`: each value of `prop` two or more counted children share, with those children.
//!
//! Findings come in rule order, then by subject ID; each finding's values are sorted, and its
//! related nodes are the children involved, sorted.

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{Node, NodeId, RuleDecl, RuleType};
use serde_json::Value;

use crate::model::Model;
use crate::next::{field, has_edge};
use crate::resolve::Index;

/// The prefix of a prop test's key.
const PROPS: &str = "props.";

/// One subject a rule flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The rule's index in the rules [`lint`] was given.
    pub rule: usize,
    pub subject: NodeId,
    /// The values the rule found uncovered or shared, sorted; empty for a rule with none.
    pub values: Vec<String>,
    /// The other nodes involved, such as the children that share a value, sorted.
    pub related: Vec<NodeId>,
}

/// Every finding of `rules` over `model`, among the nodes `scope` keeps.
pub fn lint(model: &Model, rules: &[&RuleDecl], scope: impl Fn(&Node) -> bool) -> Vec<Finding> {
    let index = Index::new(&model.nodes);
    let mut children: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
    for node in &model.nodes {
        if let Some(parent) = &node.parent {
            children.entry(parent.0.as_str()).or_default().push(node);
        }
    }
    let mut out = Vec::new();
    for (at, rule) in rules.iter().enumerate() {
        let mut found: Vec<Finding> = model
            .nodes
            .iter()
            .filter(|n| n.kind.0 == rule.subject && scope(n) && tests_hold(rule, n))
            .filter_map(|n| {
                let kids = children.get(n.id.0.as_str()).map_or(&[][..], Vec::as_slice);
                check(model, &index, rule, n, kids).map(|(values, related)| Finding {
                    rule: at,
                    subject: n.id.clone(),
                    values,
                    related,
                })
            })
            .collect();
        found.sort_by(|a, b| a.subject.0.cmp(&b.subject.0));
        out.extend(found);
    }
    out
}

/// Whether every `props.<name>` test of `rule` holds on `node`.
fn tests_hold(rule: &RuleDecl, node: &Node) -> bool {
    let Some(params) = rule.params.as_object() else { return true };
    params
        .iter()
        .filter_map(|(key, test)| key.strip_prefix(PROPS).map(|prop| (prop, test)))
        .all(|(prop, test)| field(node.props.get(prop), test))
}

/// The kinds `kinds` names: one kind or a list; `None` when the rule names none.
fn kinds(rule: &RuleDecl) -> Option<BTreeSet<&str>> {
    match rule.params.get("kinds")? {
        Value::String(kind) => Some(BTreeSet::from([kind.as_str()])),
        Value::Array(items) => Some(items.iter().filter_map(Value::as_str).collect()),
        _ => None,
    }
}

fn of_kinds(kinds: Option<&BTreeSet<&str>>, node: &Node) -> bool {
    kinds.is_none_or(|k| k.contains(node.kind.0.as_str()))
}

/// The values a node's prop holds: each item of a list, or the one value, as text.
fn values(node: &Node, prop: &str) -> BTreeSet<String> {
    let text = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    };
    match node.props.get(prop) {
        Some(Value::Array(items)) => items.iter().filter_map(text).collect(),
        Some(value) => text(value).into_iter().collect(),
        None => BTreeSet::new(),
    }
}

/// What `rule` finds on `subject`: its values and related nodes, or `None` when it passes.
fn check(
    model: &Model,
    index: &Index<'_>,
    rule: &RuleDecl,
    subject: &Node,
    children: &[&Node],
) -> Option<(Vec<String>, Vec<NodeId>)> {
    let prop = rule.params.get("prop").and_then(Value::as_str);
    let kinds = kinds(rule);
    let counted = || -> Vec<&Node> {
        children.iter().copied().filter(|c| of_kinds(kinds.as_ref(), c) && tests_hold(rule, c)).collect()
    };
    match rule.kind {
        RuleType::RequiresProp => {
            let missing = match prop.and_then(|p| subject.props.get(p)) {
                None | Some(Value::Null) => true,
                Some(Value::String(s)) => s.is_empty(),
                Some(Value::Array(items)) => items.is_empty(),
                Some(_) => false,
            };
            missing.then(|| (vec![], vec![]))
        }
        RuleType::RequiresEdge => {
            let mut test = serde_json::Map::new();
            for key in ["relation", "direction"] {
                if let Some(value) = rule.params.get(key) {
                    test.insert(key.to_owned(), value.clone());
                }
            }
            let far = |end: Option<&Node>| end.is_some_and(|n| of_kinds(kinds.as_ref(), n));
            (!has_edge(model, index, subject, &Value::Object(test), far)).then(|| (vec![], vec![]))
        }
        RuleType::MinChildren => {
            let min = rule.params.get("min").and_then(Value::as_u64).unwrap_or(1);
            let count = children.iter().filter(|c| of_kinds(kinds.as_ref(), c)).count() as u64;
            (count < min).then(|| (vec![], vec![]))
        }
        RuleType::Covers => {
            let prop = prop?;
            let counted = counted();
            if counted.is_empty() {
                return None;
            }
            let covered: BTreeSet<String> = counted.iter().flat_map(|c| values(c, prop)).collect();
            let missing: Vec<String> =
                values(subject, prop).into_iter().filter(|v| !covered.contains(v)).collect();
            (!missing.is_empty()).then(|| (missing, vec![]))
        }
        RuleType::Disjoint => {
            let prop = prop?;
            let mut holders: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
            for child in counted() {
                for value in values(child, prop) {
                    holders.entry(value).or_default().insert(child.id.0.as_str());
                }
            }
            holders.retain(|_, ids| ids.len() > 1);
            if holders.is_empty() {
                return None;
            }
            let related: BTreeSet<&str> = holders.values().flatten().copied().collect();
            let related = related.into_iter().map(|id| NodeId(id.to_owned())).collect();
            Some((holders.into_keys().collect(), related))
        }
    }
}
