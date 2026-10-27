//! The live gate and the build-wide spend log (architecture.md, Spend guards and What leaves the
//! machine; DR-49, DR-65, DR-89).
//!
//! `scan` reads the gate before it does anything: a relative `PLANROOM_SPEND_LOG`, or a
//! `PLANROOM_LIVE_*` flag with no spend log, is `invalid_argument`, and an ambient proxy alone
//! makes no model request. No command makes a live call yet (`intent next` brings the first), so
//! the cap, its margin and the shared log are checked through the models crate's adapter with a
//! transport that never touches the network, and across processes run from different worktrees
//! and folders of one checkout.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use planroom_contract::config::Claude as ClaudeConfig;
use planroom_contract::{CacheTtl, ErrorCode, ModelTier};
use planroom_models::claude::{Call, Claude};
use planroom_models::gate::LiveEnv;
use planroom_models::replay::ReplayDir;
use planroom_models::spend::{Row, SpendLog};
use planroom_models::transport::{Credential, HttpRequest, HttpResponse, Transport, TransportError};
use serde_json::{Value, json};

/// Every variable the gate or a credential reads, removed so a test starts from none of them.
const GATE_VARS: &[&str] = &[
    "PLANROOM_SPEND_LOG",
    "PLANROOM_REPLAY",
    "PLANROOM_LIVE_GATEWAY",
    "PLANROOM_LIVE_JEV",
    "AI_GATEWAY_API_KEY",
    "VERCEL_OIDC_TOKEN",
    "TYPESAFE_API_KEY",
];

/// A one-repo workspace.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    planroom_testkit::git_repo(&dir.path().join("orders"), &[("README.md".into(), b"# orders\n".to_vec())]);
    dir
}

/// `planroom scan --json` in `cwd` with only `vars` of the gate's variables set.
fn scan(cwd: &Path, vars: &[(&str, &str)]) -> (Value, Output) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(["scan", "--json"]).current_dir(cwd).env("PLANROOM_DATA", cwd.join(".data"));
    for name in GATE_VARS {
        cmd.env_remove(name);
    }
    for (k, v) in vars {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("the binary runs");
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        stdout.lines().count(),
        1,
        "one JSON line: {stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (serde_json::from_str(&stdout).unwrap(), out)
}

/// The scan was refused with `invalid_argument` on `field`, before it wrote anything.
fn refused_on(dir: &Path, env: &Value, out: &Output, field: &str) {
    assert_eq!(env["ok"], false, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    assert_eq!(env["error"]["details"]["field"], field, "{env}");
    assert_eq!(out.status.code(), Some(2));
    assert!(!env["error"]["message"].as_str().unwrap().is_empty());
    assert!(!env["error"]["hint"].as_str().unwrap().is_empty());
    assert!(
        !dir.join("orders/.git/planroom").exists() && !dir.join(".planroom").exists(),
        "nothing was written"
    );
}

fn config() -> ClaudeConfig {
    serde_json::from_value(json!({
        "backend": "auto", "base_url": "https://ai-gateway.vercel.sh", "api_key_env": "AI_GATEWAY_API_KEY",
        "api_key": "", "model_cheap": "test/cheap", "model_capable": "test/capable", "providers": ["anthropic"]
    }))
    .unwrap()
}

fn gate(vars: &[(&str, &str)]) -> LiveEnv {
    LiveEnv::from_vars(vars.iter().map(|(k, v)| (OsString::from(k), OsString::from(v)))).unwrap()
}

fn recording(log: &Path) -> LiveEnv {
    gate(&[
        ("PLANROOM_REPLAY", "record"),
        ("PLANROOM_LIVE_GATEWAY", "1"),
        ("PLANROOM_SPEND_LOG", log.to_str().unwrap()),
    ])
}

fn ask(question: &str, ceiling_usd: f64) -> Call {
    Call {
        purpose: "test.live_gate".into(),
        tier: ModelTier::Cheap,
        prefix: vec!["Rules for the test.".into()],
        ttl: CacheTtl::M5,
        tail: vec![question.into()],
        output_schema: None,
        max_tokens: 50,
        effort: None,
        ceiling_usd,
        scenario: "live_gate".into(),
    }
}

/// A transport that answers with a message costing `cost` and counts what it was sent.
struct Fake {
    cost: &'static str,
    sent: std::cell::Cell<usize>,
}

impl Transport for Fake {
    fn send(&self, _: &HttpRequest, _: Option<&Credential>) -> Result<HttpResponse, TransportError> {
        self.sent.set(self.sent.get() + 1);
        let body = json!({"id": "gen_fake", "type": "message", "content": [{"type": "text", "text": "ok"}],
            "model": "test/cheap", "stop_reason": "end_turn",
            "usage": {"input_tokens": 5, "output_tokens": 1}, "provider_metadata": {"gateway": {"cost": self.cost}}});
        Ok(HttpResponse { status: 200, headers: vec![], body: serde_json::to_vec(&body).unwrap() })
    }
}

/// A spend log at `dir/spend.jsonl` whose rows sum to `spent`.
fn spend_log(dir: &Path, spent: f64) -> PathBuf {
    let path = dir.join("spend.jsonl");
    let row = json!({"at": "2026-10-25T00:00:00Z", "generation_id": null, "provider": "gateway",
        "model": "test/cheap", "cost_usd": spent, "scenario": "seed"});
    std::fs::write(&path, format!("{row}\n")).unwrap();
    path
}

fn total(log: &Path) -> f64 {
    SpendLog::open(log).unwrap().total().unwrap()
}

#[test]
fn a_relative_spend_log_is_refused() {
    let dir = workspace();
    let (env, out) = scan(dir.path(), &[("PLANROOM_SPEND_LOG", "docs/evidence/spend.jsonl")]);
    refused_on(dir.path(), &env, &out, "PLANROOM_SPEND_LOG");
    let err = SpendLog::open(Path::new("docs/evidence/spend.jsonl")).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    // An absolute one that names no file is refused too: a new file would start at zero.
    let (env, _) =
        scan(dir.path(), &[("PLANROOM_SPEND_LOG", dir.path().join("none.jsonl").to_str().unwrap())]);
    assert_eq!(env["ok"], true, "scan only reads the gate; the file is opened for a live call");
    let err = SpendLog::open(&dir.path().join("none.jsonl")).unwrap_err();
    assert_eq!(err.details["field"], "PLANROOM_SPEND_LOG");
    // An absolute spend log that exists lets the scan run.
    let log = spend_log(dir.path(), 0.0);
    let (env, _) = scan(dir.path(), &[("PLANROOM_SPEND_LOG", log.to_str().unwrap())]);
    assert_eq!(env["ok"], true, "{env}");
}

#[test]
fn a_live_flag_without_a_spend_log_is_refused() {
    for flag in ["PLANROOM_LIVE_GATEWAY", "PLANROOM_LIVE_JEV"] {
        let dir = workspace();
        let (env, out) = scan(dir.path(), &[(flag, "1")]);
        refused_on(dir.path(), &env, &out, flag);
    }
    let dir = workspace();
    let log = spend_log(dir.path(), 0.0);
    let (env, _) =
        scan(dir.path(), &[("PLANROOM_LIVE_GATEWAY", "1"), ("PLANROOM_SPEND_LOG", log.to_str().unwrap())]);
    assert_eq!(env["ok"], true, "{env}");
    // Record mode makes live calls, so it needs a flag.
    let (env, out) =
        scan(dir.path(), &[("PLANROOM_REPLAY", "record"), ("PLANROOM_SPEND_LOG", log.to_str().unwrap())]);
    assert_eq!(env["error"]["details"]["field"], "PLANROOM_REPLAY", "{env}");
    assert_eq!(out.status.code(), Some(2));
    // Under --json too the hint is the fix that sets the variable, beside the leaf's synopsis
    // and schema handle, which describe its arguments.
    assert_eq!(env["error"]["hint"], "export PLANROOM_REPLAY=strict", "{env}");
    assert!(
        env["error"]["details"]["synopsis"].is_string() && env["error"]["details"]["schema"].is_object(),
        "{env}"
    );
    // In text mode the hint is an exact command, with no placeholder to fill in.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.arg("scan").current_dir(dir.path()).env("PLANROOM_DATA", dir.path().join(".data"));
    for name in GATE_VARS {
        cmd.env_remove(name);
    }
    let out = cmd.env("PLANROOM_REPLAY", "record").env("PLANROOM_SPEND_LOG", &log).output().unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(stderr.lines().nth(1).map(str::trim), Some("export PLANROOM_REPLAY=strict"), "{stderr}");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn the_spend_log_stops_live_calls_at_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let log = spend_log(dir.path(), 2.70);
    let env = recording(&log);
    let cfg = config();
    let fake = Fake { cost: "0.0123", sent: 0.into() };
    let adapter = Claude {
        config: &cfg,
        cap_usd: 3.0,
        env: &env,
        replay: &replay,
        transport: &fake,
        credential: None,
        secrets: vec![],
    };
    let before = std::fs::read_to_string(&log).unwrap();
    let err = adapter.call(&ask("too dear", 0.10)).unwrap_err();
    assert_eq!(err.code, ErrorCode::BudgetExceeded);
    assert_eq!(err.details["guard"], "live_tests_total_usd");
    assert_eq!(err.details["spent"], json!(2.7));
    assert_eq!(fake.sent.get(), 0, "a call past the cap is never sent");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), before, "and never written to the log");

    // A call within the cap is reserved before it is sent and settled to its reported cost.
    adapter.call(&ask("cheap enough", 0.05)).unwrap();
    assert_eq!(fake.sent.get(), 1);
    let rows: Vec<Value> =
        std::fs::read_to_string(&log).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1]["phase"], "reserved");
    assert_eq!(rows[1]["cost_usd"], json!(0.05));
    assert_eq!(rows[1]["provider"], "gateway");
    assert_eq!(rows[1]["model"], "test/cheap");
    assert_eq!(rows[2]["phase"], "settled");
    assert_eq!(rows[2]["generation_id"], "gen_fake");
    // The settled row holds the difference in whole millionths of a dollar, never a float's
    // remainder such as -0.037700000000000004.
    assert_eq!(rows[2]["cost_usd"], json!(-0.0377), "{}", std::fs::read_to_string(&log).unwrap());
    assert!((total(&log) - 2.7123).abs() < 1e-9, "the rows sum to what was spent");

    // The engine's envelope line for the refusal names the guard and the amounts.
    let shown = planroom::app::error::AppError::from(err);
    assert_eq!(shown.code, ErrorCode::BudgetExceeded);
    assert!(
        shown.message.contains("live_tests_total_usd") && shown.message.contains("$2.70"),
        "{}",
        shown.message
    );

    // Outside record mode a live call is refused before anything, whatever the log holds.
    let live = gate(&[("PLANROOM_LIVE_GATEWAY", "1"), ("PLANROOM_SPEND_LOG", log.to_str().unwrap())]);
    let adapter = Claude { env: &live, ..adapter };
    let err = adapter.call(&ask("live, not recording", 0.01)).unwrap_err();
    assert_eq!(err.code, ErrorCode::BudgetExceeded);
    assert_eq!(err.details["guard"], "replay_only");
    assert_eq!(fake.sent.get(), 1);
}

#[test]
fn the_cap_keeps_a_25_cent_margin() {
    let dir = tempfile::tempdir().unwrap();
    let log = spend_log(dir.path(), 2.70);
    let spend = SpendLog::open(&log).unwrap();
    let row = |cost: f64| Row {
        at: "2026-10-27T00:00:00Z".into(),
        generation_id: None,
        provider: "gateway".into(),
        model: "test/cheap".into(),
        cost_usd: cost,
        scenario: "live_gate".into(),
        phase: Some("reserved".into()),
    };
    assert!((spend.remaining(3.0).unwrap() - 0.05).abs() < 1e-9, "$3.00 less $0.25 less $2.70");
    // Up to $2.75 exactly is allowed; a cent past it is not, though it's well under $3.00.
    spend.reserve(3.0, &row(0.05)).unwrap();
    let err = spend.reserve(3.0, &row(0.01)).unwrap_err();
    assert_eq!(err.code, ErrorCode::BudgetExceeded);
    assert_eq!(err.details["limit"], json!(2.75));
    assert_eq!(err.details["spent"], json!(2.75));
    assert!((total(&log) - 2.75).abs() < 1e-9);
    // The cap is whatever the configuration says, less the same margin.
    spend.reserve(3.5, &row(0.50)).unwrap();
    assert!(spend.reserve(3.5, &row(0.01)).is_err());
}

/// The spend log `cargo xtask` names from `cwd`: the main checkout's `docs/evidence/spend.jsonl`,
/// found through `git rev-parse --git-common-dir` (xtask's `live::spend_log_path`, which its
/// `every_worktree_names_the_main_checkouts_spend_log` holds to this rule).
fn spend_log_from(cwd: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let common = PathBuf::from(String::from_utf8(out.stdout).unwrap().trim());
    common.parent().unwrap().join("docs/evidence/spend.jsonl").to_str().unwrap().to_owned()
}

/// One process's reservation, when the test below runs itself again from another folder: it
/// reserves `cost` in the spend log its environment names and prints whether that was allowed.
fn reserve_as_a_child(cost: &str) {
    let env = LiveEnv::from_process().unwrap();
    let log = SpendLog::open(env.spend_log.as_deref().expect("the spend log is set")).unwrap();
    let cwd = std::env::current_dir().unwrap();
    let row = Row {
        at: "2026-10-27T00:00:00Z".into(),
        generation_id: None,
        provider: "gateway".into(),
        model: "test/cheap".into(),
        cost_usd: cost.parse().unwrap(),
        scenario: cwd.file_name().unwrap().to_string_lossy().into_owned(),
        phase: Some("reserved".into()),
    };
    match log.reserve(3.0, &row) {
        Ok(_) => println!("LIVE_GATE reserved"),
        Err(e) => println!("LIVE_GATE refused {}", e.details["guard"]),
    }
}

#[test]
fn the_spend_log_is_shared_across_worktrees_and_cwds() {
    if let Ok(cost) = std::env::var("LIVE_GATE_CHILD_COST") {
        return reserve_as_a_child(&cost);
    }
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    planroom_testkit::git_repo(&main, &[("docs/evidence/spend.jsonl".into(), Vec::new())]);
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(&main).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    let linked = dir.path().join("linked");
    git(&["worktree", "add", "-q", "-b", "other", linked.to_str().unwrap()]);
    let nested = linked.join("docs/evidence");
    let log = main.join("docs/evidence/spend.jsonl");
    std::fs::write(
        &log,
        format!(
            "{}\n",
            json!({"at": "2026-10-25T00:00:00Z", "generation_id": null,
        "provider": "gateway", "model": "test/cheap", "cost_usd": 2.50, "scenario": "seed"})
        ),
    )
    .unwrap();
    let exe = std::env::current_exe().unwrap();
    // Each process gets the spend log cargo xtask names from its own folder.
    let child = |cwd: &Path, cost: &str| {
        let mut cmd = Command::new(&exe);
        cmd.args(["--exact", "the_spend_log_is_shared_across_worktrees_and_cwds", "--nocapture"])
            .args(["--test-threads", "1"])
            .current_dir(cwd);
        for name in GATE_VARS {
            cmd.env_remove(name);
        }
        let out = cmd
            .env("PLANROOM_SPEND_LOG", spend_log_from(cwd))
            .env("PLANROOM_LIVE_GATEWAY", "1")
            .env("LIVE_GATE_CHILD_COST", cost)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        stdout.find("LIVE_GATE").map(|i| stdout[i..].lines().next().unwrap().to_owned()).unwrap_or(stdout)
    };
    // The main checkout, a linked worktree and a folder inside it each add to the one file.
    assert_eq!(child(&main, "0.10"), "LIVE_GATE reserved");
    assert_eq!(child(&linked, "0.10"), "LIVE_GATE reserved");
    assert_eq!(child(&nested, "0.04"), "LIVE_GATE reserved");
    assert!((total(&log) - 2.74).abs() < 1e-9, "{}", std::fs::read_to_string(&log).unwrap());
    // So the cap counts all three: a fourth anywhere is refused.
    assert_eq!(child(&linked, "0.02"), "LIVE_GATE refused \"live_tests_total_usd\"");
    let scenarios: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["scenario"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(scenarios, ["seed", "main", "linked", "evidence"]);
    // A worktree's own copy of the file is never written.
    assert_eq!(std::fs::read_to_string(linked.join("docs/evidence/spend.jsonl")).unwrap(), "");
}

#[test]
fn the_proxy_alone_makes_no_model_request() {
    // A proxy that counts every connection it gets.
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    proxy.set_nonblocking(true).unwrap();
    let url = format!("http://{}", proxy.local_addr().unwrap());
    let proxied: Vec<(&str, &str)> = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"]
        .iter()
        .map(|k| (*k, url.as_str()))
        .collect();
    let connections = || {
        let mut n = 0;
        loop {
            match proxy.accept() {
                Ok(_) => n += 1,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return n,
                Err(e) => panic!("{e}"),
            }
        }
    };

    // A scan with only the proxy set runs, and asks no model.
    let dir = workspace();
    let (env, _) = scan(dir.path(), &proxied);
    assert_eq!(env["ok"], true, "{env}");
    assert!(env["meta"].get("models").is_none(), "no model call: {}", env["meta"]);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(connections(), 0, "scan reached the proxy");

    // With no flag and no key, the adapter refuses a live call and finds no record under strict
    // replay, and its transport is never handed a request.
    let fake = Fake { cost: "0.01", sent: 0.into() };
    let replay = ReplayDir::new(&dir.path().join("replay"));
    let cfg = config();
    for (vars, code) in
        [(vec![], ErrorCode::BudgetExceeded), (vec![("PLANROOM_REPLAY", "strict")], ErrorCode::ReplayMissing)]
    {
        let env = gate(&vars);
        let adapter = Claude {
            config: &cfg,
            cap_usd: 3.0,
            env: &env,
            replay: &replay,
            transport: &fake,
            credential: None,
            secrets: vec![],
        };
        assert_eq!(adapter.call(&ask("anything", 0.01)).unwrap_err().code, code, "{vars:?}");
    }
    assert_eq!(fake.sent.get(), 0);
    // Record mode without the gateway flag isn't a mode at all.
    let refused = LiveEnv::from_vars(
        [("PLANROOM_REPLAY", "record")].map(|(k, v)| (OsString::from(k), OsString::from(v))),
    );
    assert_eq!(refused.unwrap_err().details["field"], "PLANROOM_REPLAY");
}
