//! Model calls as files (contracts.md, Model request, response and replay): the replay record
//! that `cargo xtask live record` writes under `tests/replay/` and strict replay reads.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::format::Document;

/// `planroom.replay/v1`.
pub const REPLAY_FORMAT: &str = "planroom.replay/v1";

impl Document for ReplayRecord {
    const NAME: &'static str = "replay";
}

/// One recorded model call, at `tests/replay/<provider>/<purpose>/<key>.json`. Written only in
/// record mode; it never holds a credential, an auth header or a cookie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReplayRecord {
    /// `planroom.replay/v1`.
    pub format: String,
    /// `sha256(path ‖ canonical body)`, lowercase hex: the request path, then the request body
    /// as canonical JSON (keys sorted, no whitespace) less the gateway's routing options
    /// (`providerOptions`). It covers the model, the system blocks, the messages, the tools and
    /// the sampling and output parameters; never a header, a credential or a transport field.
    #[schemars(pattern(r"^[0-9a-f]{64}$"))]
    pub key: String,
    /// What the call asked for, such as `plan.stage.outcome`.
    pub purpose: String,
    pub request: RecordedRequest,
    pub response: RecordedResponse,
    pub measured: Measured,
}

/// The request as sent, with no auth header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordedRequest {
    pub method: String,
    pub url: String,
    /// `[name, value]` pairs from a fixed list of headers that never hold a credential.
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// The response as received, with only the headers on a fixed list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecordedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// What the recording run measured; never part of the key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Measured {
    pub wall_ms: u64,
    /// RFC 3339, UTC.
    pub recorded_at: String,
    /// The model the provider says answered, which may differ from the pinned ID it was sent.
    pub model_reported: Option<String>,
}
