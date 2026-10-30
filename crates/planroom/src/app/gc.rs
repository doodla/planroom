//! `gc` after a scan (architecture.md, Duties; DR-47). Until a duty runner exists, the process that
//! ran `scan` runs `gc` right after it, under the `gc` lease: the store prunes old snapshots, a
//! bounded few per repo a run, and the model-cache rows of models no config pins.
//!
//! A scan that succeeded stays a success: every error here, `store_busy` and `schema_mismatch`
//! included, is dropped, and a scan that finds the lease held leaves the run to its holder.

use std::path::Path;

use planroom_contract::ScanReq;
use planroom_store::{Store, process_holder};

use crate::app::error::AppError;
use crate::app::{trust, workspace};
use crate::config;

/// Run `gc` on the store of the workspace `req` scanned from `cwd`, after that scan succeeded.
pub fn after_scan(req: &ScanReq, cwd: &Path) {
    let _ = collect(req, cwd);
}

fn collect(req: &ScanReq, cwd: &Path) -> Result<(), AppError> {
    let dir = req.dir.as_deref().map_or_else(|| cwd.to_path_buf(), |d| cwd.join(d));
    let ws = workspace::discover(&dir)?;
    let Some(mut store) = Store::open_existing(&ws.store_path())? else { return Ok(()) };
    // The pins come from the configuration the scan read: the workspace's file only when trusted.
    let trusted = trust::check(&ws) == trust::Trust::Trusted;
    let loaded = config::for_workspace(&ws.state_dir(), trusted)?;
    let pinned =
        [loaded.config.claude.model_cheap, loaded.config.claude.model_capable, loaded.config.jev.model];
    store.collect(&process_holder(), &pinned)?;
    Ok(())
}
