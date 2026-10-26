//! `planroom version` and `planroom schema [--commands]`: what this engine is and what it serves.

use planroom_contract::schema::{self as contract_schema, View};
use planroom_contract::{
    Command, CommandList, CommandName, Handle, LeafSchema, SCHEMA_VERSION, SchemaReq, SchemaSummary,
    VersionInfo,
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

/// Either of `schema`'s two results.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SchemaReply {
    Summary(SchemaSummary),
    Commands(CommandList),
    Leaf(LeafSchema),
}

/// Each leaf's result type, by its name in the schema. The request type comes from `Command`'s
/// own schema; `a_schema_handle_resolves_for_every_leaf` checks this table covers the catalogue.
const RESULTS: &[(&str, &str)] = &[
    ("map", "MapView"),
    ("focus", "PaneModel"),
    ("scan", "ScanResult"),
    ("watch", "StreamFrame"),
    ("schema", "SchemaSummary"),
    ("version", "VersionInfo"),
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
    let mut closure = Map::new();
    let mut todo = vec![request.clone(), result.clone()];
    while let Some(def) = todo.pop() {
        if closure.contains_key(&def) {
            continue;
        }
        let body =
            defs.get(&def).cloned().ok_or_else(|| AppError::internal(format!("no $defs entry {def}")))?;
        refs(&body, &mut todo);
        closure.insert(def, body);
    }
    let summary = text::get("schema.leaf", &[("leaf", &name), ("request", &request), ("result", &result)]);
    Ok(LeafSchema { summary, request, result, defs: closure })
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
        label: text::get("schema.commands_label", &[]),
    }];
    let text = summary.clone();
    let result = SchemaReply::Summary(SchemaSummary { summary, id, schema_version: SCHEMA_VERSION, handles });
    Ok(Reply { result, seq: 0, warnings: vec![], text })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
