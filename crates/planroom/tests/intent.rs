//! The deterministic `intent` leaves and `ask stage` (architecture.md, "Planning: stages, ghosts
//! and lint (R5)"; DR-4, DR-23, DR-45, DR-66): versions round-trip through `intent list` and
//! `edit undo`; a proposal read from stdin writes ghosts and no file; a proposal file outside the
//! workspace is refused; `ask stage` returns the stage's checklist before any scan; and plan text
//! is masked on every output.

use std::collections::BTreeSet;
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

/// `planroom ARGS` (with `--json` when `json`) with `env` and `stdin`: the exit code, stdout and
/// stderr.
fn raw(root: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str, json: bool) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .args(args);
    if json {
        cmd.arg("--json");
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
    (out.status.code().unwrap(), text(out.stdout), text(out.stderr))
}

/// `planroom ARGS --json`: the exit code and the one envelope on stdout.
fn run(root: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    let (code, stdout, stderr) = raw(root, args, env, stdin, true);
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}\n{stderr}");
    (code, serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}")))
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

/// The argv of `command ARGS`, as one list.
fn argv<'a>(command: &'a str, args: &[&'a str]) -> Vec<&'a str> {
    command.split(' ').chain(args.iter().copied()).collect()
}

/// `planroom COMMAND ARGS --json` as the person, signed; `base` is the `--base` the args carry.
fn person(root: &Path, command: &str, args: &[&str], base: &str, stdin: &str) -> (i32, Value) {
    let signed = token(command, args, base);
    run(root, &argv(command, args), &[("PLANROOM_PERSON_TOKEN", &signed)], stdin)
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

/// The words of a POSIX shell line whose words are bare or single-quoted, as `text::shell_word`
/// quotes them.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut word, mut quoted, mut any) = (String::new(), false, false);
    for c in line.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
                any = false;
            }
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        out.push(word);
    }
    out
}

#[test]
fn intent_list_and_version_round_trip() {
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &["Add retries to the worker"], "", "");
    assert_eq!(code, 0, "{env}");
    let project = env["result"]["targets"][0].as_str().unwrap().to_owned();
    assert_eq!(project, "plan:add-retries-to-the-worker:project");

    let (code, version) = person(&root, "intent version", &["v1"], "", "");
    assert_eq!((code, version["result"]["status"].as_str()), (0, Some("applied")), "{version}");
    let at = version["meta"]["seq"].as_u64().unwrap();
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    assert_eq!(list["result"]["projects"][0]["versions"], json!([{"label": "v1", "seq": at}]), "{list}");

    // A label already live in the project is refused, and an agent never tags a version.
    // (Named by its project this time: a token signs the argv, and one argv signs once.)
    let (code, env) = person(&root, "intent version", &["--project", &project, "v1"], "", "");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["field"], "label");
    let (code, env) = run(&root, &["intent", "version", "v2"], &[("PLANROOM_ACTOR", AGENT)], "");
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");

    // `edit undo` of the version removes it.
    let event = version["result"]["event"].as_str().unwrap();
    let base = seq(&root);
    let (code, env) = person(&root, "edit", &["--base", &base, "undo", event], &base, "");
    assert_eq!(code, 0, "{env}");
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    assert_eq!(list["result"]["projects"][0]["versions"], json!([]), "{list}");

    // A spent token is refused with the exact line a person runs, which parses back as the leaf.
    let text = "Second plan, with care";
    let signed = token("intent new", &[text], "");
    let new = argv("intent new", &[text]);
    let (code, env) = run(&root, &new, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!(code, 0, "{env}");
    let (code, env) = run(&root, &new, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!((code, env["error"]["details"]["reason"].as_str()), (5, Some("spent")), "{env}");
    let hint = env["error"]["hint"].as_str().unwrap();
    let line = format!("planroom intent new {}", "'Second plan, with care'");
    assert!(hint.contains(&format!("{line}, from your own terminal")), "{hint}");
    let parsed = words(&line);
    let leaf = planroom::cli::parse_leaf("intent new", &parsed[3..]).unwrap();
    assert!(matches!(leaf, planroom_contract::Command::IntentNew(ref r) if r.text == text), "{leaf:?}");
}

/// Every file under `dir`, but the store's own and the data root's.
fn files(dir: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(at) = dirs.pop() {
        for entry in std::fs::read_dir(&at).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".data" || name.starts_with("store.db") {
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.insert(path);
            }
        }
    }
    out
}

#[test]
fn propose_from_stdin_writes_ghosts_and_no_file() {
    let (tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &["Add retries to the worker"], "", "");
    assert_eq!(code, 0, "{env}");
    let before = files(&root);
    let base = seq(&root);
    let agent = [("PLANROOM_ACTOR", AGENT)];
    let propose = ["intent", "propose", "--file", "-", "--base", base.as_str()];
    let first = json!({"items": [{"key": "retry", "type": "next_action", "name": "Retry a failed pop",
        "touches": ["worker/consumer.py"]}]});
    let second = json!({"items": [{"key": "alert", "type": "next_action", "name": "Alert on retries",
        "touches": ["platform:file:prometheus/alerts.yml"]}]});
    for proposal in [&first, &second] {
        let (code, env) = run(&root, &propose, &agent, &proposal.to_string());
        assert_eq!((code, env["result"]["status"].as_str()), (0, Some("ghost")), "{env}");
        assert_ne!(env["meta"]["replayed"], true, "two proposals at one base never share a key: {env}");
    }
    let (_, list) = run(&root, &["intent", "list"], &[], "");
    let ids: Vec<&str> =
        list["result"]["items"].as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["plan:add-retries-to-the-worker:alert", "plan:add-retries-to-the-worker:retry"]);
    assert_eq!(files(&root), before, "a proposal on stdin writes no file");

    // A missing touch target writes nothing.
    let head = seq(&root);
    let missing = json!({"items": [{"key": "ghostly", "type": "next_action", "name": "Touch nothing",
        "touches": ["worker/no_such_file.py"]}]});
    let propose = ["intent", "propose", "--file", "-", "--base", head.as_str()];
    let (code, env) = run(&root, &propose, &agent, &missing.to_string());
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("not_found")), "{env}");
    assert_eq!(seq(&root), head);

    // A stage that writes no items, named in a proposal, says so instead of an empty list.
    let staged = json!({"stage": "lint", "items": [{"key": "y", "type": "work", "name": "Y"}]});
    let (code, env) = run(&root, &propose, &agent, &staged.to_string());
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["field"], "items[0].type");
    let message = env["error"]["message"].as_str().unwrap();
    assert!(message.contains("writes no items") && !message.contains("writes ,"), "{message}");

    // A proposal file outside the workspace root is refused before it is read.
    std::fs::write(tmp.path().join("outside.json"), first.to_string()).unwrap();
    let propose = ["intent", "propose", "--file", "../outside.json", "--base", head.as_str()];
    let (code, env) = run(&root, &propose, &agent, "");
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("path_refused")), "{env}");
    assert_eq!(env["error"]["details"]["reason"], "outside_workspace");
    // Its line names the rule a proposal file keeps, the workspace root, not the repos.
    let message = env["error"]["message"].as_str().unwrap();
    assert!(message.contains("root") && !message.contains("repos"), "{message}");
    // So is one in the state directory.
    std::fs::write(root.join(".planroom/plan.json"), first.to_string()).unwrap();
    let propose = ["intent", "propose", "--file", ".planroom/plan.json", "--base", head.as_str()];
    let (code, env) = run(&root, &propose, &agent, "");
    assert_eq!(env["error"]["details"]["reason"], "outside_workspace", "{code} {env}");
}

#[test]
fn ask_stage_returns_the_method_checklist() {
    // Before any scan, in a directory with no workspace: the checklist and the item types.
    let empty = tempfile::tempdir().unwrap();
    let (code, env) = run(empty.path(), &["ask", "stage", "next_actions"], &[], "");
    assert_eq!(code, 0, "{env}");
    let ids = |env: &Value| -> Vec<String> {
        env["result"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let found = ids(&env);
    assert!(found.iter().any(|i| i == "next_actions:1"), "{env}");
    assert!(found.iter().filter(|i| i.starts_with("next_actions:")).count() >= 3, "{env}");
    let types = env["result"]["items"].as_array().unwrap().iter().find(|i| i["id"] == "next_action").unwrap();
    assert!(types["why"].as_str().unwrap().contains("done_test"), "{types}");
    assert_eq!(env["result"]["empty"]["reason"], "no_intent");
    assert_eq!(env["result"]["empty"]["next"]["args"], json!(["--command", "intent new"]));

    // After a proposal: the project's item of that stage, with the review that accepts it.
    let (_tmp, root) = workspace();
    let (code, env) = person(&root, "intent new", &["Add retries to the worker"], "", "");
    assert_eq!(code, 0, "{env}");
    let base = seq(&root);
    let proposal = json!({"items": [{"key": "retry", "type": "next_action", "name": "Retry a failed pop",
        "touches": ["worker/consumer.py"]}]});
    let propose = ["intent", "propose", "--file", "-", "--base", base.as_str()];
    let (code, env) = run(&root, &propose, &[("PLANROOM_ACTOR", AGENT)], &proposal.to_string());
    assert_eq!(code, 0, "{env}");
    let (code, env) = run(&root, &["ask", "stage", "next_actions"], &[], "");
    assert_eq!(code, 0, "{env}");
    let found = ids(&env);
    assert!(found.contains(&"plan:add-retries-to-the-worker:retry".to_owned()), "{env}");
    assert!(found.contains(&"worker:file:worker/consumer.py".to_owned()), "{env}");
    assert!(env["result"]["empty"].is_null(), "{env}");
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 1_500, "{env}");

    // An unknown stage names the stages.
    let (code, env) = run(&root, &["ask", "stage", "nope"], &[], "");
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("not_found")), "{env}");
    assert!(env["error"]["details"]["closest"].as_array().unwrap().contains(&json!("premortem")));
}

#[test]
fn plan_text_is_masked_on_every_intent_output() {
    let (_tmp, root) = workspace();
    let secret = "Rotate db_PASSWORD=hunter2 for orders";
    let check = |label: &str, out: &str| assert!(!out.contains("hunter2"), "{label}: {out}");

    let (code, new) = person(&root, "intent new", &[secret], "", "");
    assert_eq!(code, 0, "{new}");
    check("intent new --json", &new.to_string());
    let project = new["result"]["targets"][0].as_str().unwrap().to_owned();
    let slug = project.split(':').nth(1).unwrap();
    assert!(slug.starts_with("p-") && slug.len() == 10, "{project}");

    let item = |key: &str| {
        json!({"items": [{"key": key, "type": "next_action", "name": format!("Use db_PASSWORD=hunter2 in {key}"),
            "touches": ["worker/consumer.py"]}]})
        .to_string()
    };
    for (key, json) in [("one", true), ("two", false)] {
        let base = seq(&root);
        let args = ["--file", "-", "--base", base.as_str()];
        let signed = token("intent propose", &args, &base);
        let argv = argv("intent propose", &args);
        let (code, out, err) = raw(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], &item(key), json);
        assert_eq!(code, 0, "{out}{err}");
        check("intent propose", &format!("{out}{err}"));
    }
    let text_new = ["Rotate db_PASSWORD=hunter2 again"];
    let signed = token("intent new", &text_new, "");
    let (code, out, err) =
        raw(&root, &argv("intent new", &text_new), &[("PLANROOM_PERSON_TOKEN", &signed)], "", false);
    assert_eq!(code, 0, "{out}{err}");
    check("intent new", &format!("{out}{err}"));
    for args in [
        vec!["intent", "list", "--project", project.as_str()],
        vec!["intent", "lint", "--project", project.as_str()],
        vec!["ask", "stage", "next_actions", project.as_str()],
    ] {
        for json in [true, false] {
            let (code, out, err) = raw(&root, &args, &[], "", json);
            assert_eq!(code, 0, "{args:?}: {out}{err}");
            check(&format!("{args:?} json={json}"), &format!("{out}{err}"));
            assert!(out.contains(slug), "{args:?}: {out}");
        }
    }
    // The lint finding's name and the listed item's name are masked, not dropped.
    let (_, lint) = run(&root, &["intent", "lint", "--project", &project], &[], "");
    let flagged = lint["result"]["findings"].as_array().unwrap();
    assert!(flagged.iter().any(|f| f["rule"] == "next_action_done_test"), "{lint}");

    // A listing named by the project's text pages with a handle that names its ID instead.
    let many: Vec<Value> = (0..40)
        .map(|i| json!({"key": format!("g{i:02}"), "type": "next_action", "name": "n".repeat(200)}))
        .collect();
    let base = seq(&root);
    let propose = ["intent", "propose", "--file", "-", "--base", base.as_str()];
    let (code, env) = run(
        &root,
        &propose,
        &[("PLANROOM_ACTOR", AGENT)],
        &json!({"project": project, "items": many}).to_string(),
    );
    assert_eq!(code, 0, "{env}");
    for json in [true, false] {
        let (code, out, err) = raw(&root, &["intent", "list", "--project", secret], &[], "", json);
        assert_eq!(code, 0, "{out}{err}");
        check(&format!("intent list --project TEXT json={json}"), &format!("{out}{err}"));
    }
    let (_, page) = run(&root, &["intent", "list", "--project", secret], &[], "");
    let next = &page["meta"]["warnings"][0]["details"]["next"];
    assert_eq!((&next["args"][0], &next["args"][1]), (&json!("--project"), &json!(project)), "{page}");
}
