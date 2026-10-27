//! A credential goes only to `ai-gateway.vercel.sh`, `api.typesafe.ai` or the host of a
//! user-level `base_url` (architecture.md, What leaves the machine; DR-48). A request to any other
//! host is refused before the transport sees it or the credential, and before the spend log is
//! charged.

mod support;

use planroom_contract::ErrorCode;
use planroom_models::claude::Claude;
use planroom_models::hosts::Allowed;
use planroom_models::replay::ReplayDir;
use planroom_models::transport::Credential;
use serde_json::json;
use support::{Fake, call, config, recording, spend_log};

const KEY: &str = "fake-gateway-key-for-hosts-test";

#[test]
fn credentials_go_only_to_allow_listed_hosts() {
    let allowed = Allowed::new("https://ai-gateway.vercel.sh").unwrap();
    for url in ["https://ai-gateway.vercel.sh/v1/messages", "https://api.typesafe.ai/v1/systemone"] {
        assert!(allowed.check(url).is_ok(), "{url}");
    }
    for url in [
        "https://ai-gateway.vercel.sh.attacker.example/v1/messages",
        "https://attacker.example/ai-gateway.vercel.sh/v1/messages",
        "https://ai-gateway.vercel.sh@attacker.example/v1/messages",
        "https://user:pass@ai-gateway.vercel.sh/v1/messages",
        "http://ai-gateway.vercel.sh/v1/messages",
        "https://ai-gateway.vercel.sh:8443/v1/messages",
        "https://ai-gateway.vercel.sh/v1/messages?key=x",
        "ai-gateway.vercel.sh/v1/messages",
    ] {
        let err = allowed.check(url).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgument, "{url}");
        assert_eq!(err.details["field"], json!("claude.base_url"));
    }

    // End to end: the default base URL gets the credential; a base URL that hides another host is
    // refused with nothing sent and nothing charged; the user-level base URL's own host is allowed.
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let log = spend_log(dir.path(), 0.0);
    let gate = recording(&log);
    for (base, ok) in [
        ("https://ai-gateway.vercel.sh", true),
        ("https://ai-gateway.vercel.sh@attacker.example", false),
        ("http://ai-gateway.vercel.sh", false),
        ("https://gateway.internal.example", true),
    ] {
        let cfg = config(base);
        let fake = Fake::message("{}", "0.001");
        let before = std::fs::read_to_string(&log).unwrap();
        let adapter = Claude {
            config: &cfg,
            cap_usd: 3.0,
            env: &gate,
            replay: &replay,
            transport: &fake,
            credential: Credential::new(KEY),
            secrets: vec![KEY.into()],
        };
        let result = adapter.call(&call(&format!("ask {base}"), json!({})));
        if ok {
            result.unwrap();
            let sent = fake.sent.borrow();
            assert_eq!(sent.len(), 1);
            let host = base.trim_start_matches("https://");
            assert_eq!(sent[0].request.url, format!("https://{host}/v1/messages"));
            assert_eq!(sent[0].credential.as_deref(), Some(KEY));
            assert!(sent[0].request.headers.iter().all(|(_, v)| !v.contains(KEY)), "no header holds the key");
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidArgument, "{base}");
            assert_eq!(fake.count(), 0, "{base}: nothing sent");
            assert_eq!(std::fs::read_to_string(&log).unwrap(), before, "{base}: nothing charged");
            assert!(!format!("{err} {err:?}").contains(KEY));
        }
    }
}
