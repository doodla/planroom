//! App services: orchestration only. Each command is one handler here; rules live in the kernel,
//! storage in the store, extraction in extract.

pub mod actor;
pub mod data;
pub mod error;
pub mod focus;
pub mod import;
pub mod map;
pub mod person;
pub mod scan;
pub mod share;
pub mod version;
pub mod workspace;

use std::sync::OnceLock;
use std::sync::mpsc::Sender;

use planroom_contract::{EventId, Level, MapView, Seq, Warning};

/// Where this process's own commits are announced: `serve`'s live loop, which `data_version`
/// can't wake for them as fast (DR-1). Unset in a one-shot CLI process.
static OWN_COMMITS: OnceLock<Sender<Seq>> = OnceLock::new();

/// Announce every later commit this process makes to `hook`.
pub fn announce_commits_to(hook: Sender<Seq>) {
    let _ = OWN_COMMITS.set(hook);
}

/// Hook a store this process opened for writing to the live loop, when one runs.
pub fn hook(store: &mut planroom_store::Store) {
    if let Some(hook) = OWN_COMMITS.get() {
        store.set_commit_hook(hook.clone());
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

/// A fresh event ID: a UUID v7.
pub fn new_event_id() -> EventId {
    EventId(uuid::Uuid::now_v7().to_string())
}

/// A map with no blocks.
pub fn empty_view(level: Level, lens: String) -> MapView {
    MapView { level, lens, focus: None, nodes: vec![], edges: vec![], hidden: 0, empty: None, layout: None }
}
