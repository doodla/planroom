//! Golden CLI outputs the mod's tests read, built from the contract types so they can't drift
//! from the schema. They stand in until the engine's own tests write `tests/golden/cli/` with
//! `cargo xtask gen --golden`; then this module goes.

use planroom_contract::envelope::{CommandName, Envelope, Handle, Meta, ModelUse, Size};
use planroom_contract::event::{Actor, Surface, Timestamp};
use planroom_contract::model::{Level, NodeId};
use planroom_contract::view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, Focus, MiniBlock, MiniEdge, MiniMap,
    PaneModel, PresenceRow, Progress, ScanState, Stakes,
};

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
    let envelope = Envelope {
        schema_version: planroom_contract::SCHEMA_VERSION,
        ok: true,
        command: CommandName("focus".into()),
        result: Some(pane),
        error: None,
        meta: Meta {
            seq: 412,
            took_ms: 9,
            size: Size { bytes: 0, tokens_est: 0 },
            next: None,
            truncated: false,
            replayed: false,
            warnings: vec![],
            models: ModelUse::default(),
            model_requests: vec![],
        },
    };
    let mut text = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
    // The size counts the document itself, as the engine's does.
    for _ in 0..3 {
        let bytes = u32::try_from(text.len()).map_err(|e| e.to_string())?;
        let mut sized = envelope.clone();
        sized.meta.size = Size { bytes, tokens_est: bytes.div_ceil(4) };
        text = serde_json::to_string(&sized).map_err(|e| e.to_string())?;
    }
    Ok(text + "\n")
}
