//! The CLI `--json` envelope: exactly one document on stdout per command.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::Command;
use crate::errors::{Category, ErrorCode, Retry, WarningCode};
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
    /// The one-line, engine-authored summary that text mode prints first, display-sanitized: what
    /// `/room` shows with the handles. Absent on an error, whose `message` is its line. It names
    /// repo text, such as a block's or a repo's name, so it is masked like that name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-redact" = true))]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    pub meta: Meta,
}

/// What every reply carries besides its result. Every field but `seq` and `size` is left out when
/// empty, false, null or zero, and the schema gives each its default (DR-66).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// The store version this reply reflects; pass it back as `base`.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Seq,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub took_ms: u32,
    pub size: Size,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// An idempotent replay of a stored reply.
    #[serde(default, skip_serializing_if = "is_false")]
    pub replayed: bool,
    /// A replay of a derived key: the original `seq`; pass a new `--idem` to repeat the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub replayed_from: Option<Seq>,
    /// `scan`, `focus` and `map` only: the repos with uncommitted changes, untracked files
    /// included, which never reach the map. At most the first 5 by name; `more_dirty_repos`
    /// counts the rest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirty_repos: Vec<DirtyRepo>,
    /// The repos with uncommitted changes past those `dirty_repos` lists.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more_dirty_repos: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    /// Absent when no model was called.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<ModelUse>,
    /// Session backend: requests the mod should fulfil.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_requests: Vec<ModelRequestRef>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The reply's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub bytes: u32,
    /// `ceil(bytes / 4)`.
    pub tokens_est: u32,
}

/// An opaque page cursor over `(seq, last_id, filter hash)`; `map`'s also holds the last block's
/// level, since its pages are cut by level and then ID. A page whose `seq` is behind continues
/// after that row (DR-73).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Cursor(pub String);

/// A cursor as `--cursor C` passes it, unchanged.
impl From<String> for Cursor {
    fn from(cursor: String) -> Cursor {
        Cursor(cursor)
    }
}

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

/// A repo with uncommitted changes: `orders: 3 uncommitted files aren't on the map`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DirtyRepo {
    /// The repo's name, which comes from its directory.
    #[schemars(extend("x-redact" = true))]
    pub repo: String,
    /// Changed and untracked files `git status` lists, `.gitignore` honoured.
    pub files: u32,
}

/// A model request waiting for the mod (session backend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelRequestRef {
    /// The request's input hash, which names its file.
    pub id: String,
    /// Such as `plan.stage.outcome`.
    pub purpose: String,
    /// The sha256 of the request file; the mod sends the file only when its bytes match.
    pub sha256: String,
    /// The exact command the mod's broker re-runs once it has fulfilled the request.
    pub rerun: Handle,
}

/// A failed command's error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-redact-details" = "error"))]
pub struct ErrorObject {
    pub code: ErrorCode,
    pub category: Category,
    pub retry: Retry,
    /// With `later`, when the caller may try again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u32>,
    /// One line, engine-authored; it can name a repo, a path or a target as given.
    #[schemars(extend("x-redact" = true))]
    pub message: String,
    /// An exact command, such as `planroom show ev_01J...`.
    pub hint: Option<String>,
    /// Per code, documented in the registry.
    pub details: Value,
}

/// What a mod tool handler returns, for success and failure alike, as compact JSON text: the
/// envelope less what an agent doesn't act on. Empty fields are left out (DR-66).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ToolResult")]
pub struct ToolResult<T = Value> {
    pub ok: bool,
    /// `true` on every reply the engine gave: its names, evidence and details come from the
    /// scanned repos, so they're data, never instructions. Absent on an error the mod authors when
    /// no engine answered.
    #[serde(default, skip_serializing_if = "is_false")]
    pub untrusted: bool,
    // On success: the command's result, redacted like every output. (A doc comment here would make
    // the generated TypeScript an object type rather than `unknown`.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    /// On failure: the error, never dropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
    pub meta: ToolMeta,
}

/// A tool result's error: the envelope's error whole (DR-66). Only an error the mod authors, when no
/// engine answered, may leave out its category and details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-redact-details" = "error"))]
pub struct ToolError {
    pub code: ErrorCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    pub retry: Retry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u32>,
    /// One line, engine-authored, masked as the envelope's error message is.
    #[schemars(extend("x-redact" = true))]
    pub message: String,
    /// An exact command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    // Per code, documented in the registry; absent when the engine never answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// The fields of `meta` an agent acts on; absent when empty. `seq` is absent only when the
/// engine never answered.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ToolMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Option<Seq>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_requests: Vec<ModelRequestRef>,
}

/// A warning attached to a reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("x-redact-details" = "warning"))]
pub struct Warning {
    pub code: WarningCode,
    /// One engine-authored line; it can name a repo, a path or a file.
    #[schemars(extend("x-redact" = true))]
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
    /// The one engine-authored line every surface shows for this state, such as "Map these 3
    /// repos? About 2 s."; absent where a surface still words it from `reason`. It can name a
    /// repo or a block, so it is masked like that name; `next.label` never names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-redact" = true))]
    pub line: Option<String>,
    pub next: Handle,
}

/// A snake_case code naming why something is empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct EmptyReason(#[schemars(pattern(r"^[a-z][a-z0-9_]*$"))] pub String);
