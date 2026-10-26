//! App services: orchestration only. Each command is one handler here; rules live in the kernel,
//! storage in the store, extraction in extract.

pub mod error;
pub mod map;
pub mod scan;
pub mod version;
pub mod workspace;

use planroom_contract::{EventId, Level, MapView, Seq, Warning};

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
    MapView { level, lens, focus: None, nodes: vec![], edges: vec![], hidden: 0, empty: None }
}
