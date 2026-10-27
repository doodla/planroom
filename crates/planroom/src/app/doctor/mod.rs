//! `planroom doctor`: the dispatcher only. Each check is a module of its own beside this file (the
//! pack check, the backup, restore and the general report), reached by one variant of [`Check`]
//! and one arm of [`run`]. The pack check is the one built so far.

pub mod pack;

use std::path::{Path, PathBuf};

use planroom_contract::{DoctorReq, PackReport};
use serde::Serialize;

use crate::app::Reply;
use crate::app::error::AppError;

/// Which check one `doctor` call runs, from its flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// `--pack DIR [--update]`: one pack's conformance kit.
    Pack { dir: PathBuf, update: bool },
}

impl Check {
    /// The check `req`'s flags ask for, with a relative directory taken from `cwd`.
    pub fn of(req: &DoctorReq, cwd: &Path) -> Result<Check, AppError> {
        match &req.pack {
            Some(dir) => Ok(Check::Pack { dir: cwd.join(dir), update: req.update }),
            None => Err(AppError::invalid_argument("pack", "pack.needs_pack")),
        }
    }
}

/// Each check's result.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DoctorReply {
    Pack(PackReport),
}

/// Run `check` for the workspace at or above `cwd`.
pub fn run(check: Check, _cwd: &Path) -> Result<Reply<DoctorReply>, AppError> {
    match check {
        Check::Pack { dir, update } => {
            let reply = pack::run(&dir, update)?;
            Ok(Reply {
                result: DoctorReply::Pack(reply.result),
                seq: reply.seq,
                warnings: reply.warnings,
                text: reply.text,
            })
        }
    }
}
