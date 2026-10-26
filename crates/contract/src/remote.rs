//! Remote mode through the Artifact (architecture.md, "Remote mode through the Artifact (S11)";
//! contracts.md, "Artifact db layout and event format"): the published snapshot, the `db`
//! documents the session writes (`live/head`, `answers/<docId>`, `acks/<uid>`), the rows viewers
//! write under `inbox/{self}`, and what `share` and `import` return.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::envelope::{CommandName, Empty, ErrorObject, Handle};
use crate::event::Timestamp;
use crate::model::Seq;
use crate::stream::StreamFrame;
use crate::view::{DecisionCard, MapView, PaneModel, PresenceRow};

/// `data/snapshot.json`'s format tag.
pub const SNAPSHOT_FORMAT: &str = "planroom.snapshot/v1";
/// `live/head`'s format tag.
pub const LIVE_HEAD_FORMAT: &str = "planroom.live-head/v1";
/// An inbox row's format tag.
pub const INBOX_FORMAT: &str = "planroom.inbox/v1";
/// `answers/<docId>`'s format tag.
pub const ANSWER_FORMAT: &str = "planroom.answer/v1";
/// `acks/<uid>`'s format tag.
pub const ACK_FORMAT: &str = "planroom.ack/v1";
/// Where the snapshot is published, beside the page.
pub const SNAPSHOT_PATH: &str = "data/snapshot.json";
/// The snapshot's budget on the fixture.
pub const SNAPSHOT_BUDGET_BYTES: usize = 1_000_000;
/// The largest `db` document the platform takes, and so `live/head`'s and each answer's budget.
pub const DB_DOC_MAX_BYTES: usize = 256 * 1024;
/// The most writes one `ArtifactData batch` takes.
pub const BATCH_MAX_WRITES: usize = 50;
/// The largest body one `ArtifactData batch` takes.
pub const BATCH_MAX_BYTES: usize = 1024 * 1024;

/// `planroom.snapshot/v1`: what the Artifact page draws with no engine, published as
/// `data/snapshot.json`. It passes `kernel::redact`, and holds no evidence `contains` unless
/// `share --include-snippets`, and no presence unless `share --include-presence`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Snapshot {
    /// `planroom.snapshot/v1`.
    pub format: String,
    pub workspace_id: String,
    /// The store `seq` the snapshot reflects.
    pub seq: Seq,
    pub published_at: Timestamp,
    /// Whether evidence keeps its `contains` text; false by default.
    pub snippets: bool,
    /// The maps it holds, each with its layout positions when one is cached; the page asks for
    /// any other as a read request.
    pub maps: Vec<MapView>,
    pub cards: Vec<DecisionCard>,
    /// None unless `share --include-presence`.
    pub who: Option<Vec<PresenceRow>>,
    pub pane: PaneModel,
    /// Precomputed views, keyed by the handle that asks for each.
    pub answers: Vec<Answer>,
    /// The views and queries this snapshot serves: `map:<level>:<lens>` for each map it holds,
    /// `pane`, and `answers`.
    pub capabilities: Vec<String>,
    /// The engine-authored state the page shows for a view it doesn't hold; its `next` names the
    /// read request a press writes to the inbox.
    pub unavailable: Empty,
}

/// `planroom.answer/v1`: a read request's result, under `answers/<docId>`, or a view the snapshot
/// precomputed. Redacted, within the leaf's budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// `planroom.answer/v1`.
    pub format: String,
    /// The handle that asks for this view.
    pub handle: Handle,
    /// The store `seq` the result reflects.
    pub seq: Seq,
    /// The command's result, as its `--json` envelope carries it; null when it failed.
    pub result: Value,
    /// Why the read failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

/// `planroom.live-head/v1`: the small `db` document `live/head` the session writes last in each
/// sync batch; at most 256 KiB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LiveHead {
    /// `planroom.live-head/v1`.
    pub format: String,
    /// The store `seq` this head reflects.
    pub seq: Seq,
    /// The `seq` of the published snapshot it extends.
    pub snapshot_seq: Seq,
    /// The top open cards.
    pub cards: Vec<DecisionCard>,
    /// The change frames since the snapshot, newest last.
    pub changes: Vec<StreamFrame>,
    /// Engine-authored lines for the conflicts the last import found.
    pub conflicts: Vec<String>,
}

/// `planroom.inbox/v1`: one row a viewer's page writes to `inbox/{self}/events/{docId}`. Import
/// validates it, and takes the author from the path, never from the body: a field the row doesn't
/// define, such as a forged `actor`, is ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InboxRow {
    /// `planroom.inbox/v1`.
    pub format: String,
    /// Minted by the page on each load: with the uid, the writer instance.
    #[schemars(length(min = 1, max = 200))]
    pub instance: String,
    /// Per instance, monotonic; import applies rows in `(instance, counter)` order.
    pub counter: u64,
    /// `edit`, `review`, `decide` or `intent propose`; or a read request: `map`, `show`, `find`,
    /// `diff`, or an `ask` question that doesn't spend. Import refuses any other.
    pub command: CommandName,
    /// That command's request JSON.
    pub request: Value,
    /// The snapshot or live-head `seq` the viewer saw.
    pub base: Seq,
    pub at: Timestamp,
    /// `sample` when the row came from Ask Claude.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// A paired device's HMAC-SHA256 over the docId and the row without its `sig` (DR-67).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
}

/// `inbox/<uid>`: the viewer's own head, which the page bumps after each row, so the session can
/// list who wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InboxHead {
    /// The page's last row counter.
    pub seq: u64,
    pub updated_at: Timestamp,
    /// Each Ask Claude call and the model the platform applied, for the ledger.
    #[serde(default)]
    pub sample: Vec<SampleUse>,
}

/// One Ask Claude call the page made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SampleUse {
    pub at: Timestamp,
    pub prompt_chars: u32,
    pub response_chars: u32,
    pub model: String,
}

/// What import did with one row, as `acks/<uid>` lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    /// Applied, or for a read request, answered.
    Imported,
    /// Waiting for the desk, as a ghost or a pre-selected pick.
    Held,
    /// Stale against its base: a person conflict card.
    Conflict,
    /// Not importable: malformed, or a command a row may not name.
    Refused,
}

/// `planroom.ack/v1`: `acks/<uid>`, one per viewer, which the page reads to clear its queued rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ack {
    /// `planroom.ack/v1`.
    pub format: String,
    /// The store `seq` the import that wrote it reached.
    pub imported_through: Seq,
    /// Each row's outcome, by its docId.
    pub rows: BTreeMap<String, AckStatus>,
}

/// One line of `import -`'s stdin: an inbox row from the observed `ArtifactData list` result,
/// with its path, which names the author, and its document version, which the delete pins. A line
/// with no `row` gives the version of a document the session wrote before (`live/head`,
/// `acks/<uid>`), so the next write to it can pin that version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportLine {
    /// `inbox/<uid>/events/<docId>`, or `live/head` or `acks/<uid>` for a version line.
    pub path: String,
    /// The row as the list result showed it; validated as an `InboxRow`.
    #[serde(default)]
    pub row: Option<Value>,
    /// The document's version as the list result showed it.
    #[serde(default)]
    pub version: Option<u64>,
}

/// One `db` write, in the shape an `ArtifactData batch` entry takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DbWrite {
    pub op: DbOp,
    /// The collection path, such as `answers` or `inbox/u_1/events`.
    pub collection: String,
    pub doc_id: String,
    /// The JSON file whose top-level object is the document, for `set` and `update`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// The version the document was read at; absent when the write creates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub if_version: Option<u64>,
}

/// A `db` write's operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DbOp {
    Set,
    Update,
    Delete,
}

/// The writes of one `ArtifactData batch` call: at most 50, and at most 1 MiB of documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DbBatch {
    pub writes: Vec<DbWrite>,
}

/// The exact arguments of the Artifact publish the session makes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PublishArgs {
    /// The page fragment.
    pub file_path: String,
    /// Each published path and the file it comes from.
    pub files: BTreeMap<String, String>,
    /// The page's capabilities: `db` with its access rules, and `user`.
    pub capabilities: Value,
    pub title: String,
    pub description: String,
    /// On the first publish.
    pub icon: String,
}

/// One file `share` wrote, with its size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SharedFile {
    /// The published path, such as `assets/main.js` or `data/snapshot.json`; the page itself is
    /// `page.html`.
    pub path: String,
    pub bytes: u64,
}

/// What `share` returns: the publish to make, the `db` writes that seed `live/head`, and what the
/// snapshot holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShareResult {
    /// What will be shared, from the snapshot's sections; the first publish shows it.
    pub summary: String,
    pub publish: PublishArgs,
    /// The `db` writes to make after the publish, `live/head` last.
    pub db: Vec<DbBatch>,
    pub files: Vec<SharedFile>,
    /// What the snapshot holds, as `Snapshot.capabilities` names it.
    pub holds: Vec<String>,
}

/// What `import` did with one stdin line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportedRow {
    pub path: String,
    pub command: Option<CommandName>,
    /// Its ack; none when the row stays in the inbox for a person's sync.
    pub status: Option<AckStatus>,
    /// One engine-authored line when the row wasn't imported.
    pub reason: Option<String>,
}

/// What `import -` returns: each row's outcome and the `db` batches the session writes, each with
/// one `ArtifactData batch` call, in order; `live/head` is the last write of the last batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImportResult {
    pub summary: String,
    /// Whether a person's token came with the rows.
    pub person: bool,
    pub rows: Vec<ImportedRow>,
    pub batches: Vec<DbBatch>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_inbox_row_has_no_actor_to_forge() {
        let row = json!({"format": INBOX_FORMAT, "instance": "i1", "counter": 1, "command": "map",
            "request": {}, "base": 3, "at": "2026-10-26T00:00:00Z", "actor": "person:alice"});
        let parsed: InboxRow = serde_json::from_value(row).unwrap();
        assert!(serde_json::to_value(&parsed).unwrap().get("actor").is_none());
        assert!(serde_json::from_value::<InboxRow>(json!({"format": INBOX_FORMAT})).is_err());
    }

    #[test]
    fn a_db_write_leaves_out_what_it_does_not_need() {
        let w = DbWrite {
            op: DbOp::Delete,
            collection: "inbox/u_1/events".into(),
            doc_id: "d1".into(),
            file_path: None,
            if_version: Some(2),
        };
        assert_eq!(
            serde_json::to_value(&w).unwrap(),
            json!({"op": "delete", "collection": "inbox/u_1/events", "doc_id": "d1", "if_version": 2})
        );
    }
}
