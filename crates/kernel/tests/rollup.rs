//! `kernel::rollup::lift` (architecture.md, "Levels and roll-ups"; DR-42): faithful at every
//! level, no lifted edge between a node and its own ancestor, and edges inside one ancestor as its
//! internal count. Every input is built as a value: the kernel's tests do no I/O.

use std::collections::{BTreeSet, HashMap};

use planroom_contract::{
    Edge, EdgeId, Evidence, Kind, Layer, LevelId, LevelVocab, LiftedEdge, Node, NodeId, Provenance, Ref,
    Tier, Vocabulary,
};
use planroom_kernel::ladder::Ladder;
use planroom_kernel::rollup::{Rollup, lift};
use planroom_kernel::{Facts, Model, resolve};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError};

const LEVELS: [&str; 5] = ["intent", "system", "container", "component", "code"];

/// A vocabulary whose ladder is `intent` apart, then `chain` top to finest.
fn ladder_of(chain: &[&str]) -> Vocabulary {
    let level = |id: &str, apart: bool| LevelVocab {
        id: LevelId::from(id),
        label: id.into(),
        one: "{count} node".into(),
        many: "{count} nodes".into(),
        apart,
        pack: "test".into(),
    };
    let mut levels = vec![level(LevelId::INTENT, true)];
    levels.extend(chain.iter().map(|id| level(id, false)));
    Vocabulary { levels, ..Vocabulary::default() }
}

/// The test vocabulary: these tests' levels, in their order.
fn vocabulary() -> Vocabulary {
    ladder_of(&LEVELS[1..])
}

/// `model`'s edges lifted to `level` on the test vocabulary's ladder, with no lens.
fn lift_at(model: &Model, level: &str) -> Rollup {
    let vocabulary = vocabulary();
    lift(model, &Ladder::of(&vocabulary), &LevelId::from(level), None)
}

fn node(id: &str, level: &str, parent: Option<&str>) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("k.x".into()),
        level: LevelId::from(level),
        parent: parent.map(|p| NodeId(p.into())),
        name: id.rsplit(':').next().unwrap_or(id).into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

fn id(s: &str) -> Ref {
    Ref::Id(NodeId(s.into()))
}

fn edge(n: u32, kind: &str, from: Ref, to: Ref) -> Edge {
    Edge {
        id: EdgeId(format!("e:{n:012x}")),
        kind: Kind(kind.into()),
        from,
        to,
        via: None,
        via_ref: None,
        props: Default::default(),
        evidence: vec![Evidence::Line {
            path: format!("f{n}"),
            line: n + 1,
            contains: None,
            commit: None,
            branch: None,
        }],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
    }
}

fn model(nodes: Vec<Node>, edges: Vec<Edge>) -> Model {
    Model { at: 1, nodes, edges, ..Model::default() }
}

fn ends(lifted: &LiftedEdge) -> (&str, &str) {
    (lifted.from.0.as_str(), lifted.to.0.as_str())
}

/// The node and its ancestors, nearest first, from the nodes' own parents, written apart from the
/// kernel's index so the checks don't reuse what they check.
fn chain(parents: &HashMap<String, Option<String>>, start: &str) -> Vec<String> {
    let mut out = vec![start.to_owned()];
    let mut at = start.to_owned();
    while let Some(Some(parent)) = parents.get(&at) {
        if !parents.contains_key(parent) || out.contains(parent) {
            break;
        }
        out.push(parent.clone());
        at = parent.clone();
    }
    out
}

fn parents_of(nodes: &[Node]) -> HashMap<String, Option<String>> {
    nodes.iter().map(|n| (n.id.0.clone(), n.parent.as_ref().map(|p| p.0.clone()))).collect()
}

fn is_strict_ancestor(parents: &HashMap<String, Option<String>>, ancestor: &str, of: &str) -> bool {
    chain(parents, of)[1..].iter().any(|n| n == ancestor)
}

/// The orders and storefront repos under one workspace, with `redis` beside them, as the fixture
/// holds them.
fn workspace() -> Vec<Node> {
    vec![
        node("ws:workspace:ws", "system", None),
        node("orders:repo:orders", "system", Some("ws:workspace:ws")),
        node("orders:container:orders", "container", Some("orders:repo:orders")),
        node("orders:dir:internal", "component", Some("orders:container:orders")),
        node("orders:file:internal/db.go", "code", Some("orders:dir:internal")),
        node("orders:file:internal/jobs.go", "code", Some("orders:dir:internal")),
        node("orders:dir:cmd", "component", Some("orders:container:orders")),
        node("orders:file:cmd/main.go", "code", Some("orders:dir:cmd")),
        node("storefront:repo:storefront", "system", Some("ws:workspace:ws")),
        node("storefront:file:src/api.ts", "code", Some("storefront:repo:storefront")),
        node("ws:container:redis", "container", Some("ws:workspace:ws")),
    ]
}

#[test]
fn no_lifted_edge_joins_a_node_to_its_own_ancestor() {
    let nodes = workspace();
    let edges = vec![
        edge(1, "k.y", id("orders:file:internal/db.go"), id("ws:container:redis")),
        edge(2, "k.y", id("storefront:file:src/api.ts"), id("orders:container:orders")),
        // A node to its own descendant, and a node to its own ancestor.
        edge(3, "k.z", id("orders:repo:orders"), id("orders:file:cmd/main.go")),
        edge(4, "k.z", id("ws:container:redis"), id("ws:workspace:ws")),
    ];
    let model = model(nodes.clone(), edges);
    let parents = parents_of(&nodes);

    // At System, orders -> redis lifts to the orders repo -> redis, with redis beside the repos,
    // never to an edge from the repo to its own workspace.
    let system = lift_at(&model, "system");
    let lifted: Vec<(&str, &str)> = system.edges.iter().map(ends).collect();
    assert_eq!(
        lifted,
        [("orders:repo:orders", "ws:container:redis"), ("storefront:repo:storefront", "orders:repo:orders")]
    );
    assert_eq!(system.internal_count(&NodeId("orders:repo:orders".into())), 1);
    assert_eq!(system.internal_count(&NodeId("ws:workspace:ws".into())), 1);

    let container = lift_at(&model, "container");
    let lifted: Vec<(&str, &str)> = container.edges.iter().map(ends).collect();
    assert_eq!(
        lifted,
        [
            ("orders:container:orders", "ws:container:redis"),
            ("storefront:repo:storefront", "orders:container:orders")
        ]
    );

    for level in LEVELS {
        let rollup = lift_at(&model, level);
        for lifted in &rollup.edges {
            let (from, to) = ends(lifted);
            assert_ne!(from, to, "{level:?}: a self-loop {lifted:?}");
            assert!(!is_strict_ancestor(&parents, from, to), "{level:?}: {from} holds {to}");
            assert!(!is_strict_ancestor(&parents, to, from), "{level:?}: {to} holds {from}");
        }
    }
}

#[test]
fn same_ancestor_edges_are_internal() {
    let edges = vec![
        edge(1, "k.y", id("orders:file:internal/db.go"), id("orders:file:internal/jobs.go")),
        edge(2, "k.y", id("orders:file:cmd/main.go"), id("orders:file:internal/jobs.go")),
        // Beside them, orders -> redis: their nearest ancestors at System differ (the orders repo
        // and the workspace), so it lifts rather than counting inside the workspace.
        edge(3, "k.y", id("orders:file:internal/db.go"), id("ws:container:redis")),
    ];
    let model = model(workspace(), edges);
    let inside = |rollup: &Rollup, node: &str| rollup.internal_count(&NodeId(node.into()));

    let system = lift_at(&model, "system");
    assert_eq!(inside(&system, "orders:repo:orders"), 2);
    assert_eq!(
        system.edges.iter().map(ends).collect::<Vec<_>>(),
        [("orders:repo:orders", "ws:container:redis")]
    );
    assert_eq!(inside(&system, "ws:workspace:ws"), 0);

    let container = lift_at(&model, "container");
    assert_eq!(inside(&container, "orders:container:orders"), 2);
    assert_eq!(container.edges.len(), 1);

    let component = lift_at(&model, "component");
    assert_eq!(inside(&component, "orders:dir:internal"), 1);
    assert_eq!(
        component.edges.iter().map(ends).collect::<Vec<_>>(),
        [("orders:dir:cmd", "orders:dir:internal"), ("orders:dir:internal", "ws:container:redis")]
    );

    let code = lift_at(&model, "code");
    assert!(code.internal.is_empty(), "{:?}", code.internal);
    assert_eq!(code.edges.len(), 3);
}

#[test]
fn lifted_counts_sum_to_leaf_edges() {
    let mut redis = node("ws:container:redis", "container", Some("ws:workspace:ws"));
    redis.aliases = vec!["host:redis".into()];
    let mut nodes = workspace();
    nodes.retain(|n| n.id.0 != "ws:container:redis");
    nodes.push(redis);
    let edges = vec![
        edge(1, "k.y", id("orders:file:internal/db.go"), Ref::Alias("host:redis".into())),
        edge(2, "k.y", id("orders:file:internal/jobs.go"), id("ws:container:redis")),
        edge(3, "k.y", id("orders:file:cmd/main.go"), id("ws:container:redis")),
        edge(4, "k.z", id("orders:file:cmd/main.go"), id("ws:container:redis")),
        edge(5, "k.y", id("storefront:file:src/api.ts"), id("orders:file:cmd/main.go")),
        edge(6, "k.y", id("orders:file:internal/db.go"), id("orders:file:internal/jobs.go")),
        edge(7, "k.y", id("storefront:file:src/api.ts"), Ref::Alias("host:nowhere".into())),
    ];
    let leaves = edges.len();
    let model = model(nodes, edges);
    for level in LEVELS {
        let rollup = lift_at(&model, level);
        let lifted: usize = rollup.edges.iter().map(|e| e.count as usize).sum();
        let internal: usize = rollup.internal.values().map(Vec::len).sum();
        assert_eq!(lifted + internal + rollup.unresolved.len(), leaves, "{level:?}: {rollup:?}");
        for e in &rollup.edges {
            assert_eq!(e.count as usize, e.children.len());
        }
        assert_eq!(rollup.unresolved, [EdgeId(format!("e:{:012x}", 7))]);
    }
    // At System the three k.y edges from the orders repo to redis merge into one of count 3, and
    // the k.z edge between the same ends stays its own.
    let system = lift_at(&model, "system");
    let counts: Vec<(&str, (&str, &str), u32)> =
        system.edges.iter().map(|e| (e.kind.0.as_str(), ends(e), e.count)).collect();
    assert_eq!(
        counts,
        [
            ("k.y", ("orders:repo:orders", "ws:container:redis"), 3),
            ("k.y", ("storefront:repo:storefront", "orders:repo:orders"), 1),
            ("k.z", ("orders:repo:orders", "ws:container:redis"), 1),
        ]
    );
    assert_eq!(system.internal_count(&NodeId("orders:repo:orders".into())), 1);
    assert!(system.edges.iter().all(|e| e.evidence_sample.len() <= 3));
    assert_eq!(system.edges[0].evidence_sample.len(), 3);
}

/// A random forest: each node's parent is an earlier node, none, or an ID no node has; each node
/// may hold one alias from a small pool, so some aliases are ambiguous.
#[derive(Debug, Clone)]
struct Forest {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Debug, Clone)]
enum End {
    Node(usize),
    Alias(u8),
    Missing,
}

/// Every node's level is one of `levels`, by index.
fn forest(levels: Vec<String>) -> impl Strategy<Value = Forest> {
    let node_specs =
        prop::collection::vec((0usize..levels.len(), 0usize..10, prop::option::of(0u8..4)), 1..18);
    let end = prop_oneof![
        6 => (0usize..18).prop_map(End::Node),
        2 => (0u8..5).prop_map(End::Alias),
        1 => Just(End::Missing),
    ];
    let edge_specs = prop::collection::vec((end.clone(), end, 0usize..2), 0..40);
    (node_specs, edge_specs).prop_map(move |(node_specs, edge_specs)| {
        let count = node_specs.len();
        let key = |i: usize| format!("r:x:n{i}");
        let nodes: Vec<Node> = node_specs
            .iter()
            .enumerate()
            .map(|(i, &(level, parent, alias))| {
                // Most parents are an earlier node; some are none, and some name no node.
                let parent = match parent {
                    0 => None,
                    1 => Some("r:x:gone".to_owned()),
                    p if i > 0 => Some(key(p % i)),
                    _ => None,
                };
                let mut n = node(&key(i), &levels[level], parent.as_deref());
                n.aliases = alias.map(|a| vec![format!("alias:{a}")]).unwrap_or_default();
                n
            })
            .collect();
        let to_ref = |end: &End| match end {
            End::Node(i) => id(&key(i % count)),
            End::Alias(a) => Ref::Alias(format!("alias:{a}")),
            End::Missing => id("r:x:missing"),
        };
        let edges = edge_specs
            .iter()
            .enumerate()
            .map(|(n, (from, to, kind))| edge(n as u32, ["k.y", "k.z"][*kind], to_ref(from), to_ref(to)))
            .collect();
        Forest { nodes, edges }
    })
}

/// What an end resolves to, worked out apart from the kernel: an ID a node has, or an alias
/// exactly one node holds.
fn resolve_end(nodes: &[Node], end: &Ref) -> Option<String> {
    match end {
        Ref::Id(id) => nodes.iter().any(|n| n.id == *id).then(|| id.0.clone()),
        Ref::Alias(alias) => {
            let holders: Vec<&Node> = nodes.iter().filter(|n| n.aliases.contains(alias)).collect();
            (holders.len() == 1).then(|| holders[0].id.0.clone())
        }
    }
}

/// Roll-up's promises for one forest on `vocabulary`'s ladder, at each level it lists.
fn faithful(forest: Forest, vocabulary: &Vocabulary) -> Result<(), TestCaseError> {
    let Forest { nodes, edges } = forest;
    let parents = parents_of(&nodes);
    let levels: HashMap<&str, &LevelId> = nodes.iter().map(|n| (n.id.0.as_str(), &n.level)).collect();
    let ladder = Ladder::of(vocabulary);
    let rank = |level: &LevelId| ladder.rank(level);
    let raw = model(nodes.clone(), edges.clone());
    let resolved = resolve(&Facts { nodes: nodes.clone(), edges: edges.clone(), hints: vec![] }, vocabulary);
    let folded = model(resolved.nodes, resolved.edges);
    let by_id: HashMap<&EdgeId, &Edge> = edges.iter().map(|e| (&e.id, e)).collect();
    let ends_of = |e: &Edge| (resolve_end(&nodes, &e.from), resolve_end(&nodes, &e.to));

    for level in vocabulary.levels.iter().map(|l| &l.id) {
        let rollup = lift(&raw, &ladder, level, None);

        // Lifting then expanding gives back exactly the edge set: each edge lands once.
        let mut landed: Vec<&EdgeId> = rollup.edges.iter().flat_map(|e| &e.children).collect();
        landed.extend(rollup.internal.values().flatten());
        landed.extend(&rollup.unresolved);
        landed.sort();
        let mut want: Vec<&EdgeId> = edges.iter().map(|e| &e.id).collect();
        want.sort();
        prop_assert_eq!(&landed, &want, "{:?}", level);

        // An edge is unresolved exactly when an end resolves to no node or to several.
        for id in &rollup.unresolved {
            let (from, to) = ends_of(by_id[id]);
            prop_assert!(from.is_none() || to.is_none(), "{:?}: {:?} resolves", level, id);
        }

        let mut seen = BTreeSet::new();
        for lifted in &rollup.edges {
            let (from, to) = ends(lifted);
            prop_assert_eq!(lifted.count as usize, lifted.children.len());
            prop_assert!(lifted.count > 0);
            prop_assert!(lifted.children.windows(2).all(|w| w[0] < w[1]));
            prop_assert!(lifted.evidence_sample.len() <= 3);
            prop_assert!(seen.insert((lifted.kind.clone(), from, to)), "{:?}: merged twice", level);
            // No lifted edge joins a node to itself or to its own ancestor.
            prop_assert_ne!(from, to);
            prop_assert!(!is_strict_ancestor(&parents, from, to), "{:?}: {} holds {}", level, from, to);
            prop_assert!(!is_strict_ancestor(&parents, to, from), "{:?}: {} holds {}", level, to, from);
            for child in &lifted.children {
                let leaf = by_id[child];
                prop_assert_eq!(&leaf.kind, &lifted.kind);
                for evidence in &lifted.evidence_sample {
                    prop_assert!(lifted.children.iter().any(|c| by_id[c].evidence.contains(evidence)));
                }
                let (Some(a), Some(b)) = ends_of(leaf) else {
                    prop_assert!(false, "{:?}: unresolved {:?} lifted", level, child);
                    unreachable!()
                };
                let (up_a, up_b) = (chain(&parents, &a), chain(&parents, &b));
                // Each lifted end stands for its original: the original or one of its ancestors.
                prop_assert!(up_a.iter().any(|n| n == from), "{:?}: {} is not above {}", level, from, a);
                prop_assert!(up_b.iter().any(|n| n == to), "{:?}: {} is not above {}", level, to, b);
                // An end is its nearest node at or above the level (or its chain's top), or
                // a step down from a node that holds the other end.
                for (end, up, other) in [(from, &up_a, &up_b), (to, &up_b, &up_a)] {
                    let nearest = up
                        .iter()
                        .position(|n| rank(levels[n.as_str()]) <= rank(level))
                        .unwrap_or(up.len() - 1);
                    let at = up.iter().position(|n| n == end).expect("on the chain");
                    prop_assert!(
                        at == nearest || (at < nearest && other.contains(&up[at + 1])),
                        "{:?}: {} lifted below the level without a reason",
                        level,
                        end
                    );
                }
            }
        }

        // An edge inside a node has both ends at or below it.
        for (inside, ids) in &rollup.internal {
            for id in ids {
                let (Some(a), Some(b)) = ends_of(by_id[id]) else {
                    prop_assert!(false, "{:?}: unresolved {:?} counted inside", level, id);
                    unreachable!()
                };
                prop_assert!(chain(&parents, &a).contains(&inside.0), "{:?}: {} not in {}", level, a, inside);
                prop_assert!(chain(&parents, &b).contains(&inside.0), "{:?}: {} not in {}", level, b, inside);
            }
        }

        // Resolution changes where nothing lands: the roll-up of the resolved facts is the
        // same, less the edges resolution set aside, and lifting twice gives one answer.
        let again = lift(&folded, &ladder, level, None);
        prop_assert_eq!(&again.edges, &rollup.edges);
        prop_assert_eq!(&again.internal, &rollup.internal);
        prop_assert!(again.unresolved.is_empty());
        prop_assert_eq!(lift(&raw, &ladder, level, None), rollup.clone());

        // The edges lifted in reverse order roll up the same.
        let reversed = model(nodes.clone(), edges.iter().rev().cloned().collect());
        let back = lift(&reversed, &ladder, level, None);
        let back_edges: Vec<_> = back.edges.iter().map(|e| (&e.kind, &e.from, &e.to, &e.children)).collect();
        let want_edges: Vec<_> =
            rollup.edges.iter().map(|e| (&e.kind, &e.from, &e.to, &e.children)).collect();
        prop_assert_eq!(back_edges, want_edges);
        prop_assert_eq!(&back.internal, &rollup.internal);
    }
    Ok(())
}

/// A chain of `n` invented levels, top to finest, whose names sort against their order.
fn invented(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("rung_{}", char::from(b'z' - i as u8))).collect()
}

proptest! {
    // No I/O in the kernel's tests: a failing case is printed, never written to a file.
    #![proptest_config(Config { failure_persistence: None, ..Config::default() })]

    #[test]
    fn rollup_is_faithful_at_every_level(forest in forest(LEVELS.iter().map(|l| l.to_string()).collect())) {
        faithful(forest, &vocabulary())?;
    }

    /// The same promises on any ladder: 1 to 8 invented levels below intent.
    #[test]
    fn rollup_is_faithful_on_any_ladder(
        (chain, forest) in (1usize..=8).prop_flat_map(|n| {
            let chain = invented(n);
            let mut levels = vec![LevelId::INTENT.to_owned()];
            levels.extend(chain.iter().cloned());
            (Just(chain), forest(levels))
        })
    ) {
        let chain: Vec<&str> = chain.iter().map(String::as_str).collect();
        faithful(forest, &ladder_of(&chain))?;
    }
}
