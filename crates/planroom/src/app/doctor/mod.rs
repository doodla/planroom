//! `planroom doctor`: the dispatcher only. Each check is a module of its own beside this file (the
//! pack check, the backup, restore and the general report), reached by one variant of [`Check`]
//! and one arm of [`run`]. No check is built yet, so `Check` has no variant and nothing reaches
//! `run`; the catalogue gains its `doctor` leaf with the first check.

use std::path::Path;

use serde_json::Value;

use crate::app::Reply;
use crate::app::error::AppError;

/// Which check one `doctor` call runs, from its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {}

/// Run `check` for the workspace at or above `cwd`.
pub fn run(check: Check, _cwd: &Path) -> Result<Reply<Value>, AppError> {
    match check {}
}
