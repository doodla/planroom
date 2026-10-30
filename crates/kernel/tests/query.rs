//! The query shapes a question pipeline runs (architecture.md, "Lenses and questions"; DR-15):
//! `path` over a lens's relations in their flow only, `why` with every edge's evidence and
//! confidence, and `neighbors` both ways for what depends on a node. Every input is built as a
//! value: the kernel's tests do no I/O, and name no pack's kinds, relations or lenses but their
//! own.

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use planroom_contract::{
    Edge, EdgeId, Evidence, Flow, Kind, Layer, LensVocab, LevelId, LevelVocab, Node, NodeId, Provenance, Ref,
    ShapeStep, Tier, Vocabulary,
};
use planroom_kernel::Model;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::query::{self, Answer, Reason, Toward};
use serde_json::json;

fn node(id: &str, level: LevelId, parent: Option<&str>) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("t.thing".into()),
        level,
        parent: parent.map(|p| NodeId(p.into())),
        name: id.rsplit(':').next().unwrap_or(id).into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "t/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

fn line(path: &str, n: u32) -> Evidence {
    Evidence::Line {
        path: path.into(),
        line: n,
        contains: Some(format!("line {n}")),
        commit: None,
        branch: None,
    }
}

fn edge(n: u32, kind: &str, from: &str, to: &str) -> Edge {
    Edge {
        id: EdgeId(format!("e:{n:012x}")),
        kind: Kind(kind.into()),
        from: Ref::Id(NodeId(from.into())),
        to: Ref::Id(NodeId(to.into())),
        via: None,
        via_ref: None,
        props: Default::default(),
        evidence: vec![line(&format!("{from}.src"), n)],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "t/x".into(), tier: Tier::Syntactic },
    }
}

fn lens(id: &str, relations: &[&str], backward: &[&str], level: LevelId) -> LensVocab {
    LensVocab {
        id: id.into(),
        label: id.into(),
        relations: relations.iter().map(|r| Kind((*r).into())).collect(),
        flow: backward.iter().map(|r| (Kind((*r).into()), Flow::Backward)).collect(),
        kinds: vec![],
        level,
        default: false,
        yarn: None,
    }
}

/// Two lenses over five boxes: `wires`, a request and message lens whose `t.takes` flows backward,
/// as a consumer reads from its queue; and `all_wires`, which adds the shared store read `t.reads`.
fn vocabulary() -> Vocabulary {
    Vocabulary {
        lenses: vec![
            lens(
                "all_wires",
                &["t.calls", "t.sends", "t.takes", "t.reads"],
                &["t.takes"],
                LevelId::from("container"),
            ),
            lens("wires", &["t.calls", "t.sends", "t.takes"], &["t.takes"], LevelId::from("container")),
            lens("uses", &["t.uses"], &[], LevelId::from("component")),
        ],
        levels: levels(),
        ..Vocabulary::default()
    }
}

/// These tests' ladder: intent apart, then `LEVELS`' chain top to finest.
fn levels() -> Vec<LevelVocab> {
    LEVELS
        .iter()
        .map(|id| LevelVocab {
            id: LevelId::from(*id),
            label: (*id).into(),
            one: "{count} node".into(),
            many: "{count} nodes".into(),
            apart: *id == LevelId::INTENT,
            pack: "test".into(),
        })
        .collect()
}

/// Five boxes, each with a file that holds its edges: web calls api, api sends to bus, job takes
/// from bus; web and job both read the store; web uses job's library directly.
fn boxes() -> Model {
    let mut nodes = Vec::new();
    for b in ["api", "bus", "job", "store", "web"] {
        nodes.push(node(&format!("ws:box:{b}"), LevelId::from("container"), None));
        nodes.push(node(&format!("{b}:file:main"), LevelId::from("code"), Some(&format!("ws:box:{b}"))));
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let edges = vec![
        edge(1, "t.calls", "web:file:main", "api:file:main"),
        edge(2, "t.sends", "api:file:main", "ws:box:bus"),
        edge(3, "t.takes", "job:file:main", "ws:box:bus"),
        edge(4, "t.reads", "web:file:main", "ws:box:store"),
        edge(5, "t.reads", "job:file:main", "ws:box:store"),
        edge(6, "t.uses", "web:file:main", "job:file:main"),
        edge(7, "t.reads", "web:file:main", "job:file:main"),
    ];
    Model { at: 1, nodes, edges, ..Model::default() }
}

fn step(shape: &str, params: serde_json::Value) -> ShapeStep {
    serde_json::from_value(json!({"shape": shape, "params": params})).unwrap()
}

fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, NodeId> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), NodeId((*v).into()))).collect()
}

fn ids(answer: &Answer) -> Vec<&str> {
    answer.items.iter().map(|i| i.node.0.as_str()).collect()
}

#[test]
fn path_follows_flow_lens_only() {
    let (model, vocabulary) = (boxes(), vocabulary());
    let path = |lens: &str| {
        let steps = [step("path", json!({"from": "$A", "to": "$B", "lens": lens}))];
        query::run(&model, &vocabulary, &steps, &args(&[("A", "ws:box:web"), ("B", "ws:box:job")])).unwrap()
    };
    // Over the flow lens: request and message edges only, the consume edge walked backward from
    // the queue to its consumer, and no shared-store read or direct use as a shortcut.
    let answer = path("wires");
    assert_eq!(ids(&answer), ["ws:box:web", "ws:box:api", "ws:box:bus", "ws:box:job"]);
    assert_eq!(answer.items[0].reason, Reason::Start);
    let hops: Vec<(&str, &str, bool)> = answer.items[1..]
        .iter()
        .map(|i| match &i.reason {
            Reason::Hop(h) => (h.from.0.as_str(), h.relation.0.as_str(), h.backward),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        hops,
        [("ws:box:web", "t.calls", false), ("ws:box:api", "t.sends", false), ("ws:box:bus", "t.takes", true)]
    );
    // Each hop carries its edges and their evidence.
    for item in &answer.items[1..] {
        let Reason::Hop(hop) = &item.reason else { unreachable!() };
        assert_eq!(hop.edges.len(), 1, "{hop:?}");
        assert!(!hop.evidence.is_empty(), "{hop:?}");
        assert_eq!(hop.confidence, 1.0);
    }
    // A lens that also lifts the store read takes the one-hop shortcut it allows.
    let wider = path("all_wires");
    assert_eq!(ids(&wider), ["ws:box:web", "ws:box:job"]);
    // A consumer doesn't reach its producer against the flow.
    let steps = [step("path", json!({"from": "$A", "to": "$B", "lens": "wires"}))];
    let back =
        query::run(&model, &vocabulary, &steps, &args(&[("A", "ws:box:job"), ("B", "ws:box:web")])).unwrap();
    assert!(back.items.is_empty(), "{back:?}");
    // From a file, the path starts at the block that holds it.
    let file = query::run(&model, &vocabulary, &steps, &args(&[("A", "web:file:main"), ("B", "ws:box:job")]))
        .unwrap();
    assert_eq!(ids(&file), ["ws:box:web", "ws:box:api", "ws:box:bus", "ws:box:job"]);
}

/// Two repos whose code sits beside their boxes, as a scan places a repo's directories beside its
/// container: r1 holds boxes a, b and an empty box d, and a file of its own; r2 holds box c and a
/// file of its own.
fn repos(edges: &[(&str, &str)]) -> Model {
    let mut nodes = vec![
        node("r1:repo:r1", LevelId::from("system"), None),
        node("r2:repo:r2", LevelId::from("system"), None),
        node("r1:file:root", LevelId::from("code"), Some("r1:repo:r1")),
        node("r2:file:root", LevelId::from("code"), Some("r2:repo:r2")),
        node("r1:box:d", LevelId::from("container"), Some("r1:repo:r1")),
    ];
    for (b, repo) in [("a", "r1"), ("b", "r1"), ("c", "r2")] {
        nodes.push(node(
            &format!("{repo}:box:{b}"),
            LevelId::from("container"),
            Some(&format!("{repo}:repo:{repo}")),
        ));
        nodes.push(node(&format!("{b}:file:main"), LevelId::from("code"), Some(&format!("{repo}:box:{b}"))));
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let edges = edges.iter().enumerate().map(|(i, (f, t))| edge(i as u32 + 1, "t.calls", f, t)).collect();
    Model { at: 1, nodes, edges, ..Model::default() }
}

#[test]
fn a_path_runs_only_from_its_own_ends() {
    let vocabulary = vocabulary();
    let path = |model: &Model, from: &str, to: &str| {
        let steps = [step("path", json!({"from": "$A", "to": "$B", "lens": "wires"}))];
        let answer = query::run(model, &vocabulary, &steps, &args(&[("A", from), ("B", to)])).unwrap();
        ids(&answer).into_iter().map(str::to_owned).collect::<Vec<_>>()
    };
    // An edge from another file of a's repo is no path from a, nor from the empty box d.
    let beside = repos(&[("r1:file:root", "c:file:main")]);
    assert!(path(&beside, "r1:box:a", "r2:box:c").is_empty());
    assert!(path(&beside, "r1:box:d", "r2:box:c").is_empty());
    // From that file, or its repo, the path starts at the block the file lifts to.
    assert_eq!(path(&beside, "r1:file:root", "r2:box:c"), ["r1:repo:r1", "r2:box:c"]);
    assert_eq!(path(&beside, "r1:repo:r1", "r2:box:c"), ["r1:repo:r1", "r2:box:c"]);
    // Another edge out of the shared repo doesn't hide a direct path inside it.
    let both = repos(&[("a:file:main", "b:file:main"), ("r1:file:root", "c:file:main")]);
    assert_eq!(path(&both, "r1:box:a", "r1:box:b"), ["r1:box:a", "r1:box:b"]);
    // An edge that reaches only c's repo doesn't reach c.
    let short = repos(&[("a:file:main", "r2:file:root")]);
    assert!(path(&short, "r1:box:a", "r2:box:c").is_empty());
    assert_eq!(path(&short, "r1:box:a", "r2:repo:r2"), ["r1:box:a", "r2:repo:r2"]);
    // A file beside its box lifts to its repo, but the repo doesn't stand for it: an edge from
    // another file of r1 is no path from r1's root file, and an edge that reaches another file of
    // r1 doesn't reach it. `why` agrees.
    let other = |edges: &[(&str, &str)]| {
        let mut model = repos(edges);
        model.nodes.push(node("r1:file:other", LevelId::from("code"), Some("r1:repo:r1")));
        model.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        model
    };
    let out = other(&[("r1:file:other", "c:file:main")]);
    assert!(path(&out, "r1:file:root", "r2:box:c").is_empty());
    assert_eq!(path(&out, "r1:file:other", "r2:box:c"), ["r1:repo:r1", "r2:box:c"]);
    let into = other(&[("c:file:main", "r1:file:other")]);
    assert!(path(&into, "r2:box:c", "r1:file:root").is_empty());
    assert_eq!(path(&into, "r2:box:c", "r1:file:other"), ["r2:box:c", "r1:repo:r1"]);
    // A path may pass through the repo its end lifts to, on the way to that end.
    let through = other(&[
        ("c:file:main", "r1:file:other"),
        ("r1:file:other", "r2:file:root"),
        ("r2:file:root", "r1:file:root"),
    ]);
    assert_eq!(
        path(&through, "r2:box:c", "r1:file:root"),
        ["r2:box:c", "r1:repo:r1", "r2:repo:r2", "r1:repo:r1"]
    );
    for (from, to) in [("r1:file:root", "r2:box:c"), ("r2:box:c", "r1:file:root")] {
        let pair = args(&[("A", from), ("B", to)]);
        let why = |model: &Model| {
            query::run(model, &vocabulary, &[step("why", json!({"from": "$A", "to": "$B"}))], &pair).unwrap()
        };
        assert!(why(&out).items.is_empty() && why(&into).items.is_empty());
    }
    // From the root file, the first hop names only the edge that leaves that file.
    let two = other(&[("r1:file:other", "c:file:main"), ("r1:file:root", "c:file:main")]);
    let steps = [step("path", json!({"from": "$A", "to": "$B", "lens": "wires"}))];
    let pair = args(&[("A", "r1:file:root"), ("B", "r2:box:c")]);
    let answer = query::run(&two, &vocabulary, &steps, &pair).unwrap();
    let Reason::Hop(hop) = &answer.items[1].reason else { panic!("{answer:?}") };
    assert_eq!(hop.edges, [EdgeId("e:000000000002".into())]);
    assert_eq!(hop.evidence, [line("r1:file:root.src", 2)]);
    // The empty box d: path, neighbors and why agree that nothing joins it to c.
    let around = |shape: &str, params: serde_json::Value| {
        let pair = args(&[("A", "r1:box:d"), ("B", "r2:box:c")]);
        query::run(&beside, &vocabulary, &[step(shape, params)], &pair).unwrap()
    };
    assert!(
        around("neighbors", json!({"ids": ["$A"], "lens": "wires", "direction": "both"})).items.is_empty()
    );
    assert!(around("why", json!({"from": "$A", "to": "$B"})).items.is_empty());
}

#[test]
fn why_returns_evidence_and_confidence() {
    let mut model = boxes();
    // An inferred call from web to api through an endpoint whose own evidence counts for it.
    model.nodes.push(Node {
        evidence: vec![line("api/spec.yaml", 12)],
        ..node("api:endpoint:post", LevelId::from("code"), Some("api:file:main"))
    });
    model.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut inferred = edge(8, "t.calls", "web:file:main", "api:file:main");
    inferred.via = Some("POST /orders".into());
    inferred.via_ref = Some(Ref::Id(NodeId("api:endpoint:post".into())));
    inferred.confidence = 0.6;
    model.edges.push(inferred);
    model.edges.push(edge(9, "t.calls", "api:file:main", "web:file:main"));
    let steps = [step("why", json!({"from": "$A", "to": "$B"}))];
    let answer =
        query::run(&model, &vocabulary(), &steps, &args(&[("A", "ws:box:web"), ("B", "ws:box:api")]))
            .unwrap();
    let edges: Vec<_> = answer
        .items
        .iter()
        .map(|i| match &i.reason {
            Reason::Edge(e) => e,
            other => panic!("{other:?}"),
        })
        .collect();
    // Every edge between the two, whatever lens: web's two calls first, then api's call back.
    let listed: Vec<(&str, bool)> = edges.iter().map(|e| (e.edge.0.as_str(), e.back)).collect();
    assert_eq!(listed, [("e:000000000001", false), ("e:000000000008", false), ("e:000000000009", true)]);
    assert_eq!(answer.items[0].node.0, "web:file:main");
    // Each with its confidence, its provenance and its evidence, the via node's included.
    assert_eq!(edges[0].confidence, 1.0);
    assert_eq!(edges[1].confidence, 0.6);
    assert_eq!(edges[1].via.as_deref(), Some("POST /orders"));
    assert_eq!(edges[1].evidence, [line("web:file:main.src", 8), line("api/spec.yaml", 12)]);
    assert!(matches!(edges[1].provenance, Provenance::Extracted { .. }));
    assert!(edges.iter().all(|e| !e.evidence.is_empty()));
    // Nothing between two boxes no edge joins.
    let none =
        query::run(&model, &vocabulary(), &steps, &args(&[("A", "ws:box:api"), ("B", "ws:box:store")]))
            .unwrap();
    assert!(none.items.is_empty(), "{none:?}");
}

#[test]
fn impact_lists_what_depends_on_a_node() {
    let mut model = boxes();
    // A box that calls web, two hops from the bus: listed only at depth 2.
    model.nodes.push(node("ws:box:edge", LevelId::from("container"), None));
    model.nodes.push(node("edge:file:main", LevelId::from("code"), Some("ws:box:edge")));
    model.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    model.edges.push(edge(10, "t.calls", "edge:file:main", "web:file:main"));
    let impact = |of: &str, depth: u64| {
        let steps = [step(
            "neighbors",
            json!({"ids": ["$X"], "lens": ["all_wires", "uses"], "direction": "both", "depth": depth}),
        )];
        query::run(&model, &vocabulary(), &steps, &args(&[("X", of)])).unwrap()
    };
    let answer = impact("ws:box:bus", 1);
    let found: Vec<(&str, Toward)> = answer
        .items
        .iter()
        .map(|i| match &i.reason {
            Reason::Neighbor(n) => (i.node.0.as_str(), n.toward),
            other => panic!("{other:?}"),
        })
        .collect();
    // What depends on the bus: api sends to it; and what the bus reaches: job, which takes from
    // it, its flow running from the queue to its consumer. Not the bus's own file, and not web.
    assert_eq!(found, [("ws:box:api", Toward::Backward), ("ws:box:job", Toward::Forward)]);
    // Web's dependents and dependencies over both lenses, each once.
    let web = impact("ws:box:web", 1);
    let names: HashSet<&str> = ids(&web).into_iter().collect();
    assert_eq!(names, HashSet::from(["ws:box:edge", "ws:box:api", "ws:box:store", "ws:box:job"]));
    assert_eq!(ids(&web).len(), names.len());
    // At depth 2 the bus's dependents reach web, which calls api.
    let deeper = impact("ws:box:bus", 2);
    assert!(ids(&deeper).contains(&"ws:box:web"), "{deeper:?}");
    let Reason::Neighbor(n) = &deeper.items.iter().find(|i| i.node.0 == "ws:box:web").unwrap().reason else {
        unreachable!()
    };
    assert_eq!((n.depth, n.via.0.as_str()), (2, "ws:box:api"));
}

#[test]
fn a_query_stays_linear_in_the_size_of_the_model() {
    // A long call chain through many boxes, and a deep directory chain: a path, a why and a
    // neighbors step over 4 and 8 times the graph take about 2 times as long, never 4.
    let build = |n: usize| {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for i in 0..n {
            nodes.push(node(&format!("ws:box:b{i:06}"), LevelId::from("container"), None));
            nodes.push(node(
                &format!("b{i:06}:file:f"),
                LevelId::from("code"),
                Some(&format!("ws:box:b{i:06}")),
            ));
            if i > 0 {
                edges.push(edge(
                    i as u32,
                    "t.calls",
                    &format!("b{:06}:file:f", i - 1),
                    &format!("b{i:06}:file:f"),
                ));
            }
        }
        // A deep chain of files under one directory, each using a sibling directory: lifting each
        // edge climbs the chain to the child of the directory they share.
        nodes.push(node("d:dir:r", LevelId::from("component"), None));
        nodes.push(node("d:dir:s", LevelId::from("component"), Some("d:dir:r")));
        for d in 0..n {
            let parent = if d == 0 { "d:dir:r".to_owned() } else { format!("d:file:{:06}", d - 1) };
            nodes.push(node(&format!("d:file:{d:06}"), LevelId::from("code"), Some(&parent)));
            edges.push(edge((n + d) as u32, "t.uses", &format!("d:file:{d:06}"), "d:dir:s"));
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Model { at: 1, nodes, edges, ..Model::default() }
    };
    let vocabulary = vocabulary();
    let time = |n: usize| {
        let model = build(n);
        let last = format!("ws:box:b{:06}", n - 1);
        let steps = [
            step("path", json!({"from": "$A", "to": "$B", "lens": "wires"})),
            step("neighbors", json!({"relations": ["t.uses", "t.calls"], "depth": 1000000})),
        ];
        let started = Instant::now();
        let answer =
            query::run(&model, &vocabulary, &steps, &args(&[("A", "ws:box:b000000"), ("B", &last)])).unwrap();
        let deep = [step(
            "neighbors",
            json!({"ids": ["d:dir:s"], "relations": ["t.uses"], "direction": "backward"}),
        )];
        let used = query::run(&model, &vocabulary, &deep, &BTreeMap::new()).unwrap();
        assert_eq!(ids(&used), ["d:file:000000"]);
        let why = [step("why", json!({"from": "d:dir:r", "to": "d:dir:s"}))];
        assert_eq!(query::run(&model, &vocabulary, &why, &BTreeMap::new()).unwrap().items.len(), n);
        // Every box is on the path, so every one starts the walk and none is another's neighbor.
        assert!(answer.items.is_empty(), "{answer:?}");
        started.elapsed()
    };
    time(500);
    let small = (0..3).map(|_| time(4_000)).min().unwrap();
    let large = (0..3).map(|_| time(16_000)).min().unwrap();
    assert!(
        large < small * 8 + Duration::from_millis(50),
        "4 times the graph took {large:?} against {small:?}"
    );
}

const LEVELS: [&str; 5] = ["intent", "system", "container", "component", "code"];

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, cases: 200, ..Default::default() })]

    /// The query shapes lift an edge to the same ends roll-up does, so a path's hops and a node's
    /// neighbors are the edges the map draws at that level.
    #[test]
    fn a_query_lifts_each_edge_as_roll_up_does(
        tree in proptest::collection::vec((proptest::option::of(0usize..40), 0usize..5), 1..40),
        pairs in proptest::collection::vec((0usize..40, 0usize..40), 0..60),
        level in 0usize..5,
    ) {
        let n = tree.len();
        let nodes: Vec<Node> = tree
            .iter()
            .enumerate()
            .map(|(i, (parent, l))| {
                let parent = parent.filter(|p| *p < i).map(|p| format!("t:n:{p:03}"));
                node(&format!("t:n:{i:03}"), LevelId::from(LEVELS[*l]), parent.as_deref())
            })
            .collect();
        let edges: Vec<Edge> = pairs
            .iter()
            .enumerate()
            .map(|(k, (a, b))| edge(k as u32, "t.calls", &format!("t:n:{:03}", a % n), &format!("t:n:{:03}", b % n)))
            .collect();
        let model = Model { at: 1, nodes, edges, ..Model::default() };
        let level = LevelId::from(LEVELS[level]);
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);
        let rolled = planroom_kernel::rollup::lift(&model, &ladder, &level, None);
        let mut forest = planroom_kernel::query::forest::Forest::new(&model.nodes, &ladder);
        let level = ladder.rank(&level);
        forest.prepare(level);
        for edge in &model.edges {
            let (u, v) = forest.ends(edge).unwrap();
            let ours = forest.lift(u, v, level).map(|(a, b)| (forest.node(a).id.clone(), forest.node(b).id.clone()));
            let theirs = rolled.edges.iter().find(|l| l.children.contains(&edge.id)).map(|l| (l.from.clone(), l.to.clone()));
            proptest::prop_assert_eq!(ours, theirs, "edge {:?}", edge.id);
        }
    }
}
