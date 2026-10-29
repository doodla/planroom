//! `planroom version` and `planroom schema [--commands | --command LEAF | --def NAME]`: what this
//! engine is and what it serves.

use planroom_contract::schema::{self as contract_schema, View};
use std::collections::{BTreeSet, VecDeque};

use planroom_contract::{
    Command, CommandList, CommandName, DefSchema, Handle, LeafSchema, SCHEMA_VERSION, SchemaReq,
    SchemaSummary, VersionInfo,
};
use planroom_kernel::keys;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::app::Reply;
use crate::app::error::AppError;
use crate::text;

/// sha256 of the catalogue as `schema --commands` lists it: the hash `cargo xtask gen` writes into
/// `plugin/scripts/engine.env` and `catalogue.ts`.
pub fn catalogue_hash() -> String {
    let listing = serde_json::to_string(&Command::listing()).expect("the listing serializes");
    keys::sha256_plain_hex(listing.as_bytes())
}

/// This binary: its version, commit, target, schema version and catalogue hash.
pub fn info() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        git_commit: option_env!("PLANROOM_GIT_COMMIT").map(str::to_owned),
        target: env!("PLANROOM_TARGET").into(),
        schema_version: SCHEMA_VERSION,
        catalogue_hash: catalogue_hash(),
    }
}

pub fn version() -> Result<Reply<VersionInfo>, AppError> {
    let info = info();
    let text = text::get(
        "version.summary",
        &[("version", &info.version), ("target", &info.target), ("hash", &&info.catalogue_hash[..12])],
    );
    Ok(Reply { result: info, seq: 0, warnings: vec![], text })
}

/// One of `schema`'s results.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SchemaReply {
    Summary(SchemaSummary),
    Commands(CommandList),
    Leaf(LeafSchema),
    Def(DefSchema),
}

/// Each leaf's result type, by its name in the schema. The request type comes from `Command`'s
/// own schema; `a_schema_handle_resolves_for_every_leaf` checks this table covers the catalogue.
const RESULTS: &[(&str, &str)] = &[
    ("map", "MapView"),
    ("find", "FindResult"),
    ("ask", "AskResult"),
    ("focus", "PaneModel"),
    ("scan", "ScanResult"),
    ("watch", "StreamFrame"),
    ("schema", "SchemaSummary"),
    ("version", "VersionInfo"),
    ("person init", "PersonKey"),
    ("serve", "ServeInfo"),
    ("share", "ShareResult"),
    ("import", "ImportResult"),
    ("doctor", "PackReport"),
    ("edit", "EditResult"),
    ("open", "OpenResult"),
    ("review", "ReviewResult"),
    ("intent new", "EditResult"),
    ("intent propose", "EditResult"),
    ("intent lint", "LintResult"),
    ("intent version", "EditResult"),
    ("intent list", "IntentListResult"),
    ("pack add", "PackSetResult"),
    ("pack remove", "PackSetResult"),
];

/// `schema --command LEAF`: the leaf's request and result types with their `$defs` closure.
fn leaf(name: &str) -> Result<LeafSchema, AppError> {
    let unknown = || {
        AppError::new(
            planroom_contract::ErrorCode::InvalidArgument,
            &[("what", &text::get("argument.unknown_leaf", &[("leaf", &name)]))],
        )
        .details(serde_json::json!({"field": "command"}))
    };
    let root = contract_schema::generate(View::Output);
    let defs = root["$defs"].as_object().ok_or_else(|| AppError::internal("the schema has no $defs"))?;
    let tag = name.replace(' ', "_");
    let request = defs["Command"]["oneOf"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["properties"]["command"]["const"] == tag.as_str())
        .and_then(|v| v["properties"]["args"]["$ref"].as_str())
        .and_then(|r| r.strip_prefix("#/$defs/"))
        .ok_or_else(unknown)?
        .to_owned();
    let result =
        RESULTS.iter().find(|(leaf, _)| *leaf == name).map(|(_, r)| (*r).to_owned()).ok_or_else(unknown)?;
    let asks = closure(defs, &request)?;
    let mut leaf = LeafSchema {
        summary: text::get("schema.leaf", &[("leaf", &name), ("request", &request), ("result", &result)]),
        defs: closure(defs, &result)?.into_iter().chain(asks.clone()).collect(),
        request,
        result,
        handles: Vec::new(),
    };
    // Within `schema`'s budget (DR-21): the shapes stay whole, and the descriptions go first from
    // the result's types, then from the request's, which a wrong argument's reader needs most.
    let brief = |key: &str, leaf: &LeafSchema| {
        text::get(key, &[("leaf", &name), ("request", &leaf.request), ("result", &leaf.result)])
    };
    if !fits(&leaf, &leaf.summary) {
        leaf.summary = brief("schema.leaf_brief", &leaf);
        for (def, body) in leaf.defs.iter_mut() {
            if !asks.contains_key(def) {
                undescribe(body);
            }
        }
    }
    if !fits(&leaf, &leaf.summary) {
        leaf.summary = brief("schema.leaf_bare", &leaf);
        leaf.defs.values_mut().for_each(undescribe);
    }
    if !fits(&leaf, &leaf.summary) {
        // Past that, the request's types and the result's own stay, and the result's other types
        // come nearest first while they fit, the rest each by its `schema --def` handle (DR-98).
        let must = asks.keys().cloned().chain([leaf.result.clone()]).collect();
        let all = std::mem::take(&mut leaf.defs);
        let probe = |defs: &Map<String, Value>, handles: &[Handle]| {
            let summary = brief("schema.leaf_split", &leaf);
            let reply = LeafSchema { defs: defs.clone(), handles: handles.to_vec(), summary, ..leaf.clone() };
            fits(&reply, &reply.summary)
        };
        let (defs, handles) = split(&all, &must, &leaf.result, probe);
        leaf.summary = brief("schema.leaf_split", &leaf);
        leaf.defs = defs;
        leaf.handles = handles;
    }
    Ok(leaf)
}

/// `schema --def NAME`: the type and its `$defs` closure, fitted as a leaf's are.
fn def(name: &str) -> Result<DefSchema, AppError> {
    let root = contract_schema::generate(View::Output);
    let defs = root["$defs"].as_object().ok_or_else(|| AppError::internal("the schema has no $defs"))?;
    if !defs.contains_key(name) {
        return Err(AppError::new(
            planroom_contract::ErrorCode::InvalidArgument,
            &[("what", &text::get("argument.unknown_def", &[("name", &name)]))],
        )
        .details(serde_json::json!({"field": "def"})));
    }
    let brief = |key: &str| text::get(key, &[("name", &name)]);
    let mut reply = DefSchema {
        summary: brief("schema.def"),
        name: name.to_owned(),
        defs: closure(defs, name)?,
        handles: vec![],
    };
    if !fits(&reply, &reply.summary) {
        reply.summary = brief("schema.def_bare");
        reply.defs.values_mut().for_each(undescribe);
    }
    if !fits(&reply, &reply.summary) {
        let all = std::mem::take(&mut reply.defs);
        let must = BTreeSet::from([name.to_owned()]);
        let probe = |defs: &Map<String, Value>, handles: &[Handle]| {
            let summary = brief("schema.def_split");
            let probe = DefSchema { defs: defs.clone(), handles: handles.to_vec(), summary, ..reply.clone() };
            fits(&probe, &probe.summary)
        };
        let (defs, handles) = split(&all, &must, name, probe);
        reply.summary = brief("schema.def_split");
        reply.defs = defs;
        reply.handles = handles;
    }
    Ok(reply)
}

/// Whether `reply` fits `schema`'s budget, with the envelope repeating `summary` beside it.
fn fits(reply: &impl Serialize, summary: &str) -> bool {
    let budget = Command::Schema(SchemaReq::default()).spec().budget_tokens as usize;
    let size = serde_json::to_string(reply).map_or(usize::MAX, |s| s.len());
    size + summary.len() + ENVELOPE_RESERVE_BYTES <= budget * 4
}

/// The types of `all` a reply keeps: every one of `must`, then the others a kept type references,
/// breadth first from `root` and by name within a step, each while `fits` still holds; and a
/// `schema --def` handle for each type a kept one references but the reply leaves out.
fn split(
    all: &Map<String, Value>,
    must: &BTreeSet<String>,
    root: &str,
    fits: impl Fn(&Map<String, Value>, &[Handle]) -> bool,
) -> (Map<String, Value>, Vec<Handle>) {
    let mut kept: Map<String, Value> =
        all.iter().filter(|(name, _)| must.contains(*name)).map(|(n, v)| (n.clone(), v.clone())).collect();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([root.to_owned()]);
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(body) = all.get(&name) else { continue };
        // Only a kept type queues its references, so whatever is queued is wanted.
        if !kept.contains_key(&name) {
            kept.insert(name.clone(), body.clone());
            if !fits(&kept, &left_out(&kept)) {
                kept.remove(&name);
            }
        }
        if kept.contains_key(&name) {
            queue.extend(referenced(body));
        }
    }
    let handles = left_out(&kept);
    (kept, handles)
}

/// Each name `value` references, sorted, once.
fn referenced(value: &Value) -> BTreeSet<String> {
    let mut names = Vec::new();
    refs(value, &mut names);
    names.into_iter().collect()
}

/// A `schema --def` handle for each type `kept` references but doesn't hold, by name.
fn left_out(kept: &Map<String, Value>) -> Vec<Handle> {
    let names: BTreeSet<String> =
        kept.values().flat_map(referenced).filter(|name| !kept.contains_key(name)).collect();
    names
        .into_iter()
        .map(|name| Handle {
            command: CommandName("schema".into()),
            label: text::label("schema.def_label", &[("def", &name)]),
            args: vec!["--def".into(), name],
        })
        .collect()
}

/// What the envelope around a `schema` reply takes besides the result and its summary line.
const ENVELOPE_RESERVE_BYTES: usize = 512;

/// `def` and every definition it references, by name.
fn closure(defs: &Map<String, Value>, def: &str) -> Result<Map<String, Value>, AppError> {
    let mut closure = Map::new();
    let mut todo = vec![def.to_owned()];
    while let Some(def) = todo.pop() {
        if closure.contains_key(&def) {
            continue;
        }
        let body =
            defs.get(&def).cloned().ok_or_else(|| AppError::internal(format!("no $defs entry {def}")))?;
        refs(&body, &mut todo);
        closure.insert(def, body);
    }
    Ok(closure)
}

/// A schema without its descriptions. Property names stay, even one named `description`.
fn undescribe(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            if map.get("description").is_some_and(Value::is_string) {
                map.remove("description");
            }
            for (key, value) in map.iter_mut() {
                match (key.as_str(), value) {
                    ("properties" | "$defs" | "patternProperties", Value::Object(named)) => {
                        named.values_mut().for_each(undescribe)
                    }
                    (_, value) => undescribe(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(undescribe),
        _ => {}
    }
}

/// Every `#/$defs/NAME` a schema value references.
fn refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) =
                map.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/"))
            {
                out.push(name.to_owned());
            }
            map.values().for_each(|v| refs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| refs(v, out)),
        _ => {}
    }
}

pub fn schema(req: &SchemaReq) -> Result<Reply<SchemaReply>, AppError> {
    if let Some(name) = &req.def {
        let def = def(name)?;
        let text = def.summary.clone();
        return Ok(Reply { result: SchemaReply::Def(def), seq: 0, warnings: vec![], text });
    }
    if let Some(name) = &req.command {
        let leaf = leaf(name)?;
        let text = leaf.summary.clone();
        return Ok(Reply { result: SchemaReply::Leaf(leaf), seq: 0, warnings: vec![], text });
    }
    if req.commands {
        let items = Command::listing();
        let summary = text::get("schema.commands", &[("count", &items.len())]);
        let text = std::iter::once(summary.clone())
            .chain(items.iter().map(|l| format!("  {:<10} {}", l.name, l.schema.label)))
            .collect::<Vec<_>>()
            .join("\n");
        let result = SchemaReply::Commands(CommandList { summary, items });
        return Ok(Reply { result, seq: 0, warnings: vec![], text });
    }
    let id = View::Output.id();
    let summary = text::get("schema.summary", &[("version", &SCHEMA_VERSION), ("id", &id)]);
    let handles = vec![Handle {
        command: CommandName("schema".into()),
        args: vec!["--commands".into()],
        label: text::label("schema.commands_label", &[]),
    }];
    let text = summary.clone();
    let result = SchemaReply::Summary(SchemaSummary { summary, id, schema_version: SCHEMA_VERSION, handles });
    Ok(Reply { result, seq: 0, warnings: vec![], text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undescribe_keeps_a_property_named_description() {
        let mut schema = serde_json::json!({
            "description": "A type.",
            "properties": {"description": {"type": "string", "description": "Its text."}},
            "anyOf": [{"description": "One arm.", "type": "null"}],
        });
        undescribe(&mut schema);
        assert_eq!(
            schema,
            serde_json::json!({"properties": {"description": {"type": "string"}}, "anyOf": [{"type": "null"}]})
        );
    }

    #[test]
    fn a_schema_handle_resolves_for_every_leaf() {
        for listed in Command::listing() {
            assert_eq!(listed.schema.args[0], "--command");
            let leaf =
                leaf(&listed.schema.args[1]).unwrap_or_else(|e| panic!("{}: {}", listed.name, e.message));
            assert!(leaf.defs.contains_key(&leaf.request) && leaf.defs.contains_key(&leaf.result));
        }
        assert!(leaf("nope").is_err());
    }

    #[test]
    fn every_leaf_holds_or_hands_off_each_type_it_references() {
        for listed in Command::listing() {
            let leaf = leaf(&listed.schema.args[1]).unwrap();
            assert!(fits(&leaf, &leaf.summary), "{}", listed.name);
            holds_or_hands_off(&leaf.defs, &leaf.handles, &listed.name);
            assert_eq!(leaf.handles.is_empty(), !leaf.summary.contains("schema --def"), "{}", leaf.summary);
        }
    }

    #[test]
    fn every_type_fits_and_hands_off_what_it_leaves_out() {
        let root = contract_schema::generate(View::Output);
        for name in root["$defs"].as_object().unwrap().keys() {
            let reply = def(name).unwrap();
            assert!(fits(&reply, &reply.summary), "{name}");
            assert!(reply.defs.contains_key(name), "{name}");
            holds_or_hands_off(&reply.defs, &reply.handles, name);
        }
        let unknown = def("NoSuchType").unwrap_err();
        assert_eq!(unknown.details["field"], "def");
    }

    /// Every type `defs` references is in `defs` or named by one of `handles`, and each handle is a
    /// `schema --def` of a type `defs` leaves out.
    fn holds_or_hands_off(defs: &Map<String, Value>, handles: &[Handle], what: &str) {
        let named: BTreeSet<&str> = handles
            .iter()
            .map(|h| {
                assert_eq!((h.command.0.as_str(), h.args.len(), h.args[0].as_str()), ("schema", 2, "--def"));
                h.args[1].as_str()
            })
            .collect();
        assert_eq!(named.len(), handles.len(), "{what}: one handle per type");
        for body in defs.values() {
            for name in referenced(body) {
                assert!(
                    defs.contains_key(&name) || named.contains(name.as_str()),
                    "{what}: {name} is missing"
                );
            }
        }
        for name in &named {
            assert!(!defs.contains_key(*name), "{what}: a handle for {name}, which the reply holds");
        }
    }
}
