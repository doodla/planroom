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
//!
//! Both keep a fold (DR-61): the process holds one `(model, seq)` per store, and after each burst
//! of frames the loop applies the new events to it with `fold_one`, one at a time, refolding from
//! scratch on a `scan`, an `undo` or a `review`. A read in the same process, such as a map over
//! `serve`'s API, catches that fold up to its own read transaction's `seq` the same way, so it
//! never folds every event again and never answers from a fold behind its `meta.seq`. A one-shot
//! command keeps nothing between runs, so its one read folds from scratch.

use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use planroom_contract::{Edge, Event, EventBody, Node, Seq, StreamFrame, Target, Vocabulary};
use planroom_kernel::fold::{Folded, Overlay, fold_full, fold_one, refolds};
use planroom_kernel::{Facts, Model, redact};
use planroom_store::{ActiveSet, Shape, Snapshot, StoreError, Tail};

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
        keep_up(&self.tail);
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
                    // After the frames, so folding never delays one.
                    keep_up(&self.tail);
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

/// An event's stream frame, with its engine-authored one-line summary, redacted: every frame goes
/// to an output (`watch`, the server's stream, `live/head`), none of which shows it raw.
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
        EventBody::Edit(edit) => text::get("frame.edit", &[("target", &target_id(&edit.target))]),
        EventBody::Undo { of } => text::get("frame.undo", &[("event", &of.0)]),
        other => text::get("frame.other", &[("kind", &other.kind().as_str())]),
    };
    let target = match &event.body {
        // The repo's node ID (`worker:repo:worker`), as every other frame names its target (DR-82).
        EventBody::Scan { repo, .. } => repo_node(repo),
        EventBody::Edit(edit) => Some(target_id(&edit.target)),
        EventBody::Claim { target, .. } => Some(target_id(target)),
        _ => None,
    };
    let mut frame = StreamFrame::Event {
        seq: event.seq,
        kind: event.body.kind(),
        target,
        project: event.project.clone(),
        actor: event.actor.clone(),
        status: event.status,
        summary,
    };
    redact::output(&mut frame);
    frame
}

/// A repo's node ID, made as a scan makes it: from the repo kind of the built-in code pack, which
/// no other pack can replace, so the live loop reads it once.
fn repo_node(repo: &str) -> Option<String> {
    static REPO_KIND: OnceLock<Option<planroom_extract::KindRef>> = OnceLock::new();
    let kind = REPO_KIND.get_or_init(|| crate::packs::builtins().ok()?.repo_kind());
    kind.as_ref().map(|k| planroom_extract::files::repo_id(repo, k).0)
}

fn target_id(target: &Target) -> String {
    match target {
        Target::Node(id) => id.0.clone(),
        Target::Edge(id) => id.0.clone(),
    }
}

/// The fold this process keeps (DR-61): for one store, the facts of its active set, every event
/// it folded, and what the fold derived from them at the last one's `seq`.
struct Kept {
    /// The store's `workspace_id`, so a fold is never carried over to another store.
    store: String,
    vocabulary: Vocabulary,
    active: ActiveSet,
    facts: Facts,
    history: Vec<Event>,
    folded: Folded,
}

/// The one kept fold of this process.
static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

/// The kept fold, locked. A thread that panicked while it held the lock, inside `fold_one` say,
/// may have left the fold half-built, so a poisoned lock drops the fold, and the read that took
/// it folds afresh, rather than trusting it.
fn kept() -> MutexGuard<'static, Option<Kept>> {
    KEPT.lock().unwrap_or_else(|poisoned| {
        let mut kept = poisoned.into_inner();
        *kept = None;
        KEPT.clear_poison();
        kept
    })
}

/// The model at `snapshot`'s `seq`, folded from what that one read transaction holds: the kept
/// fold caught up to it, or a fold from scratch when none is kept for this store and vocabulary.
pub fn model(snapshot: &Snapshot<'_>, vocabulary: &Vocabulary) -> Result<Model, StoreError> {
    with_folded(snapshot, vocabulary, |folded| folded.model.clone())
}

/// Everything the fold derives at `snapshot`'s `seq`, the overlay and the authored revisions
/// included: what a write's compare-and-set reads inside its own transaction, and what a view
/// shows beside the model (what is hidden, where a block is pinned).
pub fn folded(snapshot: &Snapshot<'_>, vocabulary: &Vocabulary) -> Result<Folded, StoreError> {
    with_folded(snapshot, vocabulary, Folded::clone)
}

/// The model at `snapshot`'s `seq` with the overlay beside it: what an override hides, and where a
/// person pinned a block in each view.
pub fn model_and_overlay(
    snapshot: &Snapshot<'_>,
    vocabulary: &Vocabulary,
) -> Result<(Model, Overlay), StoreError> {
    with_folded(snapshot, vocabulary, |folded| (folded.model.clone(), folded.overlay.clone()))
}

/// The overlay alone at `snapshot`'s `seq`.
pub fn overlay(snapshot: &Snapshot<'_>, vocabulary: &Vocabulary) -> Result<Overlay, StoreError> {
    with_folded(snapshot, vocabulary, |folded| folded.overlay.clone())
}

/// `read` of the kept fold, caught up to `snapshot`'s `seq`.
fn with_folded<T>(
    snapshot: &Snapshot<'_>,
    vocabulary: &Vocabulary,
    read: impl FnOnce(&Folded) -> T,
) -> Result<T, StoreError> {
    let mut kept = kept();
    match catch_up(&mut kept, snapshot, vocabulary) {
        Ok(()) => Ok(read(&kept.as_ref().expect("a caught-up fold is kept").folded)),
        Err(e) => {
            // A fold that failed part way is dropped, so the next read folds from scratch.
            *kept = None;
            Err(e)
        }
    }
}

/// After a burst of frames: catch the kept fold up to the store's latest `seq`, applying the
/// burst's events one at a time. A failure drops the kept fold rather than stopping the stream;
/// the next read folds from scratch.
fn keep_up(tail: &Tail) {
    let mut kept = kept();
    let vocabulary = kept.as_ref().map(|k| k.vocabulary.clone()).unwrap_or_default();
    let caught = tail.snapshot().and_then(|snapshot| catch_up(&mut kept, &snapshot, &vocabulary));
    if caught.is_err() {
        *kept = None;
    }
}

/// Bring `kept` to `snapshot`'s `seq`. The events after the kept `seq` go through `fold_one` in
/// order, which refolds from scratch on a `scan`, an `undo` or a `review` (`refolds`); a scan
/// that changed the active set reloads its facts first. Only the active set's last facts are
/// needed: facts change only with a scan, so every event after the last scan folds over them,
/// and the result equals `fold_full` at that `seq`. A kept fold for another store or vocabulary,
/// one past the snapshot's `seq`, or one whose last event the store no longer holds (a restored
/// store) is replaced by a fold from scratch.
fn catch_up(
    kept: &mut Option<Kept>,
    snapshot: &Snapshot<'_>,
    vocabulary: &Vocabulary,
) -> Result<(), StoreError> {
    let at = snapshot.seq();
    let store = snapshot.meta("workspace_id")?.unwrap_or_default();
    let usable = match kept.as_ref() {
        Some(k) if k.store == store && &k.vocabulary == vocabulary && k.folded.at() <= at => {
            match k.history.last() {
                Some(last) => snapshot.event_at(last.seq)?.is_some_and(|e| e.id == last.id),
                None => true,
            }
        }
        _ => false,
    };
    if !usable {
        let active = snapshot.active_set()?;
        let facts = facts_of(snapshot, &active)?;
        let history = snapshot.events_after(0)?;
        let folded = fold_full(&facts, &history, vocabulary, at);
        *kept = Some(Kept { store, vocabulary: vocabulary.clone(), active, facts, history, folded });
        return Ok(());
    }
    let k = kept.as_mut().expect("a usable fold is kept");
    let new = snapshot.events_after(k.folded.at())?;
    if new.iter().any(refolds) {
        let active = snapshot.active_set()?;
        if active != k.active {
            k.facts = facts_of(snapshot, &active)?;
            k.active = active;
        }
    }
    for event in new {
        fold_one(&mut k.folded, &k.facts, &k.history, vocabulary, &event);
        k.history.push(event);
    }
    Ok(())
}

/// The nodes and edges of the active set's snapshots. No linker joins hints into edges yet, so
/// the fold doesn't read them.
fn facts_of(snapshot: &Snapshot<'_>, active: &ActiveSet) -> Result<Facts, StoreError> {
    let mut facts = Facts::default();
    let bad = |e: serde_json::Error| StoreError::Sqlite(format!("a stored fact doesn't parse: {e}"));
    for row in snapshot.facts(&active.snapshots())? {
        match row.shape {
            Shape::Node => facts.nodes.push(serde_json::from_str::<Node>(&row.body).map_err(bad)?),
            Shape::Edge => facts.edges.push(serde_json::from_str::<Edge>(&row.body).map_err(bad)?),
            Shape::Hint => {}
        }
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use planroom_store::{Init, Store};

    use super::*;

    #[test]
    fn a_fold_a_panic_left_behind_is_dropped_and_folded_afresh() {
        let tmp = tempfile::tempdir().unwrap();
        let init = Init {
            workspace_id: "ws-poisoned".into(),
            engine_version: "0.1.0".into(),
            created_at: "2026-10-27T00:00:00Z".into(),
        };
        let store = Store::open(&tmp.path().join("store.db"), &init).unwrap();
        let snapshot = store.snapshot().unwrap();
        let vocabulary = Vocabulary::default();
        let fresh = model(&snapshot, &vocabulary).unwrap();

        // A read panics part way through changing the kept fold, as a panic inside `fold_one`
        // would, and leaves it half-built: here, a model that claims a `seq` it never folded.
        let panicked = std::thread::spawn(|| {
            let mut kept = KEPT.lock().unwrap();
            let k = kept.as_mut().expect("the first read kept its fold");
            k.folded.model.at = 42;
            panic!("a fold that stopped part way");
        })
        .join();
        assert!(panicked.is_err());
        assert!(KEPT.is_poisoned());

        // The next read drops that fold and folds afresh, and the lock is whole again.
        assert_eq!(model(&snapshot, &vocabulary).unwrap(), fresh);
        assert!(!KEPT.is_poisoned());
    }
}
