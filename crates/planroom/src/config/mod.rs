//! Configuration (contracts.md, Configuration): two optional files of one schema, laid over the
//! defaults in `defaults.toml`, then `PLANROOM_*` variables over both.
//!
//! - The user-level file, `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml`, sets any key
//!   but a workspace's own (`packs`). Its `claude.api_key` counts only when the file is exactly
//!   mode 0600; otherwise the key is ignored with `config_key_ignored`.
//! - A workspace's `config.toml`, in its state directory, loads only when the workspace is
//!   trusted (DR-48), and wins over the user-level file. It never sets a credential setting or a
//!   limit across workspaces: such a key is ignored with `config_key_ignored` naming the file and
//!   the key, never its value, whatever its type.
//! - A variable named for its key, `PLANROOM_<SECTION>_<KEY>`, overrides both. A list takes TOML
//!   array syntax or comma-separated values. `packs` and `claude.api_key` have none.
//!
//! A key the schema doesn't know is left out and listed in [`Loaded::ignored`], as is a
//! workspace key in the user-level file, so a newer version's file still loads.

pub mod schema;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use planroom_contract::{ErrorCode, Warning, WarningCode};
use serde_json::json;

pub use schema::{Config, Scope};

use crate::app::data;
use crate::app::error::AppError;
use crate::text;

/// The name of the configuration file, in a state directory and under the user's config home.
pub const FILE: &str = "config.toml";

/// The configuration, and what loading it found.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    /// `config_key_ignored` for each key a file may not set.
    pub warnings: Vec<Warning>,
    /// `FILE: KEY` for each key left out with no warning: one the schema doesn't know, or a
    /// workspace key in the user-level file.
    pub ignored: Vec<String>,
}

/// Where each layer comes from.
pub struct Sources<'a> {
    /// The user-level file, if this machine gives it an absolute place.
    pub user: Option<&'a Path>,
    /// The workspace's file, only when the workspace is trusted.
    pub workspace: Option<&'a Path>,
    /// A variable's value, by name.
    pub env: &'a dyn Fn(&str) -> Option<String>,
}

/// The configuration for the workspace whose state directory is `state_dir`, from this process's
/// environment. Its file is read only when `trusted`.
pub fn for_workspace(state_dir: &Path, trusted: bool) -> Result<Loaded, AppError> {
    let user = data::user_config();
    let workspace = state_dir.join(FILE);
    let env = |name: &str| std::env::var(name).ok();
    load(&Sources { user: user.as_deref(), workspace: trusted.then_some(workspace.as_path()), env: &env })
}

/// Lay the user-level file, the workspace's file and the variables over the defaults.
pub fn load(sources: &Sources<'_>) -> Result<Loaded, AppError> {
    let mut merged = schema::defaults().clone();
    let mut warnings = Vec::new();
    let mut ignored = Vec::new();
    for (path, layer) in [(sources.user, Layer::User), (sources.workspace, Layer::Workspace)] {
        let Some(path) = path else { continue };
        let Some(table) = read(path)? else { continue };
        let shown = path.display().to_string();
        let private = layer == Layer::User && is_private(path);
        for (key_path, value) in schema::leaves(&table) {
            let Some(key) = schema::key(&key_path) else {
                ignored.push(format!("{shown}: {key_path}"));
                continue;
            };
            // Where a key may be set comes before its value: a key a file may not set is ignored
            // whatever it holds, so a workspace file can't fail the load with a user-level key.
            match (layer, key.scope) {
                (Layer::Workspace, Scope::User) => {
                    warnings.push(key_ignored("warning.config_key_ignored", &shown, &key_path));
                    continue;
                }
                (Layer::User, Scope::Workspace) => {
                    ignored.push(format!("{shown}: {key_path}"));
                    continue;
                }
                (Layer::User, _) if key.needs_private_file && !private => {
                    warnings.push(key_ignored("warning.config_key_ignored_mode", &shown, &key_path));
                    continue;
                }
                _ => {}
            }
            let value = typed(&key_path, value).map_err(|why| invalid(&shown, &key_path, &why))?;
            set(&mut merged, &key_path, value);
        }
    }
    for key in schema::keys() {
        let Some(var) = key.variable() else { continue };
        let Some(raw) = (sources.env)(&var).filter(|v| !v.is_empty()) else { continue };
        let default = schema::lookup(schema::defaults(), &key.path).expect("every key has a default");
        let value = from_variable(default, &raw)
            .and_then(|value| check(&key.path, value))
            .map_err(|why| invalid(&var, &key.path, &why))?;
        set(&mut merged, &key.path, value);
    }
    let config = toml::Value::Table(merged)
        .try_into::<Config>()
        .map_err(|e| AppError::internal(format!("the merged configuration doesn't fit its schema: {e}")))?;
    Ok(Loaded { config, warnings, ignored })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    User,
    Workspace,
}

/// The file at `path` as a table; `None` when there's no file.
fn read(path: &Path) -> Result<Option<toml::Table>, AppError> {
    let shown = path.display().to_string();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            let what = text::get("argument.config_unreadable", &[("file", &shown), ("why", &e)]);
            return Err(file_error(&shown, &what));
        }
    };
    // toml's message quotes the line it failed on, which may hold a credential: say only where.
    text.parse::<toml::Table>().map(Some).map_err(|e| {
        let at = e.span().map(|s| text[..s.start].lines().count().max(1)).unwrap_or(1);
        file_error(&shown, &text::get("argument.config_syntax", &[("file", &shown), ("line", &at)]))
    })
}

/// `invalid_argument` for a whole file Planroom can't load; `field` names the file.
fn file_error(file: &str, what: &str) -> AppError {
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": file}))
}

/// Whether the file at `path` is exactly mode 0600, the one mode contracts.md names for `api_key`.
fn is_private(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o777 == 0o600)
}

/// `value` as `key_path`'s default types it: a number where a number goes, an integer read as a
/// float, a string, or a list of strings.
fn typed(key_path: &str, value: toml::Value) -> Result<toml::Value, String> {
    use toml::Value as V;
    let default = schema::lookup(schema::defaults(), key_path).expect("every key has a default");
    match (default, value) {
        (V::Float(_), V::Integer(i)) => Ok(V::Float(i as f64)),
        (V::Float(_), v @ V::Float(_))
        | (V::String(_), v @ V::String(_))
        | (V::Boolean(_), v @ V::Boolean(_)) => Ok(v),
        (V::Array(_), V::Array(items)) if items.iter().all(V::is_str) => Ok(V::Array(items)),
        (default, _) => Err(text::get("argument.config_type", &[("type", &type_name(default))])),
    }
    .and_then(|value| check(key_path, value))
}

/// What a key's type alone can't say: `scan.exclude` holds globs that parse.
fn check(key_path: &str, value: toml::Value) -> Result<toml::Value, String> {
    if key_path == "scan.exclude" {
        let globs: Vec<&str> =
            value.as_array().into_iter().flatten().filter_map(toml::Value::as_str).collect();
        planroom_extract::walk::Exclude::new(&globs).map_err(|e| e.to_string())?;
    }
    Ok(value)
}

fn type_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::Float(_) | toml::Value::Integer(_) => "a number",
        toml::Value::Array(_) => "a list of strings",
        toml::Value::Boolean(_) => "true or false",
        _ => "a string",
    }
}

/// A variable's text as its key's type: a list as a TOML array or comma-separated values.
fn from_variable(default: &toml::Value, raw: &str) -> Result<toml::Value, String> {
    use toml::Value as V;
    let bad = || text::get("argument.config_type", &[("type", &type_name(default))]);
    match default {
        V::String(_) => Ok(V::String(raw.to_owned())),
        V::Float(_) => raw.trim().parse::<f64>().map(V::Float).map_err(|_| bad()),
        V::Boolean(_) => raw.trim().parse::<bool>().map(V::Boolean).map_err(|_| bad()),
        V::Array(_) if raw.trim_start().starts_with('[') => {
            let table: toml::Table = format!("v = {raw}").parse().map_err(|_| bad())?;
            match table.get("v") {
                Some(V::Array(items)) if items.iter().all(V::is_str) => Ok(V::Array(items.clone())),
                _ => Err(bad()),
            }
        }
        V::Array(_) => Ok(V::Array(
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| V::String(s.to_owned()))
                .collect(),
        )),
        _ => Err(bad()),
    }
}

/// Set the dotted `key_path` of `table` to `value`.
fn set(table: &mut toml::Table, key_path: &str, value: toml::Value) {
    match key_path.split_once('.') {
        Some((section, key)) => {
            let section = table
                .entry(section)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .expect("a section is a table");
            section.insert(key.to_owned(), value);
        }
        None => {
            table.insert(key_path.to_owned(), value);
        }
    }
}

/// `config_key_ignored` naming the file and the key, never the value.
fn key_ignored(message: &str, file: &str, key: &str) -> Warning {
    Warning {
        code: WarningCode::ConfigKeyIgnored,
        message: text::get(message, &[("file", &file), ("key", &key)]),
        details: json!({"file": file, "key": key}),
    }
}

/// `invalid_argument` for a setting that doesn't fit: `field` is the key, `source` the file or
/// variable that set it.
fn invalid(source: &str, field: &str, why: &str) -> AppError {
    let what = text::get("argument.config", &[("source", &source), ("key", &field), ("why", &why)]);
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": field}))
}
