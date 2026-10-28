//! Golden CLI outputs from real runs on the built fixture, which the mod's and the canvas's tests
//! read (DR-6, DR-79): `tests/golden/cli/focus.json`, `tests/golden/cli/focus-no-repos.json`,
//! `tests/golden/cli/map-container.json`, `tests/golden/cli/map-graph.json`,
//! `tests/golden/cli/map-empty.json` and `tests/golden/cli/watch-frames.json`.
//! `focus-no-repos.json` is `focus` in an empty directory, not yet a workspace, whose next action
//! is `focus` again; `map-container.json` is the scanned fixture's container level, which the
//! pane's next action opens.
//!
//! A plain run compares the engine's output with the committed files. `cargo xtask gen --golden`
//! runs this test with `PLANROOM_GOLDEN=write`, which writes them instead. The one value a run
//! can't repeat, `meta.took_ms`, is left out, and `meta.size` is measured again without it.
//! `watch-frames.json` keeps the lines `watch --json` printed as one JSON array, one frame per
//! NDJSON line in order, so the mod's tests can import it; the commits the run makes carry a
//! fixed author and date, as the fixture's own do, so the commit each frame names repeats.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use planroom_contract::{MapView, PaneModel, Position, StreamFrame};
use planroom_store::Store;
use planroom_testkit::{fixture, repo_root};
use serde_json::Value;

fn run(root: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_CLIENT")
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// The envelope without `meta.took_ms`, with `meta.size` measuring the line that is left, as the
/// engine measures its own.
fn normalized(mut env: Value) -> String {
    env["meta"].as_object_mut().unwrap().remove("took_ms");
    let mut line = env.to_string();
    for _ in 0..4 {
        let bytes = line.len() as u64;
        let size = serde_json::json!({"bytes": bytes, "tokens_est": bytes.div_ceil(4)});
        if env["meta"]["size"] == size {
            break;
        }
        env["meta"]["size"] = size;
        line = env.to_string();
    }
    line + "\n"
}

/// `value` with every string naming the temporary workspace root rewritten to `/home/you/shop`,
/// so a golden doesn't carry the test's own temp path. The canonical spelling goes first: on macOS
/// the temp directory is a link, and the engine reports `/private/var/...` for `/var/...`.
fn rooted(value: Value, root: &Path) -> Value {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let spellings = [canonical.to_string_lossy().into_owned(), root.to_string_lossy().into_owned()];
    rewrite(value, &spellings)
}

fn rewrite(value: Value, spellings: &[String]) -> Value {
    match value {
        Value::String(s) => {
            Value::String(spellings.iter().fold(s, |s, from| s.replace(from.as_str(), "/home/you/shop")))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(|v| rewrite(v, spellings)).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, rewrite(v, spellings))).collect())
        }
        other => other,
    }
}

fn check(name: &str, output: String) {
    let path = repo_root().join("tests/golden/cli").join(name);
    if std::env::var("PLANROOM_GOLDEN").as_deref() == Ok("write") {
        std::fs::write(&path, &output).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == output,
        "{} differs from the engine's output; run cargo xtask gen --golden\nengine:    {output}committed: {committed}",
        path.display()
    );
}

#[test]
fn golden_outputs_equal_engine_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let empty = run(&root, &["map", "--detail", "graph", "--json"]);
    let unscanned: MapView = serde_json::from_value(empty["result"].clone()).expect("map returns a MapView");
    assert!(unscanned.nodes.is_empty() && unscanned.empty.is_some(), "an unscanned workspace maps empty");
    check("map-empty.json", normalized(rooted(empty, &root)));

    // An empty directory is no workspace and holds no repo: the pane's empty state checks again.
    let bare = tmp.path().join("bare");
    std::fs::create_dir(&bare).unwrap();
    let no_repos = run(&bare, &["focus", "--json"]);
    assert_eq!(no_repos["result"]["next"]["command"], "focus", "{no_repos}");
    check("focus-no-repos.json", normalized(rooted(no_repos, &bare)));

    run(&root, &["scan", "--json"]);

    check("map-container.json", normalized(run(&root, &["map", "--level", "container", "--json"])));

    let focus = run(&root, &["focus", "--json"]);
    let _: PaneModel = serde_json::from_value(focus["result"].clone()).expect("focus returns a PaneModel");
    check("focus.json", normalized(focus));

    // Cache a layout the way the canvas does, so the graph carries positions.
    let unplaced = run(&root, &["map", "--detail", "graph", "--json"]);
    let view: MapView = serde_json::from_value(unplaced["result"].clone()).expect("map returns a MapView");
    let key = view.layout.expect("--detail graph names its layout key");
    let positions: BTreeMap<_, _> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), Position { x: (i % 5) as f64 * 240.0, y: (i / 5) as f64 * 160.0 }))
        .collect();
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    store
        .put_layout(&key.view_key, &key.structure_hash, &serde_json::to_string(&positions).unwrap())
        .unwrap();
    drop(store);

    let graph = run(&root, &["map", "--detail", "graph", "--json"]);
    let placed: MapView = serde_json::from_value(graph["result"].clone()).unwrap();
    assert!(placed.nodes.iter().all(|n| n.position == positions.get(&n.id).copied()));
    let scanned = graph["meta"]["seq"].as_u64().unwrap();
    check("map-graph.json", normalized(graph));

    check("watch-frames.json", watch_frames(&root, scanned));
}

/// A commit in a fixture repo with a fixed author, committer and date, so its id repeats.
fn fixture_commit(repo: &Path, file: &str, text: &str, subject: &str) {
    let path = repo.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    for args in [&["add", "--", file][..], &["-c", "commit.gpgsign=false", "commit", "-q", "-m", subject]] {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Carol Diaz")
            .env("GIT_AUTHOR_EMAIL", "carol@example.test")
            .env("GIT_AUTHOR_DATE", "2026-09-23T09:00:00Z")
            .env("GIT_COMMITTER_NAME", "Carol Diaz")
            .env("GIT_COMMITTER_EMAIL", "carol@example.test")
            .env("GIT_COMMITTER_DATE", "2026-09-23T09:00:00Z")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// `watch --json --since <two before the scan's seq>`, as the mod starts it: the replay of the
/// first scan's last two events, then the live frames of a rescan after commits in two repos.
fn watch_frames(root: &Path, scanned: u64) -> String {
    let since = scanned - 2;
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["watch", "--json", "--since", &since.to_string(), "--session", "5e55i0n1"])
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_CLIENT")
        .stdout(Stdio::piped())
        .spawn()
        .expect("watch runs");
    let out = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    // Read until a frame reaches `until`; every line must be one frame.
    let read_to = |until: u64, lines: &mut Vec<String>| loop {
        let line = rx.recv_timeout(Duration::from_secs(20)).expect("watch prints a frame within 20 s");
        let frame: StreamFrame = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
        lines.push(line);
        if let StreamFrame::Event { seq, .. } | StreamFrame::Reset { seq } = frame
            && seq >= until
        {
            return;
        }
    };
    let mut lines = Vec::new();
    read_to(scanned, &mut lines);

    fixture_commit(
        &root.join("worker"),
        "worker/audit.py",
        "def record(job_id):\n    return {\"job\": job_id}\n",
        "Add an audit record helper",
    );
    fixture_commit(&root.join("orders"), "README.md", "# orders\n\nThe order service.\n", "Trim the README");
    let rescan = run(root, &["scan", "--json"]);
    read_to(rescan["meta"]["seq"].as_u64().unwrap(), &mut lines);

    child.kill().unwrap();
    let _ = child.wait();
    drop(rx);
    reader.join().unwrap();
    let frames: Vec<Value> = lines.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    serde_json::to_string(&rooted(Value::Array(frames), root)).unwrap() + "\n"
}
