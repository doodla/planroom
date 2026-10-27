//! S11 remote, the walking skeleton's slice (WS7): `share` writes the Artifact bundle and its
//! snapshot, and `import -` takes inbox rows on stdin, answers read requests and refuses what a
//! row may not name, returning the `db` batches the session writes. Edits (M1), decisions (M2),
//! pairing and the full snapshot (M4) add their tests here as they land.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_contract::schema::{self, View};
use planroom_contract::{Actor, Cause, Client, Event, EventBody, Status, Surface};
use planroom_kernel::keys::person_token_message;
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

fn planroom(root: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args).arg("--json").current_dir(root);
    cmd.env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT");
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
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

fn scanned() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let (code, env) = planroom(&root, &["scan"], "", &[]);
    assert_eq!(code, 0, "{env}");
    (tmp, root)
}

fn share(root: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["share", "--out", ".planroom/share"];
    args.extend(extra);
    let (code, env) = planroom(root, &args, "", &[]);
    assert_eq!(code, 0, "{env}");
    env["result"].clone()
}

/// The local file a share's publish names for the published `path`: its `files` source, under
/// its `root`.
fn published(result: &Value, path: &str) -> String {
    let publish = &result["publish"];
    let source = publish["files"][path].as_str().unwrap_or_else(|| panic!("{path} isn't published"));
    Path::new(publish["root"].as_str().unwrap()).join(source).display().to_string()
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Validate `value` against the output view's `$defs/{def}`.
fn validate(def: &str, value: &Value) {
    let mut root = schema::generate(View::Output);
    root["$ref"] = json!(format!("#/$defs/{def}"));
    let validator = jsonschema::draft202012::new(&root).unwrap();
    let errors: Vec<String> =
        validator.iter_errors(value).map(|e| format!("{e} at {}", e.instance_path())).collect();
    assert!(errors.is_empty(), "{def}: {errors:#?}");
}

/// One `import -` stdin line for a row at `inbox/<uid>/events/<doc>`.
fn line(uid: &str, doc: &str, version: u64, row: Value) -> String {
    json!({"path": format!("inbox/{uid}/events/{doc}"), "row": row, "version": version}).to_string()
}

fn row(instance: &str, counter: u64, command: &str, request: Value) -> Value {
    json!({"format": "planroom.inbox/v1", "instance": instance, "counter": counter, "command": command,
        "request": request, "base": 5, "at": "2026-10-26T00:00:00Z"})
}

fn import(root: &Path, lines: &[String], env: &[(&str, &str)]) -> (i32, Value) {
    planroom(root, &["import", "-"], &(lines.join("\n") + "\n"), env)
}

/// Every write of every batch file, in order; each file is one validated `DbBatch`.
fn writes(result: &Value) -> Vec<Value> {
    let root = Path::new(result["root"].as_str().unwrap());
    assert!(root.is_absolute(), "{result}");
    result["batch_files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| {
            let batch = read_json(root.join(f.as_str().unwrap()).to_str().unwrap());
            validate("DbBatch", &batch);
            batch["writes"].as_array().unwrap().clone()
        })
        .collect()
}

fn keys_named(value: &Value, name: &str, found: &mut usize) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == name {
                    *found += 1;
                }
                keys_named(v, name, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| keys_named(v, name, found)),
        _ => {}
    }
}

#[test]
fn snapshot_validates_and_is_under_1_mb() {
    let (_tmp, root) = scanned();
    let result = share(&root, &[]);
    validate("ShareResult", &result);
    let publish = &result["publish"];
    let snapshot_path = published(&result, "data/snapshot.json");
    let bytes = std::fs::metadata(&snapshot_path).unwrap().len();
    assert!(bytes < 1_000_000, "{bytes} bytes");
    let snapshot = read_json(&snapshot_path);
    validate("Snapshot", &snapshot);
    assert_eq!(snapshot["format"], "planroom.snapshot/v1");
    assert_eq!(snapshot["maps"][0]["level"], "system");
    assert_eq!(snapshot["maps"][0]["nodes"].as_array().unwrap().len(), 10);
    assert_eq!(snapshot["capabilities"], json!(["map:system:runtime", "pane"]));
    assert_eq!(snapshot["unavailable"]["next"]["label"], "Not in this snapshot. Ask your computer for it");

    // The publish's exact arguments: the fragment, the stable-named assets and the data file,
    // db with the three documented rules and user, and the map icon.
    let page = std::fs::read_to_string(publish["file_path"].as_str().unwrap()).unwrap();
    assert!(page.starts_with("<title>Planroom Canvas</title>"), "{page}");
    assert!(page.contains(r#"<div id="root" data-transport="artifact"></div>"#));
    assert!(page.contains(r#"src="./assets/index.js""#));
    assert!(!page.contains("<!doctype") && !page.contains("<body"));
    let files = publish["files"].as_object().unwrap();
    assert!(files.contains_key("assets/index.js") && files.contains_key("assets/elk-worker.min.js"));
    // Each source is relative to the one absolute `root`, so the reply keeps to share's budget.
    let root_dir = Path::new(publish["root"].as_str().unwrap());
    assert!(root_dir.is_absolute(), "{publish}");
    for (path, source) in files {
        let source = source.as_str().unwrap();
        assert!(!path.starts_with('/') && !source.starts_with('/'), "{path}");
        assert!(root_dir.join(source).is_file(), "{path}");
    }
    // The result lists only the files made from this workspace, with their sizes.
    let made: Vec<&str> =
        result["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(made, ["page.html", "data/snapshot.json", "db/live-head.json"]);
    assert_eq!(
        publish["capabilities"],
        json!({"db": {"rules": [
            {"path": "", "read": "view", "write": "owner"},
            {"path": "inbox", "read": "owner", "write": "owner"},
            {"path": "inbox/{self}", "read": "interact", "write": "interact"},
        ]}, "user": {"scopes": ["profile"]}})
    );
    assert_eq!(publish["icon"], "map");

    // The first live/head, written by the session after the publish.
    let head_write = &result["db"][0]["writes"][0];
    assert_eq!(
        (&head_write["op"], &head_write["collection"], &head_write["doc_id"]),
        (&json!("set"), &json!("live"), &json!("head"))
    );
    let head = read_json(head_write["file_path"].as_str().unwrap());
    validate("LiveHead", &head);
    assert_eq!(head["snapshot_seq"], snapshot["seq"]);
}

#[test]
fn no_contains_by_default() {
    let (_tmp, root) = scanned();
    let result = share(&root, &[]);
    let snapshot = read_json(&published(&result, "data/snapshot.json"));
    assert_eq!(snapshot["snippets"], false);
    let mut found = 0;
    keys_named(&snapshot, "contains", &mut found);
    assert_eq!(found, 0);
    // Presence is left out unless asked for.
    assert_eq!(snapshot["who"], Value::Null);
    let with = share(&root, &["--include-presence"]);
    let snapshot = read_json(&published(&with, "data/snapshot.json"));
    assert!(snapshot["who"].is_array());
}

#[test]
fn the_snapshot_is_redacted() {
    // A repo whose directory name is a (fake, documented) AWS key: its block's name is masked; its
    // ID, an identifier, never is.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    let repo = root.join("AKIAIOSFODNN7EXAMPLE");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "hello\n").unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.test")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.test")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "--quiet", "--initial-branch", "main"]);
    git(&["add", "."]);
    git(&["commit", "--quiet", "--message", "init"]);
    let (code, env) = planroom(&root, &["scan"], "", &[]);
    assert_eq!(code, 0, "{env}");
    let result = share(&root, &[]);
    let snapshot = read_json(&published(&result, "data/snapshot.json"));
    let nodes = snapshot["maps"][0]["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());
    for node in nodes {
        assert_ne!(node["name"], "AKIAIOSFODNN7EXAMPLE");
    }
    let repo_node =
        nodes.iter().find(|n| n["id"].as_str().unwrap().contains("AKIAIOSFODNN7EXAMPLE")).unwrap();
    assert_eq!(repo_node["name"], "[redacted]");
    for crumb in snapshot["pane"]["minimap"]["blocks"].as_array().unwrap() {
        assert_ne!(crumb["name"], "AKIAIOSFODNN7EXAMPLE");
    }
}

#[test]
fn a_read_request_is_answered_by_a_tokenless_sync() {
    let (_tmp, root) = scanned();
    let shared = share(&root, &[]);
    let request = json!({"level": "container", "detail": "graph"});
    let lines = vec![
        line("u_test", "d1", 2, row("i1", 1, "map", request.clone())),
        json!({"path": "live/head", "version": 4}).to_string(),
    ];
    let (code, env) = import(&root, &lines, &[]);
    assert_eq!(code, 0, "{env}");
    let result = &env["result"];
    validate("ImportResult", result);
    assert_eq!(result["person"], false);
    assert_eq!(
        result["rows"],
        json!([{"path": "inbox/u_test/events/d1", "command": "map", "status": "imported", "reason": null}])
    );

    let writes = writes(result);
    let ops: Vec<(String, String, String)> = writes
        .iter()
        .map(|w| {
            (
                w["op"].as_str().unwrap().into(),
                w["collection"].as_str().unwrap().into(),
                w["doc_id"].as_str().unwrap().into(),
            )
        })
        .collect();
    let own = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
    assert_eq!(
        ops,
        [
            own("set", "answers", "d1"),
            own("set", "acks", "u_test"),
            own("delete", "inbox/u_test/events", "d1"),
            own("set", "live", "head")
        ]
    );
    // The answer is the engine's own map at that level, redacted.
    let answer = read_json(writes[0]["file_path"].as_str().unwrap());
    validate("Answer", &answer);
    assert_eq!(answer["format"], "planroom.answer/v1");
    assert_eq!(answer["handle"]["args"], json!(["--level", "container", "--detail", "graph"]));
    let (_, engine) = planroom(&root, &["map", "--level", "container", "--detail", "graph"], "", &[]);
    assert_eq!(answer["result"]["nodes"], engine["result"]["nodes"]);
    validate("MapView", &answer["result"]);
    // The ack, the pinned delete, and live/head written last, pinned to the version it was read at.
    let ack = read_json(writes[1]["file_path"].as_str().unwrap());
    validate("Ack", &ack);
    assert_eq!(ack["rows"], json!({"d1": "imported"}));
    assert_eq!(writes[2]["if_version"], 2);
    assert_eq!(writes[3]["if_version"], 4);
    let head = read_json(writes[3]["file_path"].as_str().unwrap());
    validate("LiveHead", &head);
    let snapshot = read_json(&published(&shared, "data/snapshot.json"));
    assert_eq!(head["snapshot_seq"], snapshot["seq"]);
}

#[test]
fn forged_actor_is_ignored() {
    let (_tmp, root) = scanned();
    share(&root, &[]);
    let mut forged = row("i1", 1, "map", json!({"level": "container"}));
    forged["actor"] = json!("person:mallory");
    let (code, env) = import(&root, &[line("u_test", "d1", 1, forged)], &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["rows"][0]["status"], "imported");
    // The path is the author: the ack is u_test's, and nothing names the forged actor.
    let writes = writes(&env["result"]);
    assert!(writes.iter().any(|w| w["collection"] == "acks" && w["doc_id"] == "u_test"));
    assert!(!env.to_string().contains("mallory"));
    for w in &writes {
        if let Some(path) = w["file_path"].as_str() {
            assert!(!std::fs::read_to_string(path).unwrap().contains("mallory"), "{path}");
        }
    }
}

#[test]
fn an_unlisted_inbox_command_is_refused() {
    let (_tmp, root) = scanned();
    let (code, env) =
        import(&root, &[line("u_test", "d9", 3, row("i1", 1, "handoff merge", json!({})))], &[]);
    assert_eq!(code, 0, "{env}");
    let r = &env["result"]["rows"][0];
    assert_eq!(r["status"], "refused");
    assert_eq!(r["reason"], "An Artifact row can't name handoff merge.");
    let writes = writes(&env["result"]);
    let ack = writes.iter().find(|w| w["collection"] == "acks").unwrap();
    assert_eq!(read_json(ack["file_path"].as_str().unwrap())["rows"], json!({"d9": "refused"}));
    assert!(writes.iter().any(|w| w["op"] == "delete" && w["doc_id"] == "d9" && w["if_version"] == 3));
    assert!(!writes.iter().any(|w| w["collection"] == "answers"));
}

#[test]
fn a_malformed_row_is_refused_not_fatal() {
    let (_tmp, root) = scanned();
    let lines = vec![
        line("u_test", "bad", 1, json!({"format": "planroom.inbox/v1", "counter": "one"})),
        line("u_test", "newer", 1, json!({"format": "planroom.inbox/v9"})),
        "not json".to_owned(),
        json!({"path": "inbox/u_test/elsewhere", "row": {}}).to_string(),
        line("u_test", "good", 1, row("i1", 1, "map", json!({"level": "container"}))),
    ];
    let (code, env) = import(&root, &lines, &[]);
    assert_eq!(code, 0, "{env}");
    let rows = env["result"]["rows"].as_array().unwrap();
    let status = |path: &str| rows.iter().find(|r| r["path"] == path).map(|r| r["status"].clone()).unwrap();
    assert_eq!(status("inbox/u_test/events/bad"), "refused");
    assert_eq!(status("inbox/u_test/events/newer"), "refused");
    assert_eq!(status("inbox/u_test/events/good"), "imported");
    // A line that names no row can't be acked; it is reported, never written back.
    assert_eq!(status(""), Value::Null);
    assert_eq!(status("inbox/u_test/elsewhere"), Value::Null);
    let ack = writes(&env["result"]).into_iter().find(|w| w["collection"] == "acks").unwrap();
    assert_eq!(
        read_json(ack["file_path"].as_str().unwrap())["rows"],
        json!({"bad": "refused", "newer": "refused", "good": "imported"})
    );
}

#[test]
fn a_large_sync_counts_its_rows_and_its_batch_files_hold_every_write() {
    // drift-3-1: the reply lists the first few rows and counts all; the acks and the batch files
    // hold every one, so the reply keeps to its budget.
    let (_tmp, root) = scanned();
    let mut lines: Vec<String> =
        (0..60).map(|i| line("u_test", &format!("d{i:02}"), 1, row("i1", i, "map", json!({})))).collect();
    lines.push(line("u_test", "x", 1, row("i1", 99, "handoff merge", json!({}))));
    lines.push(line("u_test", "e", 1, row("i1", 100, "edit", json!({}))));
    lines.push("not json".to_owned());
    let (code, env) = import(&root, &lines, &[]);
    assert_eq!(code, 0, "{env}");
    let result = &env["result"];
    validate("ImportResult", result);
    assert_eq!(
        result["counts"],
        json!({"imported": 60, "held": 0, "conflict": 0, "refused": 1, "left": 1, "skipped": 1})
    );
    assert_eq!(result["rows"].as_array().unwrap().len(), 5, "{result}");
    assert!(env["meta"]["size"]["tokens_est"].as_u64().unwrap() <= 800, "{env}");
    // 60 answers, one ack, 61 deletes and live/head: three calls of at most 50 writes.
    let files = result["batch_files"].as_array().unwrap();
    assert_eq!(files.len(), 3, "{result}");
    let writes = writes(result);
    assert_eq!(writes.len(), 123);
    assert_eq!(writes.iter().filter(|w| w["op"] == "delete").count(), 61);
    let last = writes.last().unwrap();
    assert_eq!((last["collection"].as_str(), last["doc_id"].as_str()), (Some("live"), Some("head")));
    let ack = writes.iter().find(|w| w["collection"] == "acks").unwrap();
    assert_eq!(read_json(ack["file_path"].as_str().unwrap())["rows"].as_object().unwrap().len(), 61);
}

#[test]
fn one_instances_rows_apply_in_counter_order() {
    let (_tmp, root) = scanned();
    let lines = vec![
        line("u_test", "c", 1, row("i1", 3, "map", json!({"level": "code"}))),
        line("u_test", "a", 1, row("i1", 1, "map", json!({"level": "container"}))),
        line("u_test", "b", 1, row("i1", 2, "map", json!({"level": "component"}))),
    ];
    let (code, env) = import(&root, &lines, &[]);
    assert_eq!(code, 0, "{env}");
    let applied: Vec<&str> =
        env["result"]["rows"].as_array().unwrap().iter().map(|r| r["path"].as_str().unwrap()).collect();
    assert_eq!(applied, ["inbox/u_test/events/a", "inbox/u_test/events/b", "inbox/u_test/events/c"]);
    let answered: Vec<String> = writes(&env["result"])
        .iter()
        .filter(|w| w["collection"] == "answers")
        .map(|w| w["doc_id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(answered, ["a", "b", "c"]);
}

#[test]
fn rows_come_only_from_the_mod() {
    let (_tmp, root) = scanned();
    std::fs::create_dir_all(root.join(".planroom/share/inbox")).unwrap();
    let (code, env) = planroom(&root, &["import", ".planroom/share/inbox"], "", &[]);
    assert_eq!(code, 2, "{env}");
    assert_eq!(env["error"]["code"], "invalid_argument");
    assert_eq!(env["error"]["message"], "import takes Artifact rows only on stdin: run planroom import -.");
}

// --- Person imports: the key and token helpers `actor.rs` uses (DR-72, DR-82). ---

fn key(seed: u8) -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = seed;
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

fn token(key: &SigningKey, command: &str, args: &[&str]) -> String {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let sig: Signature = key.sign(&person_token_message(command, &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

fn registered(root: &Path, k: &SigningKey) {
    let public = b64url(&k.verifying_key().to_sec1_point(false).to_bytes());
    let t = token(k, "person init", &[]);
    let (code, env) = planroom(root, &["person", "init"], &public, &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
}

#[test]
fn a_person_import_refuses_changes_until_they_land_and_a_tokenless_one_leaves_them() {
    let (_tmp, root) = scanned();
    let k = key(7);
    registered(&root, &k);
    // A review row: unsigned edit rows wait on the Apply card (inbox_edits.rs); reviews from a
    // row arrive later.
    let edit = || line("u_test", "e1", 1, row("i1", 1, "review", json!({"id": "x", "verdict": "accept"})));

    // No token: the change waits in the inbox, unacked and not deleted.
    let (code, env) = import(&root, &[edit()], &[]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["rows"][0]["status"], Value::Null);
    assert_eq!(env["result"]["rows"][0]["reason"], "Waits in the inbox for a person's /room sync.");
    assert!(!writes(&env["result"]).iter().any(|w| w["collection"] != "live"));

    // A person's import: a review from a row isn't importable yet, so it is refused, with an ack.
    let t = token(&k, "import", &["-"]);
    let (code, env) = import(&root, &[edit()], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["person"], true);
    assert_eq!(env["result"]["rows"][0]["status"], "refused");
    assert_eq!(env["result"]["rows"][0]["reason"], "review from an Artifact row isn't importable yet.");
    // The token works once.
    let (code, env) = import(&root, &[edit()], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 5, "{env}");
    assert_eq!(env["error"]["code"], "person_required");
}

#[test]
fn a_token_under_another_key_is_person_required() {
    let (_tmp, root) = scanned();
    registered(&root, &key(7));
    let t = token(&key(9), "import", &["-"]);
    let (code, env) = import(&root, &[], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 5, "{env}");
    assert_eq!(env["error"]["details"]["reason"], "bad_signature");
}

// --- Unsigned rows and the Apply card (DR-67, DR-75, DR-83, DR-89). ---

fn head(root: &Path) -> u64 {
    let (_, env) = planroom(root, &["map"], "", &[]);
    env["meta"]["seq"].as_u64().unwrap()
}

/// A person's token over `command` with the argv after it, read at `base`.
fn token_at(key: &SigningKey, command: &str, args: &[&str], base: &str) -> String {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let sig: Signature = key.sign(&person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// An inbox row from instance `i1` renaming `id`, at the head's `base`.
fn rename(root: &Path, counter: u64, id: &str, name: &str) -> Value {
    json!({"format": "planroom.inbox/v1", "instance": "i1", "counter": counter, "command": "edit",
        "request": {"op": "rename", "id": id, "name": name}, "base": head(root),
        "at": "2026-10-26T00:00:00Z"})
}

fn stored_events(root: &Path) -> Vec<Event> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    store.events_after(0, 10_000).unwrap()
}

#[test]
fn an_unsigned_or_broken_signature_joins_the_card() {
    let (_tmp, root) = scanned();
    let k = key(7);
    registered(&root, &k);
    // No pairing key checks a `sig` yet, so a row that carries one was signed by no paired device:
    // it waits on the Apply card like an unsigned row, never refused for its signature.
    let unsigned = line("u_ann", "d1", 1, rename(&root, 1, "orders:container:orders", "Desk"));
    let mut forged = rename(&root, 2, "worker:container:worker", "Runner");
    forged["sig"] = json!("bm90IGEgc2lnbmF0dXJl");
    let broken = line("u_ann", "d2", 1, forged);

    // A tokenless sync applies only rows whose signature checks: both stay in the inbox.
    let (code, env) = import(&root, &[unsigned.clone(), broken.clone()], &[]);
    assert_eq!(code, 0, "{env}");
    for r in env["result"]["rows"].as_array().unwrap() {
        assert_eq!(r["status"], Value::Null, "{env}");
    }
    assert!(!stored_events(&root).iter().any(|e| matches!(e.body, EventBody::Card(_))));

    // A person's sync holds both as ghosts on one Apply card.
    let t = token(&k, "import", &["-"]);
    let (code, env) = import(&root, &[unsigned, broken], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let rows = env["result"]["rows"].as_array().unwrap();
    let statuses: Vec<&Value> = rows.iter().map(|r| &r["status"]).collect();
    assert_eq!(statuses, [&json!("held"), &json!("held")], "{env}");
    let events = stored_events(&root);
    let cards: Vec<&Event> = events.iter().filter(|e| matches!(e.body, EventBody::Card(_))).collect();
    let [card] = cards.as_slice() else { panic!("one Apply card: {cards:?}") };
    let on_card = events
        .iter()
        .filter(|e| e.status == Status::Ghost && e.cause == Some(Cause::Event(card.id.clone())))
        .count();
    assert_eq!(on_card, 2, "each row's edit waits on the card");
}

#[test]
fn a_confirmed_row_keeps_the_viewer_as_actor() {
    let (_tmp, root) = scanned();
    let k = key(7);
    registered(&root, &k);
    let t = token(&k, "import", &["-"]);
    let row = line("u_ann", "d1", 1, rename(&root, 1, "orders:container:orders", "Desk"));
    let (code, env) = import(&root, &[row], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let events = stored_events(&root);
    let card = events.iter().find(|e| matches!(e.body, EventBody::Card(_))).unwrap().id.clone();
    let ghost = events.iter().find(|e| e.status == Status::Ghost).unwrap().id.clone();

    // The local person confirms it; the edit applies as the viewer, with the card as its cause.
    let base = head(&root).to_string();
    let args = ["--base", base.as_str(), ghost.0.as_str(), "accept"];
    let t = token_at(&k, "review", &args, &base);
    let argv: Vec<&str> = std::iter::once("review").chain(args).collect();
    let (code, env) = planroom(&root, &argv, "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let events = stored_events(&root);
    let copy = events.iter().find(|e| env["result"]["edit"] == e.id.0.as_str()).unwrap();
    assert_eq!(copy.status, Status::Applied);
    assert_eq!(copy.actor, Actor::Person { id: "person:artifact:u_ann".into(), name: None });
    assert_eq!(copy.client, Client { surface: Surface::Artifact, id: "artifact:u_ann:i1".into() });
    assert_eq!(copy.cause, Some(Cause::Event(card.clone())));
    // The review is the local person's own, and it resolved the card.
    let review = events.iter().find(|e| env["result"]["event"] == e.id.0.as_str()).unwrap();
    let local = matches!(&review.actor, Actor::Person { id, .. } if !id.starts_with("person:artifact:"));
    assert!(local, "{:?}", review.actor);
    assert_eq!(env["result"]["card_resolved"], card.0.as_str());
    let store = Store::open_existing(&planroom::app::workspace::store_path(&root)).unwrap().unwrap();
    let model =
        planroom::live::model(&store.snapshot().unwrap(), planroom::packs::vocabulary().unwrap()).unwrap();
    let orders = model.nodes.iter().find(|n| n.id.0 == "orders:container:orders").unwrap();
    assert_eq!(orders.name, "Desk", "the edit applied");
    drop(store);

    // One undo of the person's review takes back the viewer's edit too, and the card reopens.
    let base = head(&root).to_string();
    let reviewed = env["result"]["event"].as_str().unwrap();
    let args = ["--base", base.as_str(), "undo", reviewed];
    let t = token_at(&k, "edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
    let (code, env) = planroom(&root, &argv, "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let store = Store::open_existing(&planroom::app::workspace::store_path(&root)).unwrap().unwrap();
    let snapshot = store.snapshot().unwrap();
    let folded = planroom::live::folded(&snapshot, planroom::packs::vocabulary().unwrap()).unwrap();
    assert_eq!(folded.open_cards, [card]);
    let orders = folded.model.nodes.iter().find(|n| n.id.0 == "orders:container:orders").unwrap();
    assert_eq!(orders.name, "orders", "the undo took the edit back");
}

#[test]
fn a_stale_row_held_as_a_conflict_applies_as_its_viewer() {
    let (_tmp, root) = scanned();
    let k = key(7);
    registered(&root, &k);
    // The viewer's row is made at this head, and a person's rename lands before it is imported.
    let row = rename(&root, 1, "orders:container:orders", "Desk");
    let base = head(&root).to_string();
    let args = ["--base", base.as_str(), "rename", "orders:container:orders", "Mine"];
    let t = token_at(&k, "edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
    let (code, env) = planroom(&root, &argv, "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let t = token(&k, "import", &["-"]);
    let (code, env) = import(&root, &[line("u_ann", "d1", 1, row)], &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let events = stored_events(&root);
    let held = events.iter().find(|e| e.status == Status::Conflict).expect("held as a conflict").id.clone();
    assert!(!events.iter().any(|e| matches!(e.body, EventBody::Card(_))), "a conflict deals no card");

    // The local person accepts it: it applies as the viewer, as a row confirmed on the card does,
    // caused by the conflict it copies; the review is the person's own.
    let base = head(&root).to_string();
    let args = ["--base", base.as_str(), held.0.as_str(), "accept"];
    let t = token_at(&k, "review", &args, &base);
    let argv: Vec<&str> = std::iter::once("review").chain(args).collect();
    let (code, env) = planroom(&root, &argv, "", &[("PLANROOM_PERSON_TOKEN", &t)]);
    assert_eq!(code, 0, "{env}");
    let events = stored_events(&root);
    let copy = events.iter().find(|e| env["result"]["edit"] == e.id.0.as_str()).unwrap();
    assert_eq!(copy.status, Status::Applied);
    assert_eq!(copy.actor, Actor::Person { id: "person:artifact:u_ann".into(), name: None });
    assert_eq!(copy.client, Client { surface: Surface::Artifact, id: "artifact:u_ann:i1".into() });
    assert_eq!(copy.cause, Some(Cause::Event(held)));
    let review = events.iter().find(|e| env["result"]["event"] == e.id.0.as_str()).unwrap();
    let local = matches!(&review.actor, Actor::Person { id, .. } if !id.starts_with("person:artifact:"));
    assert!(local, "{:?}", review.actor);
}
