//! The Claude decoders over every Messages response the gateway research recorded
//! (`docs/contributing/research/claude-fixtures/`): each success decodes with its usage and cost,
//! the 3× cached-prefix run reads 0, then about 0.995, twice, and each of the gateway's three
//! error envelopes maps to a registered code with none of the provider's text.

mod support;

use std::path::PathBuf;

use planroom_contract::ErrorCode;
use planroom_models::claude::{self, Claude, Refusal};
use planroom_models::replay::ReplayDir;
use planroom_models::transport::Credential;
use serde_json::{Value, json};
use support::{Fake, call, config, recording, spend_log};

fn fixtures() -> Vec<(String, Value)> {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/contributing/research/claude-fixtures");
    let mut out: Vec<(String, Value)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            (name, serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap())
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The Messages responses with a JSON body (the streaming one records SSE, which the adapter
/// never asks for).
fn messages() -> Vec<(String, u16, Value)> {
    fixtures()
        .into_iter()
        .filter(|(_, f)| f["request"]["url"].as_str().is_some_and(|u| u.ends_with("/v1/messages")))
        .filter_map(|(name, f)| {
            let body = f["response"].get("body")?.clone();
            Some((name, f["response"]["status"].as_u64().unwrap() as u16, body))
        })
        .collect()
}

#[test]
fn every_recorded_message_decodes() {
    let all = messages();
    assert!(all.len() >= 25, "found {} Messages fixtures", all.len());
    for (name, status, body) in all.iter().filter(|(_, s, _)| *s == 200) {
        let reply = claude::decode(*status, body).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(reply.generation_id.as_deref().is_some_and(|id| id.starts_with("gen_")), "{name}");
        assert!(reply.model_reported.is_some(), "{name}");
        assert!(reply.usage.output > 0, "{name}");
        assert!(reply.usage.prompt_total() > 0, "{name}");
        let cost = reply.cost_usd.unwrap_or_else(|| panic!("{name}: no gateway cost"));
        assert!(cost > 0.0 && cost < 0.05, "{name}: {cost}");
        if body["stop_reason"] == json!("end_turn") {
            assert!(!reply.text.is_empty(), "{name}");
        }
    }
}

#[test]
fn the_cached_prefix_run_reads_the_cache_after_its_first_call() {
    let all = messages();
    let rate = |n: &str| {
        let (_, status, body) = all.iter().find(|(name, _, _)| name == n).unwrap();
        claude::decode(*status, body).unwrap().usage
    };
    let first = rate("cache-messages-haiku-1");
    assert_eq!((first.cache_write, first.cache_read), (5145, 0));
    assert_eq!(first.cache_hit_rate(), 0.0);
    for n in ["cache-messages-haiku-2", "cache-messages-haiku-3"] {
        let usage = rate(n);
        assert_eq!(usage.prompt_total(), 5171, "{n}");
        assert!((usage.cache_hit_rate() - 5145.0 / 5171.0).abs() < 1e-9, "{n}");
    }
    assert!(rate("cache-messages-1h-haiku-1").cache_write_1h > 0);
}

#[test]
fn each_error_envelope_maps_to_a_refusal() {
    let all = messages();
    let refusal = |n: &str| {
        let (_, status, body) = all.iter().find(|(name, _, _)| name == n).unwrap_or_else(|| panic!("{n}"));
        claude::decode(*status, body).unwrap_err()
    };
    assert_eq!(refusal("error-messages-missing-max-tokens"), Refusal::Rejected);
    assert_eq!(refusal("error-messages-empty-messages"), Refusal::Rejected);
    assert_eq!(refusal("error-messages-invalid-json"), Refusal::Rejected);
    assert_eq!(refusal("error-messages-unknown-model"), Refusal::UnknownModel);
    assert_eq!(refusal("error-messages-bad-tool-name"), Refusal::Rejected);
    assert_eq!(refusal("error-messages-bad-tool-name-only-anthropic"), Refusal::Rejected);
    assert_eq!(refusal("error-messages-bad-image"), Refusal::Rejected);
    for status in [429, 500, 503, 529] {
        assert_eq!(
            claude::decode(status, &json!({"error": {"type": "x"}})).unwrap_err(),
            Refusal::Unavailable
        );
    }
    for status in [401, 403] {
        assert_eq!(claude::decode(status, &json!({})).unwrap_err(), Refusal::Unauthorized);
    }
    let retryable =
        json!({"error": {"type": "AI_APICallError", "message": "m", "param": {"isRetryable": true}}});
    assert_eq!(claude::decode(400, &retryable).unwrap_err(), Refusal::Unavailable);
    assert_eq!(claude::decode(502, &json!("<html>bad gateway</html>")).unwrap_err(), Refusal::Unavailable);
    assert_eq!(claude::decode(400, &json!("not json")).unwrap_err(), Refusal::Unreadable);
    assert_eq!(claude::decode(200, &json!({"type": "error"})).unwrap_err(), Refusal::Unreadable);
}

#[test]
fn no_provider_text_reaches_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let log = spend_log(dir.path(), 0.0);
    let gate = recording(&log);
    let cfg = config("https://ai-gateway.vercel.sh");
    for (name, status, body) in messages().into_iter().filter(|(_, s, _)| *s != 200) {
        let fake = Fake::answering(status, &body);
        let adapter = Claude {
            config: &cfg,
            cap_usd: 3.0,
            env: &gate,
            replay: &replay,
            transport: &fake,
            credential: Credential::new("fake-key-for-decoders"),
            secrets: vec!["fake-key-for-decoders".into()],
        };
        let err = adapter.call(&call(&name, json!({}))).unwrap_err();
        let expected =
            if name.ends_with("unknown-model") { ErrorCode::InvalidArgument } else { ErrorCode::Internal };
        assert_eq!(err.code, expected, "{name}");
        let message = body["error"]["message"].as_str().unwrap_or("Invalid JSON");
        let shown = format!("{err} {err:?}");
        assert!(!shown.contains(message), "{name}: the provider's message reached the error: {shown}");
        if expected == ErrorCode::InvalidArgument {
            assert_eq!(err.details["field"], json!("claude.model_capable"));
        }
    }
}
