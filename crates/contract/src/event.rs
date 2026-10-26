//! The event model: every change is one appended `Event`, and only an `edit` changes the graph.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::DecisionRecord;
use crate::model::{EdgeId, Layer, NodeId, Seq};

/// An event ID: a UUID v7.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct EventId(#[schemars(extend("format" = "uuid"))] pub String);

/// An RFC 3339 UTC timestamp.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Timestamp(#[schemars(extend("format" = "date-time"))] pub String);

/// One appended change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    /// One global order: the stream cursor and the store version.
    pub seq: Seq,
    pub id: EventId,
    /// The idempotency key.
    pub idem: String,
    /// sha256 of the canonical request; the same `idem` with another hash is refused.
    pub request_hash: String,
    pub at: Timestamp,
    pub actor: Actor,
    /// With `actor`, the writer instance that compare-and-set compares.
    pub client: Client,
    pub project: Option<NodeId>,
    pub cause: Option<Cause>,
    pub status: Status,
    pub body: EventBody,
}

/// Who did it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    /// Such as `person:sridhar` or `person:artifact:u_123`.
    Person { id: String, name: Option<String> },
    /// Such as `agent:1a2b3c4d:chat` or `agent:cli`.
    Agent { session: String, role: String, key: Option<String> },
    /// Such as `typesafe-ai/jev-1.14.0`.
    Model { id: String },
    /// Such as `scan` or `duty:decide`.
    System { part: String },
}

/// Where a writer acted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Cli,
    Mod,
    Canvas,
    Artifact,
    Test,
}

/// The client a writer writes from, such as `tab_01J...`, `mod:<sessionId>` or `cli:4242`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Client {
    pub surface: Surface,
    pub id: String,
}

/// What an event follows from; a chat turn batches ghosts into one card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Event(EventId),
    Turn(String),
    Duty(String),
}

/// An event's state; set at commit and changed only by `review` and `undo` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Applied,
    Ghost,
    Conflict,
    Open,
    Closed,
}

/// The eight event kinds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventBody {
    /// The only event that changes the graph.
    Edit(Edit),
    /// Accepts or rejects a ghost, a conflict or an inferred fact.
    Review {
        of: EventId,
        verdict: Verdict,
    },
    Undo {
        of: EventId,
    },
    /// A tag on `seq`.
    Version {
        project: Option<NodeId>,
        label: String,
    },
    Decision(DecisionRecord),
    Claim {
        target: Target,
        until: Timestamp,
        release: bool,
    },
    Scan {
        repo: String,
        commit: String,
        branch: String,
        snapshot: u64,
        /// The workspace link snapshot of the active set this scan committed.
        link: u64,
        stats: ScanStats,
    },
    Handoff(HandoffStep),
}

/// The eight event kinds, as the `kind` tag of an `EventBody` spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Edit,
    Review,
    Undo,
    Version,
    Decision,
    Claim,
    Scan,
    Handoff,
}

impl EventKind {
    /// The kind as it appears on the wire and in the store.
    pub const fn as_str(self) -> &'static str {
        match self {
            EventKind::Edit => "edit",
            EventKind::Review => "review",
            EventKind::Undo => "undo",
            EventKind::Version => "version",
            EventKind::Decision => "decision",
            EventKind::Claim => "claim",
            EventKind::Scan => "scan",
            EventKind::Handoff => "handoff",
        }
    }
}

impl EventBody {
    pub const fn kind(&self) -> EventKind {
        match self {
            EventBody::Edit(_) => EventKind::Edit,
            EventBody::Review { .. } => EventKind::Review,
            EventBody::Undo { .. } => EventKind::Undo,
            EventBody::Version { .. } => EventKind::Version,
            EventBody::Decision(_) => EventKind::Decision,
            EventBody::Claim { .. } => EventKind::Claim,
            EventBody::Scan { .. } => EventKind::Scan,
            EventBody::Handoff(_) => EventKind::Handoff,
        }
    }
}

/// One operation on one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edit {
    pub op: Op,
    pub target: Target,
    pub layer: Layer,
    /// `put`: the whole node or edge; `set`: the changed fields.
    pub fields: BTreeMap<String, Value>,
    /// The `seq` of the view the writer acted on.
    pub base: Seq,
    /// The fact values the fields override, for scan-drift checks.
    pub over: BTreeMap<String, Value>,
}

/// The three edit operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Put,
    Set,
    Retract,
}

/// What an edit or claim is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Node(NodeId),
    Edge(EdgeId),
}

/// A review's outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accept,
    Reject,
}

/// What one repo's scan found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ScanStats {
    pub files: u32,
    pub nodes: u32,
    pub edges: u32,
    pub hints: u32,
    /// Files parsed with error nodes.
    pub partial: u32,
}

/// One step of the hand-off loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HandoffStep {
    pub key: String,
    pub from: HandoffState,
    pub to: HandoffState,
    pub note: Option<String>,
}

/// The states of a hand-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffState {
    Proposed,
    Started,
    Reported,
    Failed,
    Reviewing,
    Merged,
    Reconciled,
    Dropped,
}
