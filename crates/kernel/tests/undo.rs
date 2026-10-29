//! Undo, the fold's half (DR-3, DR-61): every edit type applied, then undone per command, folds
//! back to the view before it. Inputs are built as values, so these tests do no I/O.

use std::collections::BTreeMap;

use planroom_contract::{
    Actor, Card, CardKind, Choice, Client, CommandName, Edge, EdgeId, Edit, Event, EventBody, EventId, Kind,
    Layer, LevelId, Node, NodeId, Op, Provenance, Ref, Seq, Stakes, Status, Surface, Target, Tier, Timestamp,
    Verdict, Vocabulary,
};
use planroom_kernel::fold::{ClaimRecord, Folded, Overlay, TargetKey, VersionTag, WHOLE, Writer, fold_full};
use planroom_kernel::model::{Facts, Model};
use serde_json::{Value, json};

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

fn node(key: &str, level: LevelId, parent: Option<&str>) -> Node {
    Node {
        id: id(key),
        kind: Kind("k.x".into()),
        level,
        parent: parent.map(id),
        name: key.rsplit(':').next().unwrap().into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

fn fact_edge() -> Edge {
    Edge {
        id: EdgeId("e:000000000001".into()),
        kind: Kind("k.rel".into()),
        from: Ref::Id(id("r:x:item")),
        to: Ref::Id(id("r:x:b")),
        via: None,
        via_ref: None,
        props: Default::default(),
        evidence: vec![],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
    }
}

fn facts() -> Facts {
    Facts {
        nodes: vec![
            node("r:x:repo", LevelId::from("system"), None),
            node("r:x:a", LevelId::from("component"), Some("r:x:repo")),
            node("r:x:b", LevelId::from("component"), Some("r:x:repo")),
            node("r:x:item", LevelId::from("code"), Some("r:x:a")),
        ],
        edges: vec![fact_edge()],
        ..Facts::default()
    }
}

fn person() -> Writer {
    Writer {
        actor: Actor::Person { id: "person:ann".into(), name: None },
        client: Client { surface: Surface::Canvas, id: "tab_1".into() },
    }
}

fn agent() -> Writer {
    Writer {
        actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
        client: Client { surface: Surface::Cli, id: "agent:1a2b3c4d:chat".into() },
    }
}

fn edit(op: Op, target: Target, layer: Layer, fields: Vec<(&str, Value)>) -> EventBody {
    EventBody::Edit(Edit {
        op,
        target,
        layer,
        fields: fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        base: 0,
        over: BTreeMap::new(),
    })
}

fn set_node(target: &str, field: &str, value: Value) -> EventBody {
    edit(Op::Set, Target::Node(id(target)), Layer::Actual, vec![(field, value)])
}

fn intended_edge(n: u8, rule: Option<&str>) -> (Target, EventBody) {
    let edge_id = EdgeId(format!("e:{n:012x}"));
    let mut fields =
        vec![("kind", json!("k.rel")), ("from", json!({"id": "r:x:a"})), ("to", json!({"id": "r:x:b"}))];
    if let Some(rule) = rule {
        fields.push(("props", json!({ "rule": rule })));
    }
    (Target::Edge(edge_id.clone()), edit(Op::Put, Target::Edge(edge_id), Layer::Intended, fields))
}

fn group_put(key: &str, parent: &str) -> EventBody {
    edit(
        Op::Put,
        Target::Node(id(key)),
        Layer::Actual,
        vec![
            ("kind", json!("k.g")),
            ("level", json!("component")),
            ("name", json!("purchase")),
            ("parent", json!(parent)),
        ],
    )
}

fn card_body() -> EventBody {
    EventBody::Card(Card {
        kind: CardKind("trust".into()),
        subject: "/work/shop".into(),
        question: "card.q".into(),
        why: "card.w".into(),
        vars: BTreeMap::new(),
        stakes: Stakes::Person,
        choices: vec![Choice { id: "accept".into(), label: "card.a".into(), preselected: true }],
        default: "accept".into(),
        dealt_by: CommandName("scan".into()),
    })
}

#[derive(Default)]
struct Log {
    events: Vec<Event>,
}

impl Log {
    fn command(&mut self, writer: &Writer, status: Status, bodies: Vec<EventBody>) -> Vec<EventId> {
        let stem = format!("cmd{}", self.events.len() + 1);
        let many = bodies.len() > 1;
        let mut ids = Vec::new();
        for (i, body) in bodies.into_iter().enumerate() {
            let seq = self.events.len() as Seq + 1;
            let id = EventId(format!("ev-{seq:04}"));
            self.events.push(Event {
                seq,
                id: id.clone(),
                idem: if many { format!("{stem}#{}", i + 1) } else { stem.clone() },
                request_hash: "h".into(),
                at: Timestamp("2026-10-26T12:00:00Z".into()),
                actor: writer.actor.clone(),
                client: writer.client.clone(),
                project: None,
                cause: None,
                status,
                body,
            });
            ids.push(id);
        }
        ids
    }

    fn fold_at(&self, at: Seq) -> Folded {
        fold_full(&facts(), &self.events, &Vocabulary::default(), at)
    }

    fn fold(&self) -> Folded {
        self.fold_at(self.events.len() as Seq)
    }
}

/// What a person sees of a fold: the model and its overlay, the versions, the claims and the
/// open cards. Diffs and revisions are the record of what happened, so an undo adds to them.
type View = (Model, Overlay, Vec<VersionTag>, Vec<ClaimRecord>, Vec<EventId>);

fn view(folded: &Folded) -> View {
    let mut model = folded.model.clone();
    model.at = 0;
    (model, folded.overlay.clone(), folded.versions.clone(), folded.claims.clone(), folded.open_cards.clone())
}

/// One edit type: what the log holds before it, and the command that makes it.
struct Case {
    name: &'static str,
    setup: Vec<(Writer, Status, Vec<EventBody>)>,
    command: Vec<EventBody>,
    /// Which of the command's events the undo names; any of them undoes the whole command.
    names: usize,
}

fn case(name: &'static str, command: Vec<EventBody>) -> Case {
    Case { name, setup: vec![], command, names: 0 }
}

fn cases() -> Vec<Case> {
    let node = |n: &str| Target::Node(id(n));
    let fact = Target::Edge(fact_edge().id);
    let (relate_target, relate) = intended_edge(2, None);
    let (_, forbid) = intended_edge(3, Some("forbid"));
    let pin = vec![("pos@map:system:all", json!({"x": 10.0, "y": -4.0}))];
    vec![
        // The three operations on both target kinds.
        case(
            "put a node",
            vec![edit(
                Op::Put,
                node("plan:p:retries"),
                Layer::Intended,
                vec![("kind", json!("k.item")), ("level", json!("intent")), ("name", json!("Add retries"))],
            )],
        ),
        case("set a node", vec![set_node("r:x:item", "props.owner", json!("ann"))]),
        case("retract a node", vec![edit(Op::Retract, node("r:x:b"), Layer::Actual, vec![])]),
        case("put an edge", vec![relate.clone()]),
        case(
            "set an edge",
            vec![edit(Op::Set, fact.clone(), Layer::Actual, vec![("props.weight", json!(2))])],
        ),
        case("retract an edge", vec![edit(Op::Retract, fact.clone(), Layer::Actual, vec![])]),
        // The hand edits.
        case("move", vec![set_node("r:x:item", "parent", json!("r:x:b"))]),
        case("rename", vec![set_node("r:x:item", "name", json!("Bag"))]),
        case("hide a node", vec![set_node("r:x:item", "hidden", json!(true))]),
        case("hide an edge", vec![edit(Op::Set, fact.clone(), Layer::Actual, vec![("hidden", json!(true))])]),
        case("pin", vec![edit(Op::Set, node("r:x:item"), Layer::Actual, pin)]),
        case("relate", vec![relate.clone()]),
        case("forbid", vec![forbid]),
        Case {
            name: "unrelate",
            setup: vec![(person(), Status::Applied, vec![relate])],
            command: vec![edit(Op::Retract, relate_target, Layer::Intended, vec![])],
            names: 0,
        },
        Case {
            name: "group",
            setup: vec![],
            command: vec![
                group_put("r:group:purchase", "r:x:repo"),
                set_node("r:x:item", "parent", json!("r:group:purchase")),
                set_node("r:x:b", "parent", json!("r:group:purchase")),
            ],
            names: 2,
        },
        // Beyond edits: a review, a version tag, a claim, and an undo itself.
        Case {
            name: "review of a ghost",
            setup: vec![(agent(), Status::Ghost, vec![set_node("r:x:item", "name", json!("Sack"))])],
            command: vec![EventBody::Review { of: EventId("ev-0001".into()), verdict: Verdict::Reject }],
            names: 0,
        },
        Case {
            name: "review of a card",
            setup: vec![(person(), Status::Open, vec![card_body()])],
            command: vec![EventBody::Review { of: EventId("ev-0001".into()), verdict: Verdict::Accept }],
            names: 0,
        },
        case("version", vec![EventBody::Version { project: None, label: "v2".into() }]),
        case(
            "claim",
            vec![EventBody::Claim {
                target: node("r:x:item"),
                until: Timestamp("2026-10-26T15:30:00Z".into()),
                release: false,
            }],
        ),
        Case {
            name: "undo",
            setup: vec![
                (person(), Status::Applied, vec![set_node("r:x:item", "name", json!("Bag"))]),
                (person(), Status::Applied, vec![EventBody::Undo { of: EventId("ev-0001".into()) }]),
            ],
            command: vec![EventBody::Undo { of: EventId("ev-0002".into()) }],
            names: 0,
        },
    ]
}

#[test]
fn every_edit_type_undoes() {
    let cases = cases();
    assert_eq!(cases.len(), 20);
    for case in cases {
        let mut log = Log::default();
        for (writer, status, bodies) in case.setup {
            log.command(&writer, status, bodies);
        }
        let before = view(&log.fold());
        let ids = log.command(&person(), Status::Applied, case.command);
        let applied = view(&log.fold());
        assert_ne!(applied, before, "{}: the edit changes what a person sees", case.name);
        log.command(&person(), Status::Applied, vec![EventBody::Undo { of: ids[case.names].clone() }]);
        let undone = log.fold();
        assert_eq!(view(&undone), before, "{}: the undo folds back to the view before the edit", case.name);
        assert!(
            ids.iter().all(|e| undone.is_undone(e)),
            "{}: every event of the command is undone",
            case.name
        );
    }
}

#[test]
fn regroup_undo_is_atomic() {
    let mut log = Log::default();
    let regroup = log.command(
        &person(),
        Status::Applied,
        vec![
            group_put("r:group:purchase", "r:x:repo"),
            set_node("r:x:item", "parent", json!("r:group:purchase")),
            set_node("r:x:b", "parent", json!("r:group:purchase")),
        ],
    );
    let parent = |folded: &Folded, n: &str| {
        folded.model.nodes.iter().find(|x| x.id.0 == n).map(|x| x.parent.clone().map(|p| p.0))
    };
    let grouped = log.fold();
    assert_eq!(parent(&grouped, "r:group:purchase"), Some(Some("r:x:repo".into())));
    assert_eq!(parent(&grouped, "r:x:item"), Some(Some("r:group:purchase".into())));
    assert_eq!(parent(&grouped, "r:x:b"), Some(Some("r:group:purchase".into())));

    // The undo names one member's move; the whole regroup goes back in that one event.
    let undo =
        log.command(&person(), Status::Applied, vec![EventBody::Undo { of: regroup[1].clone() }]).remove(0);
    let undone = log.fold();
    assert_eq!(parent(&undone, "r:group:purchase"), None, "the group is gone");
    assert_eq!(parent(&undone, "r:x:item"), Some(Some("r:x:a".into())), "every member's parent is restored");
    assert_eq!(parent(&undone, "r:x:b"), Some(Some("r:x:repo".into())));
    assert!(
        undone.diffs.iter().all(|d| !matches!(d, planroom_kernel::fold::FoldDiff::ParentFallback { .. }))
    );
    // No fold sees half of it: before the undo's seq the regroup holds whole, at it none of it does.
    assert_eq!(view(&log.fold_at(3)), view(&grouped));

    // The undo is one write over all three fields: each carries the undo's revision.
    let keys = undone.undo_keys(&regroup[2]);
    assert_eq!(
        keys,
        vec![
            (TargetKey::Node(id("r:group:purchase")), WHOLE.to_owned()),
            (TargetKey::Node(id("r:x:b")), "parent".to_owned()),
            (TargetKey::Node(id("r:x:item")), "parent".to_owned()),
        ]
    );
    for (target, field) in keys {
        let rev = undone.revision(&target.target(), &field).unwrap();
        assert_eq!(rev.event, undo, "{target:?} {field}");
    }
}
