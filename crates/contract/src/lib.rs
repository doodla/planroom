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
pub mod view;

pub use command::{
    ActorRule, Class, Command, CommandSpec, Detail, FocusReq, Idea, MapReq, ScanReq, ScanResult, Tool, Undo,
    WriteCommon,
};
pub use decision::{Band, DecisionRecord, Outcome};
pub use envelope::{
    CommandName, Cursor, Empty, EmptyReason, Envelope, ErrorObject, Handle, Meta, ModelRequestRef, ModelUse,
    SCHEMA_VERSION, Size, Usage, Warning,
};
pub use errors::{Category, ErrorCode, WarningCode};
pub use event::{
    Actor, Cause, Edit, Event, EventBody, EventId, HandoffState, HandoffStep, Op, ScanStats, Status, Surface,
    Target, Timestamp, Verdict,
};
pub use model::{Edge, EdgeId, Evidence, Hint, Kind, Layer, Level, Node, NodeId, Provenance, Ref, Seq, Tier};
pub use view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, Focus, LiftedEdge, MapNode, MapView,
    MiniBlock, MiniEdge, MiniMap, PaneModel, Position, PresenceRow, Progress, ScanState, Stakes,
};
