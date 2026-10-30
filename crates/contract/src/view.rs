//! View models: what `map` and `focus` return, rendered the same way by every surface.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::PickerName;
use crate::envelope::{Empty, Handle};
use crate::event::{Actor, Surface, Timestamp};
use crate::model::{EdgeId, Evidence, Kind, LevelId, NodeId, Seq};
use crate::vocab::LevelVocab;

/// The map at one level through one lens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapView {
    pub level: LevelId,
    pub lens: String,
    pub focus: Option<NodeId>,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<LiftedEdge>,
    /// Nodes hidden by an override; a hide shows as a count, never as a silent loss.
    pub hidden: u32,
    pub empty: Option<Empty>,
    /// With `--detail graph`: the key a layout of this view is cached under, which
    /// `PUT /api/v1/layout` takes back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutKey>,
    /// With `--detail graph`: the vocabulary's levels, `intent` first and then the ladder top
    /// to finest, so a surface names and steps through levels without naming one itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<LevelVocab>,
    /// The shown lens's yarn; none under `--lens all` or for a lens that names none, which draws
    /// in the muted token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yarn: Option<Yarn>,
}

/// Where a view's layout is cached: the view and the hash of its structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutKey {
    /// Such as `map:system:all`.
    pub view_key: String,
    /// sha256 of the view's block IDs and lifted edges; a changed structure misses the cache.
    pub structure_hash: String,
}

/// One block on the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapNode {
    pub id: NodeId,
    pub kind: Kind,
    /// The kind's block color, which its pack declares; none draws as a plain card (DR-46).
    #[serde(default)]
    pub tone: Option<Tone>,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    pub level: LevelId,
    pub parent: Option<NodeId>,
    /// Descendants by level name, such as `{"component": 4}`.
    pub counts: BTreeMap<String, u32>,
    /// The stored layout position, if any.
    pub position: Option<Position>,
    /// Proposed by an agent or a model and not yet accepted by a person.
    pub ghost: bool,
    pub claimed_by: Option<Actor>,
}

/// A kind's block color: surfaces style a block from its tone, never from a kind
/// ID (DR-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Service,
    Library,
    Store,
    Infra,
}

/// A lens's edge color, a palette name its pack declares: a surface draws the lens's lifted
/// edges from it, never from a lens ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Yarn {
    Teal,
    Indigo,
    Ochre,
    Plum,
    Berry,
    Copper,
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

/// What `focus` returns: the pane, the band and the status line, fit to `focus`'s catalogue budget
/// on the CLI.
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
    /// Omitted with `focus --detail summary` (DR-82).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimap: Option<MiniMap>,
    pub empty: Option<Empty>,
}

/// Where you are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Focus {
    /// From the top down to the focused node.
    pub breadcrumb: Vec<Crumb>,
    pub level: LevelId,
    pub lens: String,
    pub project: Option<NodeId>,
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Seq,
}

/// One step of the breadcrumb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Crumb {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
}

/// A decision a person is asked to make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionCard {
    pub id: String,
    /// One line, engine-authored, filled from the card's `vars`, which name repo text.
    #[schemars(extend("x-redact" = true))]
    pub question: String,
    /// One line, filled as `question` is.
    #[schemars(extend("x-redact" = true))]
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

/// How much a wrong pick costs, as a decision kind declares it; cards rank by stakes and
/// staleness (DR-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stakes {
    /// A model's pick prefills, listed in one batch per kind.
    Low,
    /// Only a person picks: the result deals a card.
    Person,
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
    /// One engine-authored line, which names what changed.
    #[schemars(extend("x-redact" = true))]
    pub summary: String,
    pub open: Handle,
    /// Up to 5, for the first diff only.
    #[schemars(extend("x-redact" = true))]
    pub hunk: Vec<String>,
}

/// Intended rules broken and machine drift, in one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DriftLine {
    /// One engine-authored line, which can name the blocks a rule or a drift is about.
    #[schemars(extend("x-redact" = true))]
    pub summary: String,
    pub rules_broken: u32,
    pub machine_drift: u32,
    /// The top drift item's handle, which an action key acts on once its row is selected (DR-77).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<Handle>,
}

/// Who is working on what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PresenceRow {
    pub actor: Actor,
    pub surface: Surface,
    /// The writer instance that last touched this row: `mod:<session>` from the mod, or `cli:<pid>`
    /// from a `watch --session` run without `PLANROOM_CLIENT`; an ID.
    #[serde(default)]
    pub client_id: String,
    #[schemars(extend("x-redact" = true))]
    pub doing: Option<String>,
    pub focus: Option<NodeId>,
    pub seen_at: Timestamp,
}

/// What `focus --picker NAME` returns: a question, why it is asked, and up to 16 entries to pick
/// from. The pane draws the first four on its card and the rest in a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Picker {
    pub name: PickerName,
    /// The question the picker asks. Holds only counts, levels, stage labels and engine text,
    /// never repo text.
    pub question: String,
    /// One line on what the pick changes. Holds only counts, levels, stage labels and engine
    /// text, never repo text.
    pub why: String,
    pub entries: Vec<PickerEntry>,
    /// Entries left out past the 16 keys reach or past the budget.
    pub more: u32,
    /// What a pick costs, when it spends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<PickEstimate>,
}

/// One choice in a [`Picker`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PickerEntry {
    #[schemars(extend("x-redact" = true))]
    pub label: String,
    /// Counts, levels, stage labels and engine text only, never repo text.
    pub detail: String,
    /// What the pick acts on.
    pub handle: Handle,
    /// Whether this is the choice in force now.
    pub current: bool,
}

/// What picking a stage can cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PickEstimate {
    /// A per-pick ceiling: the uncached tail plus the most output a stage's reply takes.
    pub tokens: u32,
    /// The per-stage dollar ceiling.
    pub usd: f64,
    /// The shared prefix is cached.
    pub cached: bool,
}

/// What `focus` returns: the pane, or with `--picker` a picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
// The pane is the common reply and is never held in bulk, so boxing it would only change the API.
#[allow(clippy::large_enum_variant)]
pub enum FocusResult {
    Pane(PaneModel),
    Picker(Picker),
}

/// Up to 60 blocks at the focus level, edges by lens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniMap {
    pub level: LevelId,
    pub blocks: Vec<MiniBlock>,
    pub edges: Vec<MiniEdge>,
}

/// One block on the mini-map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniBlock {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    pub focused: bool,
}

/// Which pane the mod draws until the engine answers: `ready` draws the `PaneModel` (DR-57, DR-60).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EngineStatus {
    Resolving,
    Fetching,
    BuildOffered,
    Building,
    Failed,
    UnsupportedPlatform,
    VersionMismatch,
    PersonKeyMismatch,
    Ready,
}

/// The mod's `engine` atom: written by `session.start` from the resolver's reply (DR-81).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EngineState {
    pub status: EngineStatus,
    /// The resolver, `<plugin root>/scripts/planroom`, once the plugin root is known.
    pub resolver: Option<String>,
    /// The engine binary, once resolved.
    pub path: Option<String>,
    pub version: Option<String>,
    pub catalogue_hash: Option<String>,
    pub backend: Option<String>,
    /// The one status line for this state: the resolver's message where it gave one.
    pub line: Option<String>,
    /// The exact command that fixes this state, when there is one.
    pub fix: Option<String>,
}

/// The mod's `pane` atom: the last `focus --json`, its `PaneModel` or the error it returned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaneState {
    pub model: Option<PaneModel>,
    /// The error's message, masked as the envelope's is.
    #[schemars(extend("x-redact" = true))]
    pub error: Option<String>,
}

/// The mod's `lastSeq` atom: the `seq` of the last frame the watch loop read, so the loop starts
/// again from it after a reload (DR-6); null before the first frame (DR-82).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LastSeq {
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Option<Seq>,
}

/// The mod's `turnSeq` atom: the main loop's current turn and the latest `meta.seq` a tool returned
/// to the model in it, the default `base` of the tools' writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TurnSeq {
    #[serde(rename = "turnId")]
    pub turn_id: Option<String>,
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Option<Seq>,
}

/// One write in the mod's `outbox` atom: kept from before its process starts until any reply, so
/// a reload mid-write resends it with the same `--idem` and request hash, and the store writes one
/// event (DR-8). It holds a command line, as a hint does, and the names of the variables the run
/// set, never their values, so no token or key is ever in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutboxEntry {
    /// The write's `--idem`.
    pub idem: String,
    /// The command line after the binary, `--idem` and `--json` included.
    pub argv: Vec<String>,
    /// The environment variables the run set, by name.
    pub env_names: Vec<String>,
    /// What the run sent on stdin, such as a proposal's JSON, so a resend carries its body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
    /// The run's `PLANROOM_CAUSE`, `turn:<id>`, so a resent agent edit stays in its turn's batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// The session that ran it: only that session resends it, as the same actor and client.
    pub session: String,
}

/// One lifted edge on the mini-map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MiniEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub count: u32,
}
