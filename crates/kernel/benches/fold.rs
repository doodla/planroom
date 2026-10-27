//! The fold benchmark (DR-47, DR-89): the fold from scratch over facts and events built in
//! memory, the work every one-shot read does. Two cases, each timed over repeated runs:
//!
//! - `fold.20k`: 20,000 facts and 5,000 events, whose budget is 50 ms;
//! - `fold.50k`: the same facts and 50,000 events, since every one-shot read folds every event.
//!
//! It prints one JSON line per case, `{"bench": NAME, "median_ms": N}`, which
//! `cargo xtask bench fold` compares with `budgets.json`. It reads no file and no clock but the
//! monotonic timer, so the kernel still does no I/O; the result lines are its only output.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

use planroom_contract::{
    Actor, Client, Edge, EdgeId, Edit, Event, EventBody, EventId, Kind, Layer, Level, Node, NodeId, Op,
    Provenance, Ref, Rename, ScanStats, Seq, Status, Surface, Target, Tier, Timestamp, Vocabulary,
};
use planroom_kernel::fold::{FoldDiff, fold_full};
use planroom_kernel::model::Facts;
use serde_json::json;

/// Facts in each case: nodes and edges together.
const FACTS: usize = 20_000;
/// Directories, each holding the same share of the files.
const DIRS: usize = 400;
/// Edges among the files.
const EDGES: usize = 4_000;
/// Runs per case; the median is reported.
const RUNS: usize = 11;

fn node(id: String, level: Level, parent: Option<String>) -> Node {
    let name = id.rsplit([':', '/']).next().unwrap_or_default().to_owned();
    Node {
        id: NodeId(id),
        kind: Kind("b.x".into()),
        level,
        parent: parent.map(NodeId),
        name,
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "b/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

fn file(i: usize) -> String {
    format!("r:f:d{}/f{i}.ts", i % DIRS)
}

/// The ID file `i` had before a rename that a rescan recorded.
fn old_file(i: usize) -> String {
    format!("r:f:old/f{i}.ts")
}

/// The files the `FACTS` leave room for beside the directories and edges.
const FILES: usize = FACTS - EDGES - DIRS - 1;

/// The file event `i` edits.
fn edited(i: usize) -> usize {
    i * 13 % FILES
}

/// One repo, its directories, and files to make up `FACTS` with the edges.
fn facts() -> Facts {
    let files = FILES;
    let mut nodes = vec![node("r:r:r".into(), Level::System, None)];
    for d in 0..DIRS {
        nodes.push(node(format!("r:d:d{d}"), Level::Component, Some("r:r:r".into())));
    }
    for i in 0..files {
        nodes.push(node(file(i), Level::Code, Some(format!("r:d:d{}", i % DIRS))));
    }
    let edges = (0..EDGES)
        .map(|i| Edge {
            id: EdgeId(format!("e:{i:012x}")),
            kind: Kind("b.uses".into()),
            from: Ref::Id(NodeId(file(i))),
            to: Ref::Id(NodeId(file((i * 7 + 1) % files))),
            via: None,
            via_ref: None,
            props: Default::default(),
            evidence: vec![],
            confidence: 1.0,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: "b/x".into(), tier: Tier::Universal },
        })
        .collect();
    Facts { nodes, edges, ..Facts::default() }
}

/// `count` events of the kinds a workspace accumulates: renames, moves, hides and prop edits by
/// people and agents, their undos and reviews, and a rescan every 500 events. One event in ten
/// moves a file it names by the ID the file had before a rename the next rescan records, so the
/// fold follows 50 renames per rescan (DR-74).
fn events(count: usize) -> Vec<Event> {
    let person = Actor::Person { id: "person:ann".into(), name: None };
    let agent = Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None };
    let mut out: Vec<Event> = Vec::with_capacity(count);
    for i in 0..count {
        let seq = i as Seq + 1;
        let target = match i % 10 {
            5 => Target::Node(NodeId(old_file(edited(i)))),
            _ => Target::Node(NodeId(file(edited(i)))),
        };
        let set = |field: &str, value: serde_json::Value| {
            EventBody::Edit(Edit {
                op: Op::Set,
                target: target.clone(),
                layer: Layer::Actual,
                fields: BTreeMap::from([(field.to_owned(), value)]),
                base: seq - 1,
                over: BTreeMap::new(),
            })
        };
        let (actor, status, body) = match i % 10 {
            0 if i % 500 == 0 => (
                Actor::System { part: "scan".into() },
                Status::Applied,
                EventBody::Scan {
                    repo: "r".into(),
                    commit: format!("{i:040x}"),
                    branch: "main".into(),
                    snapshot: seq,
                    link: seq,
                    // The files the moves since the last rescan named by their old IDs.
                    renames: (i.saturating_sub(500)..i)
                        .filter(|j| j % 10 == 5)
                        .map(|j| Rename { from: NodeId(old_file(edited(j))), to: NodeId(file(edited(j))) })
                        .collect(),
                    stats: ScanStats::default(),
                },
            ),
            0..=3 => (person.clone(), Status::Applied, set("name", json!(format!("n{i}")))),
            4 | 5 => (person.clone(), Status::Applied, set("parent", json!(format!("r:d:d{}", i % 97)))),
            6 => (person.clone(), Status::Applied, set("hidden", json!(i % 4 == 0))),
            7 => (agent.clone(), Status::Ghost, set("props.owner", json!("team"))),
            8 => (
                person.clone(),
                Status::Applied,
                EventBody::Review {
                    of: EventId(format!("ev-{}", seq - 1)),
                    verdict: planroom_contract::Verdict::Accept,
                },
            ),
            _ => {
                (person.clone(), Status::Applied, EventBody::Undo { of: EventId(format!("ev-{}", seq - 3)) })
            }
        };
        out.push(Event {
            seq,
            id: EventId(format!("ev-{seq}")),
            idem: format!("k{seq}"),
            request_hash: "h".into(),
            at: Timestamp("2026-10-26T12:00:00Z".into()),
            actor,
            client: Client { surface: Surface::Cli, id: "cli:1".into() },
            project: None,
            cause: None,
            status,
            body,
        });
    }
    out
}

/// The median of `RUNS` timed folds, in milliseconds.
fn median_ms(facts: &Facts, events: &[Event]) -> f64 {
    let vocabulary = Vocabulary::default();
    let at = events.len() as Seq;
    // One fold first, so the timed runs start warm.
    black_box(fold_full(facts, events, &vocabulary, at));
    let mut times: Vec<f64> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            black_box(fold_full(black_box(facts), black_box(events), &vocabulary, at));
            started.elapsed().as_secs_f64() * 1_000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[RUNS / 2]
}

/// The one line a case prints: the benchmark's only output.
#[allow(clippy::disallowed_macros)]
fn report(bench: &str, median_ms: f64) {
    println!("{}", json!({"bench": bench, "median_ms": (median_ms * 100.0).round() / 100.0}));
}

fn main() {
    let facts = facts();
    assert_eq!(facts.nodes.len() + facts.edges.len(), FACTS);
    // The timed folds follow renames, so following them is inside each budget.
    let followed = fold_full(&facts, &events(5_000), &Vocabulary::default(), 5_000)
        .diffs
        .iter()
        .filter(|d| matches!(d, FoldDiff::Followed { .. }))
        .count();
    assert!(followed > 0, "the benchmark's edits follow no rename");
    report("fold.20k", median_ms(&facts, &events(5_000)));
    report("fold.50k", median_ms(&facts, &events(50_000)));
}
