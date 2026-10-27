//! The engine's side of the model boundary: it reads the live gate before any command that may
//! call a model, and turns a model error into an envelope error with the engine's own line.
//!
//! The gate's rules live in `planroom-models` (architecture.md, Spend guards; DR-65, DR-89): a
//! relative `PLANROOM_SPEND_LOG`, or a `PLANROOM_LIVE_*` flag with no spend log, is
//! `invalid_argument` before the command does anything.

use planroom_contract::ErrorCode;
use planroom_models::ModelError;
use planroom_models::gate::{self, LiveEnv};

use crate::app::error::AppError;
use crate::text;

/// The live gate, read from this process's environment.
pub fn live_env() -> Result<LiveEnv, AppError> {
    LiveEnv::from_process().map_err(AppError::from)
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
