//! The CLI `--json` envelope: exactly one document on stdout per command.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::Command;
use crate::errors::{Category, ErrorCode, WarningCode};
use crate::model::Seq;
use crate::open_set;

/// The major version of the whole public contract.
pub const SCHEMA_VERSION: u32 = 1;

fn schema_version_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "integer", "const": SCHEMA_VERSION })
}

/// The reply to one command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Envelope")]
pub struct Envelope<T = Value> {
    #[schemars(schema_with = "schema_version_schema")]
    pub schema_version: u32,
    pub ok: bool,
    /// Such as `ask` or `handoff merge`.
    pub command: CommandName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub meta: Meta,
}

/// What every reply carries besides its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// The store version this reply reflects; pass it back as `base`.
    pub seq: Seq,
    pub took_ms: u32,
    pub size: Size,
    pub next: Option<Cursor>,
    pub truncated: bool,
    /// An idempotent replay of a stored reply.
    pub replayed: bool,
    pub warnings: Vec<Warning>,
    pub models: ModelUse,
    /// Session backend: requests the mod should fulfil.
    pub model_requests: Vec<ModelRequestRef>,
}

/// The reply's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub bytes: u32,
    /// `ceil(bytes / 4)`.
    pub tokens_est: u32,
}

/// An opaque page cursor over `(seq, last_id, filter hash)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cursor(pub String);

/// Model calls this reply made.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModelUse {
    pub jev: Usage,
    pub claude: Usage,
}

/// One provider's calls.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    pub calls: u32,
    pub cache_hits: u32,
    pub cost_usd: f64,
}

/// A model request waiting for the mod (session backend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelRequestRef {
    pub id: String,
    pub purpose: String,
    pub est_tokens: u32,
}

/// A failed command's error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorObject {
    pub code: ErrorCode,
    pub category: Category,
    pub retryable: bool,
    /// One line, engine-authored.
    pub message: String,
    /// An exact command, such as `planroom show ev_01J...`.
    pub hint: Option<String>,
    /// Per code, documented in the registry.
    pub details: Value,
}

/// A warning attached to a reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Warning {
    pub code: WarningCode,
    pub message: String,
    pub details: Value,
}

/// A command name from the catalogue, its path joined with spaces, such as `handoff merge`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandName(pub String);

impl JsonSchema for CommandName {
    fn schema_name() -> Cow<'static, str> {
        "CommandName".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let names: Vec<String> = Command::SPECS.iter().map(|s| s.name()).collect();
        open_set::schema(
            "A command name from the catalogue, its path joined with spaces.",
            names.iter().map(String::as_str),
            "^[a-z]+( [a-z]+)*$",
        )
    }
}

/// An exact invocation a surface can offer, such as a drill-down or the next action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Handle {
    pub command: CommandName,
    pub args: Vec<String>,
    pub label: String,
}

/// Why a list or view is empty, and what to do next; no surface shows a blank state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Empty {
    pub reason: EmptyReason,
    pub next: Handle,
}

/// A snake_case code naming why something is empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct EmptyReason(#[schemars(pattern(r"^[a-z][a-z0-9_]*$"))] pub String);
