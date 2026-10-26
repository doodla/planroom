//! The configuration schema: every key's type, from [`Config`] in the contract
//! (`crates/contract/src/config.rs`), and every key's default, place and meaning, from
//! `defaults.toml`, the one home of each (contracts.md, Configuration).
//!
//! `defaults.toml` deserializes into [`Config`] with no key left over and none missing, so the
//! two can't drift: a key in one and not the other fails `every_key_has_a_default`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

pub use planroom_contract::config::{Budget, Claude, Config, Jev, Scan, Secret};

/// The defaults file, compiled in.
pub const DEFAULTS: &str = include_str!("defaults.toml");

/// Where a key may be set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The workspace's file only: workspace state, such as `packs` (DR-62).
    Workspace,
    /// Either file; the workspace's wins over the user-level one.
    Either,
    /// The user-level file only: a credential setting or a limit across every workspace. A
    /// workspace's file never sets one (DR-48).
    User,
}

/// One key of the schema, as `defaults.toml` declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// Dotted: `models`, `scan.exclude`, `claude.api_key`.
    pub path: String,
    pub scope: Scope,
    /// Honoured only from a user-level file at mode 0600, and never from a variable.
    pub needs_private_file: bool,
    /// No `PLANROOM_*` variable sets it: a setting that decides where a credential goes.
    pub no_variable: bool,
    /// What the key means: the rest of its comment.
    pub meaning: String,
}

impl Key {
    /// The `PLANROOM_*` variable that overrides this key, or `None` for a key no variable sets:
    /// `packs`, which is workspace state; `claude.api_key`, whose variable is the one
    /// `claude.api_key_env` names; and `claude.base_url`, the host a credential goes to.
    pub fn variable(&self) -> Option<String> {
        if self.path == "packs" || self.needs_private_file || self.no_variable {
            return None;
        }
        Some(format!("PLANROOM_{}", self.path.replace('.', "_").to_ascii_uppercase()))
    }
}

/// Every key, in the order `defaults.toml` declares them.
pub fn keys() -> &'static [Key] {
    static KEYS: OnceLock<Vec<Key>> = OnceLock::new();
    KEYS.get_or_init(|| parse_keys(DEFAULTS).expect("defaults.toml marks every key"))
}

/// The key at `path`.
pub fn key(path: &str) -> Option<&'static Key> {
    keys().iter().find(|k| k.path == path)
}

/// Each `key = value` line of `text` at the start of a line, with the `# [scope] meaning` comment
/// right above it, under its `[section]`. A key with no such comment is an error naming it.
pub fn parse_keys(text: &str) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    let mut section = String::new();
    let mut comment: Vec<&str> = Vec::new();
    for line in text.lines() {
        if let Some(c) = line.strip_prefix('#') {
            comment.push(c.trim());
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_owned();
            comment.clear();
            continue;
        }
        let Some((name, _)) =
            line.split_once('=').filter(|_| line.starts_with(|c: char| c.is_ascii_lowercase()))
        else {
            if line.trim().is_empty() {
                comment.clear();
            }
            continue;
        };
        let name = name.trim();
        let path = if section.is_empty() { name.to_owned() } else { format!("{section}.{name}") };
        let joined = comment.join(" ");
        let (tag, meaning) = joined
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .ok_or_else(|| format!("defaults.toml: {path} needs a `# [scope] meaning` comment above it"))?;
        let mut parts = tag.split(',').map(str::trim);
        let scope = match parts.next() {
            Some("workspace") => Scope::Workspace,
            Some("either") => Scope::Either,
            Some("user") => Scope::User,
            other => return Err(format!("defaults.toml: {path} has an unknown scope {other:?}")),
        };
        let flags: Vec<&str> = parts.collect();
        let needs_private_file = flags.contains(&"mode 0600");
        let no_variable = flags.contains(&"no variable");
        if let Some(other) = flags.iter().find(|f| !["mode 0600", "no variable"].contains(f)) {
            return Err(format!("defaults.toml: {path} has an unknown flag {other:?}"));
        }
        keys.push(Key { path, scope, needs_private_file, no_variable, meaning: meaning.trim().to_owned() });
        comment.clear();
    }
    Ok(keys)
}

/// `defaults.toml` as a table: the base every file and variable is laid over.
pub fn defaults() -> &'static toml::Table {
    static TABLE: OnceLock<toml::Table> = OnceLock::new();
    TABLE.get_or_init(|| DEFAULTS.parse().expect("defaults.toml parses"))
}

/// The value at the dotted `path` of `table`.
pub fn lookup<'a>(table: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
    match path.split_once('.') {
        Some((section, rest)) => table.get(section)?.as_table().and_then(|t| t.get(rest)),
        None => table.get(path),
    }
}

/// Every leaf of `table` by dotted path: a top-level value, or a value inside one section.
pub fn leaves(table: &toml::Table) -> BTreeMap<String, toml::Value> {
    let mut out = BTreeMap::new();
    for (name, value) in table {
        match value {
            toml::Value::Table(section) => {
                for (key, value) in section {
                    out.insert(format!("{name}.{key}"), value.clone());
                }
            }
            other => {
                out.insert(name.clone(), other.clone());
            }
        }
    }
    out
}
