//! Golden CLI outputs the mod's tests and the canvas's e2e specs read, built from the contract
//! types so they can't drift from the schema. They stand in until the engine's own tests write
//! `tests/golden/cli/` with `cargo xtask gen --golden`; then this module goes.

use std::collections::BTreeMap;

use planroom_contract::envelope::{CommandName, Empty, EmptyReason, Envelope, Handle, Meta, Size};
use planroom_contract::event::{Actor, Surface, Timestamp};
use planroom_contract::model::{EdgeId, Evidence, Kind, Level, NodeId};
use planroom_contract::view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, Focus, LiftedEdge, MapNode, MapView,
    MiniBlock, MiniEdge, MiniMap, PaneModel, PresenceRow, Progress, ScanState, Stakes, Tone,
};
use serde::Serialize;

fn handle(command: &str, args: &[&str], label: &str) -> Handle {
    Handle {
        command: CommandName(command.into()),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        label: label.into(),
    }
}

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

/// `planroom focus --json` in a mapped workspace with one open decision.
pub fn focus() -> Result<String, String> {
    let pane = PaneModel {
        focus: Focus {
            breadcrumb: vec![Crumb { id: id("ws:system:shop"), name: "shop".into() }],
            level: Level::System,
            lens: "calls".into(),
            project: Some(id("plan:checkout:retries")),
            seq: 412,
        },
        decision: Some(DecisionCard {
            id: "card_01JB7Q".into(),
            question: "Apply Claude's 2 edits to the worker group?".into(),
            why: "They move consumer.py under the queue it reads.".into(),
            choices: vec![
                Choice { id: "accept".into(), label: "Accept both".into(), preselected: true },
                Choice { id: "first".into(), label: "Only the first".into(), preselected: false },
                Choice { id: "reject".into(), label: "Reject".into(), preselected: false },
                Choice { id: "later".into(), label: "Later".into(), preselected: false },
            ],
            stakes: Stakes::Low,
            batch: Some(BatchInfo {
                count: 2,
                items: vec![
                    handle("show", &["ev_01JB7Q1"], "Move consumer.py"),
                    handle("show", &["ev_01JB7Q2"], "Rename the group"),
                ],
            }),
            person_only: true,
        }),
        more_decisions: 1,
        progress: Progress {
            stage: Some("next actions".into()),
            done: 3,
            total: 7,
            handoffs_running: 1,
            scan: ScanState::Done,
        },
        diffs: vec![
            DiffLine {
                summary: "worker → platform: + calls orders-api".into(),
                open: handle("diff", &["--item", "d1"], "Open"),
                hunk: vec!["@@ -1,2 +1,3 @@".into(), " worker".into(), "+  calls orders-api".into()],
            },
            DiffLine {
                summary: "storefront: ~ checkout moved to cart".into(),
                open: handle("diff", &["--item", "d2"], "Open"),
                hunk: vec![],
            },
        ],
        drift: DriftLine {
            summary: "≠ 2 intended rules broken · 1 machine drift".into(),
            rules_broken: 2,
            machine_drift: 1,
        },
        who: vec![PresenceRow {
            actor: Actor::Agent { session: "1a2b3c4d".into(), role: "implementer".into(), key: None },
            surface: Surface::Mod,
            doing: Some("worker-status-via-api".into()),
            focus: Some(id("worker:mod:consumer")),
            seen_at: Timestamp("2026-10-26T09:00:00Z".into()),
        }],
        next: handle("map", &["--level", "container"], "Open the containers"),
        minimap: MiniMap {
            level: Level::System,
            blocks: ["storefront", "worker", "platform", "orders", "kit"]
                .iter()
                .enumerate()
                .map(|(i, name)| MiniBlock {
                    id: id(&format!("ws:repo:{name}")),
                    name: (*name).into(),
                    focused: i == 0,
                })
                .collect(),
            edges: vec![MiniEdge { from: id("ws:repo:storefront"), to: id("ws:repo:orders"), count: 3 }],
        },
        empty: None,
    };
    envelope("focus", pane, 412)
}

/// `planroom map --detail graph --json` at the top of the fixture workspace, before any layout is
/// stored. A stand-in shaped by `examples/TRUTH.json`'s containers and relations, with the tones
/// the packs will declare: the engine serves neither `--detail graph` nor tones yet.
pub fn map_graph() -> Result<String, String> {
    // name, namespace and kind-short, kind, tone, counts (component, code)
    let blocks: [(&str, &str, &str, Tone, u32, u32); 10] = [
        ("storefront", "storefront:svc", "code.service", Tone::Service, 7, 21),
        ("orders", "orders:svc", "code.service", Tone::Service, 14, 23),
        ("worker", "worker:svc", "code.service", Tone::Service, 5, 18),
        ("kit", "kit:lib", "code.library", Tone::Library, 4, 11),
        ("redis", "ws:queue", "code.queue", Tone::Store, 0, 0),
        ("orders-db", "ws:db", "code.datastore", Tone::Store, 0, 0),
        ("platform", "platform:tool", "code.tooling", Tone::Infra, 8, 10),
        ("otel-collector", "ws:obs", "observability.collector", Tone::Infra, 0, 0),
        ("prometheus", "ws:obs", "observability.collector", Tone::Infra, 0, 0),
        ("grafana", "ws:obs", "observability.dashboard", Tone::Infra, 0, 0),
    ];
    let node_id = |name: &str| -> NodeId {
        let ns = blocks.iter().find(|b| b.0 == name).map_or("ws:x", |b| b.1);
        id(&format!("{ns}:{name}"))
    };
    let nodes = blocks
        .iter()
        .map(|&(name, _, kind, tone, components, code)| MapNode {
            id: node_id(name),
            kind: Kind(kind.into()),
            tone: Some(tone),
            name: name.into(),
            level: Level::Container,
            parent: None,
            counts: [("component", components), ("code", code)]
                .into_iter()
                .filter(|&(_, n)| n > 0)
                .map(|(level, n)| (level.to_owned(), n))
                .collect::<BTreeMap<_, _>>(),
            position: None,
            ghost: false,
            claimed_by: None,
        })
        .collect();
    // from, to, kind, count, one evidence line (path, line)
    let relations: [(&str, &str, &str, u32, &str, u32); 22] = [
        ("storefront", "orders", "code.http", 3, "storefront/src/api/orders.ts", 26),
        ("orders", "redis", "code.queue_publish", 2, "orders/internal/queue/publisher.go", 49),
        ("worker", "redis", "code.queue_consume", 1, "worker/worker/consumer.py", 41),
        ("orders", "orders-db", "code.db", 1, "orders/internal/store/postgres.go", 26),
        ("worker", "orders-db", "code.db", 2, "worker/worker/db.py", 22),
        ("storefront", "kit", "code.depends_on_package", 1, "storefront/package.json", 18),
        ("platform", "storefront", "code.deploys", 1, "platform/docker-compose.yml", 6),
        ("platform", "orders", "code.deploys", 1, "platform/docker-compose.yml", 16),
        ("platform", "worker", "code.deploys", 1, "platform/docker-compose.yml", 28),
        ("platform", "redis", "code.deploys", 1, "platform/docker-compose.yml", 38),
        ("platform", "orders-db", "code.deploys", 1, "platform/docker-compose.yml", 41),
        ("platform", "otel-collector", "code.deploys", 1, "platform/docker-compose.yml", 50),
        ("platform", "prometheus", "code.deploys", 1, "platform/docker-compose.yml", 59),
        ("platform", "grafana", "code.deploys", 1, "platform/docker-compose.yml", 68),
        ("prometheus", "orders", "observability.scrapes", 1, "platform/prometheus/prometheus.yml", 11),
        ("prometheus", "worker", "observability.scrapes", 1, "platform/prometheus/prometheus.yml", 14),
        (
            "prometheus",
            "otel-collector",
            "observability.scrapes",
            1,
            "platform/prometheus/prometheus.yml",
            17,
        ),
        (
            "storefront",
            "otel-collector",
            "observability.exports_telemetry",
            1,
            "storefront/src/app/telemetry.ts",
            9,
        ),
        (
            "orders",
            "otel-collector",
            "observability.exports_telemetry",
            1,
            "orders/internal/telemetry/otel.go",
            32,
        ),
        ("worker", "otel-collector", "observability.exports_telemetry", 1, "worker/worker/telemetry.py", 22),
        (
            "grafana",
            "prometheus",
            "observability.queries",
            1,
            "platform/grafana/provisioning/datasources/prometheus.yml",
            8,
        ),
        ("platform", "orders", "code.http", 2, "platform/tools/healthcheck.rb", 15),
    ];
    let mut child = 0u64;
    let edges = relations
        .iter()
        .map(|&(from, to, kind, count, path, line)| LiftedEdge {
            kind: Kind(kind.into()),
            from: node_id(from),
            to: node_id(to),
            count,
            evidence_sample: vec![Evidence::Line {
                path: path.into(),
                line,
                contains: None,
                commit: None,
                branch: None,
            }],
            children: (0..count)
                .map(|_| {
                    child += 1;
                    EdgeId(format!("e:{:012x}", 0x00a1_1ce0_0000 + child))
                })
                .collect(),
        })
        .collect();
    let view = MapView {
        level: Level::Container,
        lens: "all".into(),
        focus: None,
        nodes,
        edges,
        hidden: 0,
        empty: None,
    };
    envelope("map", view, 412)
}

/// `planroom map --detail graph --json` in a workspace whose repos have no scan yet: the empty map,
/// whose next action is the scan, with the engine's text from `en.toml`.
pub fn map_empty() -> Result<String, String> {
    let view = MapView {
        level: Level::System,
        lens: "all".into(),
        focus: None,
        nodes: vec![],
        edges: vec![],
        hidden: 0,
        empty: Some(Empty {
            reason: EmptyReason("not_scanned".into()),
            next: handle("scan", &["/home/you/shop"], "Map these 5 repos? About 2 s."),
        }),
    };
    envelope("map", view, 0)
}

/// A successful envelope around `result`, its size counting the document itself as the engine's
/// does.
fn envelope<T: Serialize + Clone>(command: &str, result: T, seq: u64) -> Result<String, String> {
    let envelope = Envelope {
        schema_version: planroom_contract::SCHEMA_VERSION,
        ok: true,
        command: CommandName(command.into()),
        result: Some(result),
        error: None,
        meta: Meta {
            seq,
            took_ms: 9,
            size: Size { bytes: 0, tokens_est: 0 },
            next: None,
            truncated: false,
            replayed: false,
            replayed_from: None,
            warnings: vec![],
            models: None,
            model_requests: vec![],
        },
    };
    let mut text = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
    for _ in 0..3 {
        let bytes = u32::try_from(text.len()).map_err(|e| e.to_string())?;
        let mut sized = envelope.clone();
        sized.meta.size = Size { bytes, tokens_est: bytes.div_ceil(4) };
        text = serde_json::to_string(&sized).map_err(|e| e.to_string())?;
    }
    Ok(text + "\n")
}
