//! The live loop that `watch` and `serve` both run (architecture.md#local-server-and-live-stream):
//! `watch` writes its frames as NDJSON, `serve` as server-sent events, and neither builds a frame
//! of its own.
//!
//! It polls `PRAGMA data_version` every 200 ms on a dedicated read-only connection that never
//! writes, so it sees every other connection's commits, and it also takes the `seq` of each of
//! this process's own commits from the store's commit hook, so frames for its own writes go out
//! with no poll (DR-1). Whenever `data_version` moves it re-reads `PRAGMA user_version` first, so
//! a loop older than the store stops with `schema_mismatch` within one poll of a migration (DR-78).
//! A client more than 5,000 events behind gets one `reset` frame instead of the events, and an
//! idle stream sends `ping` every 15 s.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use planroom_contract::{Event, EventBody, Seq, StreamFrame, Target};
use planroom_store::{StoreError, Tail};

use crate::text;

/// How often the loop checks `data_version`.
pub const POLL: Duration = Duration::from_millis(200);
/// A client this many events behind gets one `reset` frame instead.
pub const RESET_AFTER: u64 = 5_000;
/// How often an idle stream sends `ping`.
pub const PING_EVERY: Duration = Duration::from_secs(15);
/// Events read per query.
const PAGE: u32 = 500;

/// One live stream over one store.
pub struct Live {
    tail: Tail,
    cursor: Seq,
    version: i64,
    own: Option<Receiver<Seq>>,
}

impl Live {
    /// Stream the events after `since`, or after the store's latest `seq`. `own` receives the
    /// `seq` of each commit this process makes, from `Store::set_commit_hook`.
    pub fn new(tail: Tail, since: Option<Seq>, own: Option<Receiver<Seq>>) -> Result<Live, StoreError> {
        tail.check_version()?;
        let cursor = match since {
            Some(seq) => seq,
            None => tail.seq()?,
        };
        let version = tail.data_version()?;
        Ok(Live { tail, cursor, version, own })
    }

    /// The `seq` of the last event sent.
    pub fn cursor(&self) -> Seq {
        self.cursor
    }

    /// Send frames until `emit` returns false: a `ping` first, then the events after the cursor
    /// (the `--since` replay), then every event as it commits, and a `ping` when idle. Returns
    /// `schema_mismatch` once a newer engine has migrated the store.
    pub fn run(&mut self, emit: &mut dyn FnMut(&StreamFrame) -> bool) -> Result<(), StoreError> {
        if !emit(&StreamFrame::Ping) {
            return Ok(());
        }
        if !self.drain(emit)?.1 {
            return Ok(());
        }
        let mut last_frame = Instant::now();
        let mut next_poll = Instant::now();
        loop {
            let now = Instant::now();
            let wait = next_poll.saturating_duration_since(now);
            let woke_by_own = match &self.own {
                Some(own) => match own.recv_timeout(wait) {
                    Ok(seq) => seq > self.cursor,
                    Err(RecvTimeoutError::Timeout) => false,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.own = None;
                        false
                    }
                },
                None => {
                    std::thread::sleep(wait);
                    false
                }
            };
            let mut due = woke_by_own;
            if Instant::now() >= next_poll {
                next_poll = Instant::now() + POLL;
                let version = self.tail.data_version()?;
                if version != self.version {
                    self.version = version;
                    self.tail.check_version()?;
                    due = true;
                }
            }
            if due {
                let (sent, open) = self.drain(emit)?;
                if !open {
                    return Ok(());
                }
                if sent > 0 {
                    last_frame = Instant::now();
                }
            }
            if last_frame.elapsed() >= PING_EVERY {
                if !emit(&StreamFrame::Ping) {
                    return Ok(());
                }
                last_frame = Instant::now();
            }
        }
    }

    /// Send every event after the cursor, or one `reset` when there are too many, or when the
    /// cursor is past the store's last `seq` (a store restored from an older copy). Returns how
    /// many frames went out and whether the client still listens.
    pub fn drain(&mut self, emit: &mut dyn FnMut(&StreamFrame) -> bool) -> Result<(usize, bool), StoreError> {
        let latest = self.tail.seq()?;
        if self.cursor > latest || self.tail.count_after(self.cursor)? > RESET_AFTER {
            self.cursor = latest;
            return Ok((1, emit(&StreamFrame::Reset { seq: latest })));
        }
        let mut sent = 0;
        loop {
            let events = self.tail.events_after(self.cursor, PAGE)?;
            if events.is_empty() {
                return Ok((sent, true));
            }
            for event in &events {
                self.cursor = event.seq;
                sent += 1;
                if !emit(&frame(event)) {
                    return Ok((sent, false));
                }
            }
        }
    }
}

/// An event's stream frame, with its engine-authored one-line summary.
pub fn frame(event: &Event) -> StreamFrame {
    let summary = match &event.body {
        // A repo with no commit yet maps nothing, so its frame says that rather than "at ".
        EventBody::Scan { repo, commit, .. } if commit.is_empty() => {
            text::get("frame.scan_unborn", &[("repo", repo)])
        }
        EventBody::Scan { repo, commit, .. } => {
            text::get("frame.scan", &[("repo", repo), ("commit", &&commit[..commit.len().min(7)])])
        }
        EventBody::Version { label, .. } => text::get("frame.version", &[("label", label)]),
        other => text::get("frame.other", &[("kind", &other.kind().as_str())]),
    };
    let target = match &event.body {
        // The repo's node ID (`worker:repo:worker`), as every other frame names its target (DR-82).
        EventBody::Scan { repo, .. } => Some(planroom_extract::files::repo_id(repo).0),
        EventBody::Edit(edit) => Some(target_id(&edit.target)),
        EventBody::Claim { target, .. } => Some(target_id(target)),
        _ => None,
    };
    StreamFrame::Event {
        seq: event.seq,
        kind: event.body.kind(),
        target,
        project: event.project.clone(),
        actor: event.actor.clone(),
        status: event.status,
        summary,
    }
}

fn target_id(target: &Target) -> String {
    match target {
        Target::Node(id) => id.0.clone(),
        Target::Edge(id) => id.0.clone(),
    }
}
