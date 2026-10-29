//! Reading one pack: its files from a directory or the embedded build, then its manifest, with
//! each `languages/*.toml` file's `[[languages]]` and `[[extractors]]` merged in.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use planroom_contract::{ExtractorDecl, LanguageDecl, PackFile};
use planroom_kernel::keys;
use serde_json::{Map, Value};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use super::{Origin, Pack};
use crate::app::error::AppError;

/// The manifest's path in a pack.
pub const MANIFEST: &str = "pack.toml";
/// The pack's tests, which `doctor --pack` reads from its directory and no load holds.
pub const TESTS: &str = "tests/";

/// One pack's files before they are read: where they came from and their bytes by path.
#[derive(Debug, Clone)]
pub struct Source {
    pub origin: Origin,
    /// How errors and records name the pack's directory: `packs/<dir>` for a built-in, else the
    /// directory's path.
    pub label: String,
    /// The directory it was read from; none for an embedded pack.
    pub dir: Option<PathBuf>,
    /// For a pack read from its copy in the state directory, the directory the copy was made
    /// from, which its record names in place of the copy's own path.
    pub recorded: Option<String>,
    /// Every file but its tests, by its path in the pack with `/` separators.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Source {
    /// How an error names `path`, a file of this pack.
    pub fn file_label(&self, path: &str) -> String {
        format!("{}/{path}", self.label)
    }

    /// The text of `path`, when the pack holds it as UTF-8.
    pub fn text(&self, path: &str) -> Option<&str> {
        self.files.get(path).and_then(|b| std::str::from_utf8(b).ok())
    }

    /// The pack's TOML files: the manifest, then `languages/*.toml` in path order.
    pub fn tomls(&self) -> impl Iterator<Item = (&str, &str)> {
        let manifest = self.text(MANIFEST).map(|t| (MANIFEST, t));
        let languages = self
            .files
            .keys()
            .filter(|p| is_language_file(p))
            .filter_map(|p| Some((p.as_str(), self.text(p)?)));
        manifest.into_iter().chain(languages)
    }

    /// `pack_invalid` at the line of the entry `path` names, for the reason `pack.<why>`: its key's
    /// line or its value's, as `at` says. Each check names its own entry by its path, so an error
    /// never points at an earlier line that happens to hold the same text.
    pub fn invalid_at(
        &self,
        path: &KeyPath,
        at: At,
        reference: &str,
        why: &str,
        vars: &[(&str, &dyn std::fmt::Display)],
    ) -> AppError {
        let (file, line) = self.line_of(path, at);
        AppError::pack_invalid(&file, line, reference, why, vars)
    }

    /// The file and line of the entry `path` names in the merged manifest. `languages` and
    /// `extractors` entries past the manifest's own come from `languages/*.toml`, in the order
    /// [`parse`] merges them. A path that names no written entry, such as a defaulted one, falls
    /// back to its nearest written ancestor.
    pub fn line_of(&self, path: &KeyPath, at: At) -> (String, u32) {
        let mut segs: &[Seg] = &path.0;
        let local: Vec<Seg>;
        let mut tomls = self.tomls();
        let Some((mut file, mut text)) = tomls.next() else {
            return (self.file_label(MANIFEST), 1);
        };
        if let [Seg::Key(table), Seg::Index(i), rest @ ..] = segs
            && (table == "languages" || table == "extractors")
        {
            let mut i = *i;
            loop {
                let n = DeTable::parse(text)
                    .ok()
                    .and_then(|t| entry(t.get_ref(), table).map(|v| array_len(v.get_ref())))
                    .unwrap_or(0);
                if i < n {
                    break;
                }
                let Some((next_file, next_text)) = tomls.next() else { break };
                i -= n;
                (file, text) = (next_file, next_text);
            }
            local =
                [Seg::Key(table.clone()), Seg::Index(i)].into_iter().chain(rest.iter().cloned()).collect();
            segs = &local;
        }
        let line = DeTable::parse(text).map_or(1, |root| span_line(text, &locate(&root, segs, at)));
        (self.file_label(file), line)
    }

    /// The sha256 over every file, each path and its bytes, in path order.
    pub fn sha256(&self) -> String {
        let mut all = Vec::new();
        for (path, bytes) in &self.files {
            all.extend_from_slice(path.as_bytes());
            all.push(0);
            all.extend_from_slice(bytes.len().to_string().as_bytes());
            all.push(0);
            all.extend_from_slice(bytes);
        }
        keys::sha256_plain_hex(&all)
    }
}

/// One step of a [`KeyPath`]: a table's key or an array's index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// Where an entry sits in the merged manifest, such as `kinds[3].parents[1]`: what a check names
/// when it refuses the entry, so the error points at that entry's own line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyPath(pub Vec<Seg>);

impl KeyPath {
    /// The manifest itself.
    pub fn root() -> KeyPath {
        KeyPath(Vec::new())
    }

    /// The entry under `key`.
    pub fn key(&self, key: &str) -> KeyPath {
        let mut segs = self.0.clone();
        segs.push(Seg::Key(key.to_owned()));
        KeyPath(segs)
    }

    /// The `index`th item.
    pub fn at(&self, index: usize) -> KeyPath {
        let mut segs = self.0.clone();
        segs.push(Seg::Index(index));
        KeyPath(segs)
    }
}

/// A table's name as an error gives it: its keys, without indexes, such as
/// `[[decisions]] policy`; the manifest's top level is `pack.toml`.
impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self
            .0
            .iter()
            .filter_map(|s| match s {
                Seg::Key(k) => Some(k.as_str()),
                Seg::Index(_) => None,
            })
            .collect();
        match keys.split_first() {
            None => f.write_str(MANIFEST),
            Some((first, [])) => write!(f, "[[{first}]]"),
            Some((first, rest)) => write!(f, "[[{first}]] {}", rest.join(".")),
        }
    }
}

/// Which line of an entry an error names: its key's, as for a key no table declares, or its
/// value's, as for a reference that doesn't resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum At {
    Key,
    Value,
}

/// The value under `key` in `table`.
fn entry<'t, 'i>(table: &'t DeTable<'i>, key: &str) -> Option<&'t Spanned<DeValue<'i>>> {
    table.iter().find(|(k, _)| k.get_ref().as_ref() == key).map(|(_, v)| v)
}

fn array_len(value: &DeValue<'_>) -> usize {
    match value {
        DeValue::Array(items) => items.len(),
        _ => 0,
    }
}

/// The span of the entry `segs` names under `root`, or of its nearest written ancestor: its key's
/// span, or its value's, as `at` says. An array's item has no key of its own, so its value's.
fn locate(root: &Spanned<DeTable<'_>>, segs: &[Seg], at: At) -> Range<usize> {
    let (mut key, mut value) = (root.span(), root.span());
    let mut here: &DeTable<'_> = root.get_ref();
    let mut current: Option<&DeValue<'_>> = None;
    for seg in segs {
        let next: Option<(Range<usize>, &Spanned<DeValue<'_>>)> = match (seg, current) {
            (Seg::Key(k), None | Some(DeValue::Table(_))) => {
                let table = match current {
                    Some(DeValue::Table(t)) => t,
                    _ => here,
                };
                table
                    .iter()
                    .find(|(name, _)| name.get_ref().as_ref() == k.as_str())
                    .map(|(n, v)| (n.span(), v))
            }
            (Seg::Index(i), Some(DeValue::Array(items))) => items.get(*i).map(|item| (item.span(), item)),
            _ => None,
        };
        let Some((k, v)) = next else { break };
        (key, value) = (k, v.span());
        if let DeValue::Table(t) = v.get_ref() {
            here = t;
        }
        current = Some(v.get_ref());
    }
    match at {
        At::Key => key,
        At::Value => value,
    }
}

/// The 1-based line `span` starts on.
fn span_line(text: &str, span: &Range<usize>) -> u32 {
    text.get(..span.start).unwrap_or(text).matches('\n').count() as u32 + 1
}

/// The input view of `pack.toml`, generated once per process.
fn manifest_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(planroom_contract::schema::pack_manifest)
}

/// A key no table declares: its path, its span and the keys its table takes.
struct Unknown {
    path: KeyPath,
    span: Range<usize>,
    keys: String,
}

/// The first key under `value` that `schema` doesn't declare. A table an untagged or a tagged
/// enum reads is checked against the variant its required and tagged keys pick; with none, the
/// typed read says what's wrong. A table of free keys, such as a `when` predicate or an effect's
/// parameters, is left to the declared tables the validator holds.
fn undeclared(
    defs: &Map<String, Value>,
    schema: &Value,
    value: &DeValue<'_>,
    path: &KeyPath,
) -> Option<Unknown> {
    let schema = resolve(defs, schema);
    match value {
        DeValue::Table(table) => {
            let schema = match variants(schema) {
                Some(all) => all.iter().map(|v| resolve(defs, v)).find(|v| fits(v, table))?,
                None => schema,
            };
            let properties = schema.get("properties").and_then(Value::as_object);
            let extra = schema.get("additionalProperties");
            for (k, v) in table {
                let name = k.get_ref().as_ref();
                let inner = path.key(name);
                let child = match (properties.and_then(|p| p.get(name)), extra) {
                    (Some(declared), _) => declared,
                    (None, Some(extra)) if extra != &Value::Bool(false) => extra,
                    (None, None) if properties.is_none() => continue,
                    (None, _) => {
                        let keys = properties.map(|p| p.keys().cloned().collect::<Vec<_>>().join(", "));
                        return Some(Unknown { path: inner, span: k.span(), keys: keys.unwrap_or_default() });
                    }
                };
                if let Some(found) = undeclared(defs, child, v.get_ref(), &inner) {
                    return Some(found);
                }
            }
            None
        }
        DeValue::Array(items) => {
            let item = schema.get("items")?;
            items.iter().enumerate().find_map(|(i, v)| undeclared(defs, item, v.get_ref(), &path.at(i)))
        }
        _ => None,
    }
}

/// `schema` with every `$ref` followed.
fn resolve<'s>(defs: &'s Map<String, Value>, mut schema: &'s Value) -> &'s Value {
    while let Some(name) = schema.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix("#/$defs/"))
    {
        match defs.get(name) {
            Some(def) => schema = def,
            None => break,
        }
    }
    schema
}

fn variants(schema: &Value) -> Option<&Vec<Value>> {
    schema.get("anyOf").or_else(|| schema.get("oneOf")).and_then(Value::as_array)
}

/// Whether an enum's variant reads `table`: an object schema whose required keys it holds, and
/// whose tag, if it has one, it names.
fn fits(variant: &Value, table: &DeTable<'_>) -> bool {
    let typed_object =
        |t: &Value| t == "object" || t.as_array().is_some_and(|a| a.iter().any(|t| t == "object"));
    if variant.get("properties").is_none() && !variant.get("type").is_some_and(typed_object) {
        return false;
    }
    let required = variant.get("required").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    if !required.iter().filter_map(Value::as_str).all(|key| entry(table, key).is_some()) {
        return false;
    }
    let properties = variant.get("properties").and_then(Value::as_object);
    properties.into_iter().flatten().all(|(name, property)| {
        match (property.get("const"), entry(table, name)) {
            (Some(Value::String(tag)), Some(v)) => {
                matches!(v.get_ref(), DeValue::String(s) if s.as_ref() == tag)
            }
            _ => true,
        }
    })
}

fn is_language_file(path: &str) -> bool {
    path.strip_prefix("languages/").is_some_and(|rest| !rest.contains('/') && rest.ends_with(".toml"))
}

/// How far [`read_dir`] walks a pack before it stops: a pack is data a person names, read on every
/// trusted scan, so no directory can stall or exhaust the process that reads it.
pub mod bounds {
    /// Entries walked, directories and `tests/` included.
    pub const ENTRIES: usize = 1_000;
    /// Directories deep below the pack's own.
    pub const DEPTH: usize = 16;
    /// Bytes in one file the pack holds.
    pub const FILE_BYTES: u64 = 1 << 20;
    /// Bytes read in all, `tests/` left out.
    pub const TOTAL_BYTES: u64 = 4 << 20;
}

/// The pack in the directory `dir`, as an out-of-tree pack. A pack holds data only: an
/// executable file, or anything but a plain file or a directory, is `pack_invalid` (DR-9). The
/// walk is bounded ([`bounds`]): past a bound it stops at once with `pack_invalid` naming it.
pub fn read_dir(dir: &Path) -> Result<Source, AppError> {
    use std::io::Read;
    let label = dir.display().to_string();
    let mut files = BTreeMap::new();
    let mut todo = vec![(dir.to_path_buf(), 0usize)];
    let unreadable = |path: &Path, e: std::io::Error| {
        let shown = path.display().to_string();
        AppError::pack_invalid(&shown, 1, &shown, "unreadable", &[("why", &e)])
    };
    let too_large = |path: &Path, rel: &str, bound: String| {
        let shown = path.display().to_string();
        AppError::pack_invalid(&shown, 1, rel, "too_large", &[("bound", &bound)])
    };
    if !dir.join(MANIFEST).is_file() {
        let shown = dir.join(MANIFEST).display().to_string();
        return Err(AppError::pack_invalid(&shown, 1, MANIFEST, "no_manifest", &[]));
    }
    let (mut walked, mut total) = (0usize, 0u64);
    while let Some((at, depth)) = todo.pop() {
        for entry in fs::read_dir(&at).map_err(|e| unreadable(&at, e))? {
            let path = entry.map_err(|e| unreadable(&at, e))?.path();
            let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            walked += 1;
            if walked > bounds::ENTRIES {
                return Err(too_large(&path, &rel, format!("{} entries", bounds::ENTRIES)));
            }
            let meta = fs::symlink_metadata(&path).map_err(|e| unreadable(&path, e))?;
            let shown = path.display().to_string();
            if meta.is_dir() {
                if depth + 1 > bounds::DEPTH {
                    return Err(too_large(&path, &rel, format!("{} directories deep", bounds::DEPTH)));
                }
                todo.push((path, depth + 1));
                continue;
            }
            if !meta.is_file() {
                return Err(AppError::pack_invalid(&shown, 1, &rel, "not_plain", &[("path", &rel)]));
            }
            if executable(&meta) {
                return Err(AppError::pack_invalid(&shown, 1, &rel, "executable", &[("path", &rel)]));
            }
            if rel.starts_with(TESTS) {
                continue;
            }
            // Read no more than the bound allows, whatever the file grows to meanwhile.
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .and_then(|f| f.take(bounds::FILE_BYTES + 1).read_to_end(&mut bytes))
                .map_err(|e| unreadable(&path, e))?;
            if bytes.len() as u64 > bounds::FILE_BYTES {
                return Err(too_large(&path, &rel, format!("{} bytes in one file", bounds::FILE_BYTES)));
            }
            total += bytes.len() as u64;
            if total > bounds::TOTAL_BYTES {
                return Err(too_large(&path, &rel, format!("{} bytes in all", bounds::TOTAL_BYTES)));
            }
            files.insert(rel, bytes);
        }
    }
    Ok(Source { origin: Origin::OutOfTree, label, dir: Some(dir.to_path_buf()), recorded: None, files })
}

#[cfg(unix)]
fn executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_meta: &fs::Metadata) -> bool {
    false
}

/// The pack's ID as its manifest names it, before the manifest is checked: what decides whether a
/// directory `doctor --pack` reads stands for a built-in pack.
pub fn peek_id(source: &Source) -> Option<String> {
    let table: toml::Table = source.text(MANIFEST)?.parse().ok()?;
    table.get("id")?.as_str().map(str::to_owned)
}

/// Read `source`'s manifest and merge its `languages/*.toml` files into it. A key no table
/// declares, in any of them, is `pack_invalid` at that key's line.
pub fn parse(source: Source) -> Result<Pack, AppError> {
    let Some(text) = source.text(MANIFEST) else {
        let file = source.file_label(MANIFEST);
        return Err(AppError::pack_invalid(&file, 1, MANIFEST, "no_manifest", &[]));
    };
    for (path, text) in source.tomls() {
        declared_keys(&source, path, text)?;
    }
    let mut file: PackFile = toml::from_str(text).map_err(|e| syntax(&source, MANIFEST, text, &e))?;
    for (path, text) in source.tomls().filter(|(p, _)| *p != MANIFEST) {
        let mut table: toml::Table = text.parse().map_err(|e| syntax(&source, path, text, &e))?;
        let (languages, extractors) = (table.remove("languages"), table.remove("extractors"));
        if let Some(languages) = languages {
            let decls: Vec<LanguageDecl> =
                languages.try_into().map_err(|e| syntax(&source, path, text, &e))?;
            file.languages.extend(decls);
        }
        if let Some(extractors) = extractors {
            let decls: Vec<ExtractorDecl> =
                extractors.try_into().map_err(|e| syntax(&source, path, text, &e))?;
            file.extractors.extend(decls);
        }
    }
    let sha256 = source.sha256();
    Ok(Pack { file, origin: source.origin, source, sha256 })
}

/// Refuse a key that no table of the manifest declares in `text`, the file `path`, at the key's
/// own line: at any depth, in a table an untagged enum reads too. A languages file holds only
/// `[[languages]]` and `[[extractors]]` at its top.
fn declared_keys(source: &Source, path: &str, text: &str) -> Result<(), AppError> {
    let root = DeTable::parse(text).map_err(|e| syntax(source, path, text, &e))?;
    let shown = source.file_label(path);
    if path != MANIFEST
        && let Some((key, _)) =
            root.get_ref().iter().find(|(k, _)| !matches!(k.get_ref().as_ref(), "languages" | "extractors"))
    {
        let name = key.get_ref().as_ref();
        let line = span_line(text, &key.span());
        return Err(AppError::pack_invalid(&shown, line, name, "languages_file", &[("key", &name)]));
    }
    let schema = manifest_schema();
    let Some(defs) = schema["$defs"].as_object() else { return Ok(()) };
    let table = DeValue::Table(root.into_inner());
    let Some(unknown) = undeclared(defs, &schema["root"], &table, &KeyPath::root()) else { return Ok(()) };
    let mut owner = unknown.path.clone();
    let key = match owner.0.pop() {
        Some(Seg::Key(key)) => key,
        _ => String::new(),
    };
    let line = span_line(text, &unknown.span);
    let vars: &[(&str, &dyn fmt::Display)] = &[("key", &key), ("table", &owner), ("keys", &unknown.keys)];
    Err(AppError::pack_invalid(&shown, line, &key, "unknown_key", vars))
}

/// A TOML file that doesn't parse, or doesn't fit the manifest, at the line its error names.
fn syntax(source: &Source, path: &str, text: &str, e: &toml::de::Error) -> AppError {
    let line =
        e.span().map_or(1, |span| text.get(..span.start).unwrap_or(text).matches('\n').count() as u32 + 1);
    let why = e.message().to_owned();
    AppError::pack_invalid(&source.file_label(path), line, path, "syntax", &[("why", &why)])
}

/// An extractor declaration's ID.
pub fn extractor_id(decl: &ExtractorDecl) -> &str {
    match decl {
        ExtractorDecl::Builtin { id, .. }
        | ExtractorDecl::Regex { id, .. }
        | ExtractorDecl::Query { id, .. } => id,
    }
}
