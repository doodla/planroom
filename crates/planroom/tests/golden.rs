//! Golden CLI outputs from real runs on the built fixture, which the mod's and the canvas's tests
//! read (DR-6, DR-79): `tests/golden/cli/focus.json`, `tests/golden/cli/focus-no-repos.json`,
//! `tests/golden/cli/focus-plan-only.json`, `tests/golden/cli/map-container.json`,
//! `tests/golden/cli/map-graph.json`,
//! `tests/golden/cli/map-empty.json`, `tests/golden/cli/watch-frames.json` and
//! `tests/golden/cli/invalid-argument.json`, a wrong `--detail` with its synopsis and schema handle,
//! and the three pickers `focus --picker lens|project|stage` returns: `focus-picker-lens.json`,
//! `focus-picker-project.json` and `focus-picker-stage.json`, the last two over a plan project an
//! agent started.
//! `focus-no-repos.json` is `focus` in an empty directory, not yet a workspace, whose next action
//! is `schema --command "intent new"`, which starts a plan; `focus-plan-only.json` is `focus` in a
//! directory where an agent started a plan and no repo exists: `plan_only`, whose next action is
//! `intent list`; `map-container.json` is the scanned fixture's container level, which the
//! pane's next action opens.
//!
//! A plain run compares the engine's output with the committed files. `cargo xtask gen --golden`
//! runs this test with `PLANROOM_GOLDEN=write`, which writes them instead. The one value a run
//! can't repeat, `meta.took_ms`, is left out, and `meta.size` is measured again without it.
//! `watch-frames.json` keeps the lines `watch --json` printed as one JSON array, one frame per
//! NDJSON line in order, so the mod's tests can import it; the commits the run makes carry a
//! fixed author and date, as the fixture's own do, so the commit each frame names repeats. The
//! run watches as the mod does, for a session from `PLANROOM_CLIENT=mod:5e55i0n1` as the OS user
//! `you`, so its own presence frame follows the replay; that frame's `seen_at`, the other value a
//! run can't repeat, is written as `2026-01-01T00:00:00.000000Z`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use planroom_contract::{MapView, PaneModel, Position, StreamFrame};
use planroom_store::Store;
use planroom_testkit::{fixture, repo_root};
use serde_json::Value;

fn run(root: &Path, args: &[&str]) -> Value {
    let (env, ok) = run_as(root, args, &[]);
    assert!(ok, "{args:?}: {env}");
    env
}

/// One run's envelope and whether it exited 0, with `vars` set; a failure is a reply too.
fn run_as(root: &Path, args: &[&str], vars: &[(&str, &str)]) -> (Value, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_CLIENT")
        .envs(vars.iter().copied())
        .output()
        .expect("the binary runs");
    let env = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{args:?}: {e}: {}", String::from_utf8_lossy(&out.stdout)));
    (env, out.status.success())
}

/// One golden: its file, the directory its run used, the engine's raw envelope (`Null` for the
/// watch frames) and the text the file holds.
struct Golden {
    name: &'static str,
    cwd: PathBuf,
    raw: Value,
    text: String,
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
    for golden in golden_runs(tmp.path()) {
        check(golden.name, golden.text);
    }
}

/// The handle round trip's engine half (contracts.md, "Mod tool schemas"): every handle in every
/// golden reply, from the raw runs on the same fixture rather than the files `gen --golden` may be
/// writing, runs again and answers `ok`. A read runs as it is; a write runs as an agent, with a
/// fresh `--idem`, as the mod's tool handler would run it.
#[test]
fn every_golden_handle_runs_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let mut ran = 0;
    for golden in golden_runs(tmp.path()) {
        for (command, args) in handles(&golden.raw) {
            let (reply, exited_ok) =
                run_handle(&golden.cwd, &command, &args, &format!("golden-handle-{ran}"));
            assert!(ran_ok(&reply, exited_ok), "{}: {command} {args:?}: {reply}", golden.name);
            ran += 1;
        }
    }
    assert!(ran > 0, "the goldens carry handles");
}

/// A write handle whose leaf has a subcommand runs as the round trip runs it: `edit`'s reply and
/// its `undo` handle, which carries `--base` before the subcommand, both answer `ok`. No golden
/// holds such a handle, so this one comes from an edit on the scanned fixture.
#[test]
fn an_edit_and_its_undo_handle_run_as_the_round_trip_runs_them() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let scanned = run(&root, &["scan", "--json"]);
    let base = scanned["meta"]["seq"].as_u64().unwrap().to_string();
    let block = scanned["result"]["map"]["nodes"][0]["id"].as_str().unwrap().to_owned();
    let args: Vec<String> = ["--base", &base, "rename", &block, "Shopfront"].map(str::to_owned).into();
    let (edited, exited_ok) = run_handle(&root, "edit", &args, "an-edit");
    assert!(ran_ok(&edited, exited_ok), "{edited}");
    let undo = handles(&edited["result"]["undo"]);
    assert_eq!(undo.len(), 1, "an edit names its undo: {edited}");
    let (command, args) = &undo[0];
    let (undone, exited_ok) = run_handle(&root, command, args, "its-undo");
    assert!(ran_ok(&undone, exited_ok), "{command} {args:?}: {undone}");
}

/// Whether a handle ran: its reply says `ok` and the process exited 0.
fn ran_ok(reply: &Value, exited_ok: bool) -> bool {
    exited_ok && reply["ok"] == true
}

/// A reply that says `ok` from a process that exited nonzero is no successful run, nor is a
/// clean exit whose reply isn't `ok`.
#[test]
fn an_ok_reply_with_a_nonzero_exit_is_no_successful_run() {
    assert!(ran_ok(&serde_json::json!({"ok": true}), true));
    assert!(!ran_ok(&serde_json::json!({"ok": true}), false));
    assert!(!ran_ok(&serde_json::json!({"ok": false}), true));
}

/// Run one handle on `cwd` as the mod's tool handler would: a read as it is; a write as an agent,
/// with a fresh `--idem` when its leaf's own flags take one, right after the leaf's path and
/// before any subcommand, as the handler puts it. The reply, whatever the exit, and whether it
/// exited 0.
fn run_handle(cwd: &Path, command: &str, args: &[String], idem: &str) -> (Value, bool) {
    let spec = planroom_contract::Command::SPECS.iter().find(|s| s.name() == command);
    let spec = spec.unwrap_or_else(|| panic!("{command} is no catalogue leaf"));
    if spec.spends {
        // A leaf that can spend isn't run: its arguments must parse, as the picker's stage handles
        // do, and that is all a free, offline test may check.
        let word = command.rsplit(' ').next().unwrap_or(command);
        let parsed =
            spec.command().try_get_matches_from(std::iter::once(word.to_owned()).chain(args.iter().cloned()));
        assert!(parsed.is_ok(), "{command} {args:?} doesn't parse: {:?}", parsed.err());
        return (serde_json::json!({"ok": true}), true);
    }
    let mut argv: Vec<String> = command.split(' ').map(str::to_owned).collect();
    let mut vars = vec![];
    if spec.class != planroom_contract::Class::Read {
        if spec.flags().iter().any(|f| f.sub.is_none() && f.flag == "--idem") {
            argv.extend(["--idem".to_owned(), idem.to_owned()]);
        }
        vars.push(("PLANROOM_ACTOR", "agent:5e55i0n1:chat"));
    }
    argv.extend(args.iter().cloned());
    argv.push("--json".into());
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    run_as(cwd, &argv, &vars)
}

/// Every handle in an envelope, at any depth, as its command and arguments.
fn handles(value: &Value) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    match value {
        Value::Array(items) => items.iter().for_each(|v| out.extend(handles(v))),
        Value::Object(map) => {
            if let (Some(Value::String(command)), Some(Value::Array(args)), Some(Value::String(_))) =
                (map.get("command"), map.get("args"), map.get("label"))
            {
                let args: Vec<String> = args.iter().filter_map(|a| a.as_str().map(str::to_owned)).collect();
                out.push((command.clone(), args));
            }
            map.values().for_each(|v| out.extend(handles(v)));
        }
        _ => {}
    }
    out
}

/// The runs every golden comes from, on a fixture built under `tmp`, in order.
fn golden_runs(tmp: &Path) -> Vec<Golden> {
    let mut goldens = Vec::new();
    let root = tmp.join("shop");
    fixture(&root);
    let mut keep = |name: &'static str, cwd: &Path, raw: Value, text: String| {
        goldens.push(Golden { name, cwd: cwd.to_path_buf(), raw, text });
    };
    let empty = run(&root, &["map", "--detail", "graph", "--json"]);
    let unscanned: MapView = serde_json::from_value(empty["result"].clone()).expect("map returns a MapView");
    assert!(unscanned.nodes.is_empty() && unscanned.empty.is_some(), "an unscanned workspace maps empty");
    keep("map-empty.json", &root, empty.clone(), normalized(rooted(empty, &root)));

    // An empty directory is no workspace and holds no repo: the pane's empty state offers to start
    // a plan.
    let bare = tmp.join("bare");
    std::fs::create_dir(&bare).unwrap();
    let no_repos = run(&bare, &["focus", "--json"]);
    assert_eq!(no_repos["result"]["next"]["command"], "schema", "{no_repos}");
    keep("focus-no-repos.json", &bare, no_repos.clone(), normalized(rooted(no_repos, &bare)));

    // An agent starts a plan in an empty directory: the workspace holds a plan and no repo.
    let greenfield = tmp.join("greenfield");
    std::fs::create_dir(&greenfield).unwrap();
    let (begun, ok) = run_as(
        &greenfield,
        &["intent", "new", "Add a billing service", "--json"],
        &[("PLANROOM_ACTOR", "agent:aaaaaaaa:chat")],
    );
    assert!(ok, "{begun}");
    let plan_only = run(&greenfield, &["focus", "--json"]);
    assert_eq!(plan_only["result"]["empty"]["reason"], "plan_only", "{plan_only}");
    keep("focus-plan-only.json", &greenfield, plan_only.clone(), normalized(rooted(plan_only, &greenfield)));

    run(&root, &["scan", "--json"]);

    // A wrong value: the `invalid_argument` envelope with the leaf's synopsis and schema handle,
    // which a tool returns whole.
    let (wrong, ok) = run_as(&root, &["map", "--detail", "nope", "--json"], &[]);
    assert!(!ok && wrong["error"]["code"] == "invalid_argument", "{wrong}");
    keep("invalid-argument.json", &root, wrong.clone(), normalized(rooted(wrong, &root)));

    let container = run(&root, &["map", "--level", "container", "--json"]);
    keep("map-container.json", &root, container.clone(), normalized(container));

    let focus = run(&root, &["focus", "--json"]);
    let _: PaneModel = serde_json::from_value(focus["result"].clone()).expect("focus returns a PaneModel");
    keep("focus.json", &root, focus.clone(), normalized(focus));

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
    keep("map-graph.json", &root, graph.clone(), normalized(graph));

    let frames = watch_frames(&root, scanned);
    keep("watch-frames.json", &root, Value::Null, frames);

    // The pickers: the lens picker on the scanned workspace; an agent starts a plan project, which
    // waits as a ghost; then the project and stage pickers over it.
    let lens = run(&root, &["focus", "--picker", "lens", "--json"]);
    keep("focus-picker-lens.json", &root, lens.clone(), normalized(lens));
    let (started, ok) = run_as(
        &root,
        &["intent", "new", "Add retries to the worker", "--idem", "golden-project", "--json"],
        &[("PLANROOM_ACTOR", "agent:5e55i0n1:chat")],
    );
    assert!(ok, "{started}");
    let project = run(&root, &["focus", "--picker", "project", "--json"]);
    keep("focus-picker-project.json", &root, project.clone(), normalized(project));
    let stage = run(&root, &["focus", "--picker", "stage", "--json"]);
    keep("focus-picker-stage.json", &root, stage.clone(), normalized(stage));
    goldens
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
        .env("PLANROOM_CLIENT", "mod:5e55i0n1")
        .env("USER", "you")
        .env("LOGNAME", "you")
        .env_remove("PLANROOM_ACTOR")
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
    let mut frames: Vec<Value> = lines.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    for row in frames.iter_mut().filter_map(|f| f.get_mut("actors")?.as_array_mut()).flatten() {
        row["seen_at"] = Value::from("2026-01-01T00:00:00.000000Z");
    }
    serde_json::to_string(&rooted(Value::Array(frames), root)).unwrap() + "\n"
}
