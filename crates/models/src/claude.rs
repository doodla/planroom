//! The Claude adapter's gateway backend and replay (architecture.md, Model adapters, Claude;
//! DR-18, DR-49, DR-65).
//!
//! A call is `POST {base_url}/v1/messages` with:
//!
//! - **the stable cached prefix** as the system blocks: the pack's prompt file, then the output
//!   JSON Schema as canonical JSON, and a cache breakpoint on the last block (`ttl: "1h"` for a
//!   person-paced prompt). Its bytes are the same for every call that shares them, whatever the
//!   tail, so its sha256, `prefix_hash`, names the cache entry and the session affinity;
//! - **the uncached tail** as one user message: the slice as JSON, then the question;
//! - the pinned model ID for the call's tier, from the configuration, `output_config` with the
//!   effort (`low` unless the prompt says otherwise) and the output schema, and
//!   `providerOptions.gateway.only` from `[claude] providers`. No sampling parameters, no
//!   `budget_tokens` and no tools: shapes that hold on the direct API as on the gateway.
//!
//! The replay key is `sha256(path ‖ canonical body)` with the body less `providerOptions`, the
//! gateway's routing, so it covers the model, the system blocks, the messages and the output
//! parameters and never a header, a credential or a transport field.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use planroom_contract::config;
use planroom_contract::{
    CacheTtl, Measured, ModelTier, REPLAY_FORMAT, RecordedRequest, RecordedResponse, ReplayRecord,
};
use serde_json::{Map, Value, json};

use crate::canonical;
use crate::error::ModelError;
use crate::gate::{LiveEnv, Mode, Provider};
use crate::hosts::Allowed;
use crate::replay::{REQUEST_HEADERS, ReplayDir};
use crate::spend::{Row, SpendLog};
use crate::transport::{Credential, HttpRequest, HttpResponse, Transport};

/// The Messages endpoint's path.
pub const MESSAGES_PATH: &str = "/v1/messages";
/// The `anthropic-version` header, sent for parity with the direct API.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// The effort a call gets unless its prompt says otherwise.
pub const DEFAULT_EFFORT: &str = "low";
/// The directory under `tests/replay/` that holds Claude's records.
pub const REPLAY_PROVIDER: &str = "claude";
/// The variables whose values the adapter looks for before it writes a record, beside the
/// credential it used.
pub const POISON_VARIABLES: &[&str] =
    &["AI_GATEWAY_API_KEY", "VERCEL_OIDC_TOKEN", "TYPESAFE_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY"];
/// The only variables `[claude] api_key_env` may name (DR-48).
pub const KEY_VARIABLES: &[&str] = &["AI_GATEWAY_API_KEY", "VERCEL_OIDC_TOKEN", "TYPESAFE_API_KEY"];

/// One Claude call, as the engine asks for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// What the call is for, such as `plan.stage.outcome`; it names the recordings' directory.
    pub purpose: String,
    pub tier: ModelTier,
    /// The stable prefix's text blocks: the pack's prompt file first.
    pub prefix: Vec<String>,
    pub ttl: CacheTtl,
    /// The uncached tail's text blocks: the slice as JSON, then the question. The engine builds
    /// them with `planroom::app::models::request_tail`, which masks the slice by its type's marks
    /// and the question as an engine line through `kernel::redact`, so no repo text reaches a
    /// request unmasked.
    pub tail: Vec<String>,
    /// The JSON Schema the reply must validate against; it ends the prefix.
    pub output_schema: Option<Value>,
    pub max_tokens: u32,
    /// `output_config.effort`; `low` when none.
    pub effort: Option<String>,
    /// The most this call may cost, in dollars: what the spend log reserves before it is sent.
    pub ceiling_usd: f64,
    /// The spend log's `scenario` for the call.
    pub scenario: String,
}

/// A built request: its body, the bytes sent, its replay key and its prefix hash.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub model: String,
    pub url: String,
    pub body: Value,
    /// The body's bytes as sent: canonical JSON.
    pub bytes: Vec<u8>,
    pub key: String,
    /// The canonical bytes of the system blocks: the stable cached prefix.
    pub prefix: Vec<u8>,
    pub prefix_hash: String,
    pub headers: Vec<(String, String)>,
}

/// Build the request for `call` under `claude`'s settings. The model ID comes only from the
/// configuration.
pub fn build(claude: &config::Claude, call: &Call) -> Built {
    let model = match call.tier {
        ModelTier::Cheap => claude.model_cheap.clone(),
        ModelTier::Capable => claude.model_capable.clone(),
    };
    let mut blocks: Vec<String> = call.prefix.clone();
    if let Some(schema) = &call.output_schema {
        blocks.push(String::from_utf8(canonical::bytes(schema)).expect("JSON is UTF-8"));
    }
    let last = blocks.len().saturating_sub(1);
    let system: Vec<Value> = blocks
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let mut block = Map::new();
            block.insert("type".into(), json!("text"));
            block.insert("text".into(), json!(text));
            if i == last {
                block.insert(
                    "cache_control".into(),
                    match call.ttl {
                        CacheTtl::M5 => json!({"type": "ephemeral"}),
                        CacheTtl::H1 => json!({"type": "ephemeral", "ttl": "1h"}),
                    },
                );
            }
            Value::Object(block)
        })
        .collect();
    let content: Vec<Value> = call.tail.iter().map(|t| json!({"type": "text", "text": t})).collect();
    let mut output = Map::new();
    output.insert("effort".into(), json!(call.effort.as_deref().unwrap_or(DEFAULT_EFFORT)));
    if let Some(schema) = &call.output_schema {
        output.insert("format".into(), json!({"type": "json_schema", "schema": canonical::sorted(schema)}));
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(model));
    body.insert("max_tokens".into(), json!(call.max_tokens));
    body.insert("system".into(), Value::Array(system.clone()));
    body.insert("messages".into(), json!([{"role": "user", "content": content}]));
    body.insert("output_config".into(), Value::Object(output));
    let keyed = Value::Object(body.clone());
    body.insert("providerOptions".into(), json!({"gateway": {"only": claude.providers}}));
    let body = Value::Object(body);
    let key = canonical::sha256_hex(&[MESSAGES_PATH.as_bytes(), &canonical::bytes(&keyed)]);
    let prefix = canonical::bytes(&Value::Array(system));
    let prefix_hash = canonical::sha256_hex(&[&prefix]);
    let headers = vec![
        ("content-type".to_owned(), "application/json".to_owned()),
        ("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()),
        ("x-session-affinity".to_owned(), prefix_hash.clone()),
    ];
    debug_assert!(headers.iter().all(|(k, _)| REQUEST_HEADERS.contains(&k.as_str())));
    // Canonical bytes on the wire too, so the system blocks are sent exactly as `prefix` holds
    // them, whatever order a schema's keys came in.
    let bytes = canonical::bytes(&body);
    let url = format!("{}{MESSAGES_PATH}", claude.base_url.trim_end_matches('/'));
    Built { model, url, body, bytes, key, prefix, prefix_hash, headers }
}

/// Token counts as the Messages endpoint reports them: `input` counts only uncached input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: u64,
    pub cache_write: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub output: u64,
}

impl Usage {
    pub fn prompt_total(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    pub fn cache_hit_rate(&self) -> f64 {
        self.cache_read as f64 / self.prompt_total().max(1) as f64
    }
}

/// A decoded reply.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// The text blocks, joined: the structured output's JSON when the call gave a schema.
    pub text: String,
    /// `end_turn`, `max_tokens`, `refusal`, …: a caller checks it before it parses `text`.
    pub stop_reason: Option<String>,
    pub usage: Usage,
    /// The gateway's reported cost, when it gives one.
    pub cost_usd: Option<f64>,
    /// The gateway's generation ID (the message `id`).
    pub generation_id: Option<String>,
    pub model_reported: Option<String>,
}

/// Why a response is no reply. None of these keeps the provider's message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The provider refused the request Planroom built: a bug, never retried.
    Rejected,
    /// The pinned model ID isn't one the gateway knows.
    UnknownModel,
    /// The credential was refused.
    Unauthorized,
    /// Rate limits, overload, server errors or a retryable upstream failure.
    Unavailable,
    /// A body the decoder can't read.
    Unreadable,
}

/// Decode a Messages response: a `message` on 2xx, else which of the gateway's three error
/// envelopes it is (claude-gateway.md, Error shapes).
pub fn decode(status: u16, body: &Value) -> Result<Reply, Refusal> {
    if (200..300).contains(&status) {
        return decode_message(body).ok_or(Refusal::Unreadable);
    }
    let error = body.get("error");
    let kind = error.and_then(|e| e.get("type")).and_then(Value::as_str).unwrap_or_default();
    let retryable = error
        .and_then(|e| e.get("param"))
        .and_then(|p| p.get("isRetryable"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Err(match status {
        401 | 403 => Refusal::Unauthorized,
        404 if kind == "model_not_found" => Refusal::UnknownModel,
        408 | 409 | 429 | 500..=599 => Refusal::Unavailable,
        _ if kind == "overloaded_error" || kind == "rate_limit_error" || retryable => Refusal::Unavailable,
        _ if error.is_none() => Refusal::Unreadable,
        _ => Refusal::Rejected,
    })
}

fn decode_message(body: &Value) -> Option<Reply> {
    if body.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    let content = body.get("content")?.as_array()?;
    let text: String = content
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect();
    let usage = body.get("usage")?;
    let count = |v: Option<&Value>| v.and_then(Value::as_u64).unwrap_or(0);
    let usage = Usage {
        input: count(usage.get("input_tokens")),
        cache_write: count(usage.get("cache_creation_input_tokens")),
        cache_write_1h: count(usage.get("cache_creation").and_then(|c| c.get("ephemeral_1h_input_tokens"))),
        cache_read: count(usage.get("cache_read_input_tokens")),
        output: count(usage.get("output_tokens")),
    };
    let cost = body.get("provider_metadata").and_then(|m| m.get("gateway")).and_then(|g| g.get("cost"));
    let cost_usd = match cost {
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    }
    .filter(|c| c.is_finite() && *c >= 0.0);
    Some(Reply {
        text,
        stop_reason: body.get("stop_reason").and_then(Value::as_str).map(str::to_owned),
        usage,
        cost_usd,
        generation_id: body.get("id").and_then(Value::as_str).map(str::to_owned),
        model_reported: body.get("model").and_then(Value::as_str).map(str::to_owned),
    })
}

/// The Claude adapter: the configuration, the gate, the recordings and a transport.
pub struct Claude<'a> {
    pub config: &'a config::Claude,
    /// `[budget] live_tests_total_usd`.
    pub cap_usd: f64,
    pub env: &'a LiveEnv,
    pub replay: &'a ReplayDir,
    pub transport: &'a dyn Transport,
    pub credential: Option<Credential>,
    /// Values no record may hold: the credential and each of [`POISON_VARIABLES`]' values.
    pub secrets: Vec<String>,
}

impl Claude<'_> {
    /// Answer `call`: from its record under strict replay or when record mode has one; else, in
    /// record mode under `PLANROOM_LIVE_GATEWAY=1`, live through the spend log, then recorded.
    /// Every other live call is `budget_exceeded` before anything is sent.
    pub fn call(&self, call: &Call) -> Result<Reply, ModelError> {
        let built = build(self.config, call);
        let record = match self.env.mode {
            Mode::Live => return Err(self.env.live_call(Provider::Gateway).unwrap_err()),
            Mode::Strict | Mode::Record => self.replay.read(REPLAY_PROVIDER, &call.purpose, &built.key)?,
        };
        if let Some(record) = record {
            return self.decoded(call, record.response.status, &record.response.body);
        }
        if self.env.mode == Mode::Strict {
            let path = self.replay.path(REPLAY_PROVIDER, &call.purpose, &built.key)?;
            return Err(ModelError::replay_missing(&built.key, &path.display().to_string()));
        }
        let log = self.env.live_call(Provider::Gateway)?;
        Allowed::new(&self.config.base_url)?.check(&built.url)?;
        // A key variable's value in the request body, such as a planted secret the slice failed
        // to mask, would leave the machine and could never be recorded: nothing is sent.
        if crate::replay::holds_secret(&built.bytes, &self.secrets) {
            return Err(ModelError::internal());
        }
        let log = SpendLog::open(log)?;
        let ceiling = Row {
            at: now_rfc3339(),
            generation_id: None,
            provider: Provider::Gateway.as_str().to_owned(),
            model: built.model.clone(),
            cost_usd: call.ceiling_usd,
            scenario: call.scenario.clone(),
            phase: Some("reserved".to_owned()),
        };
        log.reserve(self.cap_usd, &ceiling)?;
        let request = HttpRequest {
            method: "POST",
            url: built.url.clone(),
            headers: built.headers.clone(),
            body: built.bytes,
        };
        let started = Instant::now();
        let response = self
            .transport
            .send(&request, self.credential.as_ref())
            .map_err(|_| ModelError::unavailable(Provider::Gateway.as_str()))?;
        let wall_ms = started.elapsed().as_millis() as u64;
        let (status, body) = response_json(&response);
        let reply = decode(status, &body);
        let record = ReplayRecord {
            format: REPLAY_FORMAT.to_owned(),
            key: built.key.clone(),
            purpose: call.purpose.clone(),
            request: RecordedRequest {
                method: "POST".to_owned(),
                url: built.url.clone(),
                headers: built.headers.clone(),
                body: built.body.clone(),
            },
            response: RecordedResponse { status, headers: response.headers.clone(), body: body.clone() },
            measured: Measured {
                wall_ms,
                recorded_at: now_rfc3339(),
                model_reported: reply.as_ref().ok().and_then(|r| r.model_reported.clone()),
            },
        };
        let secrets: Vec<&str> = self.secrets.iter().map(String::as_str).collect();
        let written = self.replay.write(REPLAY_PROVIDER, &record, &secrets);
        if let Ok(reply) = &reply
            && let Some(cost) = reply.cost_usd
        {
            log.append(&Row {
                at: now_rfc3339(),
                generation_id: reply.generation_id.clone(),
                cost_usd: cost - call.ceiling_usd,
                phase: Some("settled".to_owned()),
                ..ceiling
            })?;
        }
        written?;
        self.decoded(call, status, &body)
    }

    fn decoded(&self, call: &Call, status: u16, body: &Value) -> Result<Reply, ModelError> {
        decode(status, body).map_err(|refusal| match refusal {
            Refusal::UnknownModel => ModelError::invalid_argument(
                match call.tier {
                    ModelTier::Cheap => "claude.model_cheap",
                    ModelTier::Capable => "claude.model_capable",
                },
                "unknown_model",
            ),
            Refusal::Unauthorized | Refusal::Unavailable => {
                ModelError::unavailable(Provider::Gateway.as_str())
            }
            Refusal::Rejected | Refusal::Unreadable => ModelError::internal(),
        })
    }
}

/// The credential for `claude`: the value of the variable `api_key_env` names, which must be one
/// of [`KEY_VARIABLES`], else the user-level `api_key`, else none (the proxy, which counts only
/// under `PLANROOM_LIVE_GATEWAY=1`).
pub fn credential(
    claude: &config::Claude,
    var: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Credential>, ModelError> {
    if !KEY_VARIABLES.contains(&claude.api_key_env.as_str()) {
        return Err(ModelError::invalid_argument("claude.api_key_env", "api_key_env"));
    }
    Ok(var(&claude.api_key_env)
        .and_then(|v| Credential::new(&v))
        .or_else(|| claude.api_key.expose().and_then(Credential::new)))
}

/// The values no record may hold: `credential`'s and each of [`POISON_VARIABLES`]'.
pub fn secrets(credential: Option<&Credential>, var: &dyn Fn(&str) -> Option<String>) -> Vec<String> {
    let mut out: Vec<String> =
        POISON_VARIABLES.iter().filter_map(|n| var(n)).filter(|v| !v.is_empty()).collect();
    out.extend(credential.map(|c| c.expose().to_owned()));
    out.sort();
    out.dedup();
    out
}

/// The response body as JSON; a body that isn't JSON is kept as a string.
fn response_json(response: &HttpResponse) -> (u16, Value) {
    let body = serde_json::from_slice(&response.body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&response.body).into_owned()));
    (response.status, body)
}

/// Now, as RFC 3339 UTC to the second.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    rfc3339(secs)
}

/// `secs` since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ`, by the civil-from-days algorithm.
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3_600, rem % 3_600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_matches_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_793_059_200), "2026-10-27T00:00:00Z");
    }
}
