//! Plan proposals and what the `intent` leaves return (architecture.md, "Planning: stages, ghosts
//! and lint (R5)"; DR-23, DR-45).
//!
//! A project is the `kernel.project` node `plan:<slug>:project`, and a plan item the `kernel.item`
//! node `plan:<slug>:<key>` whose `type` prop names an item type a plan stage declares. What an item
//! touches is a `kernel.touches` edge from it.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::WriteCommon;
use crate::envelope::{Cursor, Empty, Handle};
use crate::event::{EventId, Status};
use crate::model::{NodeId, Seq};

/// One batch of plan items, read once from stdin or `--file` by `intent propose` and turned into
/// events. It is a request body, never stored or read back, so it carries no `format` tag: the
/// command catalogue's requests and results are its contract. It is also the one schema every
/// plan stage proposes in.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanProposal {
    /// The project: its full ID, its slug alone or its exact name; the only project when absent.
    #[serde(default)]
    pub project: Option<String>,
    /// The stage the batch belongs to; when given, every item's type is one it writes.
    #[serde(default)]
    pub stage: Option<String>,
    pub items: Vec<ProposedItem>,
}

/// One proposed item. A key the project's model holds sets the fields that differ; any other key
/// is a new item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposedItem {
    /// The item's key within its project: `[a-z0-9][a-z0-9_-]{0,63}`, never `project`.
    pub key: String,
    /// The item type, such as `next_action`, which a loaded plan stage declares.
    #[serde(rename = "type")]
    pub item_type: String,
    pub name: String,
    /// A key in this batch, a key of the project's items, or a full item ID of the project. When
    /// absent, a new item goes under the project and an existing one keeps its parent.
    #[serde(default)]
    pub parent: Option<String>,
    /// The props the item type's schema declares, by name.
    #[serde(default)]
    pub props: BTreeMap<String, Value>,
    /// What the item touches: each a node ID, an ID suffix or a name on the map.
    #[serde(default)]
    pub touches: Vec<String>,
}

/// `planroom intent new TEXT`: a new project, named by `TEXT`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct IntentNewReq {
    /// What the plan is for, such as "Add retries to the worker".
    #[arg(value_name = "TEXT")]
    pub text: String,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom intent version LABEL`: a person's tag on the project's plan at the current `seq`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct IntentVersionReq {
    /// The tag, such as `v2`, which `diff --from` takes back.
    #[arg(value_name = "LABEL")]
    pub label: String,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom intent list [--project P] [--cursor C]`: every project, and the items of the named or
/// only one.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct IntentListReq {
    /// The project whose items to list: its full ID, its slug or its exact name.
    #[arg(long)]
    #[serde(default)]
    pub project: Option<String>,
    /// The cursor `meta.next` gave, to read the page after it.
    #[arg(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

/// What `intent list` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentListResult {
    /// One engine-authored line, which can name the projects.
    #[schemars(extend("x-redact" = true))]
    pub summary: String,
    pub projects: Vec<PlanProject>,
    /// The named or only project's items: live ones first, then ghosts, each by stage order, then
    /// ID.
    pub items: Vec<PlanItem>,
    pub empty: Option<Empty>,
}

/// One project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanProject {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// `applied`, or `ghost` for an agent's project no person has accepted.
    pub status: Status,
    /// Its items in the model.
    pub items: u32,
    /// Its items and item edits waiting as ghosts.
    pub ghosts: u32,
    pub versions: Vec<PlanVersion>,
}

/// A version tag on a project's plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanVersion {
    /// A handle a caller passes back, such as `diff --from v1`, so it is never masked; `intent
    /// version` refuses a label redaction would change.
    pub label: String,
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Seq,
}

/// One plan item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanItem {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// Its `type` prop, such as `next_action`.
    pub item_type: String,
    /// The first stage that writes its type.
    pub stage: Option<String>,
    /// `applied` for an item in the model, `ghost` for one only a pending ghost holds.
    pub status: Status,
    pub parent: Option<NodeId>,
    /// What it touches: its `kernel.touches` edges' targets, pending ones included.
    pub touches: Vec<NodeId>,
    /// Its pending ghost edits, in `seq` order.
    pub pending: Vec<EventId>,
    /// The review a person runs to accept each of its first pending ghosts.
    pub handles: Vec<Handle>,
}

/// What `intent lint` returns: the project's structural lint findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LintResult {
    pub project: Option<NodeId>,
    /// One engine-authored line, which can name the items.
    #[schemars(extend("x-redact" = true))]
    pub summary: String,
    pub findings: Vec<LintFinding>,
    pub empty: Option<Empty>,
}

/// One rule a subject breaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LintFinding {
    /// The rule's ID, as its pack declares it.
    pub rule: String,
    /// The first stage whose lint runs the rule.
    pub stage: Option<String>,
    pub subject: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// The rule's message: pack text.
    pub message: String,
    /// The values the rule found missing or shared, such as a scope tag no child covers.
    #[schemars(extend("x-redact" = true))]
    pub values: Vec<String>,
    /// The other items involved, such as two children that overlap.
    pub related: Vec<NodeId>,
    /// `ask stage STAGE PROJECT`, the stage's checklist.
    pub handles: Vec<Handle>,
}
