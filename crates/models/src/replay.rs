//! Recorded model calls (architecture.md, Model adapters; contracts.md, Model request, response
//! and replay): `<dir>/<provider>/<purpose>/<key>.json`, where `<dir>` is the checkout's
//! `tests/replay/`. Strict replay reads them; only record mode, which `cargo xtask live record`
//! runs, writes them.
//!
//! A record never holds a credential: its request keeps only headers from a fixed list, its
//! response keeps only the response headers on another, and before a record is written every
//! secret the caller names is looked for in it. A response that echoes one has it masked; a
//! request that holds one is refused (the adapter never sends one), since its key covers those bytes.

use std::fs;
use std::path::{Path, PathBuf};

use planroom_contract::format::{self, FormatError};
use planroom_contract::{REPLAY_FORMAT, RecordedResponse, ReplayRecord};
use serde_json::Value;

use crate::error::ModelError;

/// Request headers a record keeps: none of them can carry a credential.
pub const REQUEST_HEADERS: &[&str] = &["content-type", "anthropic-version", "x-session-affinity"];
/// Response headers a record keeps.
pub const RESPONSE_HEADERS: &[&str] = &["content-type", "retry-after", "x-should-retry"];
/// What a masked secret reads as.
pub const MASK: &str = "[redacted]";
/// A secret shorter than this isn't looked for, since it would match ordinary text.
const MIN_SECRET_CHARS: usize = 8;

/// The recordings under one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayDir {
    root: PathBuf,
}

impl ReplayDir {
    pub fn new(root: &Path) -> ReplayDir {
        ReplayDir { root: root.to_path_buf() }
    }

    /// Where the record for `key` lives. `purpose` is a pack-declared name such as
    /// `plan.stage.outcome`: lowercase letters, digits, `.`, `_` and `-`, starting with a letter
    /// or digit, so it can't climb out of the directory.
    pub fn path(&self, provider: &str, purpose: &str, key: &str) -> Result<PathBuf, ModelError> {
        let ok = purpose.len() <= 100
            && purpose.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && purpose.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
            && !purpose.contains("..");
        if !ok {
            return Err(ModelError::invalid_argument("purpose", "purpose"));
        }
        Ok(self.root.join(provider).join(purpose).join(format!("{key}.json")))
    }

    /// The record for `key`, if there is one.
    pub fn read(&self, provider: &str, purpose: &str, key: &str) -> Result<Option<ReplayRecord>, ModelError> {
        let path = self.path(provider, purpose, key)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ModelError::internal()),
        };
        let record: ReplayRecord = serde_json::from_slice(&bytes).map_err(|_| ModelError::internal())?;
        match format::check(&record.format, "replay") {
            Ok(_) => {}
            Err(FormatError::Newer { found, supported }) => {
                return Err(ModelError {
                    code: planroom_contract::ErrorCode::SchemaMismatch,
                    details: serde_json::json!({"found": found, "supported": supported,
                        "writer_version": null, "file": path.display().to_string()}),
                    reason: None,
                });
            }
            Err(FormatError::Other { .. }) => return Err(ModelError::internal()),
        }
        if record.key != key {
            return Err(ModelError::internal());
        }
        Ok(Some(record))
    }

    /// Write `record` under `provider`'s directory, after keeping only the listed headers and masking every secret in
    /// `secrets` in its response. A record whose request holds a secret is refused.
    pub fn write(
        &self,
        provider: &str,
        record: &ReplayRecord,
        secrets: &[&str],
    ) -> Result<PathBuf, ModelError> {
        let path = self.path(provider, &record.purpose, &record.key)?;
        let secrets: Vec<&str> =
            secrets.iter().copied().filter(|s| s.chars().count() >= MIN_SECRET_CHARS).collect();
        let mut record = record.clone();
        record.format = REPLAY_FORMAT.to_owned();
        record.request.headers.retain(|(k, _)| REQUEST_HEADERS.contains(&k.to_ascii_lowercase().as_str()));
        let request = serde_json::to_string(&record.request).map_err(|_| ModelError::internal())?;
        if holds_secret(request.as_bytes(), &secrets) {
            return Err(ModelError::internal());
        }
        record.response = masked(&record.response, &secrets);
        let mut text = serde_json::to_string_pretty(&record).map_err(|_| ModelError::internal())?;
        text.push('\n');
        if holds_secret(text.as_bytes(), &secrets) {
            return Err(ModelError::internal());
        }
        let dir = path.parent().ok_or_else(ModelError::internal)?;
        fs::create_dir_all(dir).map_err(|_| ModelError::internal())?;
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &path)).map_err(|_| {
            let _ = fs::remove_file(&tmp);
            ModelError::internal()
        })?;
        Ok(path)
    }
}

/// Whether `bytes` hold any of `secrets` (those of at least 8 characters), as they are or as a
/// JSON string escapes them.
pub fn holds_secret(bytes: &[u8], secrets: &[impl AsRef<str>]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    secrets.iter().map(AsRef::as_ref).filter(|s| s.chars().count() >= MIN_SECRET_CHARS).any(|s| {
        let escaped = serde_json::to_string(s).unwrap_or_default();
        text.contains(s) || text.contains(escaped.trim_matches('"'))
    })
}

/// `response` with only the listed headers, and every secret masked in its header values and
/// every string of its body.
fn masked(response: &RecordedResponse, secrets: &[&str]) -> RecordedResponse {
    let mask = |s: &str| secrets.iter().fold(s.to_owned(), |acc, secret| acc.replace(secret, MASK));
    RecordedResponse {
        status: response.status,
        headers: response
            .headers
            .iter()
            .filter(|(k, _)| RESPONSE_HEADERS.contains(&k.to_ascii_lowercase().as_str()))
            .map(|(k, v)| (k.to_ascii_lowercase(), mask(v)))
            .collect(),
        body: mask_value(&response.body, &mask),
    }
}

fn mask_value(value: &Value, mask: &dyn Fn(&str) -> String) -> Value {
    match value {
        Value::String(s) => Value::String(mask(s)),
        Value::Array(items) => Value::Array(items.iter().map(|v| mask_value(v, mask)).collect()),
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (mask(k), mask_value(v, mask))).collect())
        }
        other => other.clone(),
    }
}
