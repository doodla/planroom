//! The live gate (architecture.md, What leaves the machine and Spend guards; DR-49, DR-65,
//! DR-89): which model calls may leave the machine, read once from the environment.
//!
//! - `PLANROOM_REPLAY` picks the mode: `strict` answers every call from `tests/replay/` and fails
//!   a missing record with `replay_missing`; `record` answers from a record when one exists and
//!   otherwise makes the live call and records it; unset is live.
//! - A live call is made only in record mode, under a `PLANROOM_LIVE_*` flag for its provider and
//!   the build's spend log. Every other live call is refused with `budget_exceeded`, guard
//!   `replay_only`, before anything is sent: in M1 model calls answer from recordings until the
//!   consent card and the daily cap arrive (DR-89).
//! - `PLANROOM_SPEND_LOG` must be an absolute path, and a `PLANROOM_LIVE_*` flag needs it, since
//!   those flags exist only for this build: both are `invalid_argument` otherwise (DR-65).
//! - An ambient proxy is never a credential: it counts for Claude only under
//!   `PLANROOM_LIVE_GATEWAY=1` (DR-49).
//! - `PLANROOM_REPLAY_DIR` names the recordings' directory by an absolute path, as the spend log
//!   is named; the engine never reads recordings relative to the directory it runs in, where a
//!   workspace could plant them. Tests set it to the checkout's `tests/replay/`.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::ModelError;

/// The variable that names the build-wide spend log, by an absolute path.
pub const SPEND_LOG: &str = "PLANROOM_SPEND_LOG";
/// The variable that picks the replay mode.
pub const REPLAY: &str = "PLANROOM_REPLAY";
/// The variable that names the recordings' directory, by an absolute path.
pub const REPLAY_DIR: &str = "PLANROOM_REPLAY_DIR";
/// Every live flag starts with this.
pub const LIVE_PREFIX: &str = "PLANROOM_LIVE_";
/// The flag that lets Claude calls go to the gateway, the build container's proxy included.
pub const LIVE_GATEWAY: &str = "PLANROOM_LIVE_GATEWAY";
/// The flag that lets Jev calls go out.
pub const LIVE_JEV: &str = "PLANROOM_LIVE_JEV";
/// The guard `budget_exceeded` names when a live call is refused outside record mode.
pub const REPLAY_ONLY: &str = "replay_only";

/// How model calls are answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Every call from a record; a missing one is `replay_missing`.
    Strict,
    /// A record when one exists, else a live call that is then recorded.
    Record,
    /// Live calls, which M1 refuses.
    Live,
}

/// Where a live call goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Claude on the Vercel AI Gateway.
    Gateway,
    /// Jev, on the gateway or TypeSafe.
    Jev,
}

impl Provider {
    /// The provider's name in the spend log and in `model_unavailable`.
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Gateway => "gateway",
            Provider::Jev => "jev",
        }
    }
}

/// The gate's reading of the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveEnv {
    pub mode: Mode,
    /// `PLANROOM_LIVE_GATEWAY=1`.
    pub gateway: bool,
    /// `PLANROOM_LIVE_JEV=1`.
    pub jev: bool,
    /// `PLANROOM_SPEND_LOG`, absolute.
    pub spend_log: Option<PathBuf>,
    /// `PLANROOM_REPLAY_DIR`, absolute: where strict replay and record mode find recordings.
    pub replay_dir: Option<PathBuf>,
}

impl LiveEnv {
    /// Read this process's environment.
    pub fn from_process() -> Result<LiveEnv, ModelError> {
        LiveEnv::from_vars(std::env::vars_os())
    }

    /// Read the gate's variables from `vars`. An empty value counts as unset.
    pub fn from_vars(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Result<LiveEnv, ModelError> {
        let mut spend_log = None;
        let mut replay = None;
        let mut replay_dir = None;
        let mut flags: Vec<(String, OsString)> = Vec::new();
        for (name, value) in vars {
            if value.is_empty() {
                continue;
            }
            let Some(name) = name.to_str() else { continue };
            match name {
                SPEND_LOG => spend_log = Some(value),
                REPLAY => replay = Some(value),
                REPLAY_DIR => replay_dir = Some(value),
                _ if name.starts_with(LIVE_PREFIX) => flags.push((name.to_owned(), value)),
                _ => {}
            }
        }
        flags.sort();
        let spend_log = match spend_log {
            None => None,
            Some(path) => {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(ModelError::invalid_argument(SPEND_LOG, "spend_log_relative"));
                }
                Some(path)
            }
        };
        let replay_dir = match replay_dir.map(PathBuf::from) {
            Some(dir) if !dir.is_absolute() => {
                return Err(ModelError::invalid_argument(REPLAY_DIR, "replay_dir_relative"));
            }
            dir => dir,
        };
        let (mut gateway, mut jev) = (false, false);
        for (name, value) in &flags {
            if spend_log.is_none() {
                return Err(ModelError::invalid_argument(name, "live_flag_without_spend_log"));
            }
            let on = match value.to_str() {
                Some("1") => true,
                _ => return Err(ModelError::invalid_argument(name, "live_flag_value")),
            };
            match name.as_str() {
                LIVE_GATEWAY => gateway = on,
                LIVE_JEV => jev = on,
                _ => {}
            }
        }
        let mode = match replay.as_ref().map(|r| r.to_str()) {
            None => Mode::Live,
            Some(Some("strict")) => Mode::Strict,
            Some(Some("record")) => Mode::Record,
            Some(_) => return Err(ModelError::invalid_argument(REPLAY, "replay_mode")),
        };
        if mode == Mode::Record && !(gateway || jev) {
            return Err(ModelError::invalid_argument(REPLAY, "record_without_live_flag"));
        }
        Ok(LiveEnv { mode, gateway, jev, spend_log, replay_dir })
    }

    /// Whether a live call to `provider` may be sent, and the spend log it must go through.
    /// Outside record mode, or without the provider's flag, it is `budget_exceeded` with guard
    /// `replay_only`, before anything is sent.
    pub fn live_call(&self, provider: Provider) -> Result<&std::path::Path, ModelError> {
        let flagged = match provider {
            Provider::Gateway => self.gateway,
            Provider::Jev => self.jev,
        };
        match (&self.mode, flagged, &self.spend_log) {
            (Mode::Record, true, Some(log)) => Ok(log),
            _ => Err(ModelError::budget_exceeded(REPLAY_ONLY, 0.0, 0.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(vars: &[(&str, &str)]) -> Result<LiveEnv, ModelError> {
        LiveEnv::from_vars(vars.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))))
    }

    #[test]
    fn a_relative_replay_dir_is_refused() {
        let err = read(&[(REPLAY, "strict"), (REPLAY_DIR, "tests/replay")]).unwrap_err();
        assert_eq!(err.details["field"], REPLAY_DIR);
        assert_eq!(err.reason, Some("replay_dir_relative"));
        let env = read(&[(REPLAY, "strict"), (REPLAY_DIR, "/checkout/tests/replay")]).unwrap();
        assert_eq!(env.replay_dir.as_deref(), Some(std::path::Path::new("/checkout/tests/replay")));
        // Empty is unset, as for every variable the gate reads.
        assert_eq!(read(&[(REPLAY_DIR, "")]).unwrap().replay_dir, None);
    }
}
