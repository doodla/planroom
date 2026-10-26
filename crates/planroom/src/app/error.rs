//! Errors as the envelope reports them: a registered code, an engine-authored line and a hint.

use planroom_contract::{ErrorCode, ErrorObject};
use planroom_extract::ExtractError;
use planroom_extract::git::GitError;
use planroom_store::StoreError;
use serde_json::{Value, json};

use crate::text;

/// A failed command, boxed so results stay small.
#[derive(Debug, Clone, PartialEq)]
pub struct AppError(Box<Failure>);

/// What a failed command reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
    pub details: Value,
}

impl std::ops::Deref for AppError {
    type Target = Failure;

    fn deref(&self) -> &Failure {
        &self.0
    }
}

impl AppError {
    /// An error whose message is `error.<code>` from the catalog, filled from `vars`.
    pub fn new(code: ErrorCode, vars: &[(&str, &dyn std::fmt::Display)]) -> AppError {
        AppError(Box::new(Failure {
            code,
            message: text::get(&format!("error.{}", code.as_str()), vars),
            hint: text::try_get(&format!("hint.{}", code.as_str())),
            details: json!({}),
        }))
    }

    pub fn details(mut self, details: Value) -> AppError {
        self.0.details = details;
        self
    }

    /// Replace the hint with an exact fix line.
    pub fn with_hint(mut self, hint: String) -> AppError {
        self.0.hint = Some(hint);
        self
    }

    pub fn invalid_argument(field: &str, key: &str) -> AppError {
        AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get(key, &[]))])
            .details(json!({"field": field}))
    }

    pub fn workspace_not_found(path: &std::path::Path) -> AppError {
        let shown = path.display();
        AppError::new(ErrorCode::WorkspaceNotFound, &[("path", &shown)])
            .details(json!({"path": shown.to_string()}))
    }

    pub fn internal(what: impl std::fmt::Display) -> AppError {
        AppError::new(ErrorCode::Internal, &[("what", &what)]).details(json!({"report": null}))
    }

    pub fn git(repo: &str, e: &GitError) -> AppError {
        let first = e.stderr.lines().next().unwrap_or_default().to_owned();
        AppError::new(ErrorCode::GitFailed, &[("repo", &repo), ("what", &first)])
            .details(json!({"argv": e.argv, "stderr": e.stderr}))
    }

    pub fn object(&self) -> ErrorObject {
        let spec = self.code.spec();
        ErrorObject {
            code: self.code,
            category: spec.category,
            retry: spec.retry,
            retry_after_ms: None,
            message: self.message.clone(),
            hint: self.hint.clone(),
            details: self.details.clone(),
        }
    }
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> AppError {
        let details = e.details();
        let base = match &e {
            StoreError::SchemaMismatch { found, supported, .. } => {
                AppError::new(e.code(), &[("found", found), ("supported", supported)])
            }
            StoreError::IdempotencyMismatch { idem, .. } => AppError::new(e.code(), &[("idem", idem)]),
            StoreError::Busy => AppError::new(e.code(), &[]),
            StoreError::Io(what) | StoreError::Sqlite(what) => return AppError::internal(what),
        };
        base.details(details)
    }
}

impl From<ExtractError> for AppError {
    fn from(e: ExtractError) -> AppError {
        AppError::internal(e)
    }
}
