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
    /// Its state when its type is a question type; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<QuestionState>,
}

/// A question's state, derived on every read and never stored (DR-116).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuestionState {
    pub status: QuestionStatus,
    /// Answered with its assumption: the answer equals the assumption, both trimmed.
    pub kept: bool,
    /// The items resting on it: those its `rests_on` names and their descendants, each once.
    pub rests: u32,
    /// The stakes weight (low 1, medium 3, high 9; low when absent) times `rests`.
    pub score: u32,
    /// One of the top two open questions by rank.
    pub sticky: bool,
}

/// Where a question stands (DR-116).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    /// Something rests on it and nobody has answered.
    Open,
    /// A person's answer, or an accepted one.
    Answered,
    /// Answered in chat and not yet confirmed: arrives with M2's chat answers; no M1 write gives it.
    FromChat,
    /// Answered by a passed proof of concept: arrives with DR-118's POCs; no M1 write gives it.
    FromPoc,
    /// Nothing rests on it.
    Dropped,
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

/// The slice an engine-run plan stage sends Claude, `intent next`'s request tail: the project, the
/// stage, every item the plan holds, live and ghost, in the order `intent list` gives them, and,
/// for a stage that writes next actions, the blocks on the map they may touch. It carries no
/// `seq`, event ID, key, time, path or evidence, so its bytes, and the replay key over them,
/// depend only on the plan and the map. It has no `format` tag: it is never stored but inside a
/// `planroom.replay/v1` record's request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StageSlice {
    pub project: NodeId,
    /// The project's name.
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// The stage to draft, such as `outcome`.
    pub stage: String,
    pub items: Vec<SliceItem>,
    /// The items cut from the end to keep the slice within its bytes.
    pub items_left_out: u32,
    pub targets: Vec<SliceTarget>,
    /// The targets cut from the end to keep the slice within its bytes.
    pub targets_left_out: u32,
}

/// One plan item in a stage's slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SliceItem {
    /// Its key within the project.
    pub key: String,
    #[serde(rename = "type")]
    pub item_type: String,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// Its parent's key in the project, or `project`.
    pub parent: Option<String>,
    /// `applied` for an accepted item, `ghost` for one waiting for a person.
    pub status: Status,
    #[schemars(extend("x-redact" = true))]
    pub props: BTreeMap<String, Value>,
    /// The blocks it touches.
    pub touches: Vec<NodeId>,
}

/// A block on the map a stage's next actions may touch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SliceTarget {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
}

/// What an engine-run plan stage asks Claude to return: [`PlanProposal`]'s strict form. Structured
/// output refuses an open object, so each item's props are name-value pairs, and the engine maps
/// the reply back to a `PlanProposal` before it checks and writes it. Like the proposal, it is
/// read once and never stored on its own, so it carries no `format` tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftReply {
    /// The stage's items, new and re-proposed.
    pub items: Vec<DraftItem>,
}

/// One drafted item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftItem {
    /// The item's key: a new one, or an existing item's to re-propose it.
    pub key: String,
    /// The item type.
    #[serde(rename = "type")]
    pub item_type: String,
    pub name: String,
    /// A key of this reply, a key of the slice's items, or `project`; null keeps an existing
    /// item's parent and puts a new one under the project.
    pub parent: Option<String>,
    /// Block IDs from the slice's targets.
    pub touches: Vec<String>,
    pub props: Vec<DraftProp>,
}

/// One prop of a drafted item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftProp {
    pub name: String,
    pub value: DraftValue,
}

/// A prop's value: text, or a list of text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DraftValue {
    Text(String),
    List(Vec<String>),
}

/// The keywords a strict schema keeps.
const STRICT_KEYWORDS: [&str; 8] =
    ["type", "properties", "required", "additionalProperties", "items", "enum", "anyOf", "description"];

/// The JSON Schema a stage's reply must validate against, in the strict form structured output
/// takes: [`DraftReply`]'s derived schema with every subschema inlined, every object closed with
/// `additionalProperties: false` and every property required, an `Option` as `anyOf [T, null]`,
/// and no keyword but `type`, `properties`, `required`, `additionalProperties`, `items`, `enum`,
/// `anyOf` and `description`. An item's `type` takes one of `types` and a prop's `name` one of
/// `props`, each sorted and deduplicated, so the schema's bytes never depend on the order they
/// came in; object keys are sorted at every depth. It shapes the derived schema and nothing more:
/// `PlanProposal`'s own schema, with its open `props` map, is one structured output refuses.
pub fn draft_reply_schema(types: &[String], props: &[String]) -> Value {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| s.inline_subschemas = true);
    let mut schema = settings.into_generator().into_root_schema_for::<DraftReply>().to_value();
    strict(&mut schema);
    let set = |names: &[String]| {
        let names: std::collections::BTreeSet<&String> = names.iter().collect();
        Value::Array(names.into_iter().map(|n| Value::String(n.clone())).collect())
    };
    let item = &mut schema["properties"]["items"]["items"];
    item["properties"]["type"]["enum"] = set(types);
    item["properties"]["props"]["items"]["properties"]["name"]["enum"] = set(props);
    sorted(schema)
}

/// Close every object, require every property, spell a nullable type as `anyOf`, and drop every
/// keyword structured output doesn't take.
fn strict(schema: &mut Value) {
    let Value::Object(map) = schema else { return };
    map.retain(|k, _| STRICT_KEYWORDS.contains(&k.as_str()));
    if let Some(Value::Array(types)) = map.get("type").cloned() {
        // `["string", "null"]`: each type becomes its own branch, which takes the type's other
        // keywords, such as `items`; the description stays outside.
        map.remove("type");
        let description = map.remove("description");
        let rest = std::mem::take(map);
        let branches: Vec<Value> = types
            .into_iter()
            .map(|t| {
                let mut branch = serde_json::Map::new();
                if t != "null" {
                    branch.extend(rest.clone());
                }
                branch.insert("type".into(), t);
                Value::Object(branch)
            })
            .collect();
        if let Some(description) = description {
            map.insert("description".into(), description);
        }
        map.insert("anyOf".into(), Value::Array(branches));
    }
    if let Some(Value::Object(properties)) = map.get_mut("properties") {
        properties.values_mut().for_each(strict);
        let required: Vec<Value> = properties.keys().map(|k| Value::String(k.clone())).collect();
        map.insert("required".into(), Value::Array(required));
        map.insert("additionalProperties".into(), Value::Bool(false));
    }
    match map.get_mut("items") {
        Some(Value::Array(list)) => list.iter_mut().for_each(strict),
        Some(value) => strict(value),
        None => {}
    }
    if let Some(Value::Array(branches)) = map.get_mut("anyOf") {
        branches.iter_mut().for_each(strict);
    }
}

/// `value` with every object's keys sorted.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde_json::json;

    use super::*;

    /// Every keyword `schema` uses, and whether each object in it is closed and requires every
    /// property it names.
    fn walk(schema: &Value, found: &mut BTreeSet<String>, closed: &mut bool) {
        match schema {
            Value::Object(map) => {
                if map.get("type") == Some(&json!("object")) {
                    *closed &= map.get("additionalProperties") == Some(&json!(false));
                    let named = map["properties"].as_object().map_or(0, |p| p.len());
                    *closed &= map["required"].as_array().map_or(0, |r| r.len()) == named;
                }
                for (k, v) in map {
                    found.insert(k.clone());
                    match k.as_str() {
                        "properties" => v.as_object().unwrap().values().for_each(|v| walk(v, found, closed)),
                        "enum" | "required" | "description" | "type" | "additionalProperties" => {}
                        _ => walk(v, found, closed),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, found, closed)),
            _ => {}
        }
    }

    #[test]
    fn the_draft_schema_is_strict_and_order_free() {
        let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let schema = draft_reply_schema(&names(&["work", "risk", "work"]), &names(&["scope", "method"]));
        let (mut found, mut closed) = (BTreeSet::new(), true);
        walk(&schema, &mut found, &mut closed);
        assert!(closed, "every object is closed and requires every property: {schema}");
        let allowed: BTreeSet<String> = STRICT_KEYWORDS.iter().map(|k| (*k).to_owned()).collect();
        assert!(found.is_subset(&allowed), "{:?}", found.difference(&allowed).collect::<Vec<_>>());
        let item = &schema["properties"]["items"]["items"];
        assert_eq!(item["properties"]["type"]["enum"], json!(["risk", "work"]));
        let prop = &item["properties"]["props"]["items"];
        assert_eq!(prop["properties"]["name"]["enum"], json!(["method", "scope"]));
        // An Option is spelled anyOf [T, null].
        let parent = &item["properties"]["parent"]["anyOf"];
        assert_eq!(parent.as_array().map(Vec::len), Some(2), "{schema}");
        assert_eq!(parent[1], json!({"type": "null"}), "{schema}");
        // Permuted inputs give the same bytes.
        let again = draft_reply_schema(&names(&["risk", "work"]), &names(&["method", "scope", "method"]));
        assert_eq!(serde_json::to_vec(&schema).unwrap(), serde_json::to_vec(&again).unwrap());
        // A reply in its shape parses as a DraftReply.
        let reply: DraftReply = serde_json::from_value(json!({"items": [{"key": "a", "type": "work",
            "name": "A", "parent": null, "touches": [], "props": [{"name": "scope", "value": ["x"]},
            {"name": "method", "value": "HTN"}]}]}))
        .unwrap();
        assert_eq!(reply.items[0].props[0].value, DraftValue::List(vec!["x".into()]));
    }
}
