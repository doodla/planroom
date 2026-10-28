//! The engine's side of the model boundary: it reads the live gate before any command that may
//! call a model, builds a request's uncached tail with its repo text masked, and turns a model
//! error into an envelope error with the engine's own line.
//!
//! The gate's rules live in `planroom-models` (architecture.md, Spend guards; DR-65, DR-89): a
//! relative `PLANROOM_SPEND_LOG`, or a `PLANROOM_LIVE_*` flag with no spend log, is
//! `invalid_argument` before the command does anything.

use planroom_contract::ErrorCode;
use planroom_kernel::redact;
use planroom_models::gate::{self, LiveEnv};
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
    Ok(vec![slice, redact::text(question)])
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
