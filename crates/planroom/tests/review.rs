//! `review ID accept|reject` (architecture.md, Writes and Actor rules; DR-5, DR-54, DR-89): a
//! person reviews ghosts one item at a time, an accept writes a copy of the reviewed edit that the
//! reviewing person wrote and the ghost caused, and an agent may only reject its own ghosts; every
//! other agent call is `person_required`, naming the exact review a person runs.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_contract::{Actor, Cause, Event, EventBody, EventId, Status};
use planroom_kernel::keys::person_token_message;
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

const AGENT: &str = "agent:1a2b3c4d:chat";

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
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
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
    let signed = token("person init", &[], "");
    let (code, env) = run(&root, &["person", "init"], &[("PLANROOM_PERSON_TOKEN", &signed)], &public);
    assert_eq!(code, 0, "{env}");
    (tmp, root)
}

fn seq(root: &Path) -> u64 {
    let (_, env) = run(root, &["map"], &[], "");
    env["meta"]["seq"].as_u64().unwrap()
}

/// `planroom edit --base HEAD ARGS` as `agent`.
fn agent_edit(root: &Path, agent: &str, op: &[&str]) -> Value {
    let base = seq(root).to_string();
    let argv: Vec<&str> = ["edit", "--base", base.as_str()].into_iter().chain(op.iter().copied()).collect();
    let (code, env) = run(root, &argv, &[("PLANROOM_ACTOR", agent)], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["status"], "ghost", "{env}");
    env
}

/// `planroom review --base HEAD ID VERDICT` as the person, signed.
fn review(root: &Path, id: &str, verdict: &str) -> (i32, Value) {
    let base = seq(root).to_string();
    let args = ["--base", base.as_str(), id, verdict];
    let signed = token("review", &args, &base);
    let argv: Vec<&str> = std::iter::once("review").chain(args).collect();
    run(root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "")
}

/// `planroom review --base HEAD ID VERDICT` as `agent`.
fn agent_review(root: &Path, agent: &str, id: &str, verdict: &str) -> (i32, Value) {
    let base = seq(root).to_string();
    run(root, &["review", "--base", &base, id, verdict], &[("PLANROOM_ACTOR", agent)], "")
}

fn events(root: &Path) -> Vec<Event> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    store.events_after(0, 10_000).unwrap()
}

fn event(root: &Path, id: &str) -> Event {
    events(root).into_iter().find(|e| e.id.0 == id).unwrap_or_else(|| panic!("no event {id}"))
}

/// The fold of the store as it stands, as every read folds it.
fn folded(root: &Path) -> planroom_kernel::fold::Folded {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    let snapshot = store.snapshot().unwrap();
    planroom::live::folded(&snapshot, planroom::packs::vocabulary().unwrap()).unwrap()
}

fn parent_of(root: &Path, id: &str) -> Option<String> {
    let folded = folded(root);
    let node = folded.model.nodes.iter().find(|n| n.id.0 == id).unwrap_or_else(|| panic!("no node {id}"));
    node.parent.as_ref().map(|p| p.0.clone())
}

fn on_map(root: &Path, id: &str) -> bool {
    folded(root).model.nodes.iter().any(|n| n.id.0 == id)
}

fn name_of(root: &Path, id: &str) -> String {
    folded(root).model.nodes.iter().find(|n| n.id.0 == id).unwrap().name.clone()
}

#[test]
fn ghosts_are_accepted_item_by_item() {
    let (_tmp, root) = workspace();
    let members = ["storefront:dir:src/api", "storefront:dir:src/app"];
    let before: Vec<Option<String>> = members.iter().map(|m| parent_of(&root, m)).collect();
    let env = agent_edit(&root, AGENT, &["group", "Front door", members[0], members[1]]);
    assert_eq!(env["result"]["count"], 3, "a group writes its node and one move per member: {env}");
    let first = env["result"]["event"].as_str().unwrap().to_owned();
    let command: Vec<Event> = events(&root).into_iter().filter(|e| e.status == Status::Ghost).collect();
    assert_eq!(command.len(), 3);
    assert_eq!(command[0].id.0, first);
    let group = "storefront:group:front-door";
    assert!(!on_map(&root, group), "a ghost stays out of the map");

    // The group's own node, alone: the members stay where they were.
    let (code, env) = review(&root, &first, "accept");
    assert_eq!(code, 0, "{env}");
    let result = &env["result"];
    assert_eq!((result["of"].as_str(), result["verdict"].as_str()), (Some(first.as_str()), Some("accept")));
    let copy = event(&root, result["edit"].as_str().unwrap());
    assert_eq!(copy.status, Status::Applied);
    assert_eq!(
        copy.cause,
        Some(Cause::Event(EventId(first.clone()))),
        "the accept's edit is caused by the ghost"
    );
    assert!(matches!(copy.actor, Actor::Person { .. }), "the reviewing person writes it: {:?}", copy.actor);
    let (EventBody::Edit(ghost), EventBody::Edit(accepted)) = (&command[0].body, &copy.body) else {
        panic!()
    };
    assert_eq!((&accepted.target, &accepted.fields), (&ghost.target, &ghost.fields), "a copy of its fields");
    assert!(on_map(&root, group), "the accepted group is on the map");
    for (member, was) in members.iter().zip(&before) {
        assert_eq!(&parent_of(&root, member), was, "{member} waits for its own review");
    }

    // One member's move accepted, the other's rejected.
    let (code, env) = review(&root, &command[1].id.0, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(parent_of(&root, members[0]).as_deref(), Some(group));
    let (code, env) = review(&root, &command[2].id.0, "reject");
    assert_eq!(code, 0, "{env}");
    assert!(env["result"].get("edit").is_none(), "a reject writes no edit: {env}");
    assert_eq!(parent_of(&root, members[1]), before[1]);

    // Each item is reviewed once.
    let (code, env) = review(&root, &command[2].id.0, "accept");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    // An applied edit isn't an item to review.
    let (code, env) = review(&root, &copy.id.0, "reject");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    // Nor is what isn't an event: an inferred fact's review comes with M2.
    let (code, env) = review(&root, "storefront:dir:src/api", "accept");
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("not_found")), "{env}");

    // An accept from a view older than another writer's edit of the same field is stored as a
    // conflict, never a silent no-op; the conflict is accepted through the same path.
    let ghost = agent_edit(&root, AGENT, &["rename", "orders:container:orders", "Desk"]);
    let ghost = ghost["result"]["event"].as_str().unwrap().to_owned();
    let view = seq(&root).to_string();
    let edit_args = ["--base", view.as_str(), "rename", "orders:container:orders", "Till"];
    let signed = token("edit", &edit_args, &view);
    let argv: Vec<&str> = std::iter::once("edit").chain(edit_args).collect();
    let (code, env) = run(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!(code, 0, "{env}");
    let args = ["--base", view.as_str(), ghost.as_str(), "accept"];
    let signed = token("review", &args, &view);
    let argv: Vec<&str> = std::iter::once("review").chain(args).collect();
    let (code, env) = run(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!((code, env["error"]["code"].as_str()), (4, Some("stale_write")), "{env}");
    assert_eq!(name_of(&root, "orders:container:orders"), "Till");
    let conflict = env["error"]["details"]["event"].as_str().unwrap().to_owned();
    assert_eq!(event(&root, &conflict).status, Status::Conflict);
    let (code, env) = review(&root, &conflict, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(name_of(&root, "orders:container:orders"), "Desk");
}

#[test]
fn an_agent_accept_is_person_required() {
    let (_tmp, root) = workspace();
    let env = agent_edit(&root, AGENT, &["rename", "orders:container:orders", "Order desk"]);
    let ghost = env["result"]["event"].as_str().unwrap().to_owned();
    let head = seq(&root);

    for id in [ghost.as_str(), "no-such-event"] {
        let (code, env) = agent_review(&root, AGENT, id, "accept");
        assert_eq!(code, 5, "{env}");
        let error = &env["error"];
        assert_eq!(error["code"], "person_required");
        assert_eq!(error["details"], json!({"command": "review", "reason": "not_a_person"}), "{env}");
        // The hint is the exact review a person runs, from where it is offered.
        let line = format!("planroom review --base {head} {id} accept");
        assert_eq!(error["hint"], format!("{line}, from your own terminal: it asks you to confirm"));
        let words: Vec<String> = line.split(' ').skip(2).map(str::to_owned).collect();
        assert!(planroom::cli::parse_leaf("review", &words).is_ok(), "{line} parses as review");
    }
    assert_eq!(seq(&root), head, "nothing was written");

    // The person's own accept of that line applies.
    let (code, env) = review(&root, &ghost, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(name_of(&root, "orders:container:orders"), "Order desk");
}

#[test]
fn an_agent_rejects_only_its_own_ghosts() {
    let (_tmp, root) = workspace();
    let mine = agent_edit(&root, AGENT, &["rename", "orders:container:orders", "Mine"]);
    let mine = mine["result"]["event"].as_str().unwrap().to_owned();
    let theirs = agent_edit(&root, "agent:9z9z9z9z:chat", &["hide", "orders:container:orders"]);
    let theirs = theirs["result"]["event"].as_str().unwrap().to_owned();
    let head = seq(&root);

    // Another session's ghost, and its own session under another role, are a person's to review.
    for (agent, id) in [(AGENT, theirs.as_str()), ("agent:1a2b3c4d:implementer", mine.as_str())] {
        let (code, env) = agent_review(&root, agent, id, "reject");
        assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{agent}: {env}");
        let hint = format!(
            "planroom review --base {head} {id} reject, from your own terminal: it asks you to confirm"
        );
        assert_eq!(env["error"]["hint"], hint);
    }
    // A person's applied edit is never an agent's to reject either.
    let base = head.to_string();
    let args = ["--base", base.as_str(), "rename", "orders:container:orders", "Desk"];
    let signed = token("edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
    let (code, env) = run(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!(code, 0, "{env}");
    let applied = env["result"]["event"].as_str().unwrap().to_owned();
    let (code, env) = agent_review(&root, AGENT, &applied, "reject");
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");
    let after_refusals = seq(&root);

    // Its own ghost, under its own session and role: the reject writes one review, as the agent.
    let (code, env) = agent_review(&root, AGENT, &mine, "reject");
    assert_eq!(code, 0, "{env}");
    assert_eq!(seq(&root), after_refusals + 1);
    let written = event(&root, env["result"]["event"].as_str().unwrap());
    assert!(
        matches!(&written.actor, Actor::Agent { session, role, .. } if session == "1a2b3c4d" && role == "chat")
    );
    assert!(matches!(&written.body, EventBody::Review { of, .. } if of.0 == mine));
    // Rejected once, it is reviewed.
    let (code, env) = agent_review(&root, AGENT, &mine, "reject");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
}

#[test]
fn an_agent_review_without_a_base_is_person_required() {
    let (_tmp, root) = workspace();
    let env = agent_edit(&root, "agent:9z9z9z9z:chat", &["rename", "orders:container:orders", "Theirs"]);
    let theirs = env["result"]["event"].as_str().unwrap().to_owned();
    let head = seq(&root);

    // An agent's accept, or its reject of a ghost not its own, is a person's to run whatever else
    // is wrong with the call: the refusal comes before the missing base, and its hint reviews at
    // the head, so the line runs as offered.
    for verdict in ["accept", "reject"] {
        let (code, env) = run(&root, &["review", &theirs, verdict], &[("PLANROOM_ACTOR", AGENT)], "");
        assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_required")), "{env}");
        let line = format!("planroom review --base {head} {theirs} {verdict}");
        assert_eq!(env["error"]["hint"], format!("{line}, from your own terminal: it asks you to confirm"));
    }
    assert_eq!(seq(&root), head, "nothing was written");

    // The reject of its own ghost is the agent's, so the missing base is what it hears about.
    let env = agent_edit(&root, AGENT, &["hide", "orders:container:orders"]);
    let mine = env["result"]["event"].as_str().unwrap().to_owned();
    let (code, env) = run(&root, &["review", &mine, "reject"], &[("PLANROOM_ACTOR", AGENT)], "");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("base_required")), "{env}");
}
