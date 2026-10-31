//! S3, the top-down plan's deterministic part (architecture.md, "Planning: stages, ghosts and lint
//! (R5)"; DR-4, DR-23, DR-89): an agent's proposal lands as ghosts that only a person accepts,
//! item by item, and structural lint reads the accepted plan alone. `intent next` drafts each
//! stage with Claude, answered from `tests/replay/claude/plan.stage.<S>/` under strict replay, and
//! its items are Claude's ghosts whoever asked. Person steps run with a one-time token under the
//! test's registered key; agent steps with `PLANROOM_ACTOR` and no token.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_testkit::fixture;
use serde_json::{Value, json};

const AGENT: &str = "agent:1a2b3c4d:chat";
const INTENT: &str = "Add retries to the worker";

/// The checkout's recordings, which `intent next` answers from under strict replay and which
/// `cargo xtask live record S03` writes in record mode.
fn replay_dir() -> PathBuf {
    std::fs::canonicalize(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/replay")).unwrap()
}

/// `planroom ARGS --json` with `env` and `stdin`: the exit code and the one envelope on stdout.
///
/// A model request's bytes are its replay key, so nothing of the user's reaches one: no
/// user-level config (`XDG_CONFIG_HOME` is a folder beside the workspace that holds none), no
/// `PLANROOM_MODELS` or `PLANROOM_CLAUDE_*`, and no marker of a temporary host
/// (`CLAUDE_CODE_REMOTE`, `CODESPACES`, `REMOTE_CONTAINERS`), so a run never takes one. The replay
/// mode, the live flag and the spend log pass through, so `cargo xtask live record S03` reaches
/// the engine.
fn run(root: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env("PLANROOM_REPLAY_DIR", replay_dir())
        .env("XDG_CONFIG_HOME", root.with_file_name("config-home"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .env_remove("PLANROOM_MODELS")
        .env_remove("CLAUDE_CODE_REMOTE")
        .env_remove("CODESPACES")
        .env_remove("REMOTE_CONTAINERS")
        .args(args)
        .arg("--json");
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(|n| n.starts_with("PLANROOM_CLAUDE_")) {
            cmd.env_remove(name);
        }
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

fn key() -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = 7;
    bytes[0] = 1;
    SigningKey::from_slice(&bytes).unwrap()
}

fn b64url(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// A person's token over `command` with the argv after it, as the mod signs one.
fn token(command: &str, args: &[&str], base: &str) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key().sign(&person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// `planroom COMMAND ARGS` as the person, signed; `base` is the `--base` the args carry, if any.
fn person(root: &Path, command: &str, args: &[&str], base: &str, stdin: &str) -> (i32, Value) {
    let signed = token(command, args, base);
    let argv: Vec<&str> = command.split(' ').chain(args.iter().copied()).collect();
    run(root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], stdin)
}

/// A scanned copy of the fixture with the test person's key registered.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let (code, env) = run(&root, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let signed = token("person init", &[], "");
    let (code, env) = run(&root, &["person", "init"], &[("PLANROOM_PERSON_TOKEN", &signed)], &public);
    assert_eq!(code, 0, "{env}");
    (tmp, root)
}

fn seq(root: &Path) -> String {
    let (_, env) = run(root, &["map"], &[], "");
    env["meta"]["seq"].as_u64().unwrap().to_string()
}

/// One proposed item.
fn item(key: &str, kind: &str, parent: Option<&str>, props: Value, touches: &[&str]) -> Value {
    json!({"key": key, "type": kind, "name": format!("The {key} item"), "parent": parent,
           "props": props, "touches": touches})
}

#[test]
fn agent_accept_is_person_required() {
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &[INTENT], "", "");
    assert_eq!((code, env["result"]["status"].as_str()), (0, Some("applied")), "{env}");

    let base = seq(&root);
    let proposal = json!({"stage": "next_actions", "items": [
        item("retry", "next_action", None, json!({"verb": "wrap"}), &["worker/consumer.py"]),
    ]});
    let argv = ["intent", "propose", "--file", "-", "--base", &base];
    let (code, env) = run(&root, &argv, &[("PLANROOM_ACTOR", AGENT)], &proposal.to_string());
    assert_eq!((code, env["result"]["status"].as_str()), (0, Some("ghost")), "{env}");

    let (_, list) = run(&root, &["intent", "list"], &[], "");
    let items = list["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{list}");
    assert_eq!(items[0]["status"], "ghost");
    assert_eq!(items[0]["touches"], json!(["worker:file:worker/consumer.py"]));
    let ghost = items[0]["pending"][0].as_str().unwrap().to_owned();

    // An agent never accepts: person_required, with the exact review a person runs.
    let head = seq(&root);
    let (code, env) =
        run(&root, &["review", "--base", &head, &ghost, "accept"], &[("PLANROOM_ACTOR", AGENT)], "");
    assert_eq!(code, 5, "{env}");
    assert_eq!(env["error"]["code"], "person_required");
    assert_eq!(env["error"]["details"]["command"], "review");
    let line = format!("planroom review --base {head} {ghost} accept");
    assert!(env["error"]["hint"].as_str().unwrap().contains(&line), "{env}");

    // The ghost is still listed, and lint reads only the accepted plan.
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    assert_eq!(list["result"]["items"][0]["status"], "ghost", "{list}");
    let (code, lint) = run(&root, &["intent", "lint"], &[], "");
    assert_eq!(code, 0, "{lint}");
    assert_eq!(lint["result"]["findings"], json!([]), "{lint}");
    assert_eq!(lint["result"]["empty"]["reason"], "no_findings");
}

#[test]
fn lint_flags_a_missing_done_test() {
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &[INTENT], "", "");
    assert_eq!(code, 0, "{env}");
    let must = || json!("must");
    let plant = json!({"items": [
        item("intent", "intent", None, json!({"when": "a job fails", "want": "retry it",
            "so_that": "orders aren't lost", "ladder": ["fewer lost orders", "retry failed jobs"]}), &[]),
        item("outcome", "outcome", None, json!({"metric": "failed jobs retried", "target": "99%",
            "deadline": "2026-12-01"}), &[]),
        item("scope", "non_goal", None, json!({}), &[]),
        item("work", "work", None, json!({"method": "HTN", "scope": ["retry", "backoff", "alert"],
            "moscow": must()}), &[]),
        item("retries", "work", Some("work"), json!({"scope": ["retry", "backoff"], "moscow": must()}), &[]),
        item("alerting", "work", Some("work"), json!({"scope": ["alert"], "moscow": "should"}), &[]),
        item("outage", "risk", None, json!({"story": "retries pile up", "likelihood": "high",
            "mitigation": "cap the attempts"}), &[]),
        item("wrap", "next_action", Some("retries"), json!({"verb": "wrap", "done_test":
            "a failing pop is retried three times", "moscow": must()}), &["worker:file:worker/consumer.py"]),
        item("tune", "next_action", Some("retries"), json!({"verb": "improve", "done_test":
            "it is better", "moscow": "could"}), &["platform:file:docker-compose.yml"]),
        item("alert", "next_action", Some("alerting"), json!({"verb": "add", "moscow": "should"}),
            &["platform:file:prometheus/alerts.yml"]),
    ]});
    std::fs::write(root.join("plant.json"), plant.to_string()).unwrap();
    let base = seq(&root);
    let (code, env) = person(&root, "intent propose", &["--file", "plant.json", "--base", &base], &base, "");
    assert_eq!((code, env["result"]["status"].as_str()), (0, Some("applied")), "{env}");

    let (code, lint) = run(&root, &["intent", "lint"], &[], "");
    assert_eq!(code, 0, "{lint}");
    let findings = lint["result"]["findings"].as_array().unwrap();
    let found: Vec<(&str, &str)> =
        findings.iter().map(|f| (f["rule"].as_str().unwrap(), f["subject"].as_str().unwrap())).collect();
    assert_eq!(found, [("next_action_done_test", "plan:add-retries-to-the-worker:alert")], "{lint}");
    assert_eq!(findings[0]["stage"], "next_actions");
    let handle = &findings[0]["handles"][0];
    assert_eq!(handle["command"], "ask");
    assert_eq!(handle["args"], json!(["stage", "next_actions", "plan:add-retries-to-the-worker:project"]));

    // Re-proposing the item with a done test, and no parent, sets that field alone: the item keeps
    // its place in the tree, and lint finds nothing.
    let fixed = json!({"items": [item("alert", "next_action", None, json!({"verb": "add",
        "done_test": "the alert fires on three failed retries", "moscow": "should"}),
        &["platform:file:prometheus/alerts.yml"])]});
    std::fs::write(root.join("plant.json"), fixed.to_string()).unwrap();
    let base = seq(&root);
    let (code, env) = person(&root, "intent propose", &["--file", "plant.json", "--base", &base], &base, "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["count"], 1, "one set of the field that differs: {env}");
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    let alert = list["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "plan:add-retries-to-the-worker:alert");
    assert_eq!(alert.unwrap()["parent"], "plan:add-retries-to-the-worker:alerting", "{list}");
    let (_, lint) = run(&root, &["intent", "lint"], &[], "");
    assert_eq!(lint["result"]["findings"], json!([]), "{lint}");
    assert_eq!(lint["result"]["empty"]["reason"], "no_findings");

    // Naming the project as its parent is a field that differs: the item moves there.
    let moved = json!({"items": [{"key": "alert", "type": "next_action", "name": "The alert item",
        "parent": "project"}]});
    std::fs::write(root.join("plant.json"), moved.to_string()).unwrap();
    let base = seq(&root);
    let (code, env) = person(&root, "intent propose", &["--file", "plant.json", "--base", &base], &base, "");
    assert_eq!(code, 0, "{env}");
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    let alert = list["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "plan:add-retries-to-the-worker:alert");
    assert_eq!(alert.unwrap()["parent"], "plan:add-retries-to-the-worker:project", "{list}");

    // A parent that closes a cycle names the proposal's item the cycle runs through, not the
    // first item: `tune` keeps its parent, and `work` under `wrap` closes work, wrap, retries.
    let cycle = json!({"items": [
        {"key": "tune", "type": "next_action", "name": "Tune the retries"},
        {"key": "work", "type": "work", "name": "The work", "parent": "wrap"},
    ]});
    std::fs::write(root.join("plant.json"), cycle.to_string()).unwrap();
    let base = seq(&root);
    let (code, env) = person(&root, "intent propose", &["--file", "plant.json", "--base", &base], &base, "");
    assert_ne!(code, 0, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    assert_eq!(env["error"]["details"]["field"], "items[1].parent", "{env}");
}

/// Every item `intent list` lists for the only project, page after page.
fn items(root: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    let mut args: Vec<String> = vec!["intent".into(), "list".into()];
    loop {
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let (code, list) = run(root, &argv, &[], "");
        assert_eq!(code, 0, "{list}");
        out.extend(list["result"]["items"].as_array().unwrap().iter().cloned());
        let Some(next) = list["meta"]["next"].as_str() else { return out };
        args = vec!["intent".into(), "list".into(), "--cursor".into(), next.into()];
    }
}

fn item_by<'a>(items: &'a [Value], id: &str) -> &'a Value {
    items.iter().find(|i| i["id"] == id).unwrap_or_else(|| panic!("no item {id} in {items:?}"))
}

/// The questions among `items`: each item `intent list` gives a question state.
fn questions(items: &[Value]) -> Vec<&Value> {
    items.iter().filter(|i| i["question"].is_object()).collect()
}

/// The intent stage's checklist asks for its guesses as questions (DR-116): the recorded draft
/// holds at least one, with an assumption and items resting on it that its `rests_on` resolved
/// to among the plan's, and `intent list` gives each its state.
#[test]
fn a_drafted_stage_writes_its_guesses_as_questions() {
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &[INTENT], "", "");
    assert_eq!(code, 0, "{env}");
    let project = env["result"]["targets"][0].as_str().unwrap().to_owned();
    let (code, env) = person(&root, "intent next", &["--stage", "intent"], "", "");
    assert_eq!((code, env["result"]["status"].as_str()), (0, Some("ghost")), "{env}");

    let listed = items(&root);
    let asked = questions(&listed);
    assert!(!asked.is_empty(), "the intent stage writes a question: {listed:?}");
    for question in &asked {
        assert_eq!(question["status"], "ghost", "{question}");
        assert!(question["stage"].is_null(), "a question belongs to no stage: {question}");
        assert!(question["question"]["rests"].as_u64().unwrap() >= 1, "rests_on resolves: {question}");
        assert_eq!(question["question"]["status"], "open", "{question}");
    }

    // `ask stage` shows each open question's assumption.
    let (code, env) = run(&root, &["ask", "stage", "intent", &project], &[], "");
    assert_eq!(code, 0, "{env}");
    let rows = env["result"]["items"].as_array().unwrap();
    let shown: Vec<&Value> = rows.iter().filter(|r| asked.iter().any(|q| q["id"] == r["id"])).collect();
    assert!(!shown.is_empty(), "{env}");
    for row in shown {
        let why = row["why"].as_str().unwrap();
        let assumption =
            why.strip_prefix("Assumed: ").and_then(|w| w.split(" · ").next()).unwrap_or_default();
        assert!(!assumption.trim().is_empty(), "the question has its assumption: {row}");
    }
}

#[test]
fn stages_produce_ghosts_until_accepted() {
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &[INTENT], "", "");
    assert_eq!((code, env["result"]["status"].as_str()), (0, Some("applied")), "{env}");

    let stages = ["intent", "outcome", "breakdown", "next_actions", "priorities", "premortem"];
    for (n, stage) in stages.into_iter().enumerate() {
        // The person asks for the stage; Claude's items wait as Claude's ghosts all the same.
        let (code, env) = person(&root, "intent next", &["--stage", stage], "", "");
        assert_eq!(code, 0, "{stage}: {env}");
        assert_eq!(env["result"]["status"], "ghost", "{stage}: {env}");
        assert!(env["result"]["count"].as_u64().unwrap() >= 1, "{stage}: {env}");
        let claude = &env["meta"]["models"]["claude"];
        assert_eq!(claude["calls"], 1, "{stage}: {env}");
        if n > 0 {
            assert_eq!(claude["cache_hits"], 1, "{stage} reads the prefix the first stage wrote: {env}");
        }
        let named: Vec<String> = env["result"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap().to_owned())
            .collect();
        assert!(!named.is_empty(), "{stage}: {env}");

        // Each drafted item is a new ghost, or a live item whose change waits, as each item
        // priorities rates does. The reply names the IDs that fit its budget and counts the rest,
        // and before the stage every item was accepted, so the items that wait now are the stage's.
        let listed = items(&root);
        if stage == "intent" {
            // The first stage writes its guesses as questions; without one, no later stage runs.
            assert!(!questions(&listed).is_empty(), "the intent stage writes a question: {listed:?}");
        }
        let waiting = |item: &Value| !item["pending"].as_array().unwrap().is_empty();
        let targets: Vec<String> =
            listed.iter().filter(|i| waiting(i)).map(|i| i["id"].as_str().unwrap().to_owned()).collect();
        let more = env["result"]["more_targets"].as_u64().unwrap() as usize;
        assert!(
            targets.len() <= named.len() + more && named.iter().all(|id| targets.contains(id)),
            "{stage}: {env}"
        );
        for id in &named {
            let item = item_by(&listed, id);
            assert!(item["status"] == "ghost" || waiting(item), "{stage}: {item}");
        }
        // Lint reads only what a person accepted.
        let (code, lint) = run(&root, &["intent", "lint"], &[], "");
        assert_eq!(code, 0, "{lint}");
        for finding in lint["result"]["findings"].as_array().into_iter().flatten() {
            let subject = finding["subject"].as_str().unwrap();
            let ghost =
                targets.iter().any(|t| t == subject) && item_by(&listed, subject)["status"] == "ghost";
            assert!(!ghost, "{stage}: lint names the ghost {subject}");
        }

        // The person accepts the items one by one, parents first, each item's events in order:
        // its node's, then its touch edges'.
        let mut left: Vec<String> = targets.clone();
        while !left.is_empty() {
            let listed = items(&root);
            let ready = left
                .iter()
                .position(|id| {
                    let parent = item_by(&listed, id)["parent"].as_str().unwrap_or_default().to_owned();
                    !left.contains(&parent)
                })
                .unwrap_or_else(|| panic!("{stage}: a parent cycle among {left:?}"));
            let id = left.remove(ready);
            let pending: Vec<String> = item_by(&listed, &id)["pending"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e.as_str().unwrap().to_owned())
                .collect();
            for event in &pending {
                let base = seq(&root);
                let (code, env) = person(&root, "review", &["--base", &base, event, "accept"], &base, "");
                assert_eq!(code, 0, "{stage}: accepting {event} of {id}: {env}");
            }
            let listed = items(&root);
            assert_eq!(item_by(&listed, &id)["pending"], json!([]), "{stage}: {id} is accepted");
            assert_eq!(item_by(&listed, &id)["status"], "applied", "{stage}: {id}");
            for other in &left {
                let waiting = item_by(&listed, other)["pending"].as_array().unwrap().len();
                assert!(waiting > 0, "{stage}: {other} still waits for the person");
            }
        }
    }

    // Every item is accepted, and with every stage drafted the next is lint, which drafts nothing.
    assert!(items(&root).iter().all(|i| i["pending"] == json!([])));
    let (code, env) = person(&root, "intent next", &[], "", "");
    assert_eq!(code, 2, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    assert_eq!(env["error"]["details"]["field"], "stage", "{env}");
    // Its line names the exact lint for the plan.
    let lint = "planroom intent lint --project plan:add-retries-to-the-worker:project";
    assert!(env["error"]["message"].as_str().unwrap().contains(lint), "{env}");
}

/// What `intent next` settles before it would ask Claude: a stage that drafts nothing, a stage no
/// pack declares, and a key the store already holds each answer at once, with no model call, so
/// they hold with no recording and with the replay mode unset, where any call would be refused.
#[test]
fn a_stage_that_drafts_nothing_is_refused_before_any_call() {
    let (_tmp, root) = workspace();
    let agent = [("PLANROOM_ACTOR", AGENT), ("PLANROOM_REPLAY", "")];
    let (code, env) = run(&root, &["intent", "new", INTENT, "--idem", "plan-start"], &agent, "");
    assert_eq!(code, 0, "{env}");
    let before = seq(&root);

    let (code, env) = run(&root, &["intent", "next", "--stage", "lint"], &agent, "");
    assert_eq!((code, env["error"]["details"]["field"].as_str()), (2, Some("stage")), "{env}");
    let lint = "planroom intent lint --project plan:add-retries-to-the-worker:project";
    assert!(env["error"]["message"].as_str().unwrap().contains(lint), "{env}");
    // The hint is the exact `intent lint`, which the details hand off as `next`; the leaf's
    // schema handle stays in the details.
    assert_eq!(env["error"]["hint"], lint, "{env}");
    let next = &env["error"]["details"]["next"];
    assert_eq!(
        (next["command"].as_str(), &next["args"]),
        (Some("intent"), &json!(["lint", "--project", "plan:add-retries-to-the-worker:project"])),
        "{env}"
    );
    assert_eq!(env["error"]["details"]["schema"]["args"], json!(["--command", "intent next"]), "{env}");

    let (code, env) = run(&root, &["intent", "next", "--stage", "nope"], &agent, "");
    assert_eq!((code, env["error"]["details"]["field"].as_str()), (2, Some("stage")), "{env}");
    assert_eq!(env["error"]["hint"], "planroom schema --command \"intent next\"", "{env}");
    assert!(env["error"]["message"].as_str().unwrap().contains("premortem"), "the stages are named: {env}");

    // A key another command stored meets idempotency_mismatch before any call.
    let (code, env) =
        run(&root, &["intent", "next", "--stage", "intent", "--idem", "plan-start"], &agent, "");
    assert_eq!((code, env["error"]["code"].as_str()), (4, Some("idempotency_mismatch")), "{env}");
    assert_eq!(seq(&root), before, "nothing was written");
}
