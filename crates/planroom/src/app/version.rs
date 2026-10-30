//! `planroom version` and `planroom schema [--commands [--cursor C] | --command LEAF | --def
//! NAME]`: what this engine is and what it serves.

use planroom_contract::schema::{self as contract_schema, View};
use std::collections::{BTreeSet, VecDeque};

use planroom_contract::{
    Command, CommandList, CommandName, Cursor, DefSchema, Handle, LeafListing, LeafSchema, SCHEMA_VERSION,
    SchemaReq, SchemaSummary, VersionInfo,
};
use planroom_kernel::keys;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::app::error::AppError;
use crate::app::{Paged, Reply};
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
    ("focus", "FocusResult"),
    ("scan", "ScanResult"),
    ("watch", "StreamFrame"),
    ("schema", "SchemaSummary"),
    ("version", "VersionInfo"),
    ("person init", "PersonKey"),
    ("serve", "ServeInfo"),
    ("share", "ShareResult"),
    ("import", "ImportResult"),
    ("doctor", "DoctorResult"),
    ("edit", "EditResult"),
    ("open", "OpenResult"),
    ("review", "ReviewResult"),
    ("intent new", "EditResult"),
    ("intent next", "EditResult"),
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
        let (defs, handles) = split(&all, &must, &leaf.result, &BTreeSet::new(), probe);
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
        // `Command` names every leaf's request type, more than one reply holds with a handle for
        // each; the ones it leaves out go by one `schema --commands` handle, whose listing names
        // each leaf's `schema --command`, which returns that leaf's request type (DR-109).
        let listed = if name == "Command" { requests(defs) } else { BTreeSet::new() };
        let split_summary = |handles: &[Handle]| {
            let by_listing = handles.iter().any(|h| h.args == ["--commands"]);
            brief(if by_listing { "schema.def_commands_split" } else { "schema.def_split" })
        };
        let probe = |defs: &Map<String, Value>, handles: &[Handle]| {
            let summary = split_summary(handles);
            let probe = DefSchema { defs: defs.clone(), handles: handles.to_vec(), summary, ..reply.clone() };
            fits(&probe, &probe.summary)
        };
        let (defs, handles) = split(&all, &must, name, &listed, probe);
        reply.summary = split_summary(&handles);
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
/// breadth first from `root` and by name within a step, each while `fits` still holds; and the
/// handles [`left_out`] gives for the types a kept one references but the reply leaves out.
fn split(
    all: &Map<String, Value>,
    must: &BTreeSet<String>,
    root: &str,
    listed: &BTreeSet<String>,
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
            if !fits(&kept, &left_out(&kept, listed)) {
                kept.remove(&name);
            }
        }
        if kept.contains_key(&name) {
            queue.extend(referenced(body));
        }
    }
    let handles = left_out(&kept, listed);
    (kept, handles)
}

/// Each name `value` references, sorted, once.
fn referenced(value: &Value) -> BTreeSet<String> {
    let mut names = Vec::new();
    refs(value, &mut names);
    names.into_iter().collect()
}

/// A handle for the types `kept` references but doesn't hold: one `schema --commands` for those
/// of `listed` (leaves' request types, which that listing reaches), and a `schema --def` for each
/// other one, by name.
fn left_out(kept: &Map<String, Value>, listed: &BTreeSet<String>) -> Vec<Handle> {
    let names: BTreeSet<String> =
        kept.values().flat_map(referenced).filter(|name| !kept.contains_key(name)).collect();
    let (by_listing, by_def): (Vec<String>, Vec<String>) =
        names.into_iter().partition(|name| listed.contains(name));
    let mut handles: Vec<Handle> = by_def
        .into_iter()
        .map(|name| Handle {
            command: CommandName("schema".into()),
            label: text::label("schema.def_label", &[("def", &name)]),
            args: vec!["--def".into(), name],
        })
        .collect();
    if !by_listing.is_empty() {
        handles.push(Handle {
            command: CommandName("schema".into()),
            args: vec!["--commands".into()],
            label: text::label("schema.commands_label", &[]),
        });
    }
    handles
}

/// Every leaf's request type: what each of `Command`'s variants takes as its `args`.
fn requests(defs: &Map<String, Value>) -> BTreeSet<String> {
    defs.get("Command")
        .and_then(|command| command["oneOf"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|variant| variant["properties"]["args"]["$ref"].as_str())
        .filter_map(|r| r.strip_prefix("#/$defs/"))
        .map(str::to_owned)
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

pub fn schema(req: &SchemaReq) -> Result<Paged<SchemaReply>, AppError> {
    if let Some(name) = &req.def {
        let def = def(name)?;
        let text = def.summary.clone();
        return Ok(Paged::whole(Reply { result: SchemaReply::Def(def), seq: 0, warnings: vec![], text }));
    }
    if let Some(name) = &req.command {
        let leaf = leaf(name)?;
        let text = leaf.summary.clone();
        return Ok(Paged::whole(Reply { result: SchemaReply::Leaf(leaf), seq: 0, warnings: vec![], text }));
    }
    if req.commands {
        return commands(req.cursor.as_ref());
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
    Ok(Paged::whole(Reply { result, seq: 0, warnings: vec![], text }))
}

/// `schema --commands [--cursor C]`: the catalogue's leaves in order, as many as fit `schema`'s
/// budget from the cursor's place, each whole; `meta.next` names the place of the page after.
/// Pages are cut from the first leaf, so a catalogue always cuts the same ones, and a cursor is
/// read only where it names a page's last leaf in this catalogue: the pages join to the whole
/// listing the catalogue hash covers.
fn commands(cursor: Option<&Cursor>) -> Result<Paged<SchemaReply>, AppError> {
    let all = Command::listing();
    let catalogue = listing_hash();
    let start = match cursor {
        None => 0,
        Some(c) => {
            let unreadable = || AppError::invalid_argument("cursor", "schema.cursor_unreadable");
            let mark = decode(c).ok_or_else(unreadable)?;
            if mark.seq != 0 || mark.catalogue != catalogue {
                return Err(unreadable());
            }
            let mut start = 0;
            loop {
                let end = page_end(&all, start, &catalogue);
                if end == all.len() {
                    return Err(unreadable());
                }
                start = end;
                if all[end - 1].name == mark.last {
                    break start;
                }
            }
        }
    };
    let end = page_end(&all, start, &catalogue);
    let list = page(&all, start, end, &catalogue);
    let text = std::iter::once(list.summary.clone())
        .chain(list.items.iter().map(|l| format!("  {:<10} {}", l.name, l.schema.label)))
        .collect::<Vec<_>>()
        .join("\n");
    let next = (end < all.len()).then(|| encode(&all[end - 1].name, &catalogue));
    let reply = Reply { result: SchemaReply::Commands(list), seq: 0, warnings: vec![], text };
    Ok(Paged { next, ..Paged::whole(reply) })
}

/// The catalogue hash a `schema --commands` cursor carries: its first 16 hex digits.
fn listing_hash() -> String {
    let mut hash = catalogue_hash();
    hash.truncate(16);
    hash
}

/// The leaves `all[start..end]` as one page, its summary naming the cursor of the page after.
fn page(all: &[LeafListing], start: usize, end: usize, catalogue: &str) -> CommandList {
    let items = all[start..end].to_vec();
    let summary = if start == 0 && end == all.len() {
        text::get("schema.commands", &[("count", &all.len())])
    } else if end == all.len() {
        text::get(
            "schema.commands_last_page",
            &[("first", &(start + 1)), ("last", &end), ("count", &all.len())],
        )
    } else {
        let more = encode(&all[end - 1].name, catalogue).0;
        text::get(
            "schema.commands_page",
            &[("first", &(start + 1)), ("last", &end), ("count", &all.len()), ("cursor", &more)],
        )
    };
    CommandList { summary, items }
}

/// Where the page that starts at `start` ends. Each leaf whole: the page grows while the next
/// leaf still fits, and holds one at least.
fn page_end(all: &[LeafListing], start: usize, catalogue: &str) -> usize {
    let mut end = start + 1;
    while end < all.len() {
        let wider = page(all, start, end + 1, catalogue);
        if !fits(&wider, &wider.summary) {
            break;
        }
        end += 1;
    }
    end
}

/// Where a `schema --commands` page ends: the `seq` it read, always 0 since the catalogue is the
/// binary's, never the store's; the catalogue's hash; and the page's last leaf.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    #[serde(rename = "s")]
    seq: u64,
    #[serde(rename = "f")]
    catalogue: String,
    #[serde(rename = "i")]
    last: String,
}

/// The cursor of the page after the one whose last leaf is `last`: its mark as JSON,
/// hex-encoded, so it passes as one shell word.
fn encode(last: &str, catalogue: &str) -> Cursor {
    let mark = Mark { seq: 0, catalogue: catalogue.into(), last: last.into() };
    let json = serde_json::to_vec(&mark).expect("a mark serializes");
    Cursor(json.iter().map(|b| format!("{b:02x}")).collect())
}

/// The mark a cursor holds, or `None` for one that isn't hex-encoded JSON of one.
fn decode(cursor: &Cursor) -> Option<Mark> {
    let hex = cursor.0.as_bytes();
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16);
    let bytes: Option<Vec<u8>> =
        hex.chunks(2).map(|pair| Some((digit(pair[0])? * 16 + digit(pair[1])?) as u8)).collect();
    serde_json::from_slice(&bytes?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_schema_commands_cursor_from_another_catalogue_or_off_a_page_edge_is_refused() {
        let all = Command::listing();
        let catalogue = listing_hash();
        let refused = |c: Cursor| {
            let Err(err) = commands(Some(&c)) else { panic!("{c:?} was read") };
            assert_eq!(err.code, planroom_contract::ErrorCode::InvalidArgument, "{c:?}");
        };
        let first = commands(None).unwrap();
        let given = first.next.clone().expect("the catalogue takes more than one page");
        let edge = decode(&given).unwrap();
        assert_eq!(edge, Mark { seq: 0, catalogue: catalogue.clone(), last: edge.last.clone() });
        // The cursor a page gave reads the page after, from the leaf after its last.
        let SchemaReply::Commands(after) = commands(Some(&given)).unwrap().reply.result else { panic!() };
        let at = all.iter().position(|l| l.name == edge.last).unwrap();
        assert_eq!(after.items[0].name, all[at + 1].name);
        // The same leaf in another catalogue, a leaf no page ends on, the last leaf, and a store
        // seq: none is a cursor any page gave.
        refused(encode(&edge.last, "0000000000000000"));
        refused(encode(&all[0].name, &catalogue));
        refused(encode(&all[all.len() - 1].name, &catalogue));
        let json = serde_json::to_vec(&Mark { seq: 7, ..edge }).unwrap();
        refused(Cursor(json.iter().map(|b| format!("{b:02x}")).collect()));
    }

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

    #[test]
    fn schema_def_command_hands_its_leaf_requests_off_by_one_handle() {
        let reply = def("Command").unwrap();
        assert!(fits(&reply, &reply.summary), "Command doesn't fit");
        // Every variant stays, with its `$ref`, and no description.
        let variants = reply.defs["Command"]["oneOf"].as_array().unwrap();
        assert_eq!(variants.len(), Command::listing().len());
        assert!(!serde_json::to_string(&reply.defs["Command"]).unwrap().contains("\"description\""));
        let listing: Vec<&Handle> = reply.handles.iter().filter(|h| h.args == ["--commands"]).collect();
        assert_eq!(listing.len(), 1, "{:?}", reply.handles);
        assert!(reply.summary.contains("schema --commands"), "{}", reply.summary);
        // Following the handle: the listing's pages name each leaf's `schema --command`, and
        // between them those give every request type the reply leaves out.
        let mut requests = BTreeSet::new();
        let mut cursor = None;
        loop {
            let page = commands(cursor.as_ref()).unwrap();
            let SchemaReply::Commands(list) = page.reply.result else { panic!("not a listing") };
            for listed in list.items {
                assert_eq!(listed.schema.args[0], "--command");
                requests.insert(leaf(&listed.schema.args[1]).unwrap().request);
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let by_def: BTreeSet<&str> =
            reply.handles.iter().filter(|h| h.args[0] == "--def").map(|h| h.args[1].as_str()).collect();
        let behind: BTreeSet<String> = reply
            .defs
            .values()
            .flat_map(referenced)
            .filter(|name| !reply.defs.contains_key(name) && !by_def.contains(name.as_str()))
            .collect();
        assert!(!behind.is_empty() && behind.is_subset(&requests), "{behind:?}");
        assert!(by_def.iter().all(|name| !requests.contains(*name)), "{by_def:?}");
    }

    /// Every type `defs` references is in `defs` or named by one of `handles`, and each handle is a
    /// `schema --def` of a type `defs` leaves out. `Command` alone may instead hand off the leaves'
    /// request types it leaves out by one `schema --commands` handle, which then covers exactly
    /// those, each the request type of a catalogue leaf.
    fn holds_or_hands_off(defs: &Map<String, Value>, handles: &[Handle], what: &str) {
        let (listing, by_def): (Vec<&Handle>, Vec<&Handle>) =
            handles.iter().partition(|h| h.command.0 == "schema" && h.args == ["--commands"]);
        let named: BTreeSet<&str> = by_def
            .iter()
            .map(|h| {
                assert_eq!((h.command.0.as_str(), h.args.len(), h.args[0].as_str()), ("schema", 2, "--def"));
                h.args[1].as_str()
            })
            .collect();
        assert_eq!(named.len(), by_def.len(), "{what}: one handle per type");
        let missing: BTreeSet<String> = defs
            .values()
            .flat_map(referenced)
            .filter(|name| !defs.contains_key(name) && !named.contains(name.as_str()))
            .collect();
        if what == "Command" {
            assert!(listing.len() <= 1, "{what}: one schema --commands handle at most");
            let requests: BTreeSet<String> =
                Command::listing().iter().map(|l| leaf(&l.schema.args[1]).unwrap().request).collect();
            assert_eq!(listing.is_empty(), missing.is_empty(), "{what}: {missing:?}");
            assert!(missing.is_subset(&requests), "{what}: {missing:?} aren't all leaf requests");
            for name in &named {
                assert!(!requests.contains(*name), "{what}: {name} goes by schema --commands");
            }
        } else {
            assert!(listing.is_empty(), "{what}: only Command hands off by schema --commands");
            assert!(missing.is_empty(), "{what}: {missing:?} missing");
        }
        for name in &named {
            assert!(!defs.contains_key(*name), "{what}: a handle for {name}, which the reply holds");
        }
    }
}
