//! Fact merging (DR-42): when two extractors emit one node ID, evidence unions and scalar fields
//! take the more specific tier's value. Every input is built as a value: no I/O.

use std::collections::BTreeMap;

use planroom_contract::{
    Edge, EdgeId, Evidence, Kind, Layer, Level, Node, NodeId, Provenance, Ref, Tier, Vocabulary,
};
use planroom_kernel::{Facts, resolve};
use serde_json::json;

fn line(path: &str, line: u32) -> Evidence {
    Evidence::Line { path: path.into(), line, contains: None, commit: None, branch: None }
}

fn node(id: &str, tier: Tier, name: &str, parent: &str, evidence: Evidence) -> Node {
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

fn parent(id: &str) -> Node {
    let mut n = node(id, Tier::Universal, id, "r:x:none", line("p", 1));
    n.parent = None;
    n.level = Level::Component;
    n
}

fn edge(tier: Tier, evidence: Evidence, uses: &str) -> Edge {
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
    let universal = node("r:x:file", Tier::Universal, "universal", "r:x:dir", line("tools/hc.rb", 1));
    let pattern = node("r:x:file", Tier::Pattern, "pattern", "r:x:box", line("tools/hc.rb", 3));
    let syntactic = node("r:x:file", Tier::Syntactic, "syntactic", "r:x:box", line("tools/hc.rb", 3));
    let facts = Facts {
        nodes: vec![universal, parent("r:x:dir"), pattern, parent("r:x:box"), syntactic],
        edges: vec![
            edge(Tier::Universal, line("tools/hc.rb", 1), "plain"),
            edge(Tier::Config, line("tools/hc.rb", 2), "config"),
        ],
        hints: vec![],
    };
    let resolved = resolve(&facts, &Vocabulary::default());

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
    assert_eq!(file.evidence, [line("tools/hc.rb", 3), line("tools/hc.rb", 1)], "evidence unions once");
    assert_eq!(file.aliases, ["alias:pattern", "alias:syntactic", "alias:universal"]);

    // Edges of one ID merge the same way.
    assert_eq!(resolved.edges.len(), 1);
    let merged = &resolved.edges[0];
    assert_eq!(merged.props["uses"], json!("config"));
    assert_eq!(merged.evidence, [line("tools/hc.rb", 2), line("tools/hc.rb", 1)]);

    // A tie between two facts of one tier goes to the one emitted first, whatever the input order
    // of other IDs.
    let first = node("r:x:t", Tier::Config, "first", "r:x:dir", line("a", 1));
    let second = node("r:x:t", Tier::Config, "second", "r:x:dir", line("a", 2));
    let tied = resolve(
        &Facts { nodes: vec![first, parent("r:x:dir"), second], ..Facts::default() },
        &Vocabulary::default(),
    );
    assert_eq!(tied.nodes.iter().find(|n| n.id.0 == "r:x:t").map(|n| n.name.as_str()), Some("first"));
}
