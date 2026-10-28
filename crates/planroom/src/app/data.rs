//! The data root (architecture.md, Where the workspace lives): one directory per user, shared by
//! the engine, the resolver and the mod, and its `workspaces.json`, which gives each workspace
//! its own canvas port (DR-58, DR-78).
//!
//! `workspaces.json` serves every engine version on the machine at once, so it changes only by
//! additions (contracts.md, Store schema and migrations; DR-78):
//!
//! - a reader checks its `format` and its `min_reader_version`, and an engine older than either
//!   refuses with `schema_mismatch` naming the file, neither reading nor rewriting it;
//! - every field this engine doesn't know, at the top level and in each entry, is kept and written
//!   back unchanged;
//! - besides the entries, it holds the state directories recorded as trusted (`trusted`) and the
//!   person key each root pinned (`person_keys`), each list left out when empty;
//! - a record holds an exclusive lock on `workspaces.json.lock` across its read, its change and its
//!   rename, so two engines recording at once lose nothing.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use planroom_contract::format::{self, Document, Extra};
use serde::{Deserialize, Serialize};
use serde_json::Map;

use crate::app::error::AppError;
use crate::text;

/// The oldest engine version that reads `workspaces.json` as this one writes it. A version that
/// changes the file so that older engines would misread it raises this, and only then (DR-78).
pub const MIN_READER_VERSION: &str = "0.1.0-dev";

/// This engine's version, which `min_reader_version` is compared with.
const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `${PLANROOM_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/planroom}`, always absolute.
///
/// The XDG spec says to ignore a relative `XDG_DATA_HOME`, so it counts as unset. With no absolute
/// value to start from, as under a duty, a cron job or a sandbox that runs with no `HOME`, the data
/// root has no place on this machine: a relative one would land in whatever directory the process
/// runs in, so the port registry, logs and login pages would land inside a workspace
/// (unasked-r3-3). That's `invalid_argument` naming `PLANROOM_DATA`, the variable that fixes it.
pub fn root() -> Result<PathBuf, AppError> {
    let absolute = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if std::env::var_os("PLANROOM_DATA").is_some_and(|d| !d.is_empty()) {
        return absolute("PLANROOM_DATA").ok_or_else(unplaced);
    }
    let base = absolute("XDG_DATA_HOME").or_else(|| absolute("HOME").map(|home| home.join(".local/share")));
    base.map(|b| b.join("planroom")).ok_or_else(unplaced)
}

/// The user-level config, `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml`, always
/// absolute: a relative `XDG_CONFIG_HOME` counts as unset, as the XDG spec says. With neither
/// giving an absolute place there's no user-level file, since a relative one would read whatever
/// file sits in the directory the process runs in, such as one a repo commits.
pub fn user_config() -> Option<PathBuf> {
    let absolute = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|home| home.join(".config")));
    base.map(|b| b.join("planroom").join(crate::config::FILE))
}

/// The data root has no absolute place: the fix sets `PLANROOM_DATA`.
fn unplaced() -> AppError {
    AppError::invalid_argument("PLANROOM_DATA", "argument.data_root")
        .with_hint(text::get("hint.data_root", &[]))
}

/// Create `dir` and its missing parents with mode 0700.
pub fn ensure_dir(dir: &Path) -> Result<(), AppError> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| AppError::internal(format!("{}: {e}", dir.display())))
}

/// Write `bytes` to `path` with mode 0600, through a temporary file and a rename, so a reader
/// never sees half a file.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let fail = |e: std::io::Error| AppError::internal(format!("{}: {e}", path.display()));
    let _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(fail)?;
    file.write_all(bytes).and_then(|()| file.sync_all()).map_err(fail)?;
    fs::rename(&tmp, path).map_err(fail)
}

/// One workspace the engine created, and its canvas port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub root: String,
    pub workspace_id: String,
    pub port: u16,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Extra,
}

impl WorkspaceEntry {
    pub fn new(root: String, workspace_id: String, port: u16) -> WorkspaceEntry {
        WorkspaceEntry { root, workspace_id, port, extra: Extra::new() }
    }
}

/// Who recorded a state directory as trusted: the engine that created it, or a person who
/// accepted its trust card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustedBy {
    Engine,
    Person,
}

/// One state directory recorded as trusted (architecture.md, Where the workspace lives: Trust).
/// It matches only the directory it was written for: its canonical root, its state directory's
/// path, its inode number and, where the filesystem reports one, its birth time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trusted {
    pub root: String,
    pub state_dir: String,
    pub ino: u64,
    /// The state directory's birth time in nanoseconds since the epoch; left out where the
    /// filesystem reports none (a network mount, 9p or drvfs, FUSE).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    pub by: TrustedBy,
    pub at: String,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Extra,
}

/// The SHA-256 of the person key a root registered, written while the workspace was trusted, so a
/// store unpacked over the state directory can't bring a key of its own (DR-88).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonKeyPin {
    pub root: String,
    pub fingerprint: String,
    pub at: String,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Extra,
}

/// `workspaces.json`: `planroom.workspaces/v1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Workspaces {
    format: String,
    /// The oldest engine version that may read this file; an older one refuses it (DR-78).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_reader_version: Option<String>,
    workspaces: Vec<WorkspaceEntry>,
    /// The state directories recorded as trusted, one per root and state directory; left out when
    /// empty. An older engine keeps the list in `extra` and writes it back unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trusted: Vec<Trusted>,
    /// The fingerprint of the person key each root registered while trusted, one per root; left
    /// out when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    person_keys: Vec<PersonKeyPin>,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    extra: Extra,
}

impl Document for Workspaces {
    const NAME: &'static str = "workspaces";
}

impl Workspaces {
    fn empty() -> Workspaces {
        Workspaces {
            format: Workspaces::format_tag(),
            min_reader_version: Some(MIN_READER_VERSION.into()),
            workspaces: vec![],
            trusted: vec![],
            person_keys: vec![],
            extra: Extra::new(),
        }
    }
}

fn path(data: &Path) -> PathBuf {
    data.join("workspaces.json")
}

/// Read and check `workspaces.json`; a missing file is an empty one.
fn load(data: &Path) -> Result<Workspaces, AppError> {
    let path = path(data);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Workspaces::empty()),
        Err(e) => return Err(AppError::internal(format!("{}: {e}", path.display()))),
    };
    let doc: Workspaces = format::read_str(&text).map_err(|e| AppError::unreadable(&path, &e))?;
    if let Some(min) = &doc.min_reader_version
        && format::reader_too_old(min, ENGINE_VERSION)
    {
        return Err(AppError::newer_file(&path, min, ENGINE_VERSION));
    }
    Ok(doc)
}

/// Every workspace `workspaces.json` records.
pub fn workspaces(data: &Path) -> Result<Vec<WorkspaceEntry>, AppError> {
    Ok(load(data)?.workspaces)
}

/// Record `entry`, replacing any entry for the same root and keeping the fields that entry held
/// which `entry` doesn't set. The whole read, change and rename runs under an exclusive lock.
pub fn record(data: &Path, mut entry: WorkspaceEntry) -> Result<(), AppError> {
    ensure_dir(data)?;
    let _lock = lock(data)?;
    let mut doc = load(data)?;
    if let Some(old) = doc.workspaces.iter().find(|w| w.root == entry.root) {
        for (key, value) in &old.extra {
            entry.extra.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    doc.workspaces.retain(|w| w.root != entry.root);
    doc.workspaces.push(entry);
    doc.workspaces.sort_by(|a, b| a.root.cmp(&b.root));
    // A floor a newer engine set stays as it is: raising it is a migration's call, never a record's.
    doc.min_reader_version.get_or_insert_with(|| MIN_READER_VERSION.into());
    let text = serde_json::to_string_pretty(&doc).map_err(AppError::internal)?;
    write_private(&path(data), text.as_bytes())
}

/// Change the recorded entries with `change`, under the same exclusive lock as `record`, so a
/// check and the write it guards see one state: `serve` checks that no other workspace records
/// a port, and moves a workspace's entry to its new root, in one step (DR-78). Nothing is written
/// when `change` fails.
pub fn update<T>(
    data: &Path,
    change: impl FnOnce(&mut Vec<WorkspaceEntry>) -> Result<T, AppError>,
) -> Result<T, AppError> {
    ensure_dir(data)?;
    let _lock = lock(data)?;
    let mut doc = load(data)?;
    let out = change(&mut doc.workspaces)?;
    doc.workspaces.sort_by(|a, b| a.root.cmp(&b.root));
    doc.min_reader_version.get_or_insert_with(|| MIN_READER_VERSION.into());
    let text = serde_json::to_string_pretty(&doc).map_err(AppError::internal)?;
    write_private(&path(data), text.as_bytes())?;
    Ok(out)
}

/// The trust records for the workspace at `root`: one per state directory it was recorded with.
pub fn trusted(data: &Path, root: &str) -> Result<Vec<Trusted>, AppError> {
    Ok(load(data)?.trusted.into_iter().filter(|t| t.root == root).collect())
}

/// Record `entry`, replacing the record for the same root and state directory and keeping the
/// fields it held that `entry` doesn't set, under the same exclusive lock as [`record`].
pub fn trust(data: &Path, mut entry: Trusted) -> Result<(), AppError> {
    change(data, |doc| {
        let same = |t: &Trusted| t.root == entry.root && t.state_dir == entry.state_dir;
        if let Some(old) = doc.trusted.iter().find(|t| same(t)) {
            for (key, value) in &old.extra {
                entry.extra.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        doc.trusted.retain(|t| !same(t));
        doc.trusted.push(entry);
        doc.trusted.sort_by(|a, b| (&a.root, &a.state_dir).cmp(&(&b.root, &b.state_dir)));
    })
}

/// Remove every trust record for the workspace at `root`.
pub fn untrust(data: &Path, root: &str) -> Result<(), AppError> {
    if !load(data)?.trusted.iter().any(|t| t.root == root) {
        return Ok(());
    }
    change(data, |doc| doc.trusted.retain(|t| t.root != root))
}

/// The person key fingerprint pinned for the workspace at `root`, if any.
pub fn person_key(data: &Path, root: &str) -> Result<Option<PersonKeyPin>, AppError> {
    Ok(load(data)?.person_keys.into_iter().find(|p| p.root == root))
}

/// Pin `pin` for its root, replacing the root's pin and keeping the fields it held.
pub fn pin_person_key(data: &Path, mut pin: PersonKeyPin) -> Result<(), AppError> {
    change(data, |doc| {
        if let Some(old) = doc.person_keys.iter().find(|p| p.root == pin.root) {
            for (key, value) in &old.extra {
                pin.extra.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        doc.person_keys.retain(|p| p.root != pin.root);
        doc.person_keys.push(pin);
        doc.person_keys.sort_by(|a, b| a.root.cmp(&b.root));
    })
}

/// Change the document with `edit` under the exclusive lock, and write it back.
fn change(data: &Path, edit: impl FnOnce(&mut Workspaces)) -> Result<(), AppError> {
    ensure_dir(data)?;
    let _lock = lock(data)?;
    let mut doc = load(data)?;
    edit(&mut doc);
    doc.min_reader_version.get_or_insert_with(|| MIN_READER_VERSION.into());
    let text = serde_json::to_string_pretty(&doc).map_err(AppError::internal)?;
    write_private(&path(data), text.as_bytes())
}

/// An exclusive lock on `workspaces.json.lock`, held until the returned file is dropped. Every
/// engine takes it around a change to `workspaces.json`.
fn lock(data: &Path) -> Result<File, AppError> {
    let path = data.join("workspaces.json.lock");
    let fail = |e: std::io::Error| AppError::internal(format!("{}: {e}", path.display()));
    let file =
        OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(&path).map_err(fail)?;
    file.lock().map_err(fail)?;
    Ok(file)
}
