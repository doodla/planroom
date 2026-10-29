//! Structural lint (architecture.md, "Planning: stages, ghosts and lint (R5)"; DR-23, DR-45): pack
//! rules over the folded model, ghosts left out until a person accepts them. Models, rules and
//! events are built as values, so these tests do no I/O.

use std::collections::BTreeMap;

use planroom_contract::{
    Actor, Client, Edit, Event, EventBody, EventId, Kind, Layer, LevelId, Node, NodeId, Op, Provenance,
    RuleDecl, RuleType, Seq, Status, Surface, Target, Tier, Timestamp, Vocabulary,
};
use planroom_kernel::fold::fold;
use planroom_kernel::lint::{Finding, lint};
use planroom_kernel::model::{Facts, Model};
use planroom_kernel::plan::{ITEM_KIND, PROJECT_KIND, TOUCHES, in_project};
use serde_json::{Value, json};

const PROJECT: &str = "plan:retries:project";

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

fn rule(rule_id: &str, kind: RuleType, params: Value) -> RuleDecl {
    RuleDecl {
        id: rule_id.into(),
        kind,
        subject: ITEM_KIND.into(),
        params,
        message: format!("{rule_id} fails."),
    }
}

/// A plan item as the fold builds it from an applied `put`.
fn item(key: &str, parent: &str, props: Value) -> Node {
    Node {
        id: id(&format!("plan:retries:{key}")),
        kind: Kind(ITEM_KIND.into()),
        level: LevelId::from("intent"),
        parent: Some(id(parent)),
        name: key.into(),
        aliases: vec![],
        props: serde_json::from_value(props).unwrap(),
        layer: Layer::Intended,
        provenance: Provenance::Authored {
            actor: Actor::Person { id: "ann".into(), name: None },
            event: EventId("ev".into()),
        },
        evidence: vec![],
    }
}

fn project() -> Node {
    Node { kind: Kind(PROJECT_KIND.into()), parent: None, ..item("project", PROJECT, json!({})) }
}

fn model(nodes: Vec<Node>) -> Model {
    let mut nodes = nodes;
    nodes.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    Model { at: 1, nodes, edges: vec![], unresolved: vec![] }
}

fn run(model: &Model, rules: &[RuleDecl]) -> Vec<Finding> {
    let rules: Vec<&RuleDecl> = rules.iter().collect();
    lint(model, &rules, |n| in_project(&n.id, &id(PROJECT)))
}

fn subjects(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| f.subject.0.as_str()).collect()
}

/// The events of one writer's command: one event, applied or a ghost.
fn event(seq: Seq, actor: Actor, status: Status, edit: Edit) -> Event {
    Event {
        seq,
        id: EventId(format!("ev-{seq}")),
        idem: format!("cmd{seq}"),
        request_hash: "h".into(),
        at: Timestamp("2026-10-28T12:00:00Z".into()),
        actor,
        client: Client { surface: Surface::Cli, id: "cli:1".into() },
        project: Some(id(PROJECT)),
        cause: None,
        status,
        body: EventBody::Edit(edit),
    }
}

fn put(target: Target, fields: Value) -> Edit {
    let fields: BTreeMap<String, Value> = serde_json::from_value(fields).unwrap();
    Edit { op: Op::Put, target, layer: Layer::Intended, fields, base: 0, over: BTreeMap::new() }
}

fn put_item(key: &str, props: Value) -> Edit {
    put(
        Target::Node(id(&format!("plan:retries:{key}"))),
        json!({"kind": ITEM_KIND, "level": "intent", "name": key, "parent": PROJECT, "props": props}),
    )
}

#[test]
fn next_action_needs_done_test() {
    let rules = vec![rule(
        "next_action_done_test",
        RuleType::RequiresProp,
        json!({"prop": "done_test", "props.type": "next_action"}),
    )];
    let person = Actor::Person { id: "ann".into(), name: None };
    let agent = Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None };
    let file = Node {
        id: id("worker:file:worker/consumer.py"),
        kind: Kind("k.file".into()),
        level: LevelId::from("code"),
        parent: None,
        name: "consumer.py".into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        evidence: vec![],
    };
    let facts = Facts { nodes: vec![file], ..Facts::default() };
    let project = put(
        Target::Node(id(PROJECT)),
        json!({"kind": PROJECT_KIND, "level": "intent", "name": "Add retries"}),
    );
    let mut events = vec![event(1, person.clone(), Status::Applied, project)];
    let action = put_item("retry", json!({"type": "next_action", "verb": "wrap"}));
    events.push(event(2, agent, Status::Ghost, action.clone()));
    let linted = |events: &[Event]| {
        let model = fold(&facts, events, &Vocabulary::default(), events.len() as Seq);
        run(&model, &rules)
    };
    // A ghost stays in the overlay, out of lint, until a person accepts it.
    assert!(linted(&events).is_empty());
    events.push(event(3, person.clone(), Status::Applied, action));
    let found = linted(&events);
    assert_eq!(subjects(&found), ["plan:retries:retry"]);
    assert_eq!(found[0].rule, 0);
    let mut done = put_item("retry", json!({"type": "next_action", "done_test": "a retried job succeeds"}));
    done.op = Op::Set;
    done.fields = BTreeMap::from([("props.done_test".to_owned(), json!("a retried job succeeds"))]);
    events.push(event(4, person.clone(), Status::Applied, done));
    assert!(linted(&events).is_empty());
    // An empty done test is no done test.
    let mut emptied = put_item("retry", json!({}));
    emptied.op = Op::Set;
    emptied.fields = BTreeMap::from([("props.done_test".to_owned(), json!(""))]);
    events.push(event(5, person.clone(), Status::Applied, emptied));
    assert_eq!(subjects(&linted(&events)), ["plan:retries:retry"]);

    // A next action's target is a touches edge to a node of the model; one a rescan removed
    // doesn't count.
    let target_rule = [rule(
        "next_action_target",
        RuleType::RequiresEdge,
        json!({"relation": TOUCHES, "direction": "out", "props.type": "next_action"}),
    )];
    let edge = |to: &str| {
        put(
            Target::Edge(planroom_contract::EdgeId("e:0000000000aa".into())),
            json!({"kind": TOUCHES, "from": {"id": "plan:retries:retry"}, "to": {"id": to}}),
        )
    };
    let mut touched = events.clone();
    touched.push(event(6, person.clone(), Status::Applied, edge("worker:file:worker/consumer.py")));
    let model = fold(&facts, &touched, &Vocabulary::default(), 6);
    let rules: Vec<&RuleDecl> = target_rule.iter().collect();
    assert!(lint(&model, &rules, |_| true).is_empty());
    let gone = fold(&Facts::default(), &touched, &Vocabulary::default(), 6);
    assert_eq!(subjects(&lint(&gone, &rules, |_| true)), ["plan:retries:retry"]);
}

#[test]
fn wbs_100_percent_rule() {
    let rules = vec![rule(
        "work_covers_parent_scope",
        RuleType::Covers,
        json!({"prop": "scope", "kinds": [ITEM_KIND], "props.type": "work"}),
    )];
    let root = |scope: Value| item("root", PROJECT, json!({"type": "work", "scope": scope}));
    let work =
        |key: &str, scope: Value| item(key, "plan:retries:root", json!({"type": "work", "scope": scope}));
    let mut nodes =
        vec![project(), root(json!(["a", "b", "c"])), work("wa", json!(["a"])), work("wb", json!(["b"]))];
    let found = run(&model(nodes.clone()), &rules);
    assert_eq!(subjects(&found), ["plan:retries:root"]);
    assert_eq!(found[0].values, ["c"]);
    nodes.push(work("wc", json!(["c"])));
    assert!(run(&model(nodes), &rules).is_empty());
    // A subject with no counted child passes: the leaf work items here, and a work item whose only
    // children are next actions.
    let only_actions = vec![
        project(),
        root(json!(["a", "b"])),
        item("act", "plan:retries:root", json!({"type": "next_action", "scope": ["a"]})),
    ];
    assert!(run(&model(only_actions), &rules).is_empty());
    assert!(run(&model(vec![project(), root(json!(["a"]))]), &rules).is_empty());
}

#[test]
fn mece_overlap() {
    let rules = vec![rule(
        "work_children_disjoint",
        RuleType::Disjoint,
        json!({"prop": "scope", "kinds": [ITEM_KIND], "props.type": "work"}),
    )];
    let work =
        |key: &str, scope: Value| item(key, "plan:retries:root", json!({"type": "work", "scope": scope}));
    let mut nodes = vec![
        project(),
        item("root", PROJECT, json!({"type": "work", "scope": ["a", "b", "c"]})),
        work("w1", json!(["a", "b"])),
        work("w2", json!(["b", "c"])),
    ];
    let found = run(&model(nodes.clone()), &rules);
    assert_eq!(subjects(&found), ["plan:retries:root"]);
    assert_eq!(found[0].values, ["b"]);
    assert_eq!(found[0].related, [id("plan:retries:w1"), id("plan:retries:w2")]);
    // A next action under the same parent is no work sibling, so its scope never overlaps.
    nodes[3] = work("w2", json!(["c"]));
    nodes.push(item("act", "plan:retries:root", json!({"type": "next_action", "scope": ["a"]})));
    assert!(run(&model(nodes), &rules).is_empty());
}
