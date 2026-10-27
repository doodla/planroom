//! Output budgets: `map` honors the budget its catalogue entry declares, on a synthetic repo of
//! 20 directories of 100 `.ts` files each (unasked-8). Over the budget it keeps the blocks that
//! fit, sets `meta.truncated` with a `truncated` warning, and hands a drill-down that works today.
//! The canvas's layout input, `--detail graph` over `serve`'s real route, takes the whole graph.
//!
//! Every other reply an agent reads keeps to its leaf's budget too, warnings included (DR-85):
//! `scan` on a repo past the walk's 50,000-file bound, and every built leaf's `--json` reply on the
//! fixture, each `schema --command LEAF` among them (correctness-17, drift-r2-1).

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_contract::{Command as Leaf, MapReq};
use planroom_kernel::keys::person_token_message;
use planroom_testkit::{fixture, git_repo};
use serde_json::{Value, json};

fn planroom(cwd: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "one JSON line: {stdout}");
    let env: Value = serde_json::from_str(&stdout).unwrap();
    assert!(out.status.success(), "{args:?}: {env}");
    env
}

/// A workspace holding one repo of 20 directories × 100 `.ts` files, scanned.
fn large_workspace(root: &Path) {
    let files: Vec<(String, Vec<u8>)> = (1..=20)
        .flat_map(|d| {
            (1..=100).map(move |f| {
                let next = f % 100 + 1;
                let body = format!(
                    "import {{ f{next} }} from './file{next}';\nexport function f{f}() {{ return f{next}; }}\n"
                );
                (format!("src/mod{d}/file{f}.ts"), body.into_bytes())
            })
        })
        .collect();
    git_repo(&root.join("app"), &files);
    planroom(root, &["scan", "--json"]);
}

#[test]
fn map_keeps_to_its_budget_and_says_where_to_go_next() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let budget = u64::from(Leaf::Map(MapReq::default()).spec().budget_tokens);
    for detail in ["summary", "graph"] {
        let env = planroom(tmp.path(), &["map", "--level", "code", "--detail", detail, "--json"]);
        let meta = &env["meta"];
        assert!(meta["size"]["tokens_est"].as_u64().unwrap() <= budget, "{detail}: {meta}");
        assert_eq!(meta["truncated"], true, "{detail}");
        let warning = meta["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "truncated").unwrap();
        let kept = env["result"]["nodes"].as_array().unwrap().len() as u64;
        assert!(kept > 0, "{detail}: the first page is kept");
        assert_eq!(warning["details"]["kept"], kept);
        assert_eq!(warning["details"]["total"], 2_000);
        // Every edge kept joins two kept blocks.
        let ids: Vec<&Value> = env["result"]["nodes"].as_array().unwrap().iter().map(|n| &n["id"]).collect();
        for edge in env["result"]["edges"].as_array().unwrap() {
            assert!(ids.contains(&&edge["from"]) && ids.contains(&&edge["to"]), "{edge}");
        }
        // The drill-down is a handle that runs today.
        let next = &warning["details"]["next"];
        assert_eq!(next["command"], "map");
        let mut args: Vec<&str> =
            next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        args.insert(0, "map");
        args.push("--json");
        let coarser = planroom(tmp.path(), &args);
        assert!(coarser["meta"]["size"]["tokens_est"].as_u64().unwrap() <= budget);
        assert!(coarser["summary"].as_str().unwrap().starts_with("21 at the top"), "{coarser}");
    }
}

#[test]
fn a_map_within_its_budget_is_whole_and_untruncated() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let env = planroom(tmp.path(), &["map", "--json"]);
    assert!(env["meta"].get("truncated").is_none(), "{env}");
    assert!(env["meta"].get("warnings").is_none(), "{env}");
}

/// A foreground `serve` in `root`, killed when dropped, signed in with the code its login page holds.
struct Served {
    child: std::process::Child,
    port: u16,
    token: String,
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Served {
    fn start(root: &Path) -> Served {
        let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
            .args(["serve", "--json"])
            .current_dir(root)
            .env("PLANROOM_DATA", root.join(".data"))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let env: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        let port = env["result"]["port"].as_u64().unwrap() as u16;
        let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
        let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
        let mut served = Served { child, port, token: String::new() };
        let session = served.post("/api/v1/session", &json!({"code": code}).to_string());
        served.token = session["result"]["token"].as_str().unwrap().to_owned();
        served
    }

    /// One `POST` from the page's origin with the tab's token; the reply's envelope.
    fn post(&self, path: &str, body: &str) -> Value {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\n\
             Authorization: Bearer {token}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len(),
            port = self.port,
            token = self.token,
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let body = response.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or_default();
        serde_json::from_str(body).unwrap_or_else(|e| panic!("{e}: {response}"))
    }
}

#[test]
fn the_canvas_graph_over_serve_takes_every_block() {
    let tmp = tempfile::tempdir().unwrap();
    large_workspace(tmp.path());
    let served = Served::start(tmp.path());
    // What the canvas asks for its layout: the whole graph, over the real route.
    let env = served.post("/api/v1/map", r#"{"level": "code", "detail": "graph"}"#);
    assert_eq!(env["ok"], true, "{}", env["error"]);
    assert_eq!(env["result"]["nodes"].as_array().unwrap().len(), 2_000);
    assert!(env["meta"].get("warnings").is_none(), "{}", env["meta"]);
    assert!(env["meta"].get("truncated").is_none(), "{}", env["meta"]);
    // A summary over serve is still an agent's read, and keeps to the budget.
    let env = served.post("/api/v1/map", r#"{"level": "code"}"#);
    let budget = u64::from(Leaf::Map(MapReq::default()).spec().budget_tokens);
    assert!(env["result"]["nodes"].as_array().unwrap().len() < 2_000, "{}", env["meta"]);
    assert_eq!(env["meta"]["truncated"], true);
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= budget);
}

/// `scan`'s reply carries the top-level map, which an agent reads too: it keeps to `scan`'s own
/// budget. One compose file of 200 services puts 200 parentless containers at the top.
#[test]
fn scan_keeps_its_top_level_map_to_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let services: String =
        (1..=200).map(|n| format!("  service{n}:\n    image: example/service{n}:1\n")).collect();
    git_repo(
        &tmp.path().join("platform"),
        &[("docker-compose.yml".into(), format!("services:\n{services}").into_bytes())],
    );
    let env = planroom(tmp.path(), &["scan", "--json"]);
    let budget = u64::from(Leaf::Scan(planroom_contract::ScanReq::default()).spec().budget_tokens);
    let meta = &env["meta"];
    assert!(meta["size"]["tokens_est"].as_u64().unwrap() <= budget, "{meta}");
    assert_eq!(meta["truncated"], true);
    let warning = meta["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "truncated").unwrap();
    assert_eq!(warning["details"]["total"], 201, "the repo and its 200 containers");
    let kept = env["result"]["map"]["nodes"].as_array().unwrap().len() as u64;
    assert!(kept > 0 && kept < 201, "the first page is kept: {kept}");
    assert_eq!(warning["details"]["kept"], kept);
}

/// A leaf's catalogue budget, by its name.
fn budget(name: &str) -> u64 {
    let spec = Leaf::SPECS.iter().find(|s| s.name() == name).unwrap_or_else(|| panic!("no leaf {name}"));
    u64::from(spec.budget_tokens)
}

/// `planroom ARGS --json` in `cwd` with `stdin` and the environment `env`; the one envelope.
fn reply(cwd: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args).arg("--json").current_dir(cwd).env("PLANROOM_DATA", cwd.join(".data"));
    for name in ["PLANROOM_PERSON_TOKEN", "PLANROOM_ACTOR", "PLANROOM_CLIENT"] {
        cmd.env_remove(name);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    // The command may refuse before it reads its stdin; its reply is what the test checks.
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
    serde_json::from_str(&stdout).unwrap()
}

/// One repo of 520 directories of 100 committed one-line files, 52,000 in all, written with
/// `git fast-import` so the test needn't write them to disk: 2,000 files past the walk's bound.
fn repo_past_the_file_bound(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let git = |args: &[&str], stdin: &[u8]| {
        let mut child = Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    let mut stream = String::from("blob\nmark :1\ndata 2\nx\n\n");
    stream.push_str(
        "commit refs/heads/main\ncommitter Test <test@example.com> 1767225600 +0000\ndata 5\ninit\n",
    );
    for d in 0..520 {
        for f in 0..100 {
            stream.push_str(&format!("M 100644 :1 d{d:03}/f{f:03}.txt\n"));
        }
    }
    stream.push('\n');
    git(&["init", "-q", "-b", "main"], b"");
    git(&["fast-import", "--quiet"], stream.as_bytes());
}

#[test]
fn a_scan_past_the_file_bound_keeps_its_reply_to_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    repo_past_the_file_bound(&tmp.path().join("app"));
    let env = reply(tmp.path(), &["scan"], "", &[]);
    assert_eq!(env["ok"], true, "{env}");
    let meta = &env["meta"];
    assert!(meta["size"]["tokens_est"].as_u64().unwrap() <= budget("scan"), "{meta}");
    let warnings = meta["warnings"].as_array().unwrap();
    let partial = warnings.iter().find(|w| w["code"] == "partial_scan").unwrap();
    let details = &partial["details"];
    assert_eq!(details["repo"], "app");
    assert_eq!(details["count"], 2_000, "{details}");
    // The first 10 uncovered paths, in sorted order.
    let sample: Vec<String> = (0..10).map(|f| format!("d500/f{f:03}.txt")).collect();
    assert_eq!(details["uncovered"], json!(sample));
    assert!(partial["message"].as_str().unwrap().contains("2000 files"), "{partial}");
}

/// A test key pair from a fixed scalar, as `actor.rs` makes them.
fn key() -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    bytes[31] = 7;
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

/// A person token over `person init` with no arguments, made now.
fn person_init_token(key: &SigningKey) -> String {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key.sign(&person_token_message("person init", &[], "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// One run: its arguments, its stdin and its environment.
type Run = (Vec<String>, String, Vec<(String, String)>);

/// Every built leaf with a budget, run on the scanned fixture as an agent or a person would: each
/// `--json` reply keeps to its catalogue budget. Only the stream leaves, budget 0, are left out.
#[test]
fn every_built_leafs_reply_on_the_fixture_keeps_to_its_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let mut runs: Vec<Run> = Vec::new();
    let mut run = |args: &[&str], stdin: &str, env: &[(&str, &str)]| {
        let owned = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
        runs.push((args.iter().map(|a| (*a).to_owned()).collect(), stdin.to_owned(), owned(env)));
    };
    run(&["scan"], "", &[]);
    for level in ["intent", "system", "container", "component", "code"] {
        for detail in ["summary", "graph"] {
            run(&["map", "--level", level, "--detail", detail], "", &[]);
        }
    }
    run(&["map"], "", &[]);
    run(&["focus"], "", &[]);
    run(&["schema"], "", &[]);
    run(&["schema", "--commands"], "", &[]);
    let leaves: Vec<String> = Leaf::SPECS.iter().map(|s| s.name()).collect();
    for leaf in &leaves {
        run(&["schema", "--command", leaf], "", &[]);
    }
    run(&["version"], "", &[]);
    run(&["share", "--out", ".planroom/share"], "", &[]);
    let read = json!({"path": "inbox/u_test/events/d1", "version": 1, "row": {
        "format": "planroom.inbox/v1", "instance": "i1", "counter": 1, "command": "map",
        "request": {"level": "container"}, "base": 5, "at": "2026-10-26T00:00:00Z"}});
    run(&["import", "-"], &format!("{read}\n"), &[]);
    // drift-3-1: import's reply keeps to its budget however many rows a sync brings: 40 read
    // requests from two viewers, and 40 rows that are refused or left, each with its reason.
    let row = |uid: &str, doc: usize, command: &str| {
        json!({"path": format!("inbox/{uid}/events/doc{doc:03}"), "version": 1, "row": {
            "format": "planroom.inbox/v1", "instance": format!("i-{uid}"), "counter": doc,
            "command": command, "request": {"level": "container", "detail": "graph"}, "base": 5,
            "at": "2026-10-26T00:00:00Z"}})
        .to_string()
    };
    let reads: Vec<String> =
        (0..40).map(|i| row(if i % 2 == 0 { "u_one" } else { "u_two" }, i, "map")).collect();
    let head = json!({"path": "live/head", "version": 3}).to_string();
    run(&["import", "-"], &format!("{}\n{head}\n", reads.join("\n")), &[]);
    let others: Vec<String> = (0..40)
        .map(|i| match i % 4 {
            0 => row("u_one", 100 + i, "handoff merge"),
            1 => row("u_two", 100 + i, "edit"),
            2 => row("u_one", 100 + i, "find"),
            _ => format!(
                "{{\"path\": \"inbox/u_two/events/bad{i}\", \"row\": {{\"format\": \"planroom.inbox/v1\"}}}}"
            ),
        })
        .collect();
    run(&["import", "-"], &format!("{}\n", others.join("\n")), &[]);
    let key = key();
    let public = b64url(&key.verifying_key().to_sec1_point(false).to_bytes());
    run(&["person", "init"], &public, &[("PLANROOM_PERSON_TOKEN", &person_init_token(&key))]);

    let mut covered = BTreeSet::new();
    for (args, stdin, env) in &runs {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let reply = reply(&root, &args, stdin, &env);
        assert_eq!(reply["ok"], true, "{args:?}: {reply}");
        let name = reply["command"].as_str().unwrap().to_owned();
        let tokens = reply["meta"]["size"]["tokens_est"].as_u64().unwrap();
        assert!(tokens <= budget(&name), "{args:?}: {tokens} tokens over {name}'s {}", budget(&name));
        // Only a leaf's own cut marks a reply truncated; no reply here passes its budget.
        for warning in reply["meta"]["warnings"].as_array().into_iter().flatten() {
            assert!(warning["details"].get("left_out").is_none(), "{args:?}: {warning}");
        }
        covered.insert(name);
    }
    let budgeted: BTreeSet<String> =
        Leaf::SPECS.iter().filter(|s| s.budget_tokens > 0).map(|s| s.name()).collect();
    assert_eq!(covered, budgeted, "every built leaf with a budget is run");
}
