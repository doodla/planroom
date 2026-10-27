//! What the models tests share: a transport that never touches the network, a configuration
//! with test model IDs, and a gate read from a fixed set of variables.

#![allow(dead_code)]

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::Path;

use planroom_contract::config::Claude as ClaudeConfig;
use planroom_contract::{CacheTtl, ModelTier};
use planroom_models::claude::Call;
use planroom_models::gate::LiveEnv;
use planroom_models::transport::{Credential, HttpRequest, HttpResponse, Transport, TransportError};
use serde_json::{Value, json};

/// One request a [`Fake`] was handed.
#[derive(Debug, Clone)]
pub struct Sent {
    pub request: HttpRequest,
    pub credential: Option<String>,
}

/// A transport that answers every request with one canned response and remembers what it got.
pub struct Fake {
    pub response: HttpResponse,
    pub sent: RefCell<Vec<Sent>>,
}

impl Fake {
    pub fn answering(status: u16, body: &Value) -> Fake {
        Fake {
            response: HttpResponse {
                status,
                headers: vec![("content-type".into(), "application/json".into())],
                body: serde_json::to_vec(body).unwrap(),
            },
            sent: RefCell::new(Vec::new()),
        }
    }

    /// A message reply with `text`, which reports `cost` dollars.
    pub fn message(text: &str, cost: &str) -> Fake {
        Fake::answering(200, &message(text, cost))
    }

    pub fn count(&self) -> usize {
        self.sent.borrow().len()
    }
}

impl Transport for Fake {
    fn send(
        &self,
        request: &HttpRequest,
        credential: Option<&Credential>,
    ) -> Result<HttpResponse, TransportError> {
        self.sent
            .borrow_mut()
            .push(Sent { request: request.clone(), credential: credential.map(|c| c.expose().to_owned()) });
        Ok(self.response.clone())
    }
}

/// A Messages reply body.
pub fn message(text: &str, cost: &str) -> Value {
    json!({
        "id": "gen_test_1", "type": "message", "role": "assistant",
        "content": [{"type": "text", "text": text}],
        "model": "test/capable", "stop_reason": "end_turn",
        "usage": {"input_tokens": 12, "output_tokens": 3, "cache_creation_input_tokens": 0,
                  "cache_read_input_tokens": 0},
        "provider_metadata": {"gateway": {"cost": cost}}
    })
}

/// The `[claude]` section with test model IDs and `base_url`.
pub fn config(base_url: &str) -> ClaudeConfig {
    serde_json::from_value(json!({
        "backend": "auto", "base_url": base_url, "api_key_env": "AI_GATEWAY_API_KEY", "api_key": "",
        "model_cheap": "test/cheap", "model_capable": "test/capable", "providers": ["anthropic"]
    }))
    .unwrap()
}

/// The gate as these variables give it.
pub fn env(vars: &[(&str, &str)]) -> LiveEnv {
    LiveEnv::from_vars(vars.iter().map(|(k, v)| (OsString::from(k), OsString::from(v)))).unwrap()
}

/// Record mode under the gateway flag, with the spend log at `log`.
pub fn recording(log: &Path) -> LiveEnv {
    env(&[
        ("PLANROOM_REPLAY", "record"),
        ("PLANROOM_LIVE_GATEWAY", "1"),
        ("PLANROOM_SPEND_LOG", log.to_str().unwrap()),
    ])
}

/// A call with this tail and a schema whose keys come in `schema`'s order.
pub fn call(tail: &str, schema: Value) -> Call {
    Call {
        purpose: "test.purpose".into(),
        tier: ModelTier::Capable,
        prefix: vec!["You group components. Rule 1: keep services whole.".into()],
        ttl: CacheTtl::H1,
        tail: vec![r#"{"slice":[1,2,3]}"#.into(), tail.into()],
        output_schema: Some(schema),
        max_tokens: 200,
        effort: None,
        ceiling_usd: 0.05,
        scenario: "test".into(),
    }
}

/// A spend log holding one seed row of `spent` dollars.
pub fn spend_log(dir: &Path, spent: f64) -> std::path::PathBuf {
    let path = dir.join("spend.jsonl");
    std::fs::write(
        &path,
        format!(
            "{}\n",
            json!({"at": "2026-10-25T00:00:00Z", "generation_id": null, "provider": "gateway",
                   "model": "test/capable", "cost_usd": spent, "scenario": "seed", "note": "a seed row"})
        ),
    )
    .unwrap();
    path
}
