//! The fold over facts and events built as values (architecture.md, "The fold"; DR-61): no I/O.

use std::collections::BTreeMap;

use planroom_contract::{
    Actor, Card, CardKind, Cause, Choice, Client, CommandName, Edge, EdgeId, Edit, Event, EventBody, EventId,
    Kind, Layer, Level, Node, NodeId, Op, Provenance, Ref, Rename, ScanStats, Seq, Stakes, Status, Surface,
    Target, Tier, Timestamp, Verdict, Vocabulary,
};
use planroom_kernel::cas::copy_for_review;
use planroom_kernel::fold::{FoldDiff, Folded, Writer, claims, fold, fold_full, fold_one, open_cards};
use planroom_kernel::model::Facts;
use proptest::prelude::*;
use proptest::test_runner::Config;
use serde_json::{Value, json};

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

fn node(key: &str, level: Level, parent: Option<&str>) -> Node {
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

fn edge(n: u8, from: &str, to: &str) -> Edge {
    Edge {
        id: EdgeId(format!("e:{n:012x}")),
        kind: Kind("k.rel".into()),
        from: Ref::Id(id(from)),
        to: Ref::Id(id(to)),
        via: None,
        via_ref: None,
        props: Default::default(),
        evidence: vec![],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
    }
}

/// A repo with two boxes, `a` and `b`, and an item inside `a`.
fn facts() -> Facts {
    Facts {
        nodes: vec![
            node("r:x:repo", Level::System, None),
            node("r:x:a", Level::Component, Some("r:x:repo")),
            node("r:x:b", Level::Component, Some("r:x:repo")),
            node("r:x:item", Level::Code, Some("r:x:a")),
        ],
        edges: vec![edge(1, "r:x:item", "r:x:b")],
        ..Facts::default()
    }
}

fn tab(n: u8) -> Writer {
    Writer {
        actor: Actor::Person { id: "person:ann".into(), name: None },
        client: Client { surface: Surface::Canvas, id: format!("tab_{n}") },
    }
}

fn agent() -> Writer {
    Writer {
        actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
        client: Client { surface: Surface::Cli, id: "agent:1a2b3c4d:chat".into() },
    }
}

fn system() -> Writer {
    Writer {
        actor: Actor::System { part: "scan".into() },
        client: Client { surface: Surface::Cli, id: "cli:1".into() },
    }
}

fn set(target: Target, field: &str, value: Value) -> Edit {
    Edit {
        op: Op::Set,
        target,
        layer: Layer::Actual,
        fields: BTreeMap::from([(field.to_owned(), value)]),
        base: 0,
        over: BTreeMap::new(),
    }
}

fn set_node(target: &str, field: &str, value: Value) -> Edit {
    set(Target::Node(id(target)), field, value)
}

fn scan(snapshot: u64) -> EventBody {
    EventBody::Scan {
        repo: "r".into(),
        commit: format!("c{snapshot}"),
        branch: "main".into(),
        snapshot,
        link: snapshot,
        renames: vec![],
        stats: ScanStats::default(),
    }
}

#[derive(Default)]
struct Log {
    events: Vec<Event>,
}

impl Log {
    /// One command's events, keyed `idem#1` to `idem#n` when it writes several.
    fn command(&mut self, writer: &Writer, status: Status, bodies: Vec<EventBody>) -> Vec<EventId> {
        let parts = bodies.into_iter().map(|body| (status, None, body)).collect();
        self.command_of(writer, parts)
    }

    /// One command's events, each with its own status and cause, as `review` writes them.
    fn command_of(
        &mut self,
        writer: &Writer,
        parts: Vec<(Status, Option<Cause>, EventBody)>,
    ) -> Vec<EventId> {
        let stem = format!("cmd{}", self.events.len() + 1);
        let many = parts.len() > 1;
        let mut ids = Vec::new();
        for (i, (status, cause, body)) in parts.into_iter().enumerate() {
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
                cause,
                status,
                body,
            });
            ids.push(id);
        }
        ids
    }

    fn push(&mut self, writer: &Writer, status: Status, body: EventBody) -> EventId {
        self.command(writer, status, vec![body]).remove(0)
    }

    fn edit(&mut self, writer: &Writer, edit: Edit) -> EventId {
        self.push(writer, Status::Applied, EventBody::Edit(edit))
    }

    fn fold(&self, facts: &Facts) -> Folded {
        fold_full(facts, &self.events, &Vocabulary::default(), self.events.len() as Seq)
    }
}

fn parent_of(folded: &Folded, n: &str) -> Option<String> {
    folded.model.nodes.iter().find(|x| x.id.0 == n).unwrap().parent.as_ref().map(|p| p.0.clone())
}

fn name_of(folded: &Folded, n: &str) -> String {
    folded.model.nodes.iter().find(|x| x.id.0 == n).unwrap().name.clone()
}

#[test]
fn a_parent_cycle_breaks_by_seq() {
    // An older store or an import holds two moves that each put one box under the other.
    let mut log = Log::default();
    log.edit(&tab(1), set_node("r:x:a", "parent", json!("r:x:b")));
    let loser = log.edit(&tab(2), set_node("r:x:b", "parent", json!("r:x:a")));
    let folded = log.fold(&facts());
    assert_eq!(parent_of(&folded, "r:x:a").as_deref(), Some("r:x:b"), "the lower seq's move stays");
    assert_eq!(parent_of(&folded, "r:x:b").as_deref(), Some("r:x:repo"), "the higher seq's move loses");
    assert!(folded.diffs.contains(&FoldDiff::CycleBroken {
        event: loser,
        node: id("r:x:b"),
        parent: id("r:x:a"),
        cycle: vec![id("r:x:b"), id("r:x:a"), id("r:x:b")],
    }));

    // A loop through three nodes: the last move to close it loses, the two before it stay.
    let mut log = Log::default();
    log.edit(&tab(1), set_node("r:x:item", "parent", json!("r:x:b")));
    log.edit(&tab(1), set_node("r:x:b", "parent", json!("r:x:a")));
    let loser = log.edit(&tab(1), set_node("r:x:a", "parent", json!("r:x:item")));
    let folded = log.fold(&facts());
    assert_eq!(parent_of(&folded, "r:x:item").as_deref(), Some("r:x:b"));
    assert_eq!(parent_of(&folded, "r:x:b").as_deref(), Some("r:x:a"));
    assert_eq!(parent_of(&folded, "r:x:a").as_deref(), Some("r:x:repo"));
    assert!(folded.diffs.iter().any(|d| matches!(d, FoldDiff::CycleBroken { event, .. } if *event == loser)));

    // A parent that doesn't resolve falls back to the fact parent, and is listed.
    let mut log = Log::default();
    let moved = log.edit(&tab(1), set_node("r:x:item", "parent", json!("r:x:gone")));
    let folded = log.fold(&facts());
    assert_eq!(parent_of(&folded, "r:x:item").as_deref(), Some("r:x:a"));
    assert!(folded.diffs.contains(&FoldDiff::ParentFallback {
        event: moved,
        node: id("r:x:item"),
        parent: id("r:x:gone"),
        fallback: Some(id("r:x:a")),
    }));
}

#[test]
fn an_unresolved_parent_falls_back_to_the_fact_parent() {
    // A person moves the item under a node no fact or edit holds.
    let mut log = Log::default();
    let moved = log.edit(&tab(1), set_node("r:x:item", "parent", json!("r:x:gone")));
    let folded = log.fold(&facts());
    assert_eq!(parent_of(&folded, "r:x:item").as_deref(), Some("r:x:a"), "the fact parent");
    let fallbacks: Vec<&FoldDiff> =
        folded.diffs.iter().filter(|d| matches!(d, FoldDiff::ParentFallback { .. })).collect();
    assert_eq!(
        fallbacks,
        [&FoldDiff::ParentFallback {
            event: moved.clone(),
            node: id("r:x:item"),
            parent: id("r:x:gone"),
            fallback: Some(id("r:x:a")),
        }],
        "listed in Diffs once"
    );

    // A node with no fact parent falls back to the top.
    let mut log = Log::default();
    let top = log.edit(&tab(1), set_node("r:x:repo", "parent", json!("r:x:gone")));
    let folded = log.fold(&facts());
    assert_eq!(parent_of(&folded, "r:x:repo"), None);
    assert!(folded.diffs.contains(&FoldDiff::ParentFallback {
        event: top,
        node: id("r:x:repo"),
        parent: id("r:x:gone"),
        fallback: None,
    }));

    // The move stays: once a rescan brings the parent, the item sits under it, and nothing is listed.
    let mut log = Log::default();
    log.edit(&tab(1), set_node("r:x:item", "parent", json!("r:x:gone")));
    log.push(&system(), Status::Applied, scan(2));
    let mut rescanned = facts();
    rescanned.nodes.push(node("r:x:gone", Level::Component, Some("r:x:repo")));
    let folded = log.fold(&rescanned);
    assert_eq!(parent_of(&folded, "r:x:item").as_deref(), Some("r:x:gone"));
    assert!(!folded.diffs.iter().any(|d| matches!(d, FoldDiff::ParentFallback { .. })));
}

#[test]
fn override_survives_changed_fact_and_raises_scan_diff() {
    let mut log = Log::default();
    log.push(&system(), Status::Applied, scan(1));
    let mut rename = set_node("r:x:item", "name", json!("Bag"));
    rename.over = BTreeMap::from([("name".to_owned(), json!("item"))]);
    let edit = log.edit(&tab(1), rename);
    let before = log.fold(&facts());
    assert_eq!(name_of(&before, "r:x:item"), "Bag");
    assert!(!before.diffs.iter().any(|d| matches!(d, FoldDiff::ScanChangedUnderEdit { .. })));

    // A rescan renames the fact under the override.
    log.push(&system(), Status::Applied, scan(2));
    let mut rescanned = facts();
    rescanned.nodes[3].name = "item-v2".into();
    let after = log.fold(&rescanned);
    assert_eq!(name_of(&after, "r:x:item"), "Bag", "a scan never silently overrides a person's edit");
    assert_eq!(
        after.diffs,
        vec![FoldDiff::ScanChangedUnderEdit {
            event: edit.clone(),
            target: Target::Node(id("r:x:item")),
            field: "name".into(),
            over: json!("item"),
            fact: json!("item-v2"),
        }]
    );

    // A rescan in which the target vanished keeps the override and lists the target.
    let mut gone = facts();
    gone.nodes.pop();
    let vanished = log.fold(&gone);
    assert!(
        vanished
            .diffs
            .contains(&FoldDiff::TargetVanished { event: edit, target: Target::Node(id("r:x:item")) })
    );
}

#[test]
fn fold_is_reproducible_across_claim_expiry() {
    let mut log = Log::default();
    log.edit(&tab(1), set_node("r:x:item", "name", json!("Bag")));
    let without = log.fold(&facts());
    log.push(
        &agent(),
        Status::Applied,
        EventBody::Claim {
            target: Target::Node(id("r:x:item")),
            until: Timestamp("2026-10-26T15:30:00Z".into()),
            release: false,
        },
    );
    let at = log.events.len() as Seq;
    let first = log.fold(&facts());
    let again = fold_full(&facts(), &log.events, &Vocabulary::default(), at);
    assert_eq!(first, again, "the fold reads no clock");
    assert_eq!((&first.model.nodes, &first.model.edges), (&without.model.nodes, &without.model.edges));
    assert_eq!(first.overlay, without.overlay, "a claim never changes the model or its overlay");

    // The overlay applies the clock: the claim holds before its expiry and not after.
    let before = claims::active(&first.claims, &Timestamp("2026-10-26T15:00:00Z".into()));
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].holder, agent().actor);
    assert!(claims::active(&first.claims, &Timestamp("2026-10-26T16:00:00Z".into())).is_empty());
    assert!(claims::active(&first.claims, &Timestamp("2026-10-26T15:30:00Z".into())).is_empty());
    // An older fold reproduces too.
    assert_eq!(fold(&facts(), &log.events, &Vocabulary::default(), 1), without.model);

    // A release ends the holder's claim.
    log.push(
        &agent(),
        Status::Applied,
        EventBody::Claim {
            target: Target::Node(id("r:x:item")),
            until: Timestamp("2026-10-26T15:30:00Z".into()),
            release: true,
        },
    );
    let released = log.fold(&facts());
    assert!(claims::active(&released.claims, &Timestamp("2026-10-26T15:00:00Z".into())).is_empty());
}

#[test]
fn a_card_body_is_skipped_and_a_card_is_open_until_reviewed() {
    let mut log = Log::default();
    let card = log.push(&system(), Status::Open, card_body());
    let unknown: EventBody = serde_json::from_value(json!({"kind": "presence_ping", "who": "x"})).unwrap();
    log.push(&system(), Status::Applied, unknown);
    let folded = log.fold(&facts());
    assert_eq!(folded.open_cards, std::slice::from_ref(&card));
    assert_eq!(folded.model, fold(&facts(), &[], &Vocabulary::default(), 2));
    log.push(&tab(1), Status::Applied, EventBody::Review { of: card, verdict: Verdict::Accept });
    assert!(log.fold(&facts()).open_cards.is_empty());
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

// The incremental fold: one random history of scans, edits, ghosts, conflicts, undos (redos
// included) and reviews of older events, folded one event at a time, equals a fold from scratch
// after every event.

#[derive(Debug, Clone)]
enum Step {
    Scan(u8),
    Set {
        writer: u8,
        status: u8,
        node: u8,
        field: u8,
        value: u8,
    },
    PutNode {
        writer: u8,
        status: u8,
        key: u8,
        parent: u8,
    },
    PutEdge {
        writer: u8,
        key: u8,
        from: u8,
        to: u8,
    },
    Retract {
        writer: u8,
        edge: bool,
        key: u8,
    },
    Group {
        writer: u8,
        members: Vec<u8>,
    },
    Undo {
        writer: u8,
        of: usize,
    },
    Review {
        of: usize,
        accept: bool,
    },
    Claim {
        key: u8,
        release: bool,
    },
    Version,
    Card,
    /// A ghost on the latest card, as an unsigned inbox edit row is.
    CardGhost {
        node: u8,
        value: u8,
    },
    /// What `review` writes for a pick of an older event: its review, an accept's copy caused by
    /// it, and the card's own review with its last open ghost.
    Pick {
        of: usize,
        accept: bool,
    },
    /// A ghost as an unsigned inbox edit row is stored: on the open card, or on one its write
    /// deals, keyed apart from the row, when none is open.
    InboxGhost {
        node: u8,
        value: u8,
    },
    /// What `edit undo` writes for a person's undo of an older event's command.
    UndoAsEdit {
        of: usize,
    },
}

const NODES: [&str; 6] = ["r:x:repo", "r:x:a", "r:x:b", "r:x:item", "r:x:p0", "r:x:gone"];

fn writer(n: u8) -> Writer {
    match n % 3 {
        0 => tab(1),
        1 => tab(2),
        _ => agent(),
    }
}

fn status(n: u8) -> Status {
    match n % 4 {
        0 | 1 => Status::Applied,
        2 => Status::Ghost,
        _ => Status::Conflict,
    }
}

/// Three snapshot sets: the base, one that renames and moves, one that drops the item.
fn facts_variant(v: u8) -> Facts {
    let mut f = facts();
    match v % 3 {
        0 => {}
        1 => {
            f.nodes[3].name = "item-v2".into();
            f.nodes[2].parent = Some(id("r:x:a"));
            f.edges.push(edge(2, "r:x:a", "r:x:b"));
        }
        _ => {
            f.nodes.pop();
            f.edges.clear();
        }
    }
    f
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0u8..3).prop_map(Step::Scan),
        (0u8..3, 0u8..4, 0u8..6, 0u8..5, 0u8..6).prop_map(|(writer, status, node, field, value)| Step::Set {
            writer,
            status,
            node,
            field,
            value
        }),
        (0u8..3, 0u8..4, 0u8..3, 0u8..6).prop_map(|(writer, status, key, parent)| Step::PutNode {
            writer,
            status,
            key,
            parent
        }),
        (0u8..3, 0u8..3, 0u8..6, 0u8..6).prop_map(|(writer, key, from, to)| Step::PutEdge {
            writer,
            key,
            from,
            to
        }),
        (0u8..3, any::<bool>(), 0u8..6).prop_map(|(writer, edge, key)| Step::Retract { writer, edge, key }),
        (0u8..3, prop::collection::vec(0u8..6, 1..4))
            .prop_map(|(writer, members)| Step::Group { writer, members }),
        (0u8..3, any::<usize>()).prop_map(|(writer, of)| Step::Undo { writer, of }),
        (any::<usize>(), any::<bool>()).prop_map(|(of, accept)| Step::Review { of, accept }),
        (0u8..6, any::<bool>()).prop_map(|(key, release)| Step::Claim { key, release }),
        Just(Step::Version),
        Just(Step::Card),
        (0u8..6, 0u8..6).prop_map(|(node, value)| Step::CardGhost { node, value }),
        (any::<usize>(), any::<bool>()).prop_map(|(of, accept)| Step::Pick { of, accept }),
        (0u8..6, 0u8..6).prop_map(|(node, value)| Step::InboxGhost { node, value }),
        any::<usize>().prop_map(|of| Step::UndoAsEdit { of }),
    ]
}

/// The steps `review`, `edit undo` and inbox rows take on cards, beside plain edits and scans:
/// the writes that keep a card open exactly while a ghost waits on it.
fn card_step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0u8..3).prop_map(Step::Scan),
        (0u8..3, 0u8..4, 0u8..6, 0u8..6).prop_map(|(writer, status, node, value)| Step::Set {
            writer,
            status,
            node,
            field: 0,
            value
        }),
        (0u8..6, 0u8..6).prop_map(|(node, value)| Step::InboxGhost { node, value }),
        (any::<usize>(), any::<bool>()).prop_map(|(of, accept)| Step::Pick { of, accept }),
        any::<usize>().prop_map(|of| Step::UndoAsEdit { of }),
    ]
}

fn put_node(key: u8, parent: u8) -> Edit {
    let mut fields = BTreeMap::from([
        ("kind".to_owned(), json!("k.p")),
        ("level".to_owned(), json!("component")),
        ("name".to_owned(), json!(format!("p{key}"))),
    ]);
    if parent < NODES.len() as u8 {
        fields.insert("parent".into(), json!(NODES[parent as usize]));
    }
    Edit {
        op: Op::Put,
        target: Target::Node(id(&format!("r:x:p{key}"))),
        layer: Layer::Intended,
        fields,
        base: 0,
        over: BTreeMap::new(),
    }
}

/// The events and the facts in force after each one.
fn history(steps: &[Step]) -> (Vec<Event>, Vec<Facts>) {
    let mut log = Log::default();
    let mut facts_after = Vec::new();
    let mut current = facts_variant(0);
    for step in steps {
        let before = log.events.len();
        let pick = |n: usize| (before > 0).then(|| log.events[n % before].id.clone());
        match step.clone() {
            Step::Scan(v) => {
                log.push(&system(), Status::Applied, scan(u64::from(v) + 1));
                current = facts_variant(v);
            }
            Step::Set { writer: w, status: s, node, field, value } => {
                let target = NODES[node as usize];
                let (field, value) = match field {
                    0 => ("name".to_owned(), json!(format!("v{value}"))),
                    1 => ("parent".to_owned(), json!(NODES[value as usize])),
                    2 => ("hidden".to_owned(), json!(value % 2 == 0)),
                    3 => ("props.p".to_owned(), if value == 0 { Value::Null } else { json!(value) }),
                    _ => (format!("pos@map:v{}", value % 2), json!({"x": value, "y": 1})),
                };
                let mut edit = set_node(target, &field, value);
                edit.over = BTreeMap::from([(field, json!("item"))]);
                log.push(&writer(w), status(s), EventBody::Edit(edit));
            }
            Step::PutNode { writer: w, status: s, key, parent } => {
                log.push(&writer(w), status(s), EventBody::Edit(put_node(key, parent)));
            }
            Step::PutEdge { writer: w, key, from, to } => {
                let e = edge(10 + key, NODES[from as usize], NODES[to as usize]);
                let mut fields: BTreeMap<String, Value> =
                    serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
                fields.remove("provenance");
                let edit = Edit {
                    op: Op::Put,
                    target: Target::Edge(e.id.clone()),
                    layer: Layer::Intended,
                    fields,
                    base: 0,
                    over: BTreeMap::new(),
                };
                log.push(&writer(w), Status::Applied, EventBody::Edit(edit));
            }
            Step::Retract { writer: w, edge: is_edge, key } => {
                let target = if is_edge {
                    Target::Edge(EdgeId(format!("e:{:012x}", if key % 2 == 0 { 1 } else { 10 + key % 3 })))
                } else {
                    Target::Node(id(NODES[key as usize]))
                };
                let edit = Edit {
                    op: Op::Retract,
                    target,
                    layer: Layer::Actual,
                    fields: BTreeMap::new(),
                    base: 0,
                    over: BTreeMap::new(),
                };
                log.push(&writer(w), Status::Applied, EventBody::Edit(edit));
            }
            Step::Group { writer: w, members } => {
                let mut bodies = vec![EventBody::Edit(put_node(9, 0))];
                bodies.extend(
                    members
                        .iter()
                        .map(|m| EventBody::Edit(set_node(NODES[*m as usize], "parent", json!("r:x:p9")))),
                );
                log.command(&writer(w), Status::Applied, bodies);
            }
            Step::Undo { writer: w, of } => {
                if let Some(of) = pick(of) {
                    let status = if w == 2 { Status::Conflict } else { Status::Applied };
                    log.push(&writer(w), status, EventBody::Undo { of });
                }
            }
            Step::Review { of, accept } => {
                if let Some(of) = pick(of) {
                    let verdict = if accept { Verdict::Accept } else { Verdict::Reject };
                    log.push(&tab(1), Status::Applied, EventBody::Review { of, verdict });
                }
            }
            Step::Claim { key, release } => {
                let body = EventBody::Claim {
                    target: Target::Node(id(NODES[key as usize])),
                    until: Timestamp("2026-10-26T15:30:00Z".into()),
                    release,
                };
                log.push(&agent(), Status::Applied, body);
            }
            Step::Version => {
                log.push(&tab(1), Status::Applied, EventBody::Version { project: None, label: "v".into() });
            }
            Step::Card => {
                log.push(&system(), Status::Open, card_body());
            }
            Step::CardGhost { node, value } => {
                let card = log.events.iter().rev().find(|e| matches!(e.body, EventBody::Card(_)));
                if let Some(card) = card.map(|c| c.id.clone()) {
                    let edit = set_node(NODES[node as usize], "name", json!(format!("g{value}")));
                    log.command_of(
                        &agent(),
                        vec![(Status::Ghost, Some(Cause::Event(card)), EventBody::Edit(edit))],
                    );
                }
            }
            Step::Pick { of, accept } => {
                if let Some(of) = pick(of) {
                    pick_as_review(&mut log, &current, &of, accept);
                }
            }
            Step::InboxGhost { node, value } => {
                let folded = log.fold(&current);
                let card = match folded.open_cards.last() {
                    Some(open) => open.clone(),
                    None => log.push(&system(), Status::Open, card_body()),
                };
                let edit = set_node(NODES[node as usize], "name", json!(format!("i{value}")));
                log.command_of(
                    &agent(),
                    vec![(Status::Ghost, Some(Cause::Event(card)), EventBody::Edit(edit))],
                );
            }
            Step::UndoAsEdit { of } => {
                if let Some(of) = pick(of) {
                    undo_as_edit(&mut log, &current, &of);
                }
            }
        }
        while facts_after.len() < log.events.len() {
            facts_after.push(current.clone());
        }
    }
    (log.events, facts_after)
}

/// The events `review` writes for a person's pick of `of`, planned over the fold as it stands:
/// nothing for an item that isn't open, as `review` refuses it.
fn pick_as_review(log: &mut Log, facts: &Facts, of: &EventId, accept: bool) {
    let folded = log.fold(facts);
    let verdict = if accept { Verdict::Accept } else { Verdict::Reject };
    let found = log.events.iter().find(|e| &e.id == of).cloned().unwrap();
    let review = (Status::Applied, None, EventBody::Review { of: of.clone(), verdict });
    match (&found.body, found.status) {
        // A card is picked whole only while no ghost waits on it; the Apply card's ghosts are
        // reviewed one by one.
        (EventBody::Card(_), _)
            if folded.open_cards.contains(of)
                && folded.ghosts_of(of).iter().all(|g| folded.verdict(g).is_some()) =>
        {
            log.command_of(&tab(1), vec![review]);
        }
        (EventBody::Edit(edit), Status::Ghost | Status::Conflict)
            if folded.verdict(of).is_none() && !folded.is_undone(of) =>
        {
            let mut parts = vec![review];
            if accept {
                let copy = copy_for_review(edit, log.events.len() as Seq);
                parts.push((Status::Applied, Some(Cause::Event(of.clone())), EventBody::Edit(copy)));
            }
            log.command_of(&tab(1), parts);
            // The card's own review, with its last open ghost's, keyed apart from that review.
            if let Some(Cause::Event(card)) = &found.cause
                && folded.open_cards.contains(card)
                && folded.ghosts_of(card).iter().all(|g| g == of || folded.verdict(g).is_some())
            {
                let accepted = accept
                    || folded.ghosts_of(card).iter().any(|g| folded.verdict(g) == Some(Verdict::Accept));
                let verdict = if accepted { Verdict::Accept } else { Verdict::Reject };
                log.push(&tab(1), Status::Applied, EventBody::Review { of: card.clone(), verdict });
            }
        }
        _ => {}
    }
}

/// The events `edit undo` writes for a person's undo of `of`'s command, planned over the fold as
/// it stands: the undo, an undo of each card review that would stand while one of its card's
/// ghosts waits again, and a review, keyed apart, of each card that would be left open with none
/// waiting. Nothing for a card or an event already undone, as `edit undo` refuses them.
fn undo_as_edit(log: &mut Log, facts: &Facts, of: &EventId) {
    let folded = log.fold(facts);
    let found = log.events.iter().find(|e| &e.id == of).cloned().unwrap();
    if matches!(found.body, EventBody::Card(_) | EventBody::Scan { .. }) || folded.is_undone(of) {
        return;
    }
    let after = folded.after_undo(of);
    let mut undos = vec![EventBody::Undo { of: of.clone() }];
    let mut closes = Vec::new();
    for card in folded.cards() {
        let left = after.ghosts_of(card);
        if after.is_undone(card) || (left.is_empty() && folded.ghosts_of(card).is_empty()) {
            continue;
        }
        let waiting = left.iter().any(|g| after.verdict(g).is_none());
        match (after.card_open(card), waiting) {
            (true, false) => {
                let accepted = left.iter().any(|g| after.verdict(g) == Some(Verdict::Accept));
                closes.push((card.clone(), if accepted { Verdict::Accept } else { Verdict::Reject }));
            }
            (false, true) => {
                undos.extend(after.reviews_of(card).into_iter().map(|of| EventBody::Undo { of }))
            }
            _ => {}
        }
    }
    log.command(&tab(1), Status::Applied, undos);
    for (card, verdict) in closes {
        log.push(&tab(1), Status::Applied, EventBody::Review { of: card, verdict });
    }
}

/// Whether every card that holds ghosts is open exactly while one of them waits, as `review` and
/// `edit undo` keep it.
fn cards_follow_their_ghosts(folded: &Folded) -> bool {
    folded.cards().iter().all(|card| {
        let ghosts = folded.ghosts_of(card);
        let waiting = ghosts.iter().any(|g| folded.verdict(g).is_none());
        ghosts.is_empty() || folded.open_cards.contains(card) == waiting
    })
}

/// The events the store's open-card read returns: every card and undo, each review of a card, and
/// each event an undo names.
fn card_events(events: &[Event]) -> Vec<Event> {
    let cards: Vec<&EventId> =
        events.iter().filter(|e| matches!(e.body, EventBody::Card(_))).map(|e| &e.id).collect();
    let named: Vec<&EventId> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::Undo { of } => Some(of),
            _ => None,
        })
        .collect();
    events
        .iter()
        .filter(|e| match &e.body {
            EventBody::Card(_) | EventBody::Undo { .. } => true,
            EventBody::Review { of, .. } => cards.contains(&of),
            _ => false,
        } || named.contains(&&e.id))
        .cloned()
        .collect()
}

proptest! {
    // No I/O in the kernel's tests: a failing case is printed, never written to a file.
    #![proptest_config(Config { failure_persistence: None, cases: 256, ..Config::default() })]

    #[test]
    fn incremental_fold_equals_full_fold(steps in prop::collection::vec(step(), 1..40)) {
        let (events, facts_after) = history(&steps);
        let vocabulary = Vocabulary::default();
        let mut kept = fold_full(&facts_variant(0), &[], &vocabulary, 0);
        for (k, event) in events.iter().enumerate() {
            fold_one(&mut kept, &facts_after[k], &events[..k], &vocabulary, event);
            let full = fold_full(&facts_after[k], &events[..=k], &vocabulary, event.seq);
            prop_assert_eq!(&kept, &full, "after event {} of {:?}", k, steps);
            // The open cards the store's read and the kernel decide are the fold's.
            prop_assert_eq!(&open_cards(&card_events(&events[..=k])), &full.open_cards, "after event {} of {:?}", k, steps);
        }
        // Applying an event twice changes nothing.
        if let (Some(last), Some(facts)) = (events.last(), facts_after.last()) {
            let before = kept.clone();
            fold_one(&mut kept, facts, &events, &vocabulary, last);
            prop_assert_eq!(kept, before);
        }
    }

    // Through inbox rows, reviews and undos in any order, a card that holds ghosts is open exactly
    // while one of them waits, in the fold and in the store's open-card read alike: an undo that
    // sets a ghost waiting again reopens its card, and one that leaves none waiting closes it.
    #[test]
    fn cards_stay_open_exactly_while_a_ghost_waits(steps in prop::collection::vec(card_step(), 1..40)) {
        let vocabulary = Vocabulary::default();
        let (events, facts_after) = history(&steps);
        for k in 0..events.len() {
            let full = fold_full(&facts_after[k], &events[..=k], &vocabulary, events[k].seq);
            prop_assert_eq!(&open_cards(&card_events(&events[..=k])), &full.open_cards, "after event {} of {:?}", k, steps);
        }
        // The rule holds once each write commits; inside one, a ghost's review comes before its
        // card's.
        for n in 1..=steps.len() {
            let (events, facts_after) = history(&steps[..n]);
            let (Some(last), Some(facts)) = (events.last(), facts_after.last()) else { continue };
            let full = fold_full(facts, &events, &vocabulary, last.seq);
            prop_assert!(cards_follow_their_ghosts(&full), "after step {} of {:?}", n, steps);
        }
    }
}

// Fact merging (DR-42): when two extractors emit one node ID, evidence unions and scalar fields take
// the more specific tier's value. The fold's facts come from this merge.

fn merge_line(path: &str, line: u32) -> planroom_contract::Evidence {
    planroom_contract::Evidence::Line { path: path.into(), line, contains: None, commit: None, branch: None }
}

fn merge_node(id: &str, tier: Tier, name: &str, parent: &str, evidence: planroom_contract::Evidence) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("k.x".into()),
        level: Level::Code,
        parent: Some(NodeId(parent.into())),
        name: name.into(),
        aliases: vec![format!("alias:{name}")],
        props: BTreeMap::from([(format!("from_{name}"), json!(true)), ("shared".to_owned(), json!(name))]),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: format!("p/{name}"), tier },
        evidence: vec![evidence],
    }
}

fn merge_parent(id: &str) -> Node {
    let mut n = merge_node(id, Tier::Universal, id, "r:x:none", merge_line("p", 1));
    n.parent = None;
    n.level = Level::Component;
    n
}

fn merge_edge(tier: Tier, evidence: planroom_contract::Evidence, uses: &str) -> Edge {
    Edge {
        id: EdgeId("e:0000000000aa".into()),
        kind: Kind("k.y".into()),
        from: Ref::Id(NodeId("r:x:file".into())),
        to: Ref::Id(NodeId("r:x:dir".into())),
        via: None,
        via_ref: None,
        props: BTreeMap::from([("uses".to_owned(), json!(uses))]),
        evidence: vec![evidence],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier },
    }
}

#[test]
fn fact_merge_by_tier() {
    // The universal tier sees the file under its directory; the pattern tier moves it under a
    // container; the syntactic tier names it. Each tier above the next wins its scalar fields.
    let universal =
        merge_node("r:x:file", Tier::Universal, "universal", "r:x:dir", merge_line("tools/hc.rb", 1));
    let pattern = merge_node("r:x:file", Tier::Pattern, "pattern", "r:x:box", merge_line("tools/hc.rb", 3));
    let syntactic =
        merge_node("r:x:file", Tier::Syntactic, "syntactic", "r:x:box", merge_line("tools/hc.rb", 3));
    let facts = Facts {
        nodes: vec![universal, merge_parent("r:x:dir"), pattern, merge_parent("r:x:box"), syntactic],
        edges: vec![
            merge_edge(Tier::Universal, merge_line("tools/hc.rb", 1), "plain"),
            merge_edge(Tier::Config, merge_line("tools/hc.rb", 2), "config"),
        ],
        hints: vec![],
    };
    let resolved = planroom_kernel::resolve(&facts, &Vocabulary::default());

    let ids: Vec<&str> = resolved.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(ids, ["r:x:box", "r:x:dir", "r:x:file"], "one node per ID, sorted");
    let file = &resolved.nodes[2];
    assert_eq!(file.name, "syntactic", "the most specific tier names it");
    assert_eq!(
        file.provenance,
        Provenance::Extracted { extractor: "p/syntactic".into(), tier: Tier::Syntactic }
    );
    assert_eq!(file.parent, Some(NodeId("r:x:box".into())), "the more specific tier's parent wins");
    assert_eq!(file.props["shared"], json!("syntactic"), "a shared prop takes the specific tier's value");
    assert_eq!(file.props.len(), 4, "props fill in from every tier: {:?}", file.props);
    assert_eq!(
        file.evidence,
        [merge_line("tools/hc.rb", 3), merge_line("tools/hc.rb", 1)],
        "evidence unions once"
    );
    assert_eq!(file.aliases, ["alias:pattern", "alias:syntactic", "alias:universal"]);

    // Edges of one ID merge the same way.
    assert_eq!(resolved.edges.len(), 1);
    let merged = &resolved.edges[0];
    assert_eq!(merged.props["uses"], json!("config"));
    assert_eq!(merged.evidence, [merge_line("tools/hc.rb", 2), merge_line("tools/hc.rb", 1)]);

    // A tie between two facts of one tier goes to the one emitted first, whatever the input order
    // of other IDs.
    let first = merge_node("r:x:t", Tier::Config, "first", "r:x:dir", merge_line("a", 1));
    let second = merge_node("r:x:t", Tier::Config, "second", "r:x:dir", merge_line("a", 2));
    let tied = planroom_kernel::resolve(
        &Facts { nodes: vec![first, merge_parent("r:x:dir"), second], ..Facts::default() },
        &Vocabulary::default(),
    );
    assert_eq!(tied.nodes.iter().find(|n| n.id.0 == "r:x:t").map(|n| n.name.as_str()), Some("first"));
}

/// A `scan` event that records `renames`, old ID to new.
fn scan_renaming(snapshot: u64, renames: &[(&str, &str)]) -> EventBody {
    let EventBody::Scan { repo, commit, branch, link, stats, .. } = scan(snapshot) else { unreachable!() };
    let renames = renames.iter().map(|(from, to)| Rename { from: id(from), to: id(to) }).collect();
    EventBody::Scan { repo, commit, branch, snapshot, link, renames, stats }
}

/// A repo with a directory `src` and the given files and directories under it, each a fact.
fn tree(paths: &[&str]) -> Facts {
    let mut nodes =
        vec![node("r:x:repo", Level::System, None), node("r:d:src", Level::Component, Some("r:x:repo"))];
    for path in paths {
        let (up, _) = path.rsplit_once('/').unwrap();
        let file = path.contains('.');
        let parent = format!("r:d:{up}");
        let level = if file { Level::Code } else { Level::Component };
        let key = format!("r:{}:{path}", if file { "f" } else { "d" });
        let mut n = node(&key, level, Some(&parent));
        n.name = path.rsplit('/').next().unwrap().into();
        nodes.push(n);
    }
    Facts { nodes, ..Facts::default() }
}

fn followed(folded: &Folded) -> Vec<&FoldDiff> {
    folded.diffs.iter().filter(|d| matches!(d, FoldDiff::Followed { .. })).collect()
}

#[test]
fn an_override_follows_a_git_rename() {
    let mut log = Log::default();
    log.push(&system(), Status::Applied, scan(1));
    let mut hide = set_node("r:f:src/a.ts", "hidden", json!(true));
    hide.fields.insert("props.owner".into(), json!("payments"));
    let edit = log.edit(&tab(1), hide);
    log.edit(&tab(1), set_node("r:f:src/a.ts", "name", json!("Entry")));
    // git reports `src/a.ts` renamed to `src/b.ts`, then `src/b.ts` to `src/c.ts`.
    log.push(&system(), Status::Applied, scan_renaming(2, &[("r:f:src/a.ts", "r:f:src/b.ts")]));
    let later = log.edit(&tab(2), set_node("r:f:src/b.ts", "props.team", json!("checkout")));
    log.push(&system(), Status::Applied, scan_renaming(3, &[("r:f:src/b.ts", "r:f:src/c.ts")]));
    let folded = log.fold(&tree(&["src/c.ts"]));

    let c = folded.model.nodes.iter().find(|n| n.id.0 == "r:f:src/c.ts").unwrap();
    assert_eq!(c.name, "Entry", "the rename followed both moves");
    assert_eq!(c.props.get("owner"), Some(&json!("payments")));
    assert_eq!(c.props.get("team"), Some(&json!("checkout")), "a later edit follows only later moves");
    assert!(folded.overlay.hidden.contains(&planroom_kernel::fold::TargetKey::Node(id("r:f:src/c.ts"))));
    let rev = folded.revision(&Target::Node(id("r:f:src/c.ts")), "props.owner").expect("a revision");
    assert_eq!(rev.event, edit, "compare-and-set reads the moved node's revision");
    assert!(
        folded.revision(&Target::Node(id("r:f:src/c.ts")), "props.team").is_some_and(|r| r.event == later)
    );
    assert!(
        !folded.diffs.iter().any(|d| matches!(d, FoldDiff::TargetVanished { .. })),
        "nothing vanished: {:?}",
        folded.diffs
    );
    // Each move is listed once, though three edits followed the first.
    assert_eq!(
        followed(&folded),
        [
            &FoldDiff::Followed { event: edit.clone(), from: id("r:f:src/a.ts"), to: id("r:f:src/c.ts") },
            &FoldDiff::Followed { event: later, from: id("r:f:src/b.ts"), to: id("r:f:src/c.ts") },
        ]
    );

    // Folded before the renames, the edits stay on the file as it was.
    let before = fold_full(&tree(&["src/a.ts"]), &log.events, &Vocabulary::default(), 3);
    assert_eq!(name_of(&before, "r:f:src/a.ts"), "Entry");
    assert!(followed(&before).is_empty());

    // A node the facts still hold is never moved, whatever git reported.
    let both = log.fold(&tree(&["src/a.ts", "src/c.ts"]));
    assert_eq!(name_of(&both, "r:f:src/a.ts"), "Entry");
    assert_eq!(name_of(&both, "r:f:src/c.ts"), "c.ts");

    // A node no rename reaches is listed as vanished, as before.
    let mut log = Log::default();
    let gone = log.edit(&tab(1), set_node("r:f:src/a.ts", "name", json!("Entry")));
    log.push(&system(), Status::Applied, scan_renaming(2, &[("r:f:src/x.ts", "r:f:src/c.ts")]));
    let folded = log.fold(&tree(&["src/c.ts"]));
    assert!(followed(&folded).is_empty());
    assert!(
        folded
            .diffs
            .contains(&FoldDiff::TargetVanished { event: gone, target: Target::Node(id("r:f:src/a.ts")) })
    );
}

#[test]
fn a_moved_directory_carries_its_regroup() {
    let mut log = Log::default();
    log.push(&system(), Status::Applied, scan(1));
    // The regroup: a group put, and the directory moved under it, in one command.
    let group = Edit {
        op: Op::Put,
        target: Target::Node(id("r:g:purchase")),
        layer: Layer::Intended,
        fields: BTreeMap::from([
            ("kind".to_owned(), json!("k.group")),
            ("level".to_owned(), json!("component")),
            ("name".to_owned(), json!("purchase")),
            ("parent".to_owned(), json!("r:x:repo")),
        ]),
        base: 0,
        over: BTreeMap::new(),
    };
    let mut move_cart = set_node("r:d:src/cart", "parent", json!("r:g:purchase"));
    move_cart.over = BTreeMap::from([("parent".to_owned(), json!("r:d:src"))]);
    let regroup =
        log.command(&tab(1), Status::Applied, vec![EventBody::Edit(group), EventBody::Edit(move_cart)]);
    // `git mv src/cart src/basket`: the directory and its file.
    log.push(
        &system(),
        Status::Applied,
        scan_renaming(2, &[("r:f:src/cart/a.ts", "r:f:src/basket/a.ts"), ("r:d:src/cart", "r:d:src/basket")]),
    );
    let folded = log.fold(&tree(&["src/basket", "src/basket/a.ts"]));
    assert_eq!(parent_of(&folded, "r:d:src/basket").as_deref(), Some("r:g:purchase"), "the regroup followed");
    assert_eq!(parent_of(&folded, "r:f:src/basket/a.ts").as_deref(), Some("r:d:src/basket"));
    assert_eq!(
        followed(&folded),
        [&FoldDiff::Followed {
            event: regroup[1].clone(),
            from: id("r:d:src/cart"),
            to: id("r:d:src/basket")
        }],
        "the move is listed once"
    );
    assert!(
        !folded.diffs.iter().any(|d| matches!(
            d,
            FoldDiff::TargetVanished { .. }
                | FoldDiff::ParentFallback { .. }
                | FoldDiff::ScanChangedUnderEdit { .. }
        )),
        "{:?}",
        folded.diffs
    );
    // The regroup's undo still reverses the moved directory's parent.
    assert!(
        folded
            .undo_keys(&regroup[0])
            .contains(&(planroom_kernel::fold::TargetKey::Node(id("r:d:src/basket")), "parent".to_owned()))
    );

    // A move into a directory that was renamed follows it too.
    let mut log = Log::default();
    let into = log.edit(&tab(1), set_node("r:f:src/b.ts", "parent", json!("r:d:src/cart")));
    log.push(&system(), Status::Applied, scan_renaming(2, &[("r:d:src/cart", "r:d:src/basket")]));
    let folded = log.fold(&tree(&["src/basket", "src/b.ts"]));
    assert_eq!(parent_of(&folded, "r:f:src/b.ts").as_deref(), Some("r:d:src/basket"));
    assert_eq!(
        followed(&folded),
        [&FoldDiff::Followed { event: into, from: id("r:d:src/cart"), to: id("r:d:src/basket") }]
    );
}

/// The model is what a fold returns, never what it reads: a caller that changes the model it was
/// handed changes no fact value, no undo's parents, and nothing a later event folds over.
#[test]
fn a_caller_changing_the_model_changes_nothing_the_fold_reads() {
    let facts = facts();
    let vocabulary = Vocabulary::default();
    let mut log = Log::default();
    let moved = log.edit(&tab(1), set_node("r:x:item", "parent", json!("r:x:b")));
    log.edit(&tab(1), set_node("r:x:a", "name", json!("A")));
    log.edit(&tab(1), set_node("r:x:b", "name", json!("B")));
    let (before, last) = log.events.split_at(2);
    let kept = fold_full(&facts, before, &vocabulary, 2);
    let mut changed = kept.clone();
    changed.model.nodes.reverse();
    changed.model.nodes.truncate(1);
    for node in &mut changed.model.nodes {
        node.name = "changed".into();
        node.parent = None;
    }
    changed.model.edges.clear();
    let item = Target::Node(id("r:x:item"));
    for field in ["name", "parent"] {
        assert_eq!(changed.fact_value(&item, field), kept.fact_value(&item, field), "{field}");
    }
    assert_eq!(changed.parents_after_undo(&moved), kept.parents_after_undo(&moved));
    let mut want = kept.clone();
    fold_one(&mut want, &facts, before, &vocabulary, &last[0]);
    fold_one(&mut changed, &facts, before, &vocabulary, &last[0]);
    assert_eq!(changed.model, want.model);
    assert_eq!(changed.model, fold_full(&facts, &log.events, &vocabulary, 3).model);
}
