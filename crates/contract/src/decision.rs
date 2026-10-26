//! Decision kinds, as a pack declares them, and decision records: the body of a `decision` event.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::event::Target;
use crate::pack::{ModelTier, Shape};
use crate::view::Stakes;

/// `[[decisions]]` in a pack: one kind of question a model answers about one kind of subject
/// (DR-50, DR-63).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionKind {
    pub id: String,
    /// A bump invalidates the cache and the eval result.
    pub version: u32,
    pub subject: SubjectKind,
    /// The deterministic precheck, in the filter shape's predicate form.
    #[serde(default)]
    pub when: Predicate,
    /// Kinds with the same group and subject share one Jev request.
    #[serde(default)]
    pub group: Option<String>,
    pub question: Question,
    pub slice: SliceSpec,
    pub effect: Effect,
    pub stakes: Stakes,
    /// The answer that leaves the deterministic result as it is.
    pub default: Value,
    pub policy: Policy,
    /// Such as `evals/next_action_concrete`.
    #[serde(default)]
    pub eval: Option<String>,
    pub budget: DecisionBudget,
    /// The kernel role this kind serves, if any.
    #[serde(default)]
    pub role: Option<Role>,
}

/// The filter shape's predicate: `{ field = value }`, `{ field = { lt | gt | in = ... } }`, and
/// `with_edge` or `without_edge = { relation, direction, layer? }`.
pub type Predicate = BTreeMap<String, Value>;

/// What a decision kind is about: a node or edge kind, or the subjects a kernel role's raiser
/// supplies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SubjectKind {
    Kind(String),
    Role { role: Role },
}

/// A kernel role a pack binds a decision kind to (DR-17, DR-63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Role {
    /// The fold raises it for an override whose target vanished.
    #[serde(rename = "rescan.retarget")]
    RescanRetarget,
    /// The reconciler raises it for a change unit its rules can't place.
    #[serde(rename = "reconcile.residual")]
    ReconcileResidual,
    /// The intent fold raises it for an accepted plan item with nothing it touches.
    #[serde(rename = "attach.target")]
    AttachTarget,
}

/// The question a model answers, by its type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    Boolean {
        instructions: String,
        #[serde(default)]
        criteria: Option<BoolCriteria>,
    },
    Choice {
        instructions: String,
        /// Static options, ID to label, in the order the pack lists them; empty when
        /// `options_from` is set.
        #[serde(default)]
        #[schemars(with = "BTreeMap<String, String>")]
        options: Map<String, Value>,
        #[serde(default)]
        options_from: Option<OptionsFrom>,
        /// Static options only; with `options_from` any candidate may auto-act.
        #[serde(default)]
        auto_choices: Vec<String>,
    },
    Score {
        instructions: String,
        /// 2 to 10, low to high.
        levels: Vec<String>,
    },
}

/// What a yes and a no mean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BoolCriteria {
    #[serde(rename = "true")]
    pub yes: String,
    #[serde(rename = "false")]
    pub no: String,
}

/// Candidate nodes from a query shape over the subject, at most `max`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OptionsFrom {
    pub shape: Shape,
    #[serde(default)]
    pub params: Value,
    pub max: u32,
}

/// The slice of the model a request carries; evidence goes in as path and contains, never line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SliceSpec {
    pub query: Shape,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub fields: Vec<String>,
    pub max_tokens: u32,
}

/// What a pick does: one of six kernel effects, with its own parameters, such as
/// `{ on = "false", do = "lint", rule = "vague_action" }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Effect {
    /// The answer the effect fires on; every answer when absent.
    #[serde(default)]
    pub on: Option<String>,
    #[serde(rename = "do")]
    pub action: EffectOp,
    /// The effect's parameters, such as `rule`, `prop` or `relation`; a value of `$pick` is the
    /// chosen candidate.
    #[serde(flatten)]
    pub params: Map<String, Value>,
}

/// The closed set of kernel effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectOp {
    None,
    Lint,
    SetProp,
    PutEdge,
    RetractEdge,
    /// Only on a `rescan.retarget` role subject in a person-stakes kind (DR-63).
    Retarget,
}

/// When a kind acts on its own, deals a card or asks Claude.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Absent: never auto, and never asks Claude up front.
    #[serde(default)]
    pub auto_at: Option<f64>,
    pub card_band: [f64; 2],
    pub escalate_band: [f64; 2],
    #[serde(default)]
    pub escalate_to: ModelTier,
}

/// How many model calls a kind may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionBudget {
    /// Counted per scan event, which its one decide duty run spends, and per explicit
    /// `decide --run` (DR-65).
    pub max_calls_per_scan: u32,
}

/// One decision a model or a person made about one subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DecisionRecord {
    /// The decision kind's ID. Named `decision_kind`, not `kind`, because `kind` is the event
    /// body's tag.
    pub decision_kind: String,
    pub kind_version: u32,
    pub subject: Target,
    pub input_hash: String,
    pub backend: String,
    pub model_requested: String,
    pub model_reported: Option<String>,
    /// The normalized Jev or Claude result.
    pub scores: Value,
    pub pick: Option<String>,
    pub band: Band,
    pub outcome: Outcome,
    pub cost_usd: f64,
    pub cache_hit: bool,
    pub request_id: Option<String>,
}

/// Which policy band a score fell in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    Auto,
    Card,
    Escalate,
}

/// What happened with the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Acted,
    Card,
    Escalated,
    Picked,
    Overridden,
}
