//! The event model: every change is one appended `Event`, and only an `edit` changes the graph.

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::card::Card;
use crate::decision::DecisionRecord;
use crate::model::{EdgeId, Layer, NodeId, Seq};
use crate::open_set::{self, CODE_PATTERN};

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

/// An event's body, tagged by its kind. Event kinds are an open set: a body whose `kind` this
/// version doesn't know decodes as `Unknown`, which holds the raw object, `kind` included, and
/// serializes it back unchanged, so an older reader neither fails nor drops it.
//
// The derived serde code is generated as inherent functions (`remote = "Self"`), and the trait
// impls below wrap it, so a known kind whose body doesn't match its shape is an error rather than
// an `Unknown`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", remote = "Self")]
pub enum EventBody {
    /// The only event that changes the graph.
    Edit(Edit),
    /// Accepts or rejects a ghost, a conflict, an inferred fact or a card.
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
        /// The renames git reports since the repo's last snapshot, files and wholly moved
        /// directories; the fold resolves a vanished edit target through them (DR-74).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        renames: Vec<Rename>,
        stats: ScanStats,
    },
    Handoff(HandoffStep),
    /// A card a write dealt; open while no `review` names it (DR-89).
    Card(Card),
    /// A kind a later version added: the raw body, `kind` included, kept as it was read. The fold
    /// skips it; a read-modify-write or an export writes it back unchanged.
    #[serde(untagged)]
    #[schemars(schema_with = "unknown_body_schema")]
    Unknown(Map<String, Value>),
}

impl Serialize for EventBody {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        EventBody::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for EventBody {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<EventBody, D::Error> {
        use serde::de::Error;
        let raw = Map::<String, Value>::deserialize(d)?;
        let kind = match raw.get("kind") {
            Some(Value::String(kind)) => EventKind::parse(kind),
            _ => return Err(D::Error::missing_field("kind")),
        };
        if let EventKind::Unknown(_) = kind {
            return Ok(EventBody::Unknown(raw));
        }
        match EventBody::deserialize(Value::Object(raw)) {
            Ok(EventBody::Unknown(_)) | Err(_) => Err(D::Error::custom(format!(
                "a `{}` event body doesn't have that kind's fields",
                kind.as_str()
            ))),
            Ok(body) => Ok(body),
        }
    }
}

/// An unknown kind's body: an object whose `kind` follows the code pattern and is no known kind,
/// so `oneOf` still matches exactly one branch.
fn unknown_body_schema(_: &mut SchemaGenerator) -> Schema {
    let known: Vec<&str> = EventKind::KNOWN.iter().map(EventKind::as_str).collect();
    json_schema!({
        "description": "A kind a later version added, kept as it was read.",
        "type": "object",
        "required": ["kind"],
        "properties": {
            "kind": { "type": "string", "pattern": CODE_PATTERN, "not": { "enum": known } },
        },
    })
}

/// An event's kind, as the `kind` tag of an `EventBody` spells it. Kinds are an open set: one a
/// later version added reads as `Unknown`, with its spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventKind {
    Edit,
    Review,
    Undo,
    Version,
    Decision,
    Claim,
    Scan,
    Handoff,
    Card,
    /// A kind this version doesn't know.
    Unknown(String),
}

impl EventKind {
    /// The kinds this version knows, in registry order.
    pub const KNOWN: &'static [EventKind] = &[
        EventKind::Edit,
        EventKind::Review,
        EventKind::Undo,
        EventKind::Version,
        EventKind::Decision,
        EventKind::Claim,
        EventKind::Scan,
        EventKind::Handoff,
        EventKind::Card,
    ];

    /// The kind as it appears on the wire and in the store.
    pub fn as_str(&self) -> &str {
        match self {
            EventKind::Edit => "edit",
            EventKind::Review => "review",
            EventKind::Undo => "undo",
            EventKind::Version => "version",
            EventKind::Decision => "decision",
            EventKind::Claim => "claim",
            EventKind::Scan => "scan",
            EventKind::Handoff => "handoff",
            EventKind::Card => "card",
            EventKind::Unknown(kind) => kind,
        }
    }

    /// The kind a spelling names; one this version doesn't know is `Unknown`.
    pub fn parse(kind: &str) -> EventKind {
        EventKind::KNOWN
            .iter()
            .find(|k| k.as_str() == kind)
            .cloned()
            .unwrap_or_else(|| EventKind::Unknown(kind.to_owned()))
    }
}

impl Serialize for EventKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<EventKind, D::Error> {
        Ok(EventKind::parse(&String::deserialize(d)?))
    }
}

impl JsonSchema for EventKind {
    fn schema_name() -> Cow<'static, str> {
        "EventKind".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        open_set::schema(
            "An event's kind, as the `kind` tag of an `EventBody` spells it; an open set.",
            EventKind::KNOWN.iter().map(EventKind::as_str),
            CODE_PATTERN,
        )
    }
}

impl EventBody {
    pub fn kind(&self) -> EventKind {
        match self {
            EventBody::Edit(_) => EventKind::Edit,
            EventBody::Review { .. } => EventKind::Review,
            EventBody::Undo { .. } => EventKind::Undo,
            EventBody::Version { .. } => EventKind::Version,
            EventBody::Decision(_) => EventKind::Decision,
            EventBody::Claim { .. } => EventKind::Claim,
            EventBody::Scan { .. } => EventKind::Scan,
            EventBody::Handoff(_) => EventKind::Handoff,
            EventBody::Card(_) => EventKind::Card,
            EventBody::Unknown(raw) => {
                EventKind::parse(raw.get("kind").and_then(Value::as_str).unwrap_or_default())
            }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accept,
    Reject,
}

/// A rename git reported between two snapshots of one repo: the old and the new ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rename {
    pub from: NodeId,
    pub to: NodeId,
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
    #[schemars(extend("x-redact" = true))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_unknown_event_kind_round_trips_its_raw_body() {
        let raw = json!({"kind": "presence_ping", "who": "person:ann", "nested": {"a": [1, 2]}});
        let body: EventBody = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(body.kind(), EventKind::Unknown("presence_ping".into()));
        assert_eq!(serde_json::to_value(&body).unwrap(), raw);
        let kind: EventKind = serde_json::from_value(json!("presence_ping")).unwrap();
        assert_eq!(serde_json::to_value(&kind).unwrap(), json!("presence_ping"));
    }

    #[test]
    fn a_known_kind_still_decodes_as_itself() {
        let raw = json!({"kind": "version", "project": null, "label": "v1"});
        let body: EventBody = serde_json::from_value(raw.clone()).unwrap();
        assert!(matches!(body, EventBody::Version { ref label, .. } if label == "v1"));
        assert_eq!(body.kind(), EventKind::Version);
        assert_eq!(serde_json::to_value(&body).unwrap(), raw);
    }

    #[test]
    fn a_malformed_known_kind_is_not_taken_for_an_unknown_one() {
        let raw = json!({"kind": "version", "label": 7});
        assert!(serde_json::from_value::<EventBody>(raw).is_err());
    }
}
