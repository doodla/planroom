//! Strict replay and the stable cached prefix (architecture.md, Model adapters, Claude): a call
//! with no record fails with `replay_missing` and sends nothing; the prefix's bytes don't move
//! with the tail or with a schema's key order; and a recorded call replays with no network.

mod support;

use planroom_contract::ErrorCode;
use planroom_models::claude::{self, Claude};
use planroom_models::replay::ReplayDir;
use serde_json::json;
use support::{Fake, call, config, env, recording, spend_log};

#[test]
fn a_missing_record_is_replay_missing() {
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let cfg = config("https://ai-gateway.vercel.sh");
    let strict = env(&[("PLANROOM_REPLAY", "strict")]);
    let fake = Fake::message("{}", "0.001");
    let adapter = Claude {
        config: &cfg,
        cap_usd: 3.0,
        env: &strict,
        replay: &replay,
        transport: &fake,
        credential: None,
        secrets: vec![],
    };
    let asked = call("Which group?", json!({"type": "object"}));
    let err = adapter.call(&asked).unwrap_err();
    assert_eq!(err.code, ErrorCode::ReplayMissing);
    let key = claude::build(&cfg, &asked).key;
    assert_eq!(err.details["key"], json!(key));
    let path = err.details["path"].as_str().unwrap();
    assert!(path.ends_with(&format!("claude/test.purpose/{key}.json")), "{path}");
    assert_eq!(fake.count(), 0, "strict replay sends nothing");
}

#[test]
fn the_cache_prefix_is_byte_stable() {
    let cfg = config("https://ai-gateway.vercel.sh");
    // The same schema with its keys inserted in two orders.
    let schema_a: serde_json::Value = serde_json::from_str(
        r#"{"type":"object","required":["group"],"properties":{"group":{"type":"string"}}}"#,
    )
    .unwrap();
    let schema_b: serde_json::Value = serde_json::from_str(
        r#"{"properties":{"group":{"type":"string"}},"required":["group"],"type":"object"}"#,
    )
    .unwrap();
    let one = claude::build(&cfg, &call("Which group holds orders?", schema_a.clone()));
    let two = claude::build(&cfg, &call("Which group holds payments?", schema_b));
    assert_eq!(one.prefix, two.prefix, "the prefix doesn't move with the tail or the schema's key order");
    assert_eq!(one.prefix_hash, two.prefix_hash);
    assert_ne!(one.key, two.key, "the tail is part of the key");
    for built in [&one, &two] {
        let sent = String::from_utf8(built.bytes.clone()).unwrap();
        let prefix = String::from_utf8(built.prefix.clone()).unwrap();
        assert!(sent.contains(&prefix), "the system blocks go out exactly as the prefix holds them");
        assert!(prefix.contains(r#""cache_control":{"ttl":"1h","type":"ephemeral"}"#), "{prefix}");
    }
    let again = claude::build(&cfg, &call("Which group holds orders?", schema_a));
    assert_eq!(one.bytes, again.bytes, "a rebuild sends the same bytes");
    assert_eq!(one.key, again.key);
    // The key covers the body, never the gateway's routing.
    let mut routed = cfg.clone();
    routed.providers = vec!["anthropic".into(), "bedrock".into()];
    let rerouted = claude::build(&routed, &call("Which group holds orders?", json!({})));
    let plain = claude::build(&cfg, &call("Which group holds orders?", json!({})));
    assert_eq!(rerouted.key, plain.key);
    assert_ne!(rerouted.bytes, plain.bytes);
}

#[test]
fn a_recorded_call_replays_with_no_network() {
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let log = spend_log(dir.path(), 0.5);
    let cfg = config("https://ai-gateway.vercel.sh");
    let asked = call("Which group?", json!({"type": "object"}));
    let record_env = recording(&log);
    let fake = Fake::message(r#"{"group":"orders"}"#, "0.002");
    let recorder = Claude {
        config: &cfg,
        cap_usd: 3.0,
        env: &record_env,
        replay: &replay,
        transport: &fake,
        credential: None,
        secrets: vec![],
    };
    let first = recorder.call(&asked).unwrap();
    assert_eq!(fake.count(), 1);
    // Record mode answers a call it has a record for from the record.
    assert_eq!(recorder.call(&asked).unwrap(), first);
    assert_eq!(fake.count(), 1);
    let strict = env(&[("PLANROOM_REPLAY", "strict")]);
    let offline = Fake::answering(500, &json!({}));
    let replayer = Claude {
        config: &cfg,
        cap_usd: 3.0,
        env: &strict,
        replay: &replay,
        transport: &offline,
        credential: None,
        secrets: vec![],
    };
    let replayed = replayer.call(&asked).unwrap();
    assert_eq!(replayed, first);
    assert_eq!(replayed.text, r#"{"group":"orders"}"#);
    assert_eq!(offline.count(), 0);
}
