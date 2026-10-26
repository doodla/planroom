//! Every public Planroom type, the error and warning registries and the command catalogue.
//!
//! The Rust types here are the one source of the public contract. `cargo xtask gen` turns them
//! into `schema/planroom.v1.schema.json` (what Planroom emits), `schema/planroom.v1.input.schema.json`
//! (what Planroom accepts) and the TypeScript types in `packages/contract/src/generated/`.

pub mod command;
pub mod decision;
pub mod envelope;
pub mod errors;
pub mod event;
pub mod model;
mod open_set;
pub mod schema;
pub mod stream;
pub mod view;

pub use command::{
    ActorRule, Class, Command, CommandSpec, Detail, FocusReq, Idea, MapReq, ScanReq, ScanResult, Tool, Undo,
    WatchReq, WriteCommon,
};
pub use decision::{Band, DecisionRecord, Outcome};
pub use envelope::{
    CommandName, Cursor, Empty, EmptyReason, Envelope, ErrorObject, Handle, Meta, ModelRequestRef, ModelUse,
    SCHEMA_VERSION, Size, Usage, Warning,
};
pub use errors::{Category, ErrorCode, Retry, WarningCode};
pub use event::{
    Actor, Cause, Client, Edit, Event, EventBody, EventId, EventKind, HandoffState, HandoffStep, Op,
    ScanStats, Status, Surface, Target, Timestamp, Verdict,
};
pub use model::{
    CONTAINS_MAX_CHARS, Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Hint, HintRef, Kind, Layer, Level,
    Node, NodeId, PartialFile, Provenance, Ref, Seq, Tier,
};
pub use stream::StreamFrame;
pub use view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, Focus, LiftedEdge, MapNode, MapView,
    MiniBlock, MiniEdge, MiniMap, PaneModel, Position, PresenceRow, Progress, ScanState, Stakes,
};
