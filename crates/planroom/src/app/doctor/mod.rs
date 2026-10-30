//! `planroom doctor`: the dispatcher only. Each check is a module of its own beside this file,
//! reached by one variant of [`Check`] and one arm of [`run`]: with no flag, the general report of
//! git, the store, its migrations, the pack set and the event backup ([`general`], with the backup
//! check in [`backup`]); with `--pack DIR`, one pack's conformance kit ([`pack`]); with
//! `--restore BACKUP`, a person's put-back of a backup ([`restore`]), where the backup a scan offers
//! back and its card are too.

pub mod backup;
pub mod general;
pub mod pack;
pub mod restore;

use std::path::{Path, PathBuf};

use planroom_contract::{DoctorReq, DoctorResult};

use crate::app::Reply;
use crate::app::actor::{Caller, Terminal};
use crate::app::error::AppError;

/// Which check one `doctor` call runs, from its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// No flag: the general report.
    General,
    /// `--pack DIR [--update]`: one pack's conformance kit.
    Pack { dir: PathBuf, update: bool },
    /// `--restore BACKUP`: put a backup back; `given` is BACKUP as the caller wrote it.
    Restore { path: PathBuf, given: String },
}

impl Check {
    /// The check `req`'s flags ask for, with a relative directory taken from `cwd`.
    pub fn of(req: &DoctorReq, cwd: &Path) -> Result<Check, AppError> {
        match (&req.pack, &req.restore) {
            (Some(dir), _) => Ok(Check::Pack { dir: cwd.join(dir), update: req.update }),
            (None, Some(given)) => Ok(Check::Restore { path: cwd.join(given), given: given.clone() }),
            (None, None) => Ok(Check::General),
        }
    }
}

/// The git on `PATH` is new enough for the adapter, checked before any other doctor check, or
/// `git_too_old` (DR-78).
pub fn git() -> Result<(), AppError> {
    planroom_extract::git::check_version().map_err(|e| AppError::git_too_old(&e))
}

/// Run `check` for the workspace at or above `cwd`, as `caller`, who confirms at `terminal` where
/// a check needs a person.
pub fn run(
    check: Check,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Reply<DoctorResult>, AppError> {
    match check {
        Check::General => general::run(cwd).map(|r| as_result(r, DoctorResult::General)),
        Check::Pack { dir, update } => pack::run(&dir, update).map(|r| as_result(r, DoctorResult::Pack)),
        Check::Restore { path, given } => {
            restore::run(&path, &given, cwd, caller, terminal).map(|r| as_result(r, DoctorResult::Restore))
        }
    }
}

/// `reply` with its result made the variant `wrap` gives.
fn as_result<T>(reply: Reply<T>, wrap: fn(T) -> DoctorResult) -> Reply<DoctorResult> {
    Reply { result: wrap(reply.result), seq: reply.seq, warnings: reply.warnings, text: reply.text }
}
