//! No credential reaches a recording, the spend log or an error (architecture.md, Security and
//! privacy: the poison-value test). Fake values stand in for every key variable; a recording run
//! whose provider echoes them back, in its body and in headers a record never keeps, leaves none
//! of them on disk, and a request that carries one is never recorded.

mod support;

use std::path::Path;

use planroom_contract::ErrorCode;
use planroom_models::claude::{self, Claude};
use planroom_models::replay::ReplayDir;
use planroom_models::transport::HttpResponse;
use serde_json::json;
use support::{Fake, call, config, recording, spend_log};

const POISON: &[(&str, &str)] = &[
    ("AI_GATEWAY_API_KEY", "poison-gateway-4f1c9a0e7d2b"),
    ("TYPESAFE_API_KEY", "poison-typesafe-8b3e6d1f0a92"),
    ("OPENAI_API_KEY", "poison-openai-2c7a9e4b1d03"),
    ("GEMINI_API_KEY", "poison-gemini-6e0d3b8a5f14"),
];

fn var(name: &str) -> Option<String> {
    POISON.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_owned())
}

/// Every file under `dir`, as text.
fn every_file(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push((
                    path.display().to_string(),
                    String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into(),
                ));
            }
        }
    }
    out
}

#[test]
fn poison_values_never_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let log = spend_log(dir.path(), 0.0);
    let cfg = config("https://ai-gateway.vercel.sh");
    let credential = claude::credential(&cfg, &var).unwrap();
    assert_eq!(credential.as_ref().map(|c| c.expose()), var("AI_GATEWAY_API_KEY").as_deref());
    assert!(!format!("{credential:?}").contains("poison"), "a credential's Debug shows no value");
    let secrets = claude::secrets(credential.as_ref(), &var);
    assert_eq!(secrets.len(), POISON.len());
    let gate = recording(&log);

    // A provider that echoes every poison value back, in its body and in headers.
    let echo: String = POISON.iter().map(|(_, v)| *v).collect::<Vec<_>>().join(" ");
    let mut body = support::message(&format!("you sent {echo}"), "0.001");
    body["error_hint"] = json!({ "seen": echo });
    let mut fake = Fake::answering(200, &body);
    fake.response = HttpResponse {
        status: 200,
        headers: vec![
            ("content-type".into(), format!("application/json; {echo}")),
            ("set-cookie".into(), format!("session={echo}")),
            ("x-api-key".into(), echo.clone()),
        ],
        body: fake.response.body.clone(),
    };
    let adapter = Claude {
        config: &cfg,
        cap_usd: 3.0,
        env: &gate,
        replay: &replay,
        transport: &fake,
        credential: credential.clone(),
        secrets: secrets.clone(),
    };
    adapter.call(&call("Which group?", json!({}))).unwrap();
    assert_eq!(fake.count(), 1);
    assert_eq!(fake.sent.borrow()[0].credential.as_deref(), var("AI_GATEWAY_API_KEY").as_deref());

    // A request that carries a poison value (a planted secret the engine failed to mask) is never
    // sent, charged or recorded, and its error says nothing of the value.
    let log_before = std::fs::read_to_string(&log).unwrap();
    let planted = call(&format!("Which group? {}", POISON[2].1), json!({}));
    let err = adapter.call(&planted).unwrap_err();
    assert_eq!(err.code, ErrorCode::Internal);
    assert_eq!(fake.count(), 1, "the planted request was never sent");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), log_before, "nor charged");

    let files = every_file(dir.path());
    assert!(files.iter().any(|(p, _)| p.ends_with(".json")), "the first call was recorded");
    for (path, text) in &files {
        for (name, value) in POISON {
            assert!(!text.contains(value), "{path} holds {name}'s value");
        }
        assert!(!text.to_ascii_lowercase().contains("set-cookie"), "{path} keeps a cookie header");
        assert!(!text.to_ascii_lowercase().contains("authorization"), "{path} keeps an auth header");
    }
    for (name, value) in POISON {
        assert!(!format!("{err} {err:?}").contains(value), "the error holds {name}'s value");
    }
}
