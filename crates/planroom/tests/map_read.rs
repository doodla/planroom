//! `map` reads the fold (architecture.md, The fold; DR-61, DR-73, DR-85): one read transaction
//! per reply, a fold that a scan changes, kept between replies in `serve` and caught up there one
//! event at a time, and `--focus`, the blocks under one node.

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use planroom_store::{Shape, Store};
use planroom_testkit::fixture;
use serde_json::{Value, json};

fn planroom(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", root.join(".data"));
    cmd
}

/// A `--json` command's envelope, whatever its exit code.
fn json(root: &Path, args: &[&str]) -> Value {
    let out = planroom(root).args(args).arg("--json").output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"))
}

fn ok(root: &Path, args: &[&str]) -> Value {
    let env = json(root, args);
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    env
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// Add a file to `repo` and commit it; returns the new head.
fn add_file(repo: &Path, file: &str) -> String {
    std::fs::write(repo.join(file), "def audit():\n    return 1\n").unwrap();
    git(repo, &["add", "--", file]);
    git(repo, &["commit", "-q", "-m", "Add a file"]);
    git(repo, &["rev-parse", "HEAD"])
}

/// A top-level block's count of its code-level descendants.
fn code_count(view: &Value, id: &str) -> u64 {
    let node =
        view["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("{id}"));
    node["counts"]["code"].as_u64().unwrap_or(0)
}

/// A scanned fixture workspace.
fn scanned() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    ok(&root, &["scan"]);
    (tmp, root)
}

/// A foreground `serve` on a scanned workspace, killed when dropped, with a signed-in tab.
struct Serve {
    child: Child,
    port: u16,
    token: String,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Serve {
    fn start(root: &Path) -> Serve {
        let mut child = planroom(root).args(["serve", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let env: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(env["ok"], true, "{env}");
        let port = env["result"]["port"].as_u64().unwrap() as u16;
        let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
        let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
        let mut serve = Serve { child, port, token: String::new() };
        let (status, reply) = serve.http("/api/v1/session", &json!({"code": code}).to_string(), false);
        assert_eq!(status, 200, "{reply}");
        serve.token =
            serde_json::from_str::<Value>(&reply).unwrap()["result"]["token"].as_str().unwrap().into();
        serve
    }

    /// One POST from the page's origin; returns the status and the body.
    fn http(&self, path: &str, body: &str, bearer: bool) -> (u16, String) {
        let port = self.port;
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let auth = if bearer { format!("Authorization: Bearer {}\r\n", self.token) } else { String::new() };
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\n{auth}Connection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response.split(' ').nth(1).unwrap().parse().unwrap();
        let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
        (status, body)
    }

    fn call(&self, command: &str, body: Value) -> Value {
        let (status, reply) = self.http(&format!("/api/v1/{command}"), &body.to_string(), true);
        assert_eq!(status, 200, "{command}: {reply}");
        serde_json::from_str(&reply).unwrap()
    }
}

#[test]
fn a_scan_refolds_the_map() {
    let (_tmp, root) = scanned();
    let top = ok(&root, &["map"]);
    let before = code_count(&top["result"], "worker:repo:worker");

    // Through the CLI: each read folds from scratch, and a scan's new snapshot is in the next one.
    add_file(&root.join("worker"), "audit.py");
    let scan = ok(&root, &["scan"]);
    let after = ok(&root, &["map"]);
    assert_eq!(code_count(&after["result"], "worker:repo:worker"), before + 1, "{}", after["summary"]);
    assert_eq!(after["meta"]["seq"], scan["meta"]["seq"]);

    // Through serve, which keeps its fold between replies: a scan it runs, and a scan another
    // process runs, each refold it, and every reply carries the seq of what it folded.
    let serve = Serve::start(&root);
    let kept = serve.call("map", json!({}));
    assert_eq!(code_count(&kept["result"], "worker:repo:worker"), before + 1);
    assert_eq!(kept["meta"]["seq"], scan["meta"]["seq"]);
    add_file(&root.join("worker"), "ledger.py");
    let own = serve.call("scan", json!({}));
    let mapped = serve.call("map", json!({}));
    assert_eq!(code_count(&mapped["result"], "worker:repo:worker"), before + 2, "{}", mapped["summary"]);
    assert_eq!(mapped["meta"]["seq"], own["meta"]["seq"]);
    add_file(&root.join("worker"), "report.py");
    let other = ok(&root, &["scan"]);
    let mapped = serve.call("map", json!({}));
    assert_eq!(code_count(&mapped["result"], "worker:repo:worker"), before + 3, "{}", mapped["summary"]);
    assert_eq!(mapped["meta"]["seq"], other["meta"]["seq"]);
    // A move back to an earlier head is a scan too, and the kept fold follows it.
    git(&root.join("worker"), &["reset", "-q", "--hard", "HEAD~2"]);
    let back = ok(&root, &["scan"]);
    let mapped = serve.call("map", json!({}));
    assert_eq!(code_count(&mapped["result"], "worker:repo:worker"), before + 1, "{}", mapped["summary"]);
    assert_eq!(mapped["meta"]["seq"], back["meta"]["seq"]);
    // The kept fold answers what a fresh fold does.
    let fresh = ok(&root, &["map"]);
    assert_eq!(mapped["result"], fresh["result"]);
}

#[test]
fn map_focus_returns_the_blocks_under_one_node() {
    let (_tmp, root) = scanned();
    // The facts' parents, to check each block against.
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let active = store.active_set().unwrap();
    let mut parents: HashMap<String, Option<String>> = HashMap::new();
    let mut levels: HashMap<String, String> = HashMap::new();
    for row in store.facts(&active.snapshots()).unwrap() {
        if row.shape == Shape::Node {
            let node: Value = serde_json::from_str(&row.body).unwrap();
            parents.insert(row.id.clone(), node["parent"].as_str().map(str::to_owned));
            levels.insert(row.id.clone(), node["level"].as_str().unwrap().to_owned());
        }
    }
    drop(store);
    let under = |id: &str, top: &str| {
        let mut at = parents.get(id).cloned().flatten();
        while let Some(p) = at {
            if p == top {
                return true;
            }
            at = parents.get(&p).cloned().flatten();
        }
        false
    };

    // A directory at the code level: exactly its files, in the view focused on it.
    let src = ok(&root, &["map", "--focus", "kit:dir:src", "--level", "code"]);
    let ids: BTreeSet<String> = src["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .collect();
    let expected: BTreeSet<String> =
        parents.keys().filter(|id| under(id, "kit:dir:src") && levels[*id] == "code").cloned().collect();
    assert!(!expected.is_empty());
    assert_eq!(ids, expected);
    assert_eq!(src["result"]["focus"], "kit:dir:src");
    assert_eq!(src["result"]["level"], "code");
    assert_eq!(
        src["summary"].as_str().unwrap().split(':').next().unwrap(),
        format!("{} under src at the code level", ids.len())
    );

    // A repo with no level: the level below its own, every block inside it, none drawn twice.
    let repo = ok(&root, &["map", "--focus", "storefront:repo:storefront"]);
    assert_eq!(repo["result"]["level"], "container");
    let nodes = repo["result"]["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|n| n["id"] == "storefront:container:storefront"), "{repo}");
    for node in nodes {
        let id = node["id"].as_str().unwrap();
        assert!(under(id, "storefront:repo:storefront"), "{id} is under the repo");
        assert!(!nodes.iter().any(|n| under(id, n["id"].as_str().unwrap())), "{id} sits in no other block");
    }
    for edge in repo["result"]["edges"].as_array().unwrap() {
        assert!(nodes.iter().any(|n| n["id"] == edge["from"]) && nodes.iter().any(|n| n["id"] == edge["to"]));
    }

    // A name or an ID suffix names a node when only one has it.
    let by_name = ok(&root, &["map", "--focus", "kit", "--level", "code"]);
    assert_eq!(by_name["result"]["focus"], "kit:repo:kit");
    let by_suffix = ok(&root, &["map", "--focus", "repo:kit", "--level", "code"]);
    assert_eq!(by_suffix["result"], by_name["result"]);

    // A leaf has nothing under it: an empty state whose next action shows it among its peers.
    let leaf = ok(&root, &["map", "--focus", "kit:file:src/money.ts"]);
    let empty = &leaf["result"]["empty"];
    assert_eq!(empty["reason"], "nothing_under", "{leaf}");
    assert_eq!(leaf["summary"], empty["line"]);
    let mut next = vec![empty["next"]["command"].as_str().unwrap()];
    next.extend(empty["next"]["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()));
    assert_eq!(next[0], "map");
    let peers = ok(&root, &next);
    assert!(
        peers["result"]["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "kit:file:src/money.ts"),
        "{peers}"
    );

    // A target that names no node is not_found, and one that names several is a usage error.
    let missing = json(&root, &["map", "--focus", "no-such-block"]);
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    let several = json(&root, &["map", "--focus", "src"]);
    assert_eq!(several["error"]["code"], "invalid_argument", "{several}");
    assert_eq!(several["error"]["details"]["field"], "focus");
}

#[test]
fn one_reply_reads_one_transaction() {
    let (_tmp, root) = scanned();
    let worker = root.join("worker");
    let base = git(&worker, &["rev-parse", "HEAD"]);
    let plain = code_count(&ok(&root, &["map"])["result"], "worker:repo:worker");
    let added = add_file(&worker, "audit.py");
    // The seq each scan made, and the worker's code count it maps.
    let scans: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let first = ok(&root, &["scan"]);
    scans.lock().unwrap().push((first["meta"]["seq"].as_u64().unwrap(), plain + 1));

    // One process moves the worker back and forth and scans; another reads the map meanwhile.
    let done = Arc::new(AtomicBool::new(false));
    let writer = {
        let (root, worker, scans, done) = (root.clone(), worker.clone(), scans.clone(), done.clone());
        std::thread::spawn(move || {
            for i in 0..8 {
                let (head, count) = if i % 2 == 0 { (&base, plain) } else { (&added, plain + 1) };
                git(&worker, &["reset", "-q", "--hard", head]);
                let scan = ok(&root, &["scan"]);
                scans.lock().unwrap().push((scan["meta"]["seq"].as_u64().unwrap(), count));
            }
            done.store(true, Ordering::SeqCst);
        })
    };
    let mut replies = Vec::new();
    while !done.load(Ordering::SeqCst) {
        let reply = ok(&root, &["map"]);
        replies.push((
            reply["meta"]["seq"].as_u64().unwrap(),
            code_count(&reply["result"], "worker:repo:worker"),
        ));
    }
    writer.join().unwrap();

    // Every reply shows exactly the map at its own seq: the last scan at or below it.
    let scans = scans.lock().unwrap().clone();
    assert!(replies.len() > 1);
    for (seq, count) in &replies {
        let expected = scans.iter().filter(|(at, _)| at <= seq).max_by_key(|(at, _)| *at).map(|(_, c)| *c);
        assert_eq!(
            Some(*count),
            expected,
            "the reply at seq {seq} shows the map at that seq; scans {scans:?}"
        );
    }
}

/// Another writer, on its own connection to the store: it moves one of the worker's files to the
/// top of the tree and back, over and over, a fraction of a millisecond apart, until it is
/// stopped, and logs each move's seq and whether the file then sat at the top. At the top the file
/// is a block of its own at every level, so the top of the map and the container level both show
/// one block more.
struct Mover {
    file: String,
    done: Arc<AtomicBool>,
    log: Arc<Mutex<Vec<(u64, bool)>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Mover {
    fn start(root: &Path) -> Mover {
        use planroom_contract::{
            Actor, Client, Edit, EventBody, EventId, Layer, NodeId, Op, Status, Surface, Target, Timestamp,
        };
        use planroom_store::{NewEvent, Write};

        let mut store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
        // A file of the worker's, and the parent it was found under.
        let active = store.active_set().unwrap();
        let (file, parent) = store
            .facts(&active.snapshots())
            .unwrap()
            .into_iter()
            .filter(|row| row.shape == Shape::Node && row.id.starts_with("worker:"))
            .find_map(|row| {
                let node: Value = serde_json::from_str(&row.body).unwrap();
                let parent = node["parent"].as_str()?.to_owned();
                (node["level"] == "code").then_some((row.id, parent))
            })
            .expect("a file of the worker's");
        let (done, log) = (Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(Vec::new())));
        let thread = {
            let (file, done, log) = (file.clone(), done.clone(), log.clone());
            std::thread::spawn(move || {
                let mut n = 0u64;
                while !done.load(Ordering::SeqCst) {
                    n += 1;
                    let top = n % 2 == 1;
                    let to = if top { Value::Null } else { json!(parent) };
                    let edit = Edit {
                        op: Op::Set,
                        target: Target::Node(NodeId(file.clone())),
                        layer: Layer::Actual,
                        fields: [("parent".to_owned(), to)].into(),
                        base: 0,
                        over: Default::default(),
                    };
                    let event = NewEvent {
                        id: EventId(format!("move-{n}")),
                        at: Timestamp("2026-10-27T00:00:00Z".into()),
                        actor: Actor::System { part: "test".into() },
                        client: Client { surface: Surface::Test, id: "test".into() },
                        project: None,
                        cause: None,
                        status: Status::Applied,
                        target: Some(file.clone()),
                        body: EventBody::Edit(edit),
                    };
                    let write =
                        Write { idem: format!("move-{n}"), request_hash: "h".into(), events: vec![event] };
                    let committed = store.commit(write, |seqs| json!({"seq": seqs[0]})).unwrap();
                    log.lock().unwrap().push((committed.seqs[0], top));
                    // A gap between writes, so the commands under test take the write lock too.
                    std::thread::sleep(std::time::Duration::from_micros(300));
                }
            })
        };
        Mover { file, done, log, thread: Some(thread) }
    }

    /// Stop moving; returns every move's seq and whether it put the file at the top, in order.
    fn stop(mut self) -> Vec<(u64, bool)> {
        self.done.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.log.lock().unwrap().clone()
    }
}

/// Whether the moved file sat at the top at `seq`: as the last move at or below it left it.
fn on_top_at(moves: &[(u64, bool)], seq: u64) -> bool {
    moves.iter().rev().find(|(at, _)| *at <= seq).is_some_and(|(_, top)| *top)
}

fn holds(nodes: &Value, id: &str) -> bool {
    nodes.as_array().unwrap().iter().any(|n| n["id"] == id)
}

#[test]
fn a_scan_reply_reads_its_map_and_seq_in_one_transaction() {
    let (_tmp, root) = scanned();
    let containers = ok(&root, &["map", "--level", "container"])["result"]["nodes"].as_array().unwrap().len();
    let mover = Mover::start(&root);
    let mut replies = Vec::new();
    for _ in 0..20 {
        replies.push(ok(&root, &["scan"]));
    }
    let file = mover.file.clone();
    let moves = mover.stop();
    assert!(!moves.is_empty());

    // Each scan's map and its summary's container count hold no write past its seq, and every
    // write at or below it.
    for scan in &replies {
        let seq = scan["meta"]["seq"].as_u64().unwrap();
        let top = on_top_at(&moves, seq);
        assert_eq!(
            holds(&scan["result"]["map"]["nodes"], &file),
            top,
            "the map in the scan reply at seq {seq}"
        );
        let summary = scan["summary"].as_str().unwrap();
        let counted: usize =
            summary.split(": ").nth(1).and_then(|s| s.split(' ').next()).unwrap().parse().unwrap();
        assert_eq!(counted, containers + usize::from(top), "the containers in {summary:?} at seq {seq}");
    }
}

#[test]
fn a_share_snapshot_reads_its_map_pane_and_seq_in_one_transaction() {
    let (_tmp, root) = scanned();
    let mover = Mover::start(&root);
    let mut snapshots = Vec::new();
    for _ in 0..20 {
        ok(&root, &["share", "--out", ".planroom/share"]);
        let path = root.join(".planroom/share").join(planroom_contract::remote::SNAPSHOT_PATH);
        snapshots.push(serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap());
    }
    let file = mover.file.clone();
    let moves = mover.stop();
    assert!(!moves.is_empty());

    // Each snapshot's map and pane come from the version of the store at the seq it records.
    for snapshot in &snapshots {
        let seq = snapshot["seq"].as_u64().unwrap();
        let top = on_top_at(&moves, seq);
        assert_eq!(holds(&snapshot["maps"][0]["nodes"], &file), top, "the map in the snapshot at seq {seq}");
        assert_eq!(snapshot["pane"]["focus"]["seq"], seq, "the pane in the snapshot at seq {seq}");
        assert_eq!(
            holds(&snapshot["pane"]["minimap"]["blocks"], &file),
            top,
            "the pane's mini-map in the snapshot at seq {seq}"
        );
    }
}
