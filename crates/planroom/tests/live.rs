//! WS4: the live loop under load (DR-1). `planroom watch --json` polls `data_version` on its own
//! read-only connection while three writer processes commit 500 events each and a rescan commits
//! its snapshot and link; every commit reaches a frame within 1 s. In one process, the store's
//! commit hook sends the process's own commits through its live loop with no poll.
//!
//! The writers are this test binary run again with `WS4_LIVE_STORE` set. The measured numbers go
//! to `target/tmp/ws4-live.json` for docs/contributing/research/ws4-concurrency.md.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use planroom::clock::unix_micros;
use planroom::live::Live;
use planroom_contract::{Actor, Client, EventBody, EventId, Status, StreamFrame, Surface, Timestamp};
use planroom_store::{NewEvent, Store, StoreError, Tail, Write};
use planroom_testkit::fixture;
use serde_json::{Value, json};

const WRITERS: usize = 3;
const COMMITS: usize = 500;

/// The two measuring tests run one at a time, so neither loads the other's numbers.
static SERIAL: Mutex<()> = Mutex::new(());

fn version_write(writer: &str, i: usize) -> Write {
    Write {
        idem: format!("{writer}-{i}"),
        request_hash: "h".into(),
        events: vec![NewEvent {
            id: EventId(format!("{writer}-{i}")),
            at: Timestamp(unix_micros().to_string()),
            actor: Actor::System { part: format!("ws4:{writer}") },
            client: Client { surface: Surface::Test, id: writer.into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: format!("{writer}-{i}") },
        }],
    }
}

/// A commit, retried on `store_busy`; returns the seq and how many retries it took.
fn commit(store: &mut Store, write: &Write) -> (u64, u64) {
    let mut busy = 0;
    loop {
        match store.commit(write.clone(), |_| Value::Null) {
            Ok(done) => return (done.seqs[0], busy),
            Err(StoreError::Busy) => busy += 1,
            Err(e) => panic!("{e}"),
        }
    }
}

/// One writer process: `WS4 <seq> <committed at µs> <busy retries>` per commit.
#[test]
fn live_writer_child() {
    let Ok(path) = std::env::var("WS4_LIVE_STORE") else { return };
    let writer = format!("writer:{}", std::env::var("WS4_WRITER").unwrap());
    let start = PathBuf::from(std::env::var("WS4_START").unwrap());
    let mut store = Store::open_existing(Path::new(&path)).unwrap().unwrap();
    while !start.exists() {
        std::thread::sleep(Duration::from_millis(1));
    }
    for i in 0..COMMITS {
        let began = Instant::now();
        let (seq, busy) = commit(&mut store, &version_write(&writer, i));
        let took = began.elapsed().as_micros();
        println!("WS4 {seq} {} {busy} {took}", unix_micros());
    }
}

fn percentiles(mut v: Vec<u64>) -> Value {
    v.sort_unstable();
    let at = |p: f64| v[((v.len() as f64 - 1.0) * p).round() as usize];
    json!({"n": v.len(), "p50": at(0.5), "p95": at(0.95), "p99": at(0.99), "max": v[v.len() - 1]})
}

fn git(dir: &Path, args: &[&str]) {
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
}

fn report(key: &str, value: Value) {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ws4-live.json");
    let _guard = SERIAL_FILE.lock().unwrap();
    let mut all: BTreeMap<String, Value> =
        std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    all.insert(key.into(), value);
    std::fs::write(&path, serde_json::to_string_pretty(&all).unwrap()).unwrap();
}
static SERIAL_FILE: Mutex<()> = Mutex::new(());

#[test]
fn watch_frames_every_commit_of_three_writers_and_a_rescan_within_1_s() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    let bin = env!("CARGO_BIN_EXE_planroom");
    let scanned = Command::new(bin).args(["scan", ".", "--json"]).current_dir(&root).output().unwrap();
    assert!(scanned.status.success());
    let store = root.join(".planroom/store.db");
    let base = Store::open_existing(&store).unwrap().unwrap().seq().unwrap();

    // The watcher: every frame's seq and when it arrived.
    let mut watch = Command::new(bin)
        .args(["watch", "--json"])
        .current_dir(&root)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let frames: Arc<Mutex<Vec<(u64, u64)>>> = Arc::default();
    let reader = {
        let frames = frames.clone();
        let out = watch.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                let at = unix_micros();
                match serde_json::from_str::<StreamFrame>(&line).unwrap() {
                    StreamFrame::Ping => {
                        let _ = ready_tx.send(());
                    }
                    StreamFrame::Event { seq, .. } => frames.lock().unwrap().push((seq, at)),
                    StreamFrame::Reset { seq } => panic!("unexpected reset at {seq}"),
                }
            }
        })
    };
    ready_rx.recv_timeout(Duration::from_secs(10)).expect("watch starts with a ping");

    // Three writers, released together.
    let start = dir.path().join("start");
    let exe = std::env::current_exe().unwrap();
    let writers: Vec<_> = (0..WRITERS)
        .map(|w| {
            Command::new(&exe)
                .args(["--exact", "live_writer_child", "--nocapture", "--test-threads", "1"])
                .env("WS4_LIVE_STORE", &store)
                .env("WS4_WRITER", w.to_string())
                .env("WS4_START", &start)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(200));
    let began = Instant::now();
    std::fs::write(&start, "go").unwrap();

    // Meanwhile a head moves and a rescan commits its snapshot, facts and link among the writers.
    std::thread::sleep(Duration::from_millis(50));
    git(
        &root.join("worker"),
        &["-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "Touch"],
    );
    let scan_began = Instant::now();
    let rescan = Command::new(bin).args(["scan", ".", "--json"]).current_dir(&root).output().unwrap();
    let scan_wall = scan_began.elapsed();
    let rescan: Value = serde_json::from_slice(&rescan.stdout).unwrap();
    assert_eq!(rescan["ok"], true, "{rescan}");

    let mut commits: Vec<(u64, u64, u64, u64)> = Vec::new();
    for mut child in writers {
        let out = child.stdout.take().unwrap();
        for line in BufReader::new(out).lines() {
            let line = line.unwrap();
            let Some(rest) = line.find("WS4 ").map(|at| &line[at + 4..]) else { continue };
            let f: Vec<u64> = rest.split(' ').map(|x| x.parse().unwrap()).collect();
            commits.push((f[0], f[1], f[2], f[3]));
        }
        assert!(child.wait().unwrap().success());
    }
    let wall = began.elapsed();
    let last = base + (WRITERS * COMMITS) as u64 + 1;
    let deadline = Instant::now() + Duration::from_secs(10);
    while frames.lock().unwrap().last().is_none_or(|f| f.0 < last) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    watch.kill().unwrap();
    let _ = watch.wait();
    reader.join().unwrap();

    let frames = frames.lock().unwrap().clone();
    let seqs: Vec<u64> = frames.iter().map(|f| f.0).collect();
    assert_eq!(seqs, ((base + 1)..=last).collect::<Vec<_>>(), "every commit once, in order");
    assert_eq!(commits.len(), WRITERS * COMMITS);
    let arrived: BTreeMap<u64, u64> = frames.into_iter().collect();
    let latency_us: Vec<u64> =
        commits.iter().map(|(seq, at, _, _)| arrived[seq].saturating_sub(*at)).collect();
    let busy: u64 = commits.iter().map(|c| c.2).sum();
    let summary = json!({
        "writers": WRITERS,
        "commits_per_writer": COMMITS,
        "wall_ms": wall.as_millis() as u64,
        "commits_per_s": (WRITERS * COMMITS) as f64 / wall.as_secs_f64(),
        "commit_to_frame_us": percentiles(latency_us.clone()),
        "busy_retries": busy,
        "commit_us": percentiles(commits.iter().map(|c| c.3).collect()),
        "rescan_during_load": {"wall_ms": scan_wall.as_millis() as u64, "took_ms": rescan["meta"]["took_ms"], "seq": rescan["meta"]["seq"]},
        "poll_ms": planroom::live::POLL.as_millis() as u64,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    });
    eprintln!("{summary}");
    report("watch_under_load", summary);
    assert_eq!(busy, 0, "no writer met SQLITE_BUSY past the 5 s busy timeout");
    let max = *latency_us.iter().max().unwrap();
    assert!(max < 1_000_000, "a commit took {max} µs to reach a frame");
}

#[test]
fn own_commits_reach_the_live_loop_through_the_commit_hook_without_a_poll() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let init = planroom_store::Init {
        workspace_id: "ws4".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    };
    let mut results = BTreeMap::new();
    for own in [true, false] {
        let mut store = Store::open(&path, &init).unwrap();
        let (hook_tx, hook_rx) = mpsc::channel();
        if own {
            store.set_commit_hook(hook_tx);
        }
        let tail = Tail::open(&path).unwrap().unwrap();
        let mut live = Live::new(tail, None, own.then_some(hook_rx)).unwrap();
        let (frame_tx, frame_rx) = mpsc::channel::<(u64, u64)>();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let looping = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut emit = |frame: &StreamFrame| {
                    if let StreamFrame::Event { seq, .. } = frame {
                        let _ = frame_tx.send((*seq, unix_micros()));
                    }
                    !stop.load(std::sync::atomic::Ordering::SeqCst)
                };
                live.run(&mut emit).unwrap();
            })
        };
        let mut latency = Vec::new();
        let label = if own { "own" } else { "poll" };
        for i in 0..100 {
            let (seq, _) = commit(&mut store, &version_write(label, i));
            let committed = unix_micros();
            let (got, at) = frame_rx.recv_timeout(Duration::from_secs(2)).expect("a frame within 2 s");
            assert_eq!(got, seq);
            latency.push(at.saturating_sub(committed));
            // Space commits so each lands at a random point of the poll interval.
            std::thread::sleep(Duration::from_millis(7));
        }
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        commit(&mut store, &version_write(label, 1_000));
        looping.join().unwrap();
        results.insert(label, percentiles(latency));
    }
    let summary = json!(results);
    eprintln!("{summary}");
    report("own_commits_in_process", summary.clone());
    let own_p95 = summary["own"]["p95"].as_u64().unwrap();
    let poll_p50 = summary["poll"]["p50"].as_u64().unwrap();
    assert!(own_p95 < 50_000, "own commits reach the loop at once: p95 {own_p95} µs");
    assert!(poll_p50 > own_p95, "without the hook, frames wait for the poll");
}
