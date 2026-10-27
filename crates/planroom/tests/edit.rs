//! `edit` and its undo, from the CLI and over `serve` (architecture.md, Writes and Versions and
//! revisions; DR-5, DR-7, DR-60, DR-61, DR-73): every hand edit shows on the map and its undo
//! folds the map back; a move that would close a loop is refused; one agent is one writer
//! instance across processes; and a tab's write keeps the client ID it names. A person signs each
//! write with a test key registered through `person init`, as the mod does.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

const AGENT: &str = "agent:1a2b3c4d:chat";

fn planroom(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT");
    cmd
}

/// `planroom ARGS --json` with `env`; the exit code and the one envelope on stdout.
fn run(root: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    let mut cmd = planroom(root);
    cmd.args(args).arg("--json");
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

/// A test key pair from a fixed scalar.
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

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// A person's token over `command` with the argv after it, as the mod signs one.
fn token(command: &str, args: &[&str], base: &str) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = now();
    let sig: Signature = key().sign(&person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// A scanned copy of the fixture with the test person's key registered.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let (code, env) = run(&root, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let (code, env) = run(
        &root,
        &["person", "init"],
        &[("PLANROOM_PERSON_TOKEN", &token("person init", &[], ""))],
        &public,
    );
    assert_eq!(code, 0, "{env}");
    (tmp, root)
}

/// `planroom edit --base BASE ARGS` as the person, signed.
fn edit(root: &Path, base: u64, op: &[&str]) -> (i32, Value) {
    let base = base.to_string();
    let args: Vec<&str> = ["--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
    let signed = token("edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args.iter().copied()).collect();
    run(root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "")
}

/// `planroom edit --base BASE ARGS` as the test agent, from a process of its own.
fn agent_edit(root: &Path, base: u64, op: &[&str]) -> (i32, Value) {
    let base = base.to_string();
    let argv: Vec<&str> = ["edit", "--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
    run(root, &argv, &[("PLANROOM_ACTOR", AGENT)], "")
}

/// The store's latest `seq`, as `map` reports it.
fn seq(root: &Path) -> u64 {
    let (_, env) = run(root, &["map"], &[], "");
    env["meta"]["seq"].as_u64().unwrap()
}

/// Everything the edits below change, as `map` shows it: the canvas's graph of the containers
/// with every relation, and the blocks under the storefront repo and its `src`. The `seq`s are
/// left out, since every write moves them.
fn views(root: &Path) -> Vec<Value> {
    let maps: [&[&str]; 3] = [
        &["map", "--detail", "graph", "--level", "container", "--lens", "all"],
        &[
            "map",
            "--detail",
            "graph",
            "--level",
            "component",
            "--focus",
            "storefront:repo:storefront",
            "--lens",
            "all",
        ],
        &[
            "map",
            "--detail",
            "graph",
            "--level",
            "component",
            "--focus",
            "storefront:dir:src",
            "--lens",
            "all",
        ],
    ];
    maps.iter()
        .map(|args| {
            let (code, env) = run(root, args, &[], "");
            assert_eq!(code, 0, "{env}");
            json!({"result": env["result"], "summary": env["summary"]})
        })
        .collect()
}

fn ok(code: i32, env: &Value) {
    assert_eq!((code, &env["ok"]), (0, &json!(true)), "{env}");
}

#[test]
fn every_edit_type_undoes_from_the_cli() {
    let (_tmp, root) = workspace();
    // Unrelate takes back a relation that exists.
    let (code, env) = edit(
        &root,
        seq(&root),
        &["relate", "storefront:container:storefront", "orders:container:orders", "--kind", "http"],
    );
    ok(code, &env);
    let cases: [(&str, &[&str]); 8] = [
        ("move", &["move", "storefront:dir:src/cart", "storefront:dir:test"]),
        ("group", &["group", "purchase", "storefront:dir:src/cart", "storefront:dir:src/checkout"]),
        ("rename", &["rename", "storefront:dir:src/cart", "basket"]),
        ("hide", &["hide", "storefront:dir:src/cart"]),
        (
            "pin",
            &["pin", "orders:container:orders", "--view", "map:container:all", "--x", "12.5", "--y", "-40"],
        ),
        ("relate", &["relate", "worker:container:worker", "orders:container:orders", "--kind", "http"]),
        (
            "unrelate",
            &["unrelate", "storefront:container:storefront", "orders:container:orders", "--kind", "http"],
        ),
        ("forbid", &["forbid", "worker:container:worker", "ws:container:orders-db", "--kind", "db"]),
    ];
    for (name, op) in cases {
        let before = views(&root);
        let (code, env) = edit(&root, seq(&root), op);
        ok(code, &env);
        assert_eq!(env["result"]["status"], "applied", "{name}: {env}");
        let applied = views(&root);
        assert_ne!(applied, before, "{name}: the edit shows on the map");

        // The undo handle the reply carries is the exact command, base included.
        let undo = &env["result"]["undo"];
        assert_eq!(undo["command"], "edit", "{name}");
        let args: Vec<&str> = undo["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
        assert_eq!((args[0], args[2]), ("--base", "undo"), "{name}: {undo}");
        let (code, undone) = edit(&root, args[1].parse().unwrap(), &args[2..]);
        ok(code, &undone);
        let events = env["result"]["count"].as_u64().unwrap();
        let said = if events == 1 { "Undid the edit ".to_owned() } else { format!(": {events} events.") };
        assert!(
            undone["summary"].as_str().unwrap().contains(&said),
            "{name}: the undo reverses every event of the command: {undone}"
        );
        assert!(!undone["result"]["targets"].as_array().unwrap().is_empty(), "{name}: {undone}");
        assert_eq!(views(&root), before, "{name}: the undo folds the map back to the view before the edit");
    }
}

#[test]
fn an_edit_shows_on_the_map() {
    let (_tmp, root) = workspace();
    let base = seq(&root);
    let (code, env) = edit(&root, base, &["rename", "storefront:container:storefront", "shopfront"]);
    ok(code, &env);
    let written = env["meta"]["seq"].as_u64().unwrap();
    assert!(written > base, "{env}");
    assert_eq!(env["result"]["targets"], json!(["storefront:container:storefront"]));
    assert!(env["summary"].as_str().unwrap().contains("shopfront"), "{env}");

    let (code, map) = run(&root, &["map", "--level", "container"], &[], "");
    ok(code, &map);
    assert!(map["meta"]["seq"].as_u64().unwrap() >= written, "a read after the write reflects it: {map}");
    let block = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "storefront:container:storefront")
        .cloned()
        .unwrap();
    assert_eq!(block["name"], "shopfront", "{map}");

    // A hide shows as a count, never as a silent loss.
    let (code, env) = edit(&root, written, &["hide", "ws:container:redis"]);
    ok(code, &env);
    let (_, map) = run(&root, &["map", "--level", "container"], &[], "");
    assert_eq!(map["result"]["hidden"], 1, "{map}");
    assert!(map["result"]["nodes"].as_array().unwrap().iter().all(|n| n["id"] != "ws:container:redis"));

    // Every reply keeps to the leaf's budget, however long a name or however many blocks it names.
    let budget = 400;
    let long = "x".repeat(5_000);
    let (code, env) = edit(&root, seq(&root), &["rename", "orders:container:orders", &long]);
    ok(code, &env);
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= budget, "{env}");
    let (_, map) = run(&root, &["map", "--level", "code", "--focus", "storefront:repo:storefront"], &[], "");
    let mut members: Vec<String> = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .filter(|id| id.starts_with("storefront:file:"))
        .collect();
    members.truncate(30);
    assert!(members.len() >= 10, "{map}");
    let op: Vec<&str> =
        ["group", "everything"].into_iter().chain(members.iter().map(String::as_str)).collect();
    let (code, env) = edit(&root, seq(&root), &op);
    ok(code, &env);
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= budget, "{env}");
    let listed = env["result"]["targets"].as_array().unwrap().len() as u64;
    assert_eq!(listed + env["result"]["more_targets"].as_u64().unwrap(), members.len() as u64 + 1, "{env}");
    assert_eq!(env["result"]["count"], members.len() + 1, "the group's node and one move per member");
}

#[test]
fn a_cycle_making_move_is_refused() {
    let (_tmp, root) = workspace();
    let base = seq(&root);
    let before = views(&root);
    let (code, env) = edit(&root, base, &["move", "storefront:dir:src", "storefront:dir:src/cart"]);
    assert_eq!(code, 2, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    assert_eq!(env["error"]["details"]["field"], "parent");
    assert_eq!(
        env["error"]["details"]["cycle"],
        json!(["storefront:dir:src", "storefront:dir:src/cart", "storefront:dir:src"]),
        "the refusal names the loop"
    );
    assert_eq!(seq(&root), base, "nothing was written");
    assert_eq!(views(&root), before);

    // A group that would hold its own parent is refused the same way, in its one transaction.
    let (code, env) =
        edit(&root, base, &["group", "loop", "storefront:dir:src", "storefront:repo:storefront"]);
    assert_eq!((code, &env["error"]["code"]), (2, &json!("invalid_argument")), "{env}");
    assert!(env["error"]["details"]["cycle"].as_array().is_some_and(|c| c.len() >= 3), "{env}");
    assert_eq!(seq(&root), base);
}

#[test]
fn an_agent_does_not_conflict_with_itself() {
    let (_tmp, root) = workspace();
    let base = seq(&root);
    // Two writes by one agent from two processes, the second from the base before the first.
    let (code, first) = agent_edit(&root, base, &["rename", "storefront:container:storefront", "shop"]);
    ok(code, &first);
    let (code, second) = agent_edit(&root, base, &["rename", "storefront:container:storefront", "store"]);
    ok(code, &second);
    for env in [&first, &second] {
        assert_eq!(env["result"]["status"], "ghost", "an agent's edit waits for a person: {env}");
    }
    // Both carry the agent's own ID as their client: one writer instance, whatever process wrote.
    let store = Store::open_existing(&planroom::app::workspace::store_path(&root)).unwrap().unwrap();
    let events = store.events_after(base, 10).unwrap();
    assert_eq!(events.len(), 2);
    for event in &events {
        assert_eq!(serde_json::to_value(&event.client).unwrap(), json!({"surface": "cli", "id": AGENT}));
    }

    // A person's two writers, by contrast, are two instances: a write from the older base over
    // the person's own earlier one from another terminal process is a conflict, stored as one.
    let (code, env) = edit(&root, base, &["rename", "storefront:dir:src/cart", "basket"]);
    ok(code, &env);
    let (code, env) = edit(&root, base, &["rename", "storefront:dir:src/cart", "trolley"]);
    assert_eq!((code, &env["error"]["code"]), (4, &json!("stale_write")), "{env}");
    assert_eq!(env["error"]["details"]["base"], base);
}

/// A foreground `serve` on the workspace, killed when dropped.
struct Serve {
    child: Child,
    port: u16,
    root: PathBuf,
    code: String,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn serve(root: &Path) -> Serve {
    let mut child = planroom(root).args(["serve", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(env["ok"], true, "{env}");
    let port = env["result"]["port"].as_u64().unwrap() as u16;
    let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
    let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
    Serve { child, port, root: root.to_path_buf(), code }
}

/// One request from the page's origin with the tab's token; the status and the envelope.
fn post(s: &Serve, token: &str, path: &str, body: &Value) -> (u16, Value) {
    let body = body.to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\n\
         Authorization: Bearer {token}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len(),
        port = s.port,
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
    (status, serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}")))
}

fn sign_in(s: &Serve, code: &str) -> String {
    let (status, env) = post(s, "", "/api/v1/session", &json!({"code": code}));
    assert_eq!(status, 200, "{env}");
    env["result"]["token"].as_str().unwrap().to_owned()
}

/// A second tab's code, registered with the control token as `planroom open` does.
fn second_code(s: &Serve) -> String {
    let record = s.root.join(".data/serve").join(format!("{}.json", s.port));
    let record: Value = serde_json::from_str(&std::fs::read_to_string(record).unwrap()).unwrap();
    let control = record["control_token"].as_str().unwrap();
    let code = "00112233445566778899aabbccddeeff";
    let body = json!({"code_sha256": planroom::server::auth::code_digest(code)});
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let body = body.to_string();
    let request = format!(
        "POST /api/v1/code HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {control}\r\n\
         Connection: close\r\nContent-Length: {}\r\n\r\n{body}",
        s.port,
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 204"), "{response}");
    code.to_owned()
}

#[test]
fn an_http_write_keeps_its_client_id() {
    let (_tmp, root) = workspace();
    let s = serve(&root);
    let tab_a = sign_in(&s, &s.code.clone());
    let tab_b = sign_in(&s, &second_code(&s));
    let (_, map) = post(&s, &tab_a, "/api/v1/map", &json!({"level": "container"}));
    let base = map["meta"]["seq"].as_u64().unwrap();
    let rename = |name: &str, client: &str| json!({"op": "rename", "id": "storefront:container:storefront", "name": name, "base": base, "client": client});

    let (status, env) = post(&s, &tab_a, "/api/v1/edit", &rename("shop", "tab_A"));
    assert_eq!(status, 200, "{env}");
    assert_eq!(env["result"]["status"], "applied", "a signed-in tab writes as the person: {env}");

    // Tab B flushes an edit tab A made while offline: it sends tab A's ID, so the edit is tab A's
    // own later write over its own, never a conflict with it (DR-73).
    let (status, env) = post(&s, &tab_b, "/api/v1/edit", &rename("store", "tab_A"));
    assert_eq!(status, 200, "{env}");
    // Tab B's own edit from the same old base is another writer instance's: a conflict.
    let (status, env) = post(&s, &tab_b, "/api/v1/edit", &rename("market", "tab_B"));
    assert_eq!((status, &env["error"]["code"]), (409, &json!("stale_write")), "{env}");

    let store = Store::open_existing(&planroom::app::workspace::store_path(&root)).unwrap().unwrap();
    let clients: Vec<Value> = store
        .events_after(base, 10)
        .unwrap()
        .iter()
        .map(|e| serde_json::to_value(&e.client).unwrap())
        .collect();
    assert_eq!(
        clients,
        vec![
            json!({"surface": "canvas", "id": "tab_A"}),
            json!({"surface": "canvas", "id": "tab_A"}),
            json!({"surface": "canvas", "id": "tab_B"}),
        ],
        "each event keeps the client its request named"
    );
    let (_, map) = post(&s, &tab_a, "/api/v1/map", &json!({"level": "container"}));
    let name = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "storefront:container:storefront")
        .map(|n| n["name"].clone());
    assert_eq!(name, Some(json!("store")), "the conflict merged nothing: {map}");
}

/// A base past the store's head names a version no writer read, so the edit is refused rather
/// than taken as covering every write before it: a person's rename at that base can't overwrite
/// another writer instance's newer one (DR-73).
#[test]
fn a_base_past_the_head_is_refused() {
    let (_tmp, root) = workspace();
    let head = seq(&root);
    let (code, env) = edit(&root, head, &["rename", "storefront:container:storefront", "shop"]);
    ok(code, &env);
    let (code, env) = edit(&root, 999_999, &["rename", "storefront:container:storefront", "store"]);
    assert_ne!(code, 0, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    assert_eq!(env["error"]["details"]["field"], "base", "{env}");
    let (_, map) = run(&root, &["map", "--level", "container"], &[], "");
    let name = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "storefront:container:storefront")
        .map(|n| n["name"].clone());
    assert_eq!(name, Some(json!("shop")), "the refused edit wrote nothing: {map}");
}

/// Over HTTP each tab is its own writer instance, so an edit names its tab's client: one that
/// names none is refused rather than written as a shared default, which would make every tab of
/// a person one writer that never conflicts with another (DR-5).
#[test]
fn an_http_write_without_its_client_is_refused() {
    let (_tmp, root) = workspace();
    let s = serve(&root);
    let tab = sign_in(&s, &s.code.clone());
    let (_, map) = post(&s, &tab, "/api/v1/map", &json!({"level": "container"}));
    let base = map["meta"]["seq"].as_u64().unwrap();
    let body = json!({"op": "rename", "id": "storefront:container:storefront", "name": "shop", "base": base});
    let (status, env) = post(&s, &tab, "/api/v1/edit", &body);
    assert_eq!((status, &env["error"]["code"]), (400, &json!("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["field"], "client", "{env}");
}

/// `edit` from the CLI resolves its actor by the actor rules: a write with no name, no token and
/// no terminal is `actor_required`, and an agent stays an agent even with a valid person token
/// beside its name, so its edit waits as a ghost rather than landing as a person's (DR-4, DR-64).
#[test]
fn an_edit_from_the_cli_resolves_its_actor_by_the_actor_rules() {
    let (_tmp, root) = workspace();
    let base = seq(&root).to_string();
    let op = ["rename", "storefront:container:storefront", "shop"];
    let argv: Vec<&str> = ["edit", "--base", base.as_str()].into_iter().chain(op).collect();
    let (code, env) = run(&root, &argv, &[], "");
    assert_eq!((code, &env["error"]["code"]), (2, &json!("actor_required")), "{env}");

    let args: Vec<&str> = ["--base", base.as_str()].into_iter().chain(op).collect();
    let signed = token("edit", &args, &base);
    let (code, env) = run(&root, &argv, &[("PLANROOM_ACTOR", AGENT), ("PLANROOM_PERSON_TOKEN", &signed)], "");
    ok(code, &env);
    assert_eq!(env["result"]["status"], "ghost", "the agent's token makes it no person: {env}");
}

/// A person claim the call can't back is refused, and the refusal names this very edit, quoted for
/// a shell, as the line a person runs from a terminal: split as a shell splits it, that line parses
/// back to the same edit.
#[test]
fn a_refused_edit_names_its_own_command_line() {
    let (_tmp, root) = workspace();
    let base = seq(&root).to_string();
    let ops: [&[&str]; 4] = [
        &["rename", "storefront:container:storefront", "Store's front & back"],
        &["rename", "--", "storefront:container:storefront", "-store"],
        &["pin", "storefront:dir:src", "--view", "map:system:all", "--x", "-12.5", "--y", "3"],
        &["forbid", "storefront:container:storefront", "orders:container:orders"],
    ];
    for op in ops {
        let args: Vec<&str> =
            ["edit", "--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
        // `--json` first, since a `--` makes every later word an operand.
        let out =
            planroom(&root).arg("--json").args(&args).env("PLANROOM_ACTOR", "person:me").output().unwrap();
        let env: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_ne!(out.status.code(), Some(0), "{env}");
        assert_eq!(env["error"]["code"], "person_required", "{env}");
        let hint = env["error"]["hint"].as_str().unwrap();
        let line = hint
            .split_once(" or ")
            .and_then(|(_, rest)| rest.split_once(", from your own terminal"))
            .map(|(line, _)| line)
            .unwrap_or_else(|| panic!("no terminal line: {hint}"));
        let words = shell_split(line);
        assert_eq!(words[..2], ["planroom", "edit"], "{line}");
        let named = planroom::cli::parse_leaf("edit", &words[2..]).unwrap_or_else(|e| panic!("{line}: {e}"));
        let ran: Vec<String> = args[1..].iter().map(|a| (*a).to_owned()).collect();
        assert_eq!(named, planroom::cli::parse_leaf("edit", &ran).unwrap(), "{line}");
    }
}

/// `line` split into words as a POSIX shell splits the plain, single-quoted and backslash-escaped
/// words a hint writes.
fn shell_split(line: &str) -> Vec<String> {
    let (mut words, mut word, mut any, mut quoted) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => quoted = !quoted,
            '\\' if !quoted => word.extend(chars.next()),
            ' ' if !quoted => {
                if any {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
                continue;
            }
            c => word.push(c),
        }
        any = true;
    }
    if any {
        words.push(word);
    }
    words
}
