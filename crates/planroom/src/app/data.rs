//! The data root (architecture.md, Where the workspace lives): one directory per user, shared by
//! the engine, the resolver and the mod, and its `workspaces.json`, which gives each workspace
//! its own canvas port (DR-58, DR-78).

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

const FORMAT: &str = "planroom.workspaces/v1";

/// `${PLANROOM_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/planroom}`.
pub fn root() -> PathBuf {
    if let Some(dir) = std::env::var_os("PLANROOM_DATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    base.join("planroom")
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub root: String,
    pub workspace_id: String,
    pub port: u16,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Workspaces {
    format: String,
    workspaces: Vec<WorkspaceEntry>,
}

/// Every workspace `workspaces.json` records.
pub fn workspaces(data: &Path) -> Result<Vec<WorkspaceEntry>, AppError> {
    let path = data.join("workspaces.json");
    match fs::read_to_string(&path) {
        Ok(text) => {
            let parsed: Workspaces = serde_json::from_str(&text)
                .map_err(|e| AppError::internal(format!("{}: {e}", path.display())))?;
            Ok(parsed.workspaces)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(AppError::internal(format!("{}: {e}", path.display()))),
    }
}

/// Record `entry`, replacing any entry for the same root.
pub fn record(data: &Path, entry: WorkspaceEntry) -> Result<(), AppError> {
    ensure_dir(data)?;
    let mut all = workspaces(data)?;
    all.retain(|w| w.root != entry.root);
    all.push(entry);
    all.sort_by(|a, b| a.root.cmp(&b.root));
    let doc = Workspaces { format: FORMAT.into(), workspaces: all };
    let text = serde_json::to_string_pretty(&doc).map_err(AppError::internal)?;
    write_private(&data.join("workspaces.json"), text.as_bytes())
}
