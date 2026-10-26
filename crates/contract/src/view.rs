//! View models: what `map` and `focus` return, rendered the same way by every surface.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::{Empty, Handle};
use crate::event::{Actor, Surface, Timestamp};
use crate::model::{EdgeId, Evidence, Kind, Level, NodeId, Seq};

/// The map at one level through one lens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapView {
    pub level: Level,
    pub lens: String,
    pub focus: Option<NodeId>,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<LiftedEdge>,
    /// Nodes hidden by an override; a hide shows as a count, never as a silent loss.
    pub hidden: u32,
    pub empty: Option<Empty>,
}

/// One block on the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapNode {
    pub id: NodeId,
    pub kind: Kind,
    /// The kind's block color, which its pack declares; none draws as a plain card (DR-46).
    #[serde(default)]
    pub tone: Option<Tone>,
    pub name: String,
    pub level: Level,
    pub parent: Option<NodeId>,
    /// Descendants by level name, such as `{"component": 4}`.
    pub counts: BTreeMap<String, u32>,
    /// The stored layout position, if any.
    pub position: Option<Position>,
    /// Proposed by an agent or a model and not yet accepted by a person.
    pub ghost: bool,
    pub claimed_by: Option<Actor>,
}

/// A Container-level kind's block color: surfaces style a block from its tone, never from a kind
/// ID (DR-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Service,
    Library,
    Store,
    Infra,
}

/// A layout position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Edges lifted to the nearest ancestors at the view's level; `count == children.len()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LiftedEdge {
    pub kind: Kind,
    pub from: NodeId,
    pub to: NodeId,
    pub count: u32,
    /// At most 3.
    pub evidence_sample: Vec<Evidence>,
    pub children: Vec<EdgeId>,
}

/// What `focus` returns: the pane, the band and the status line; at most 4 KB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaneModel {
    pub focus: Focus,
    /// The one open decision.
    pub decision: Option<DecisionCard>,
    pub more_decisions: u32,
    pub progress: Progress,
    /// The top 3 one-liners; the first with up to 5 hunk lines.
    pub diffs: Vec<DiffLine>,
    pub drift: DriftLine,
    pub who: Vec<PresenceRow>,
    /// The band's next best action.
    pub next: Handle,
    pub minimap: MiniMap,
    pub empty: Option<Empty>,
}

/// Where you are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Focus {
    /// From the top down to the focused node.
    pub breadcrumb: Vec<Crumb>,
    pub level: Level,
    pub lens: String,
    pub project: Option<NodeId>,
    pub seq: Seq,
}

/// One step of the breadcrumb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Crumb {
    pub id: NodeId,
    pub name: String,
}

/// A decision a person is asked to make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionCard {
    pub id: String,
    /// One line, engine-authored.
    pub question: String,
    /// One line.
    pub why: String,
    /// 2 to 4.
    pub choices: Vec<Choice>,
    pub stakes: Stakes,
    /// Set when the card applies a batch, such as "Apply Claude's 2 edits".
    pub batch: Option<BatchInfo>,
    pub person_only: bool,
}

/// One choice on a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub preselected: bool,
}

/// How much a wrong pick costs; cards rank by stakes and staleness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stakes {
    Low,
    Medium,
    High,
}

/// The items one card applies together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BatchInfo {
    pub count: u32,
    pub items: Vec<Handle>,
}

/// The plan stage and the work in flight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Progress {
    pub stage: Option<String>,
    pub done: u32,
    pub total: u32,
    pub handoffs_running: u32,
    pub scan: ScanState,
}

/// Whether the workspace has been scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScanState {
    Never,
    Running,
    Done,
}

/// One pending diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffLine {
    pub summary: String,
    pub open: Handle,
    /// Up to 5, for the first diff only.
    pub hunk: Vec<String>,
}

/// Intended rules broken and machine drift, in one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriftLine {
    pub summary: String,
    pub rules_broken: u32,
    pub machine_drift: u32,
}

/// Who is working on what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PresenceRow {
    pub actor: Actor,
    pub surface: Surface,
    pub doing: Option<String>,
    pub focus: Option<NodeId>,
    pub seen_at: Timestamp,
}

/// Up to 60 blocks at the focus level, edges by lens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniMap {
    pub level: Level,
    pub blocks: Vec<MiniBlock>,
    pub edges: Vec<MiniEdge>,
}

/// One block on the mini-map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniBlock {
    pub id: NodeId,
    pub name: String,
    pub focused: bool,
}

/// One lifted edge on the mini-map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub count: u32,
}
