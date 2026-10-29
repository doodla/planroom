//! `kernel::ladder::Ladder`: levels are pack data, so the kernel orders them only through the
//! ladder a vocabulary declares, the kernel's `intent` apart and then the chain top to finest.
//! Every ladder here is invented, and the views, roll-up, lookup and search run on it as on any
//! other. Every input is built as a value: the kernel's tests do no I/O.

use planroom_contract::{
    Edge, EdgeId, Kind, Layer, LevelId, LevelVocab, Node, NodeId, Provenance, Ref, Tier, Vocabulary,
};
use planroom_kernel::find::find;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::query::lookup::{Lookup, lookup};
use planroom_kernel::rollup::lift;
use planroom_kernel::tree::{blocks_at, top_level};
use planroom_kernel::{Facts, Model, resolve};

fn level(id: &str, apart: bool) -> LevelVocab {
    LevelVocab {
        id: LevelId::from(id),
        label: id.into(),
        one: "{count} thing".into(),
        many: "{count} things".into(),
        apart,
        pack: if apart { "kernel" } else { "made_up" }.into(),
    }
}

/// A vocabulary whose ladder is `intent`, then `chain` top to finest.
fn ladder_of(chain: &[&str]) -> Vocabulary {
    let mut levels = vec![level(LevelId::INTENT, true)];
    levels.extend(chain.iter().map(|id| level(id, false)));
    Vocabulary { levels, ..Vocabulary::default() }
}

fn l(id: &str) -> LevelId {
    LevelId::from(id)
}

fn node(id: &str, at: &str, parent: Option<&str>, name: &str) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("made_up.thing".into()),
        level: l(at),
        parent: parent.map(|p| NodeId(p.into())),
        name: name.into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "made_up/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

fn edge(n: u32, from: &str, to: &str) -> Edge {
    Edge {
        id: EdgeId(format!("e:{n:012x}")),
        kind: Kind("made_up.uses".into()),
        from: Ref::Id(NodeId(from.into())),
        to: Ref::Id(NodeId(to.into())),
        via: None,
        via_ref: None,
        props: Default::default(),
        evidence: vec![],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "made_up/x".into(), tier: Tier::Universal },
    }
}

fn ids<T>(items: &[T], id: impl Fn(&T) -> &NodeId) -> Vec<String> {
    items.iter().map(|i| id(i).0.clone()).collect()
}

const ROOMS: [&str; 4] = ["realm", "region", "site", "room"];

#[test]
fn the_ladder_puts_intent_apart_then_the_chain_top_to_finest() {
    let vocabulary = ladder_of(&ROOMS);
    let ladder = Ladder::of(&vocabulary);
    let ranks: Vec<u8> =
        ["intent", "realm", "region", "site", "room", "attic"].iter().map(|id| ladder.rank(&l(id))).collect();
    assert_eq!(ranks, [0, 1, 2, 3, 4, 5], "intent, the chain top to finest, then a level it doesn't hold");
    assert_eq!(ladder.top(), &l("realm"));
    assert_eq!(ladder.finest(), &l("room"));
    assert!(ladder.is_apart(&l("intent")));
    assert!(!ladder.is_apart(&l("realm")));
    assert!(ladder.holds(&l("site")) && ladder.holds(&l("intent")));
    assert!(!ladder.holds(&l("attic")));
    assert_eq!(ladder.levels(), vocabulary.levels.as_slice());
    // Intent is apart whether or not the vocabulary lists it.
    let unlisted = Vocabulary { levels: vocabulary.levels[1..].to_vec(), ..Vocabulary::default() };
    let unlisted = Ladder::of(&unlisted);
    assert!(unlisted.is_apart(&l("intent")));
    assert_eq!(unlisted.rank(&l("intent")), 0);
    assert_eq!(unlisted.rank(&l("realm")), 1);
}

#[test]
fn below_opens_the_top_from_intent_and_stops_at_the_finest() {
    let vocabulary = ladder_of(&ROOMS);
    let ladder = Ladder::of(&vocabulary);
    assert_eq!(ladder.below(&l("intent")), &l("realm"));
    assert_eq!(ladder.below(&l("realm")), &l("region"));
    assert_eq!(ladder.below(&l("site")), &l("room"));
    assert_eq!(ladder.below(&l("room")), &l("room"));
}

#[test]
fn above_is_none_at_the_top_and_at_intent() {
    let vocabulary = ladder_of(&ROOMS);
    let ladder = Ladder::of(&vocabulary);
    assert_eq!(ladder.above(&l("realm")), None);
    assert_eq!(ladder.above(&l("intent")), None);
    assert_eq!(ladder.above(&l("region")), Some(&l("realm")));
    assert_eq!(ladder.above(&l("room")), Some(&l("site")));
}

#[test]
fn lookup_tries_the_level_then_above_then_below_on_any_ladder() {
    let vocabulary = ladder_of(&ROOMS);
    let ladder = Ladder::of(&vocabulary);
    let order = |at: &str| -> Vec<String> {
        let at = l(at);
        ladder.order_from(&at).iter().map(|l| l.to_string()).collect()
    };
    assert_eq!(order("site"), ["site", "region", "realm", "intent", "room"]);
    assert_eq!(order("intent"), ["intent", "realm", "region", "site", "room"]);
    assert_eq!(order("attic"), ["attic", "room", "site", "region", "realm", "intent"]);

    let nodes = vec![
        node("a:realm:web", "realm", None, "web"),
        node("a:site:web", "site", None, "web"),
        node("a:room:web", "room", None, "web"),
        node("b:room:hall", "room", None, "hall"),
        node("c:room:hall", "room", None, "hall"),
    ];
    let found = |target: &str, at: &str| match lookup(&nodes, target, &l(at), &ladder) {
        Lookup::Found(n) => n.id.0.clone(),
        other => panic!("{target} at {at}: {other:?}"),
    };
    assert_eq!(found("web", "site"), "a:site:web");
    assert_eq!(found("web", "region"), "a:realm:web", "the level above comes before the one below");
    assert_eq!(found("web", "room"), "a:room:web");
    assert_eq!(found("web", "intent"), "a:realm:web", "from intent the top opens first");
    assert!(matches!(lookup(&nodes, "hall", &l("realm"), &ladder), Lookup::Ambiguous(v) if v.len() == 2));
    assert!(matches!(lookup(&nodes, "nowhere", &l("realm"), &ladder), Lookup::Missing(_)));
}

/// Seven levels whose names sort the other way from their ladder order, so any comparison that
/// falls back to the strings' order puts the blocks, the roll-up's ends, the lookup and the
/// search's ties wrong.
#[test]
fn the_kernel_runs_on_an_invented_seven_level_ladder() {
    const SEVEN: [&str; 7] = ["zone", "yard", "ward", "unit", "tile", "spot", "pin"];
    let vocabulary = ladder_of(&SEVEN);
    let ladder = Ladder::of(&vocabulary);
    assert_eq!(ladder.top(), &l("zone"));
    assert_eq!(ladder.finest(), &l("pin"));
    let mut nodes = Vec::new();
    // Two chains from the top to the finest, under one zone.
    for branch in ["a", "b"] {
        let mut parent: Option<String> = (branch == "b").then(|| "m:zone:a".to_owned());
        for (i, at) in SEVEN.iter().enumerate() {
            if branch == "b" && i == 0 {
                continue;
            }
            let id = format!("m:{at}:{branch}");
            nodes.push(node(&id, at, parent.as_deref(), &format!("{at}-{branch}")));
            parent = Some(id);
        }
    }
    // A finest node with no parent, named to sort before every other.
    nodes.push(node("m:pin:loose", "pin", None, "aaa"));
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let top = top_level(&nodes, &ladder);
    assert_eq!(ids(&top, |n| &n.id), ["m:zone:a", "m:pin:loose"], "the top sorts by rank, not by name");
    assert_eq!(top[0].counts.get("pin"), Some(&2));

    let wards = blocks_at(&nodes, &ladder, &l("ward"), None);
    assert_eq!(
        ids(&wards, |n| &n.id),
        ["m:ward:a", "m:ward:b", "m:pin:loose"],
        "every ward, and beside them the finer node no ward holds"
    );
    assert!(blocks_at(&nodes, &ladder, &l("intent"), None).is_empty());

    let model = Model {
        at: 1,
        nodes: nodes.clone(),
        edges: vec![edge(1, "m:pin:a", "m:pin:b"), edge(2, "m:spot:a", "m:pin:a")],
        ..Model::default()
    };
    let at_ward = lift(&model, &ladder, &l("ward"), None);
    assert_eq!(at_ward.edges.len(), 1);
    assert_eq!((at_ward.edges[0].from.0.as_str(), at_ward.edges[0].to.0.as_str()), ("m:ward:a", "m:ward:b"));
    assert_eq!(at_ward.internal_count(&NodeId("m:ward:a".into())), 1);
    let at_zone = lift(&model, &ladder, &l("zone"), None);
    assert!(at_zone.edges.is_empty());
    assert_eq!(at_zone.internal_count(&NodeId("m:zone:a".into())), 2);

    let mut named = nodes.clone();
    named.push(node("m:yard:x", "yard", Some("m:zone:a"), "x"));
    named.push(node("m:tile:x", "tile", Some("m:unit:a"), "x"));
    match lookup(&named, "x", &l("ward"), &ladder) {
        Lookup::Found(n) => assert_eq!(n.id.0, "m:yard:x", "the yard is above the ward"),
        other => panic!("{other:?}"),
    }
    named.push(node("m:zone:same", "zone", None, "same"));
    named.push(node("m:pin:same", "pin", None, "same"));
    let hits = find(&named, &ladder, "same");
    assert_eq!(hits[0].node.id.0, "m:zone:same", "a tie goes to the coarser level");
    assert_eq!(hits[1].node.id.0, "m:pin:same");
}

#[test]
fn a_level_no_pack_declares_resolves_at_the_finest() {
    let vocabulary = ladder_of(&ROOMS);
    let facts = Facts {
        nodes: vec![
            node("r:x:attic", "attic", None, "attic"),
            node("r:x:plan", "intent", None, "plan"),
            node("r:x:site", "site", None, "site"),
        ],
        ..Facts::default()
    };
    let levels: Vec<(String, String)> = resolve(&facts, &vocabulary)
        .nodes
        .into_iter()
        .map(|n| (n.id.0, n.level.as_str().to_owned()))
        .collect();
    assert_eq!(
        levels,
        [
            ("r:x:attic".to_owned(), "room".to_owned()),
            ("r:x:plan".to_owned(), "intent".to_owned()),
            ("r:x:site".to_owned(), "site".to_owned()),
        ]
    );
}

#[test]
fn an_empty_ladder_keeps_every_level_as_it_came() {
    let facts = Facts { nodes: vec![node("r:x:attic", "attic", None, "attic")], ..Facts::default() };
    let vocabulary = Vocabulary::default();
    assert_eq!(resolve(&facts, &vocabulary).nodes[0].level, l("attic"));
    let ladder = Ladder::of(&vocabulary);
    assert_eq!(ladder.top(), &l("intent"));
    assert_eq!(ladder.finest(), &l("intent"));
    assert_eq!(ladder.below(&l("intent")), &l("intent"));
    assert_eq!(ladder.rank(&l("intent")), 0);
    assert_eq!(ladder.rank(&l("attic")), 1);
}
