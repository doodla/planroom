//! What a model call reports when it fails: a registered code and its `details`, never a
//! provider's own error text and never a credential.

use std::fmt;

use planroom_contract::ErrorCode;
use serde_json::{Value, json};

/// A failed model call or gate check. The engine turns it into an envelope error with its own
/// line from the string catalogue; `reason` names that line for an `invalid_argument`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelError {
    pub code: ErrorCode,
    /// The code's `details`, from values this crate chose: IDs, hashes, paths, variable names and
    /// amounts, never a provider's message or a credential.
    pub details: Value,
    /// For `invalid_argument`: the `argument.<reason>` line that says what's wrong.
    pub reason: Option<&'static str>,
}

impl ModelError {
    /// `invalid_argument` on `field`, said by the line `argument.<reason>`.
    pub fn invalid_argument(field: &str, reason: &'static str) -> ModelError {
        ModelError {
            code: ErrorCode::InvalidArgument,
            details: json!({ "field": field }),
            reason: Some(reason),
        }
    }

    /// `budget_exceeded`: the guard `guard` stopped the call, with `spent` of `limit` gone.
    pub fn budget_exceeded(guard: &str, limit: f64, spent: f64) -> ModelError {
        ModelError {
            code: ErrorCode::BudgetExceeded,
            details: json!({ "guard": guard, "limit": round_usd(limit), "spent": round_usd(spent) }),
            reason: None,
        }
    }

    /// `replay_missing`: strict replay has no record `key`, which would live at `path`.
    pub fn replay_missing(key: &str, path: &str) -> ModelError {
        ModelError {
            code: ErrorCode::ReplayMissing,
            details: json!({ "key": key, "path": path }),
            reason: None,
        }
    }

    /// `model_unavailable` for `provider` (`gateway`, `typesafe`).
    pub fn unavailable(provider: &str) -> ModelError {
        ModelError {
            code: ErrorCode::ModelUnavailable,
            details: json!({ "provider": provider }),
            reason: None,
        }
    }

    /// `internal`: Planroom built something the provider or the disk refused, which is a bug.
    pub fn internal() -> ModelError {
        ModelError { code: ErrorCode::Internal, details: json!({ "report": null }), reason: None }
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code.as_str(), self.details)?;
        if let Some(reason) = self.reason {
            write!(f, " ({reason})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ModelError {}

/// Dollars to the micro-dollar, so a sum of floats prints as the amounts it adds up.
pub fn round_usd(usd: f64) -> f64 {
    (usd * 1e6).round() / 1e6
}
