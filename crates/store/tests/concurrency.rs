//! WS4, bet 1 (DR-1): three writer processes × 500 commits each against one store, every commit
//! one `BEGIN IMMEDIATE` transaction in WAL mode with `synchronous = FULL` and a 5 s busy
//! timeout. No commit is lost, no `seq` repeats or skips, and no writer meets `store_busy`.
//!
//! The writers are this test binary run again with `WS4_WRITER_STORE` set. The measured numbers
//! go to `target/tmp/ws4-store-concurrency.json` for docs/contributing/research/ws4-concurrency.md.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use planroom_contract::{Actor, Client, EventBody, EventId, Status, Surface, Timestamp};
use planroom_store::{Init, NewEvent, Store, StoreError, Write};
use serde_json::json;

const WRITERS: usize = 3;
const COMMITS: usize = 500;

fn micros() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64
}

fn init() -> Init {
    Init {
        workspace_id: "ws4".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

/// One writer process: wait for the start file, then commit `COMMITS` events, printing one
/// `WS4 <seq> <commit µs> <done-at µs> <busy retries>` line per commit.
#[test]
fn writer_child() {
    let Ok(store) = std::env::var("WS4_WRITER_STORE") else { return };
    let writer: usize = std::env::var("WS4_WRITER").unwrap().parse().unwrap();
    let start = PathBuf::from(std::env::var("WS4_START").unwrap());
    let mut store = Store::open_existing(Path::new(&store)).unwrap().unwrap();
    while !start.exists() {
        std::thread::sleep(Duration::from_millis(1));
    }
    for i in 0..COMMITS {
        let event = NewEvent {
            id: EventId(format!("w{writer}-{i}")),
            at: Timestamp(format!("{}", micros())),
            actor: Actor::System { part: format!("ws4:{writer}") },
            client: Client { surface: Surface::Test, id: format!("writer:{writer}") },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: format!("w{writer}-{i}") },
        };
        let write = Write { idem: format!("w{writer}-{i}"), request_hash: "h".into(), events: vec![event] };
        let mut busy = 0;
        let began = Instant::now();
        let done = loop {
            match store.commit(write.clone(), |seqs| json!({"seq": seqs[0]})) {
                Ok(done) => break done,
                Err(StoreError::Busy) => busy += 1,
                Err(e) => panic!("writer {writer}: {e}"),
            }
        };
        let took = began.elapsed().as_micros();
        println!("WS4 {} {took} {} {busy}", done.seqs[0], micros());
    }
}

/// The numbers one run measured.
pub struct Run {
    pub wall: Duration,
    /// Per commit: (seq, µs the commit took, µs since the epoch when it returned, busy retries).
    pub commits: Vec<(u64, u64, u64, u64)>,
}

/// Start `WRITERS` writer processes of `exe` against `store` and wait for them.
fn run_writers(store: &Path, scratch: &Path) -> Run {
    let start = scratch.join("start");
    let exe = std::env::current_exe().unwrap();
    let children: Vec<_> = (0..WRITERS)
        .map(|w| {
            Command::new(&exe)
                .args(["--exact", "writer_child", "--nocapture", "--test-threads", "1"])
                .env("WS4_WRITER_STORE", store)
                .env("WS4_WRITER", w.to_string())
                .env("WS4_START", &start)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap()
        })
        .collect();
    std::thread::sleep(Duration::from_millis(200));
    let began = Instant::now();
    std::fs::write(&start, "go").unwrap();
    let readers: Vec<_> = children
        .into_iter()
        .map(|mut child| {
            let out = child.stdout.take().unwrap();
            std::thread::spawn(move || {
                let lines: Vec<String> = BufReader::new(out).lines().map(Result::unwrap).collect();
                assert!(child.wait().unwrap().success(), "a writer failed");
                lines
            })
        })
        .collect();
    let mut commits = Vec::new();
    for reader in readers {
        for line in reader.join().unwrap() {
            // libtest may print "test writer_child ... " ahead of the first line.
            let Some(rest) = line.find("WS4 ").map(|at| &line[at + 4..]) else { continue };
            let f: Vec<u64> = rest.split(' ').map(|x| x.parse().unwrap()).collect();
            commits.push((f[0], f[1], f[2], f[3]));
        }
    }
    Run { wall: began.elapsed(), commits }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    sorted[((sorted.len() as f64 - 1.0) * p).round() as usize]
}

#[test]
fn three_writer_processes_commit_500_each_with_no_loss_and_no_busy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    drop(Store::open(&path, &init()).unwrap());
    let run = run_writers(&path, dir.path());

    assert_eq!(run.commits.len(), WRITERS * COMMITS, "every writer reported every commit");
    let mut seqs: Vec<u64> = run.commits.iter().map(|c| c.0).collect();
    seqs.sort_unstable();
    assert_eq!(seqs, (1..=(WRITERS * COMMITS) as u64).collect::<Vec<_>>(), "no seq lost or repeated");
    let store = Store::open_existing(&path).unwrap().unwrap();
    let events = store.events_after(0, 10_000).unwrap();
    assert_eq!(events.len(), WRITERS * COMMITS);
    for w in 0..WRITERS {
        let labels: Vec<String> = events
            .iter()
            .filter(|e| e.client.id == format!("writer:{w}"))
            .map(|e| match &e.body {
                EventBody::Version { label, .. } => label.clone(),
                _ => unreachable!(),
            })
            .collect();
        let expected: Vec<String> = (0..COMMITS).map(|i| format!("w{w}-{i}")).collect();
        assert_eq!(labels, expected, "writer {w}'s commits land in its order");
    }
    let busy: u64 = run.commits.iter().map(|c| c.3).sum();

    let mut took: Vec<u64> = run.commits.iter().map(|c| c.1).collect();
    took.sort_unstable();
    let report = json!({
        "writers": WRITERS,
        "commits_per_writer": COMMITS,
        "wall_ms": run.wall.as_millis() as u64,
        "commits_per_s": (WRITERS * COMMITS) as f64 / run.wall.as_secs_f64(),
        "commit_us": {"p50": percentile(&took, 0.5), "p95": percentile(&took, 0.95), "p99": percentile(&took, 0.99), "max": took[took.len() - 1]},
        "busy_retries": busy,
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    });
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ws4-store-concurrency.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    eprintln!("{report}");
    assert_eq!(busy, 0, "no writer met SQLITE_BUSY past the 5 s busy timeout");
}
