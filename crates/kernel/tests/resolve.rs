//! Resolution (architecture.md, "The fold", step 2; "Levels and roll-ups"): parents attach to a
//! node that resolves, falling back to the parent another fact names. Inputs are values: no I/O.

use planroom_contract::{Kind, Layer, Level, Node, NodeId, Provenance, Tier, Vocabulary};
use planroom_kernel::{Facts, resolve};

fn node(id: &str, tier: Tier, level: Level, parent: Option<&str>) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("k.x".into()),
        level,
        parent: parent.map(|p| NodeId(p.into())),
        name: id.rsplit(':').next().unwrap_or(id).into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier },
        evidence: vec![],
    }
}

fn parent_of<'a>(nodes: &'a [Node], id: &str) -> Option<&'a str> {
    nodes.iter().find(|n| n.id.0 == id).and_then(|n| n.parent.as_ref()).map(|p| p.0.as_str())
}

#[test]
fn a_fact_parent_that_doesnt_resolve_gives_way_to_a_less_specific_facts_parent() {
    // The pattern tier puts the script under a container no fact defines; the universal tier's
    // fact parent, its directory, is where it stays.
    let facts = Facts {
        nodes: vec![
            node("platform:repo:platform", Tier::Universal, Level::System, None),
            node("platform:dir:tools", Tier::Universal, Level::Component, Some("platform:repo:platform")),
            node("platform:file:tools/hc.rb", Tier::Universal, Level::Code, Some("platform:dir:tools")),
            node(
                "platform:file:tools/hc.rb",
                Tier::Pattern,
                Level::Code,
                Some("platform:container:healthcheck"),
            ),
        ],
        ..Facts::default()
    };
    let resolved = resolve(&facts, &Vocabulary::default());
    assert_eq!(parent_of(&resolved.nodes, "platform:file:tools/hc.rb"), Some("platform:dir:tools"));
    assert_eq!(resolved.nodes.len(), 3);

    // Once the container exists, the more specific tier's parent wins.
    let mut with_container = facts.clone();
    with_container.nodes.push(node(
        "platform:container:healthcheck",
        Tier::Pattern,
        Level::Container,
        Some("platform:repo:platform"),
    ));
    let resolved = resolve(&with_container, &Vocabulary::default());
    assert_eq!(
        parent_of(&resolved.nodes, "platform:file:tools/hc.rb"),
        Some("platform:container:healthcheck")
    );

    // With no fact parent that resolves, and for a node that names itself, the node sits at the top.
    let lost = Facts {
        nodes: vec![
            node("x:dir:lost", Tier::Universal, Level::Component, Some("x:repo:missing")),
            node("x:dir:self", Tier::Universal, Level::Component, Some("x:dir:self")),
        ],
        ..Facts::default()
    };
    let resolved = resolve(&lost, &Vocabulary::default());
    assert_eq!(parent_of(&resolved.nodes, "x:dir:lost"), None);
    assert_eq!(parent_of(&resolved.nodes, "x:dir:self"), None);
}
