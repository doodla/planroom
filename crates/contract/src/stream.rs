//! The live stream: one frame per SSE message and per `watch` NDJSON line.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::event::{Actor, EventKind, Status};
use crate::model::{NodeId, Seq};

/// One frame of the live stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamFrame {
    /// One committed event.
    Event {
        #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
        seq: Seq,
        kind: EventKind,
        target: Option<String>,
        project: Option<NodeId>,
        actor: Actor,
        status: Status,
        /// One engine-authored line, at most 2 KB; it can name a repo.
        #[schemars(extend("x-redact" = true))]
        summary: String,
    },
    /// The client is too far behind: refetch every view at `seq`.
    Reset {
        #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
        seq: Seq,
    },
    /// The stream is alive.
    Ping,
}
