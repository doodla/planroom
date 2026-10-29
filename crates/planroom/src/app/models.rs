//! The engine's side of the model boundary: it reads the live gate before any command that may
//! call a model, builds a request's uncached tail with its repo text masked, and turns a model
//! error into an envelope error with the engine's own line.
//!
//! The gate's rules live in `planroom-models` (architecture.md, Spend guards; DR-65, DR-89): a
//! relative `PLANROOM_SPEND_LOG`, or a `PLANROOM_LIVE_*` flag with no spend log, is
//! `invalid_argument` before the command does anything.

use std::path::Path;
use std::time::Duration;

use planroom_contract::ErrorCode;
use planroom_contract::config::Config;
use planroom_kernel::redact;
use planroom_models::claude::{self, Call, Claude, REPLAY_PROVIDER, Reply};
use planroom_models::gate::{self, LiveEnv, Mode, Provider};
use planroom_models::replay::ReplayDir;
use planroom_models::transport::Live;
use planroom_models::{ModelError, canonical};
use serde::Serialize;

use crate::app::error::AppError;
use crate::text;

/// The live gate, read from this process's environment.
pub fn live_env() -> Result<LiveEnv, AppError> {
    LiveEnv::from_process().map_err(AppError::from)
}

/// A model request's uncached tail, the two blocks `planroom_models::claude::Call::tail` holds:
/// first the slice, masked by its own type's marks and written as canonical JSON, then the
/// question, masked as an engine line built from names, like `Envelope.summary` (contracts.md, the
/// redaction paragraph; DR-49, DR-64). Every model request's tail is built here.
///
/// The slice goes inside a fence, `<repo-data id="r…">` … `</repo-data>`, that the prompt prefix
/// names as data, never instructions (architecture.md, the prompt injection row). Its ID is `r` and
/// the first 16 hex characters of the sha256 of the masked canonical JSON, so the same slice always
/// gives the same bytes and its replay key holds. Every `<` in the JSON is written `\u003c`, which
/// it can only hold inside a string, so the JSON stays valid and no text in it closes the fence.
///
/// It takes no raw flag and reads no environment, so a model request stays redacted whatever
/// `PLANROOM_RAW` says: `cli::raw_for` stays that variable's one reader, and the variable reaches
/// only a read command's `--json` stdout (DR-75).
///
/// A slice whose type `kernel::redact` doesn't list, such as a `Vec<Node>` or a wrapper of the
/// app's own, would pass through unmasked, and so would what a listed type's open value holds,
/// such as an `Envelope`'s or a `ToolResult`'s `result`; so `redact::is_listed` refuses each, and
/// the slice is refused with `internal`. A slice is always a contract type whose marks reach every
/// string it holds, such as `Node`, `MapView` or `FactBatch`.
pub fn request_tail<T: Serialize>(slice: &T, question: &str) -> Result<Vec<String>, AppError> {
    if !redact::is_listed::<T>() {
        let name = std::any::type_name::<T>();
        return Err(AppError::internal(format!(
            "a model request's slice of type {name} has no redaction entry"
        )));
    }
    let mut json = serde_json::to_value(slice).map_err(AppError::internal)?;
    redact::value_of::<T>(&mut json);
    let slice = String::from_utf8(canonical::bytes(&json)).map_err(AppError::internal)?;
    let id = &planroom_kernel::keys::sha256_plain_hex(slice.as_bytes())[..FENCE_HEX];
    let fenced = format!("<repo-data id=\"r{id}\">{}</repo-data>", slice.replace('<', "\\u003c"));
    Ok(vec![fenced, redact::text(question)])
}

/// Hex characters of the slice's hash a fence's ID keeps.
const FENCE_HEX: usize = 16;

/// How long one Claude call may take before the transport gives up.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
/// Where a missing record would live, as `replay_missing` names it when no
/// `PLANROOM_REPLAY_DIR` is set: the checkout's `tests/replay/`.
const REPLAY_HOME: &str = "tests/replay";

/// Ask Claude `call` through the gateway adapter, the one place the engine assembles it: the
/// configuration's `[claude]` settings and `live_tests_total_usd`, the credential and the values no
/// record may hold from this process's environment, the recordings under `PLANROOM_REPLAY_DIR`,
/// and the live transport. The gate in `env` decides between a recording, a recorded live call
/// and a refusal (DR-49, DR-65, DR-89).
///
/// Under strict replay or record mode with no `PLANROOM_REPLAY_DIR`, the call is `replay_missing`
/// naming where its record would live, and nothing is read: recordings are never looked for
/// relative to the directory the command runs in, where a workspace could plant one. A live call
/// outside record mode is the gate's `budget_exceeded`, guard `replay_only`, before anything else.
pub fn ask_claude(call: &Call, env: &LiveEnv, config: &Config) -> Result<Reply, AppError> {
    let replay = match (&env.replay_dir, env.mode) {
        (Some(dir), _) => ReplayDir::new(dir),
        (None, Mode::Live) => {
            // Outside record mode the gate refuses every live call.
            let refused = env.live_call(Provider::Gateway).err().unwrap_or_else(ModelError::internal);
            return Err(refused.into());
        }
        (None, Mode::Strict | Mode::Record) => {
            let built = claude::build(&config.claude, call);
            let path =
                ReplayDir::new(Path::new(REPLAY_HOME)).path(REPLAY_PROVIDER, &call.purpose, &built.key)?;
            return Err(ModelError::replay_missing(&built.key, &path.display().to_string()).into());
        }
    };
    let var = |name: &str| std::env::var(name).ok();
    let credential = claude::credential(&config.claude, &var)?;
    let secrets = claude::secrets(credential.as_ref(), &var);
    let transport = Live::new(CALL_TIMEOUT)?;
    let adapter = Claude {
        config: &config.claude,
        cap_usd: config.budget.live_tests_total_usd,
        env,
        replay: &replay,
        transport: &transport,
        credential,
        secrets,
    };
    Ok(adapter.call(call)?)
}

impl From<ModelError> for AppError {
    fn from(e: ModelError) -> AppError {
        let d = &e.details;
        let field = d["field"].as_str().unwrap_or_default();
        match e.code {
            ErrorCode::InvalidArgument => {
                let reason = e.reason.unwrap_or("model_setting");
                AppError::invalid_argument(field, &format!("argument.{reason}"))
                    .with_hint(text::get(&format!("hint.model_{reason}"), &[("field", &field)]))
            }
            ErrorCode::BudgetExceeded if d["guard"] == gate::REPLAY_ONLY => AppError::new(e.code, &[])
                .details(e.details.clone())
                .with_message(text::get("models.replay_only", &[])),
            ErrorCode::BudgetExceeded => {
                let (guard, limit, spent) = (d["guard"].clone(), d["limit"].clone(), d["spent"].clone());
                let (guard, limit, spent) =
                    (guard.as_str().unwrap_or_default(), dollars(&limit), dollars(&spent));
                AppError::new(e.code, &[("guard", &guard), ("limit", &limit), ("spent", &spent)])
                    .details(e.details.clone())
            }
            ErrorCode::ReplayMissing => {
                let key = d["key"].as_str().unwrap_or_default();
                AppError::new(e.code, &[("key", &key)]).details(e.details.clone())
            }
            ErrorCode::ModelUnavailable => {
                let provider = d["provider"].as_str().unwrap_or_default();
                AppError::new(e.code, &[("provider", &provider)]).details(e.details.clone())
            }
            ErrorCode::Internal => AppError::internal(text::get("models.internal", &[])),
            code => AppError::new(code, &[]).details(e.details.clone()),
        }
    }
}

/// `$0.25` from a JSON number of dollars.
fn dollars(v: &serde_json::Value) -> String {
    format!("${:.2}", v.as_f64().unwrap_or_default())
}
