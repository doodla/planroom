//! The schema's two gates (Contracts, Versioning rules): the integer lint and the breaking-change
//! guard.
//!
//! The lint fails on a 64-bit integer without the `maximum` that keeps it below 2^53. The guard
//! compares the schema `cargo xtask gen` writes with the committed baseline in `schema/baseline/`,
//! and, once a release exists, with the schema at its `v*` tag. It fails on a removed type,
//! property, enum value or variant, and on a new required input field, unless `schema_version` was
//! bumped. The baseline moves only with that bump, through `cargo xtask gen --schema-baseline`
//! [DR-59]. Nothing here reads the network or a branch tip: a tag the guard needs and can't find
//! fails the check.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use planroom_contract::SCHEMA_VERSION;
use planroom_contract::schema::{self, View};
use serde_json::Value;

use crate::util;

/// Where `gen --schema-baseline` writes the two views.
pub const BASELINE: &str = "schema/baseline";

/// The release notes whose `## [X.Y.Z]` headings name the releases, newest first.
pub const CHANGELOG: &str = "CHANGELOG.md";

/// The integer formats schemars writes for 64-bit and pointer-sized integers, which can pass 2^53.
const WIDE_INTEGERS: [&str; 4] = ["uint64", "int64", "uint", "int"];

/// 2^53 - 1: the largest integer a JSON reader holds exactly.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Every wide integer in `schema` without a `maximum` at or below 2^53 - 1 (and, when signed, a
/// `minimum` at or above its negative), by JSON pointer.
pub fn lint(schema: &Value) -> Vec<String> {
    let mut out = Vec::new();
    lint_at(schema, String::new(), &mut out);
    out
}

fn lint_at(v: &Value, at: String, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if let Some(format) = map.get("format").and_then(Value::as_str)
                && WIDE_INTEGERS.contains(&format)
            {
                let max_ok =
                    map.get("maximum").and_then(Value::as_f64).is_some_and(|m| m <= MAX_SAFE_INTEGER as f64);
                let signed = !format.starts_with('u');
                let min_ok = !signed
                    || map
                        .get("minimum")
                        .and_then(Value::as_f64)
                        .is_some_and(|m| m >= -(MAX_SAFE_INTEGER as f64));
                if !(max_ok && min_ok) {
                    let at = if at.is_empty() { "/".to_owned() } else { at.clone() };
                    out.push(format!(
                        "{at}: a {format} without `maximum` {MAX_SAFE_INTEGER}; add #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))] to its field"
                    ));
                }
            }
            for (k, child) in map {
                lint_at(child, format!("{at}/{}", pointer_escape(k)), out);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                lint_at(child, format!("{at}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// The breaking changes from `old` to `new` in one view: a removed type, property, enum value or
/// variant, and, in the input view, a field that became required in a type that already existed.
/// An added type, property, value, variant or optional field, and a new required output field, are
/// additive.
pub fn breaking_changes(old: &Value, new: &Value, view: View) -> Vec<String> {
    let mut out = Vec::new();
    let empty = serde_json::Map::new();
    let old_defs = old.get("$defs").and_then(Value::as_object).unwrap_or(&empty);
    let new_defs = new.get("$defs").and_then(Value::as_object).unwrap_or(&empty);
    for (name, old_def) in old_defs {
        match new_defs.get(name) {
            None => out.push(format!("$defs/{name}: the type was removed")),
            Some(new_def) => compare(old_def, new_def, &format!("$defs/{name}"), view, &mut out),
        }
    }
    out
}

fn compare(old: &Value, new: &Value, at: &str, view: View, out: &mut Vec<String>) {
    let (Some(old), Some(new)) = (old.as_object(), new.as_object()) else { return };
    let empty = serde_json::Map::new();
    let old_props = old.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    let new_props = new.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    for (name, old_prop) in old_props {
        match new_props.get(name) {
            None => out.push(format!("{at}: property `{name}` was removed")),
            Some(new_prop) => compare(old_prop, new_prop, &format!("{at}/{name}"), view, out),
        }
    }
    if let Some(old_enum) = old.get("enum").and_then(Value::as_array) {
        let new_enum = new.get("enum").and_then(Value::as_array).cloned().unwrap_or_default();
        for value in old_enum.iter().filter(|v| !new_enum.contains(v)) {
            out.push(format!("{at}: enum value {value} was removed"));
        }
    }
    if view == View::Input {
        let names = |m: &serde_json::Map<String, Value>| -> BTreeSet<String> {
            m.get("required")
                .and_then(Value::as_array)
                .map(|r| r.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default()
        };
        for name in names(new).difference(&names(old)) {
            out.push(format!("{at}: input field `{name}` became required"));
        }
    }
    for key in ["items", "additionalProperties", "not"] {
        if let (Some(o), Some(n)) = (old.get(key), new.get(key)) {
            compare(o, n, &format!("{at}/{key}"), view, out);
        }
    }
    for key in ["anyOf", "oneOf", "allOf", "prefixItems"] {
        let Some(old_items) = old.get(key).and_then(Value::as_array) else { continue };
        let new_items = new.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        for (id, old_item, new_item) in pair_variants(old_items, &new_items) {
            match new_item {
                None => out.push(format!("{at}/{key}: variant {id} was removed")),
                Some(new_item) => compare(old_item, new_item, &format!("{at}/{key}[{id}]"), view, out),
            }
        }
    }
}

/// Each old variant of a union with the new variant it became, or `None` when it's gone, so an
/// added variant never shifts the others. A variant with a name keeps it: its `$ref`, `const`,
/// open-set `enum` or `pattern`, the constant of its tag property, or, with no properties, its
/// type; two variants with one name are told apart by their order. A variant of an untagged or
/// externally tagged union has only its properties, and a field added to it adds to them (and, in
/// the output view, to its required names), so it pairs with the unpaired new variant that has all
/// its properties and the fewest others. One no new variant covers was removed or lost a property.
fn pair_variants<'a>(old: &'a [Value], new: &'a [Value]) -> Vec<(String, &'a Value, Option<&'a Value>)> {
    let new_named = named(new);
    let mut out: Vec<(String, &Value, Option<&Value>)> =
        named(old).into_iter().map(|(id, o)| (id.clone(), o, new_named.get(&id).copied())).collect();
    let mut unpaired: Vec<(&Value, BTreeSet<&str>)> =
        new.iter().filter(|item| name(item).is_none()).map(|item| (item, fields(item))).collect();
    for item in old.iter().filter(|item| name(item).is_none()) {
        let have = fields(item);
        let best = unpaired
            .iter()
            .enumerate()
            .filter(|(_, (_, f))| f.is_superset(&have))
            .min_by_key(|(i, (_, f))| (f.len(), *i))
            .map(|(i, _)| i);
        let id = format!("fields {}", have.into_iter().collect::<Vec<_>>().join(","));
        out.push((id, item, best.map(|i| unpaired.remove(i).0)));
    }
    out
}

/// The named variants of a union by name.
fn named(items: &[Value]) -> BTreeMap<String, &Value> {
    let mut out = BTreeMap::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        let Some(base) = name(item) else { continue };
        let n = seen.entry(base.clone()).or_default();
        let id = if *n == 0 { base } else { format!("{base}#{n}") };
        *n += 1;
        out.insert(id, item);
    }
    out
}

/// A variant's name, or `None` for one known only by its properties.
fn name(item: &Value) -> Option<String> {
    if let Some(r) = item.get("$ref").and_then(Value::as_str) {
        return Some(format!("$ref {r}"));
    }
    if let Some(c) = item.get("const") {
        return Some(format!("const {c}"));
    }
    if item.get("enum").is_some() {
        return Some("enum".into());
    }
    if let Some(p) = item.get("pattern").and_then(Value::as_str) {
        return Some(format!("pattern {p}"));
    }
    if let Some(props) = item.get("properties").and_then(Value::as_object)
        && !props.is_empty()
    {
        let tags: Vec<String> =
            props.iter().filter_map(|(name, p)| p.get("const").map(|c| format!("{name}={c}"))).collect();
        return (!tags.is_empty()).then(|| format!("tag {}", tags.join(",")));
    }
    Some(match item.get("type") {
        Some(t) => format!("type {t}"),
        None => "untyped".into(),
    })
}

/// A variant's property names.
fn fields(item: &Value) -> BTreeSet<&str> {
    item.get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// The schema version a view's file name carries: `1` from `planroom.v1.input.schema.json`.
pub fn version_of(file_name: &str) -> Option<(u32, View)> {
    let rest = file_name.strip_prefix("planroom.v")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let view = match &rest[digits.len()..] {
        ".schema.json" => View::Output,
        ".input.schema.json" => View::Input,
        _ => return None,
    };
    Some((digits.parse().ok()?, view))
}

/// One comparison's other side: the views it holds, by view, with their schema version.
struct Side {
    what: String,
    version: u32,
    views: Vec<(View, Value)>,
}

fn parse_view(what: &str, text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| format!("{what}: {e}"))
}

/// Read a side's two views from `names` (file names) with `read`.
fn side(
    what: String,
    names: &[String],
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Side, String> {
    let mut version = None;
    let mut views = Vec::new();
    for name in names {
        let Some((v, view)) = version_of(name) else { continue };
        if version.is_some_and(|known| known != v) {
            return Err(format!("{what} holds schema files of two versions"));
        }
        version = Some(v);
        views.push((view, parse_view(&format!("{what}/{name}"), &read(name)?)?));
    }
    let version = version.ok_or_else(|| format!("{what} holds no planroom.vN schema file"))?;
    for view in View::ALL {
        if !views.iter().any(|(v, _)| *v == view) {
            return Err(format!(
                "{what} has no {} view",
                if view == View::Input { "input" } else { "output" }
            ));
        }
    }
    Ok(Side { what, version, views })
}

fn baseline(root: &Path) -> Result<Option<Side>, String> {
    let dir = root.join(BASELINE);
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| format!("{BASELINE}: {e}"))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    let read = |name: &str| fs::read_to_string(dir.join(name)).map_err(|e| format!("{BASELINE}/{name}: {e}"));
    side(format!("{BASELINE}/"), &names, &read).map(Some)
}

/// The newest release `CHANGELOG.md` names before the tree's own version `own`, as `X.Y.Z`; none
/// before the first release. A release commit sets `own` to `X.Y.Z` and dates its section before
/// the owner tags it (Architecture, Versions and branches), so that section is the tree itself,
/// not a release to compare with.
pub fn last_release(changelog: &str, own: Option<&str>) -> Option<String> {
    changelog.lines().find_map(|line| {
        let rest = line.strip_prefix("## [")?;
        let version = &rest[..rest.find(']')?];
        let semver = version.split('.').count() == 3 && version.split('.').all(|p| p.parse::<u64>().is_ok());
        (semver && Some(version) != own).then(|| version.to_owned())
    })
}

/// The schema at the last release's tag, or `Err` when a release exists and its tag doesn't.
fn at_last_tag(root: &Path) -> Result<Option<Side>, String> {
    let Ok(changelog) = fs::read_to_string(root.join(CHANGELOG)) else { return Ok(None) };
    let own = crate::plugin::workspace_version(root).ok();
    let Some(version) = last_release(&changelog, own.as_deref()) else { return Ok(None) };
    let tag = format!("v{version}");
    let rev = format!("refs/tags/{tag}^{{commit}}");
    if !util::run(root, "git", &["rev-parse", "--verify", "--quiet", &rev]).0 {
        return Err(format!(
            "{CHANGELOG} names release {version}, and its tag {tag} isn't here; fetch the tags (git fetch --tags, or fetch-depth: 0 in CI)"
        ));
    }
    let listing = util::capture(root, "git", &["ls-tree", "--name-only", &tag, "schema/"])?;
    let names: Vec<String> =
        listing.lines().filter_map(|l| l.strip_prefix("schema/")).map(str::to_owned).collect();
    let read = |name: &str| util::capture(root, "git", &["show", &format!("{tag}:schema/{name}")]);
    side(format!("the schema at {tag}"), &names, &read).map(Some)
}

/// What the guard found: its problems, and one line per side it compared.
#[derive(Debug, Default)]
pub struct Guard {
    pub compared: Vec<String>,
    pub problems: Vec<String>,
}

/// The breaking-change guard over the in-memory schema `gen` would write.
pub fn guard(root: &Path) -> Result<Guard, String> {
    let current: Vec<(View, Value)> = View::ALL.iter().map(|&v| (v, schema::generate(v))).collect();
    guard_against(root, &current, SCHEMA_VERSION)
}

/// The guard for `current`, a schema of version `version`.
pub fn guard_against(root: &Path, current: &[(View, Value)], version: u32) -> Result<Guard, String> {
    let mut found = Guard::default();
    let Some(base) = baseline(root)? else {
        found.problems.push(format!(
            "there is no {BASELINE}/ to compare with; write it with cargo xtask gen --schema-baseline"
        ));
        return Ok(found);
    };
    let mut sides = vec![base];
    // A release whose tag isn't here fails the check, never skips it.
    match at_last_tag(root) {
        Ok(tag) => sides.extend(tag),
        Err(e) => found.problems.push(e),
    }
    for side in sides {
        if side.version > version {
            found.problems.push(format!(
                "{} is schema_version {}, newer than this tree's {version}",
                side.what, side.version
            ));
            continue;
        }
        if side.version < version {
            found
                .compared
                .push(format!("{} (v{}): schema_version bumped to {version}", side.what, side.version));
            continue;
        }
        let mut n = 0;
        for (view, old) in &side.views {
            let Some((_, new)) = current.iter().find(|(v, _)| v == view) else { continue };
            for p in breaking_changes(old, new, *view) {
                n += 1;
                found.problems.push(format!("{} ({}): {p}", side.what, view.file_name()));
            }
        }
        found.compared.push(format!("{} (v{}): {n} breaking", side.what, side.version));
    }
    Ok(found)
}

/// `cargo xtask gen --schema-baseline`: copy both views into `schema/baseline/`. It refuses when a
/// baseline of this `schema_version` exists, since the baseline moves only with a bump.
pub fn write_baseline(root: &Path) -> Result<Vec<String>, String> {
    if let Some(base) = baseline(root)?
        && base.version >= SCHEMA_VERSION
    {
        return Err(format!(
            "{BASELINE}/ already holds schema_version {}; the baseline moves only with a schema_version bump (Contracts, Versioning rules)",
            base.version
        ));
    }
    let dir = root.join(BASELINE);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("{BASELINE}: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("{BASELINE}: {e}"))?;
    let mut written = Vec::new();
    for view in View::ALL {
        let path = format!("{BASELINE}/{}", view.file_name());
        fs::write(root.join(&path), schema::render(view)).map_err(|e| format!("{path}: {e}"))?;
        written.push(path);
    }
    Ok(written)
}
