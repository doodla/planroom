//! `cargo xtask fixture OUT [--no-packs]`: `examples/build-fixture OUT`, then
//! `OUT/.planroom/config.toml` loading `examples/packs/ruby` (DR-13). The workspace's first
//! trusted scan reads that list and copies each pack into the state directory's `packs/`, and
//! the store records the set (DR-62); `planroom-testkit`'s `fixture_with_packs` builds the same
//! state for the tests.
//!
//! A workspace config loads only from a state directory the data root records as trusted (DR-48),
//! so with the config this records `OUT/.planroom/` as trusted, `by: engine`, in the data root of
//! the environment it runs in, as the engine records a state directory it creates. The writer
//! follows the engine's rules for `workspaces.json` (`crates/planroom/src/app/data.rs`): the
//! exclusive lock on `workspaces.json.lock`, the format and reader-floor check, every field it
//! doesn't know kept, mode 0600. It is a few lines here rather than a dependency on the engine,
//! which would build the engine for every xtask run.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::util;

/// `workspaces.json`'s format tag and the reader floor a new file gets, as the engine writes them.
const FORMAT: &str = "planroom.workspaces/v1";
const MIN_READER_VERSION: &str = "0.1.0-dev";

/// Build the fixture into `out`; returns `build-fixture`'s JSON map of repo to branch heads.
pub fn build(root: &Path, out: &Path, packs: bool) -> Result<String, String> {
    let out = std::env::current_dir().map_err(|e| e.to_string())?.join(out);
    let script = root.join("examples/build-fixture");
    let out_str = out.to_str().ok_or("OUT must be a UTF-8 path")?;
    let heads =
        util::capture(root, "python3", &[script.to_str().ok_or("the checkout path isn't UTF-8")?, out_str])?;
    if packs {
        let dir = out.join(".planroom");
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        let pack = root.join("examples/packs/ruby");
        let pack = serde_json::to_string(pack.to_str().ok_or("the checkout path isn't UTF-8")?)
            .map_err(|e| e.to_string())?;
        let config = dir.join("config.toml");
        fs::write(&config, format!("packs = [{pack}]\n"))
            .map_err(|e| format!("{}: {e}", config.display()))?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        record_trusted(&data_root()?, &out)?;
    }
    Ok(heads)
}

/// The data root, `${PLANROOM_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/planroom}`, each only
/// when absolute, as the engine places it.
fn data_root() -> Result<PathBuf, String> {
    let absolute = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if std::env::var_os("PLANROOM_DATA").is_some_and(|d| !d.is_empty()) {
        return absolute("PLANROOM_DATA").ok_or_else(|| "PLANROOM_DATA isn't an absolute path".into());
    }
    let base = absolute("XDG_DATA_HOME").or_else(|| absolute("HOME").map(|home| home.join(".local/share")));
    base.map(|b| b.join("planroom")).ok_or_else(|| "no absolute PLANROOM_DATA, XDG_DATA_HOME or HOME".into())
}

/// One trust record, as the engine writes it for a state directory it created: `by: engine`,
/// with the directory's inode number and, where the filesystem reports one, its birth time.
pub fn trusted_record(root: &str, state_dir: &str, ino: u64, created: Option<u64>, at: &str) -> Value {
    let mut record = json!({"root": root, "state_dir": state_dir, "ino": ino, "by": "engine", "at": at});
    if let Some(created) = created {
        record["created"] = json!(created);
    }
    record
}

/// Record `out`'s `.planroom/` as trusted in `data`'s `workspaces.json`.
fn record_trusted(data: &Path, out: &Path) -> Result<(), String> {
    let fail = |e: std::io::Error| e.to_string();
    let root = fs::canonicalize(out).map_err(fail)?;
    let state = root.join(".planroom");
    let meta = fs::symlink_metadata(&state).map_err(fail)?;
    let created = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok());
    let at = rfc3339(SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs());
    let (root, state) = (root.display().to_string(), state.display().to_string());
    let record = trusted_record(&root, &state, meta.ino(), created, &at);
    DirBuilder::new().recursive(true).mode(0o700).create(data).map_err(fail)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(data.join("workspaces.json.lock"))
        .map_err(fail)?;
    lock.lock().map_err(fail)?;
    let path = data.join("workspaces.json");
    let mut doc = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Value>(&text).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            json!({"format": FORMAT, "min_reader_version": MIN_READER_VERSION, "workspaces": []})
        }
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let doc = with_record(&mut doc, record).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(fail)?;
    file.write_all(text.as_bytes()).and_then(|()| file.sync_all()).map_err(fail)?;
    fs::rename(&tmp, &path).map_err(fail)?;
    Ok(())
}

/// `doc` with `record` in its `trusted` list, replacing the one for the same root and state
/// directory and keeping the fields that one held which `record` doesn't set. A file this engine
/// version may not rewrite, by its format or its reader floor, is refused and left as it is.
fn with_record(doc: &mut Value, mut record: Value) -> Result<&Value, String> {
    if doc["format"] != FORMAT {
        return Err(format!("its format is {}, not {FORMAT}", doc["format"]));
    }
    if let Some(min) = doc["min_reader_version"].as_str()
        && planroom_contract::format::reader_too_old(min, env!("CARGO_PKG_VERSION"))
    {
        return Err(format!("it needs Planroom {min} or newer"));
    }
    let key = (record["root"].clone(), record["state_dir"].clone());
    let same = |t: &Value| t["root"] == key.0 && t["state_dir"] == key.1;
    let list =
        doc.as_object_mut().ok_or("it isn't a JSON object")?.entry("trusted").or_insert_with(|| json!([]));
    let items = list.as_array_mut().ok_or("its trusted isn't a list")?;
    if let Some(old) = items.iter().find(|t| same(t)).and_then(Value::as_object) {
        let fields = record.as_object_mut().expect("a record is an object");
        for (key, value) in old {
            fields.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    items.retain(|t| !same(t));
    items.push(record);
    Ok(doc)
}

/// Unix seconds as an RFC 3339 UTC time. Hand-rolled: the civil-from-days conversion is a dozen
/// lines, and xtask has no date crate.
fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3_600, rem % 3_600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record the engine's trust check reads; `crates/planroom/tests/trust.rs` writes this same
    /// literal and checks that the engine trusts it.
    pub const LITERAL: &str = r#"{"root":"/w","state_dir":"/w/.planroom","ino":7,"by":"engine","at":"2026-10-27T00:00:00Z","created":11}"#;

    #[test]
    fn a_fixture_record_is_the_engines_record() {
        let record = trusted_record("/w", "/w/.planroom", 7, Some(11), "2026-10-27T00:00:00Z");
        assert_eq!(record, serde_json::from_str::<Value>(LITERAL).unwrap());
        let mut doc = json!({"format": FORMAT, "workspaces": [], "later": 1,
            "trusted": [{"root": "/w", "state_dir": "/w/.planroom", "ino": 1, "by": "person", "kept": true}]});
        let doc = with_record(&mut doc, record).unwrap().clone();
        assert_eq!(doc["later"], 1, "an unknown top-level field stays");
        assert_eq!(doc["trusted"].as_array().unwrap().len(), 1, "one record per root and state directory");
        assert_eq!(doc["trusted"][0]["ino"], 7);
        assert_eq!(doc["trusted"][0]["by"], "engine");
        assert_eq!(doc["trusted"][0]["kept"], true, "an unknown field of the old record stays");
        let mut newer = json!({"format": FORMAT, "min_reader_version": "99.0.0", "workspaces": []});
        assert!(with_record(&mut newer, json!({})).is_err(), "a newer file is never rewritten");
    }

    #[test]
    fn rfc3339_counts_days_as_the_calendar_does() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_793_059_200), "2026-10-27T00:00:00Z");
        assert_eq!(rfc3339(951_782_400 + 3_661), "2000-02-29T01:01:01Z");
    }
}
