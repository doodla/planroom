//! Decision records: the body of a `decision` event.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::Target;

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
