//! App services: orchestration only. Each command is one handler here; rules live in the kernel,
//! storage in the store, extraction in extract.

pub mod actor;
pub mod ask;
pub mod cards;
pub mod confine;
pub mod data;
pub mod dirty;
pub mod doctor;
pub mod edit;
pub mod error;
pub mod find;
pub mod focus;
pub mod fs_type;
pub mod import;
pub mod intent;
pub mod map;
pub mod models;
pub mod open;
pub mod owner;
pub mod pack;
pub mod person;
pub mod review;
pub mod scan;
pub mod share;
pub mod trust;
pub mod version;
pub mod workspace;

use std::sync::OnceLock;
use std::sync::mpsc::Sender;

use planroom_contract::{Cursor, EventId, Level, MapView, Seq, Warning};

/// Where this process's own commits are announced: `serve`'s live loop, which `data_version`
/// can't wake for them as fast (DR-1). Unset in a one-shot CLI process.
static OWN_COMMITS: OnceLock<Sender<Seq>> = OnceLock::new();

/// Announce every later commit this process makes to `hook`.
pub fn announce_commits_to(hook: Sender<Seq>) {
    let _ = OWN_COMMITS.set(hook);
}

/// Hook a store this process opened for writing to the live loop, when one runs, and install the
/// pack guard every authored write runs under the write lock: an engine older than the one that
/// recorded the workspace's pack set is refused with `packs_mismatch`, and any other records its
/// own built-ins at its first write (DR-62).
pub fn hook(store: &mut planroom_store::Store) {
    if let Some(hook) = OWN_COMMITS.get() {
        store.set_commit_hook(hook.clone());
    }
    if let Some(guard) = crate::packs::state::default_guard() {
        store.guard_packs(guard);
    }
}

/// A command's result, the `seq` it reflects, its warnings and its text-mode line.
#[derive(Debug, Clone)]
pub struct Reply<T> {
    pub result: T,
    pub seq: Seq,
    pub warnings: Vec<Warning>,
    pub text: String,
}

/// A page of a reply, and the cursor of the page after it, which `meta.next` carries; none on the
/// last page.
#[derive(Debug, Clone)]
pub struct Paged<T> {
    pub reply: Reply<T>,
    pub next: Option<Cursor>,
    /// The repos with uncommitted work, which `meta.dirty_repos` carries: on `scan`, `focus` and
    /// `map` only.
    pub dirty: dirty::Dirty,
}

impl<T> Paged<T> {
    /// A whole reply: no page after it, and no repos with uncommitted work named.
    pub fn whole(reply: Reply<T>) -> Paged<T> {
        Paged { reply, next: None, dirty: dirty::Dirty::default() }
    }
}

/// A fresh event ID: a UUID v7.
pub fn new_event_id() -> EventId {
    EventId(uuid::Uuid::now_v7().to_string())
}

/// A map with no blocks.
pub fn empty_view(level: Level, lens: String) -> MapView {
    MapView { level, lens, focus: None, nodes: vec![], edges: vec![], hidden: 0, empty: None, layout: None }
}
