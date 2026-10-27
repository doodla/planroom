//! Cards a write deals (DR-89), dealt here through import's Apply card: `focus` lists the open
//! card face up; a card is picked once, and only by a person; no dealer deals a second open card of
//! one kind and subject; the Apply card resolves with its last ghost's review, reopens when an undo
//! takes that review back, and `review` on the card itself names its ghosts. The store's open-card
//! read and the fold agree throughout.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_contract::{Event, EventBody, EventId, Status, Verdict};
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

/// One unsigned inbox row asking to rename `id`, from viewer `uid`, as the mod passes it.
fn rename_row(root: &Path, uid: &str, doc: &str, counter: u64, id: &str, name: &str) -> String {
    let row = json!({"format": "planroom.inbox/v1", "instance": "i1", "counter": counter, "command": "edit",
        "request": {"op": "rename", "id": id, "name": name}, "base": seq(root), "at": "2026-10-26T00:00:00Z"});
    json!({"path": format!("inbox/{uid}/events/{doc}"), "version": 1, "row": row}).to_string()
}

/// A person's `import -` of `lines`.
fn import(root: &Path, lines: &[String]) -> Value {
    let signed = token("import", &["-"], "");
    let (code, env) =
        run(root, &["import", "-"], &[("PLANROOM_PERSON_TOKEN", &signed)], &(lines.join("\n") + "\n"));
    assert_eq!(code, 0, "{env}");
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

fn focus(root: &Path) -> Value {
    let (code, env) = run(root, &["focus"], &[], "");
    assert_eq!(code, 0, "{env}");
    env
}

fn events(root: &Path) -> Vec<Event> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    store.events_after(0, 10_000).unwrap()
}

fn cards(root: &Path) -> Vec<Event> {
    events(root).into_iter().filter(|e| matches!(e.body, EventBody::Card(_))).collect()
}

/// The open cards as the store's read and the kernel decide them, after checking that the fold
/// of the whole store says the same.
fn open_cards(root: &Path) -> Vec<EventId> {
    let store = Store::open_existing(&planroom::app::workspace::store_path(root)).unwrap().unwrap();
    let snapshot = store.snapshot().unwrap();
    let read = planroom_kernel::fold::open_cards(&snapshot.card_events().unwrap());
    let folded = planroom::live::folded(&snapshot, planroom::packs::vocabulary().unwrap()).unwrap();
    assert_eq!(read, folded.open_cards, "the store's open-card read and the fold agree");
    read
}

/// The ghosts on `card`, in order.
fn ghosts_on(root: &Path, card: &EventId) -> Vec<EventId> {
    events(root)
        .into_iter()
        .filter(|e| {
            e.status == Status::Ghost && e.cause == Some(planroom_contract::Cause::Event(card.clone()))
        })
        .map(|e| e.id)
        .collect()
}

#[test]
fn a_dealt_card_lists_in_focus_face_up() {
    let (_tmp, root) = workspace();
    assert!(focus(&root)["result"]["decision"].is_null(), "no card before a write deals one");
    import(
        &root,
        &[
            rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk"),
            rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner"),
        ],
    );
    let [card] = cards(&root).try_into().unwrap();
    let env = focus(&root);
    let pane = &env["result"];
    let decision = &pane["decision"];
    assert_eq!(decision["id"], card.id.0.as_str());
    assert_eq!(decision["question"], "Apply 2 edits from the shared page?");
    assert_eq!(decision["person_only"], true);
    assert_eq!(decision["stakes"], "person");
    assert_eq!(
        decision["choices"],
        json!([{"id": "accept", "label": "Apply", "preselected": true},
               {"id": "reject", "label": "Discard", "preselected": false}])
    );
    let head = pane["focus"]["seq"].as_u64().unwrap().to_string();
    let ghosts = ghosts_on(&root, &card.id);
    assert_eq!(decision["batch"]["count"], 2);
    for (item, ghost) in decision["batch"]["items"].as_array().unwrap().iter().zip(&ghosts) {
        assert_eq!(item["command"], "review");
        assert_eq!(item["args"], json!(["--base", head, ghost.0, "accept"]));
        assert_eq!(item["label"], "Apply this edit");
    }
    assert_eq!(pane["more_decisions"], 0);
    // The pane's text names it too.
    let text = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .arg("focus")
        .current_dir(&root)
        .env("PLANROOM_DATA", root.join(".data"))
        .output()
        .unwrap();
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Decide: Apply 2 edits from the shared page?"), "{text}");
    // A read deals nothing.
    assert_eq!(cards(&root).len(), 1);
}

#[test]
fn a_card_picked_twice_is_already_resolved() {
    let (_tmp, root) = workspace();
    import(&root, &[rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk")]);
    let [card] = cards(&root).try_into().unwrap();
    let [ghost] = ghosts_on(&root, &card.id).try_into().unwrap();
    let (code, env) = review(&root, &ghost.0, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());

    let (code, env) = review(&root, &card.id.0, "reject");
    assert_eq!(code, 4, "{env}");
    let error = &env["error"];
    assert_eq!(error["code"], "already_resolved");
    assert_eq!(error["details"]["pick"], "accept");
    assert!(error["details"]["by"].as_str().is_some_and(|by| !by.is_empty()), "{env}");
    assert!(error["details"]["at"].as_str().is_some_and(|at| at.ends_with('Z')), "{env}");
    assert!(open_cards(&root).is_empty());
}

#[test]
fn an_agent_pick_of_a_card_is_person_required() {
    let (_tmp, root) = workspace();
    import(&root, &[rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk")]);
    let [card] = cards(&root).try_into().unwrap();
    let [ghost] = ghosts_on(&root, &card.id).try_into().unwrap();
    let head = seq(&root);
    for (id, verdict) in [(&card.id, "accept"), (&card.id, "reject"), (&ghost, "accept"), (&ghost, "reject")]
    {
        let (code, env) = run(
            &root,
            &["review", "--base", &head.to_string(), &id.0, verdict],
            &[("PLANROOM_ACTOR", AGENT)],
            "",
        );
        assert_eq!(code, 5, "{env}");
        let error = &env["error"];
        assert_eq!(error["code"], "person_required");
        // The card a person picks, whether the agent named the card or a ghost on it.
        assert_eq!(error["details"]["card"], card.id.0.as_str(), "{env}");
        // The Apply card is picked through its ghosts, so the line a person runs reviews its first
        // waiting ghost, which runs; the card's own line would be refused.
        let line = format!("planroom review --base {head} {} {verdict}", ghost.0);
        assert_eq!(error["hint"], format!("{line}, from your own terminal: it asks you to confirm"));
    }
    assert_eq!(seq(&root), head, "an agent's pick writes nothing");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    // The person runs the hinted line, and it resolves the card.
    let (code, env) = review(&root, &ghost.0, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());
}

#[test]
fn a_dealer_never_deals_a_second_open_card() {
    let (_tmp, root) = workspace();
    import(&root, &[rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk")]);
    // Another sync, another viewer: their rows join the open card.
    import(
        &root,
        &[
            rename_row(&root, "u_bob", "d1", 1, "worker:container:worker", "Runner"),
            rename_row(&root, "u_bob", "d2", 2, "storefront:container:storefront", "Shop"),
        ],
    );
    let [card] = cards(&root).try_into().unwrap();
    assert_eq!(ghosts_on(&root, &card.id).len(), 3);
    // A rescan deals nothing the open card already deals.
    let (code, env) = run(&root, &["scan"], &[], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(cards(&root).len(), 1);
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    assert_eq!(focus(&root)["result"]["decision"]["batch"]["count"], 3);

    // Once it resolves, the next unsigned row deals a new one, which is the only one open.
    for ghost in ghosts_on(&root, &card.id) {
        let (code, env) = review(&root, &ghost.0, "reject");
        assert_eq!(code, 0, "{env}");
    }
    assert!(open_cards(&root).is_empty());
    import(&root, &[rename_row(&root, "u_ann", "d9", 9, "orders:container:orders", "Till")]);
    let all = cards(&root);
    assert_eq!(all.len(), 2);
    assert_eq!(open_cards(&root), [all[1].id.clone()]);
}

#[test]
fn the_apply_card_resolves_with_its_last_ghost() {
    let (_tmp, root) = workspace();
    import(
        &root,
        &[
            rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk"),
            rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner"),
        ],
    );
    let [card] = cards(&root).try_into().unwrap();
    let [first, last] = ghosts_on(&root, &card.id).try_into().unwrap();

    let (code, env) = review(&root, &first.0, "reject");
    assert_eq!(code, 0, "{env}");
    assert!(env["result"].get("card_resolved").is_none(), "one ghost still waits: {env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    assert_eq!(focus(&root)["result"]["decision"]["question"], "Apply 1 edits from the shared page?");

    let (code, env) = review(&root, &last.0, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());
    assert!(open_cards(&root).is_empty());
    assert!(focus(&root)["result"]["decision"].is_null());
    // The card's own review is written with the last ghost's, accepted since one was, under a key
    // of its own beside that review's, so a later undo can reverse it alone.
    let review_event = env["result"]["event"].as_str().unwrap().to_owned();
    let all = events(&root);
    let reviewed = all.iter().find(|e| e.id.0 == review_event).unwrap();
    let stem = reviewed.idem.rsplit_once('#').map_or(reviewed.idem.as_str(), |(s, _)| s).to_owned();
    let picked = all
        .iter()
        .find(|e| matches!(&e.body, EventBody::Review { of, .. } if of == &card.id))
        .expect("the card's own review");
    assert_eq!(picked.idem, format!("{stem}#card:{}", card.id.0), "beside {}", reviewed.idem);
    assert_eq!(picked.seq, reviewed.seq + 2, "in the same write, after the accept's copy");
    assert!(matches!(picked.body, EventBody::Review { verdict: Verdict::Accept, .. }));

    // Undoing that review takes back all of it: the card reopens with its one waiting ghost, in
    // the store's read and in the fold alike.
    let base = seq(&root).to_string();
    let args = ["--base", base.as_str(), "undo", review_event.as_str()];
    let signed = token("edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
    let (code, env) = run(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
    assert_eq!(code, 0, "{env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    let decision = &focus(&root)["result"]["decision"];
    assert_eq!(decision["batch"]["count"], 1);
    assert_eq!(decision["batch"]["items"][0]["args"][2], last.0.as_str());
    // Reviewed again, it resolves again.
    let (code, env) = review(&root, &last.0, "reject");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());
    assert!(open_cards(&root).is_empty());

    // A card whose last waiting ghost is undone, not reviewed, closes with that undo, and reopens
    // when the undo is undone.
    import(
        &root,
        &[
            rename_row(&root, "u_bob", "d1", 1, "orders:container:orders", "Till"),
            rename_row(&root, "u_bob", "d2", 2, "worker:container:worker", "Courier"),
        ],
    );
    let second = cards(&root).pop().unwrap();
    let [kept, dropped] = ghosts_on(&root, &second.id).try_into().unwrap();
    let (code, env) = review(&root, &kept.0, "accept");
    assert_eq!(code, 0, "{env}");
    let undo = |event: &str| {
        let base = seq(&root).to_string();
        let args = ["--base", base.as_str(), "undo", event];
        let signed = token("edit", &args, &base);
        let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
        let (code, env) = run(&root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
        assert_eq!(code, 0, "{env}");
        env["result"]["event"].as_str().unwrap().to_owned()
    };
    let undone = undo(&dropped.0);
    assert!(open_cards(&root).is_empty(), "no card stays open with nothing waiting on it");
    let closing = events(&root).pop().unwrap();
    assert!(
        matches!(&closing.body, EventBody::Review { of, verdict: Verdict::Accept } if of == &second.id),
        "closed in the undo's write, as accepted since one ghost was: {closing:?}"
    );
    undo(&undone);
    assert_eq!(open_cards(&root), std::slice::from_ref(&second.id));
    assert_eq!(focus(&root)["result"]["decision"]["batch"]["count"], 1);
}

#[test]
fn review_of_the_apply_card_names_its_ghosts() {
    let (_tmp, root) = workspace();
    let ids = [
        "orders:container:orders",
        "worker:container:worker",
        "storefront:container:storefront",
        "storefront:dir:src/api",
        "storefront:dir:src/app",
        "storefront:dir:src/cart",
    ];
    let rows: Vec<String> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| rename_row(&root, "u_ann", &format!("d{i}"), i as u64, id, &format!("n{i}")))
        .collect();
    import(&root, &rows);
    let [card] = cards(&root).try_into().unwrap();
    let ghosts = ghosts_on(&root, &card.id);
    assert_eq!(ghosts.len(), 6);
    let head = seq(&root);

    let (code, env) = review(&root, &card.id.0, "accept");
    assert_eq!(code, 2, "{env}");
    let error = &env["error"];
    assert_eq!(error["code"], "invalid_argument");
    let shown: Vec<&str> = ghosts.iter().take(5).map(|g| g.0.as_str()).collect();
    assert_eq!(error["details"]["ghosts"], json!(shown));
    assert_eq!(error["details"]["count"], 6);
    assert_eq!(error["details"]["field"], "id");
    // The hint reviews its first ghost: an exact command that runs from here.
    assert_eq!(error["hint"], format!("planroom review --base {head} {} accept", ghosts[0].0));
    assert_eq!(seq(&root), head, "nothing was written");
    let (code, env) = review(&root, &ghosts[0].0, "accept");
    assert_eq!(code, 0, "{env}");
}

/// `planroom edit --base HEAD undo EVENT` as the person, signed.
fn undo(root: &Path, event: &str) -> (i32, Value) {
    let base = seq(root).to_string();
    let args = ["--base", base.as_str(), "undo", event];
    let signed = token("edit", &args, &base);
    let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
    run(root, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "")
}

/// The open card `focus` puts face up: its ID and the ghosts its batch lists.
fn face_up(root: &Path) -> Option<(String, Vec<String>)> {
    let env = focus(root);
    let decision = &env["result"]["decision"];
    let id = decision["id"].as_str()?.to_owned();
    let items = decision["batch"]["items"].as_array().unwrap();
    let ghosts = items.iter().map(|i| i["args"][2].as_str().unwrap().to_owned()).collect();
    assert_eq!(decision["batch"]["count"].as_u64(), Some(items.len() as u64), "{env}");
    Some((id, ghosts))
}

#[test]
fn undoing_the_row_that_dealt_the_apply_card_keeps_the_card_for_the_rest() {
    let (_tmp, root) = workspace();
    import(
        &root,
        &[
            rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk"),
            rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner"),
        ],
    );
    let [card] = cards(&root).try_into().unwrap();
    let [first, second] = ghosts_on(&root, &card.id).try_into().unwrap();

    // Undoing the row that dealt the card takes back that row's ghost, not the card: the other
    // row's ghost still waits on it.
    let (code, env) = undo(&root, &first.0);
    assert_eq!(code, 0, "{env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    assert_eq!(face_up(&root), Some((card.id.0.clone(), vec![second.0.clone()])));

    // A later row joins it, and the card counts both.
    import(&root, &[rename_row(&root, "u_bob", "d1", 1, "storefront:container:storefront", "Shop")]);
    assert_eq!(cards(&root).len(), 1, "no second card while the first is open");
    let waiting = ghosts_on(&root, &card.id);
    assert_eq!(waiting.len(), 3);
    let (_, listed) = face_up(&root).unwrap();
    assert_eq!(listed, [second.0.clone(), waiting[2].0.clone()]);

    // A card is picked or resolved, never undone: undoing it would leave its ghosts on no card.
    let head = seq(&root);
    let (code, env) = undo(&root, &card.id.0);
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(seq(&root), head, "nothing was written");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));

    // Its last waiting ghosts resolve it.
    for ghost in [&second, &waiting[2]] {
        let (code, env) = review(&root, &ghost.0, "reject");
        assert_eq!(code, 0, "{env}");
    }
    assert!(open_cards(&root).is_empty());
    assert_eq!(face_up(&root), None);
}

#[test]
fn undoing_a_review_reopens_the_card_its_last_ghost_closed() {
    let (_tmp, root) = workspace();
    import(
        &root,
        &[
            rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk"),
            rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner"),
        ],
    );
    let [card] = cards(&root).try_into().unwrap();
    let [first, last] = ghosts_on(&root, &card.id).try_into().unwrap();
    let (code, env) = review(&root, &first.0, "reject");
    assert_eq!(code, 0, "{env}");
    let rejected = env["result"]["event"].as_str().unwrap().to_owned();
    let (code, env) = review(&root, &last.0, "accept");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());
    assert!(open_cards(&root).is_empty());

    // Undoing the earlier review sets its ghost waiting again, so the card reopens with it, in
    // the store's read and in the fold alike, and the later accept stands.
    let (code, env) = undo(&root, &rejected);
    assert_eq!(code, 0, "{env}");
    let reopening = env["result"]["event"].as_str().unwrap().to_owned();
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));
    assert_eq!(face_up(&root), Some((card.id.0.clone(), vec![first.0.clone()])));
    let (code, env) = review(&root, &card.id.0, "accept");
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["ghosts"], json!([first.0]));

    // Undoing that undo brings the reject back, and the card closes again; undoing it once more
    // reopens it.
    let (code, env) = undo(&root, &reopening);
    assert_eq!(code, 0, "{env}");
    let restoring = env["result"]["event"].as_str().unwrap().to_owned();
    assert!(open_cards(&root).is_empty());
    assert_eq!(face_up(&root), None);
    let (code, env) = undo(&root, &restoring);
    assert_eq!(code, 0, "{env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&card.id));

    // Reviewed again, the ghost resolves the card, accepted since one of its ghosts was.
    let (code, env) = review(&root, &first.0, "reject");
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["card_resolved"], card.id.0.as_str());
    assert!(open_cards(&root).is_empty());
    let picks: Vec<Verdict> = events(&root)
        .into_iter()
        .filter_map(|e| match e.body {
            EventBody::Review { of, verdict } if of == card.id => Some(verdict),
            _ => None,
        })
        .collect();
    assert_eq!(picks.last(), Some(&Verdict::Accept));
}

/// The refusal of an undo that would reopen a card beside the open `open`, whose one waiting ghost
/// is `ghost`: nothing is written, and the hint is the review that runs as it is offered.
fn assert_refused_beside(root: &Path, undoing: &str, open: &EventId, ghost: &EventId) {
    let head = seq(root);
    let (code, env) = undo(root, undoing);
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")), "{env}");
    assert_eq!(env["error"]["details"]["card"], open.0.as_str(), "{env}");
    assert_eq!(env["error"]["hint"], format!("planroom review --base {head} {} accept", ghost.0));
    assert_eq!(seq(root), head, "nothing was written");
    assert_eq!(open_cards(root), std::slice::from_ref(open));
    assert_eq!(face_up(root), Some((open.0.clone(), vec![ghost.0.clone()])));
}

#[test]
fn an_undo_never_reopens_a_card_beside_an_open_one() {
    // An undo of the review that closed the first card, once a later row dealt a second.
    let (_tmp, root) = workspace();
    import(&root, &[rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk")]);
    let [first] = cards(&root).try_into().unwrap();
    let [waited] = ghosts_on(&root, &first.id).try_into().unwrap();
    let (code, env) = review(&root, &waited.0, "reject");
    assert_eq!(code, 0, "{env}");
    let rejected = env["result"]["event"].as_str().unwrap().to_owned();
    import(&root, &[rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner")]);
    let [_, second] = cards(&root).try_into().unwrap();
    let [newer] = ghosts_on(&root, &second.id).try_into().unwrap();
    assert_refused_beside(&root, &rejected, &second.id, &newer);
    // A later row still joins the one open card.
    import(&root, &[rename_row(&root, "u_bob", "d1", 1, "storefront:container:storefront", "Shop")]);
    assert_eq!(cards(&root).len(), 2);
    let [_, joined] = ghosts_on(&root, &second.id).try_into().unwrap();
    // Once the open card resolves, the undo applies and reopens the first card alone.
    for ghost in [&newer, &joined] {
        let (code, env) = review(&root, &ghost.0, "reject");
        assert_eq!(code, 0, "{env}");
    }
    let (code, env) = undo(&root, &rejected);
    assert_eq!(code, 0, "{env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&first.id));
    assert_eq!(face_up(&root), Some((first.id.0.clone(), vec![waited.0.clone()])));

    // An undo of the undo that closed the first card by taking back its only row.
    let (_tmp, root) = workspace();
    import(&root, &[rename_row(&root, "u_ann", "d1", 1, "orders:container:orders", "Desk")]);
    let [first] = cards(&root).try_into().unwrap();
    let [taken] = ghosts_on(&root, &first.id).try_into().unwrap();
    let (code, env) = undo(&root, &taken.0);
    assert_eq!(code, 0, "{env}");
    let taking = env["result"]["event"].as_str().unwrap().to_owned();
    assert!(open_cards(&root).is_empty());
    import(&root, &[rename_row(&root, "u_ann", "d2", 2, "worker:container:worker", "Runner")]);
    let [_, second] = cards(&root).try_into().unwrap();
    let [newer] = ghosts_on(&root, &second.id).try_into().unwrap();
    assert_refused_beside(&root, &taking, &second.id, &newer);
    let (code, env) = review(&root, &newer.0, "accept");
    assert_eq!(code, 0, "{env}");
    let (code, env) = undo(&root, &taking);
    assert_eq!(code, 0, "{env}");
    assert_eq!(open_cards(&root), std::slice::from_ref(&first.id));
}
