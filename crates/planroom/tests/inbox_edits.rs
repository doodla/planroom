//! Unsigned inbox edits (contracts.md, Remote mode's import rules; DR-83, DR-89): in a person's
//! import, every `edit` row no paired device signed applies nothing on its own. Its edits wait as
//! ghosts under `person:artifact:<uid>`, each caused by the one open Apply card, which the first
//! such row deals in its own write; a sync that brings a row again replays it, and a tokenless
//! sync leaves the rows in the inbox.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_contract::{Actor, Cause, Event, EventBody, Status, Surface};
use planroom_kernel::keys::person_token_message;
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

/// `planroom ARGS --json` with `env`; the exit code and the one envelope on stdout.
fn run(root: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .args(args)
        .arg("--json");
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
    let at = fresh_second();
    let sig: Signature = key().sign(&person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// A second within the token window no earlier token of this test binary signed, so two
/// otherwise equal tokens, such as two imports' within one second, are never one spent token.
fn fresh_second() -> u64 {
    static USED: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut used = USED.lock().unwrap();
    let at = (now - 29..=now).rev().find(|s| !used.contains(s)).expect("at most 30 tokens a second");
    used.insert(at);
    at
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

fn seq(root: &Path) -> u64 {
    let (_, env) = run(root, &["map"], &[], "");
    env["meta"]["seq"].as_u64().unwrap()
}

/// One inbox row from viewer `uid`, as the mod passes it.
fn line(uid: &str, doc: &str, counter: u64, command: &str, request: Value, base: u64) -> String {
    let row = json!({"format": "planroom.inbox/v1", "instance": "i1", "counter": counter, "command": command,
        "request": request, "base": base, "at": "2026-10-26T00:00:00Z"});
    json!({"path": format!("inbox/{uid}/events/{doc}"), "version": 1, "row": row}).to_string()
}

/// `import -` of `lines`, as a person when `person` holds.
fn import(root: &Path, lines: &[String], person: bool) -> Value {
    let signed = token("import", &["-"], "");
    let env: &[(&str, &str)] = if person { &[("PLANROOM_PERSON_TOKEN", &signed)] } else { &[] };
    let (code, env) = run(root, &["import", "-"], env, &(lines.join("\n") + "\n"));
    assert_eq!(code, 0, "{env}");
    env
}

fn events(root: &Path) -> Vec<Event> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    store.events_after(0, 10_000).unwrap()
}

fn name_of(root: &Path, id: &str) -> String {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    let snapshot = store.snapshot().unwrap();
    let model = planroom::live::model(&snapshot, planroom::packs::vocabulary().unwrap()).unwrap();
    model.nodes.iter().find(|n| n.id.0 == id).unwrap().name.clone()
}

#[test]
fn unsigned_edit_rows_become_ghosts_on_one_apply_card() {
    let (_tmp, root) = workspace();
    let base = seq(&root);
    let rename = |uid: &str, doc: &str, counter: u64, id: &str, name: &str| {
        line(uid, doc, counter, "edit", json!({"op": "rename", "id": id, "name": name}), base)
    };
    let ann = rename("u_ann", "d1", 1, "orders:container:orders", "Desk");
    let bob = line(
        "u_bob",
        "d1",
        1,
        "edit",
        json!({"op": "move", "id": "storefront:dir:src/cart", "parent": "storefront:dir:test"}),
        base,
    );
    // A row with a `sig` no paired device made (none can be paired yet) waits like an unsigned
    // one; an undo and a review keep today's handling.
    let signed = {
        let mut row: Value =
            serde_json::from_str(&rename("u_ann", "d2", 2, "worker:container:worker", "R")).unwrap();
        row["row"]["sig"] = json!("c2lnbmVk");
        row.to_string()
    };
    let undo = line("u_ann", "d3", 3, "edit", json!({"op": "undo", "event": "x"}), base);
    let review = line("u_ann", "d4", 4, "review", json!({"id": "x", "verdict": "accept"}), base);
    let all = [ann.clone(), bob.clone(), signed, undo, review];

    // A tokenless sync leaves every change in the inbox and writes nothing.
    let env = import(&root, &all, false);
    assert_eq!(env["result"]["counts"]["left"], 5, "{env}");
    assert_eq!(seq(&root), base);

    let env = import(&root, &all, true);
    let rows = env["result"]["rows"].as_array().unwrap();
    let status: Vec<&str> = rows.iter().map(|r| r["status"].as_str().unwrap()).collect();
    assert_eq!(status, ["held", "held", "held", "refused", "refused"], "{env}");
    assert_eq!(env["result"]["counts"]["held"], 3);

    // One card, written first, and each row's edits as ghosts it caused, under the row's viewer.
    let written: Vec<Event> = events(&root).into_iter().filter(|e| e.seq > base).collect();
    let EventBody::Card(card) = &written[0].body else { panic!("the card comes first: {:?}", written[0]) };
    assert_eq!((card.kind.0.as_str(), card.dealt_by.0.as_str()), ("apply", "import"));
    let workspace_id = Store::open_existing(&planroom::app::workspace::store_path(&root))
        .unwrap()
        .unwrap()
        .meta("workspace_id")
        .unwrap()
        .unwrap();
    assert_eq!(card.subject, workspace_id, "the Apply card is about the workspace");
    // Dealt in the first row's own write, under a key of its own, so an undo of that row's ghosts
    // leaves it for the rows that join it.
    assert!(written[0].idem.starts_with("card:apply:"), "{}", written[0].idem);
    assert_eq!(written[1].seq, written[0].seq + 1);
    let ghosts = &written[1..];
    assert_eq!(ghosts.len(), 3);
    for (ghost, (uid, idem)) in ghosts.iter().zip([
        ("u_ann", "artifact:u_ann:d1"),
        ("u_bob", "artifact:u_bob:d1"),
        ("u_ann", "artifact:u_ann:d2"),
    ]) {
        assert_eq!(ghost.status, Status::Ghost);
        assert_eq!(ghost.cause, Some(Cause::Event(written[0].id.clone())));
        assert_eq!(ghost.actor, Actor::Person { id: format!("person:artifact:{uid}"), name: None });
        assert_eq!(
            (ghost.client.surface, ghost.client.id.as_str()),
            (Surface::Artifact, &*format!("artifact:{uid}:i1"))
        );
        assert_eq!(ghost.idem, idem);
        assert!(matches!(ghost.body, EventBody::Edit(_)));
    }
    assert_eq!(name_of(&root, "orders:container:orders"), "orders", "a ghost applies nothing");

    // The same rows again, as a sync that couldn't ack them brings them: nothing new is written.
    let head = seq(&root);
    let env = import(&root, &[ann, bob], true);
    assert_eq!(env["result"]["counts"]["held"], 2, "{env}");
    assert_eq!(seq(&root), head);

    // A later row joins the open card instead of dealing another.
    let later = rename("u_cy", "d1", 1, "worker:container:worker", "Runner");
    import(&root, &[later], true);
    let cards = events(&root).into_iter().filter(|e| matches!(e.body, EventBody::Card(_))).count();
    assert_eq!(cards, 1);
    let last = events(&root).pop().unwrap();
    assert_eq!(last.cause, Some(Cause::Event(written[0].id.clone())));

    // A row naming a block that isn't on the map is refused with why, and writes nothing.
    let head = seq(&root);
    let missing = rename("u_ann", "d9", 9, "nowhere:container:none", "X");
    let env = import(&root, &[missing], true);
    assert_eq!(env["result"]["rows"][0]["status"], "refused", "{env}");
    assert!(env["result"]["rows"][0]["reason"].as_str().unwrap().starts_with("There is no block"), "{env}");
    assert_eq!(seq(&root), head);
}
