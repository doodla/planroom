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

    /// Replace the message with another engine-authored line, for a case the code's own line
    /// doesn't fit.
    pub fn with_message(mut self, message: String) -> AppError {
        self.0.message = message;
        self
    }

    /// Replace the hint with an exact fix line.
    pub fn with_hint(mut self, hint: String) -> AppError {
        self.0.hint = Some(hint);
        self
    }

    /// Drop the code's catalog hint, where no command a person could run fixes this case.
    pub fn without_hint(mut self) -> AppError {
        self.0.hint = None;
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

    /// `schema_mismatch` for a persisted file this engine may not read or rewrite: its `format` is
    /// newer than this engine's, or its `min_reader_version` is above this engine's version
    /// (contracts.md, Versioning rules; DR-78). `found` and `supported` name what the file asks
    /// for and what this engine has.
    pub fn newer_file(file: &std::path::Path, found: &str, supported: &str) -> AppError {
        let shown = file.display().to_string();
        let mut e = AppError::new(ErrorCode::SchemaMismatch, &[("found", &found), ("supported", &supported)])
            .details(json!({"found": found, "supported": supported, "writer_version": null, "file": shown}));
        e.0.message =
            text::get("data.newer_file", &[("file", &shown), ("found", &found), ("supported", &supported)]);
        e
    }

    /// A persisted document's read failure, as `newer_file` or `internal`.
    pub fn unreadable(file: &std::path::Path, e: &planroom_contract::format::ReadError) -> AppError {
        use planroom_contract::format::{FormatError, ReadError};
        match e {
            ReadError::Format(FormatError::Newer { found, supported }) => {
                AppError::newer_file(file, found, supported)
            }
            other => AppError::internal(format!("{}: {other}", file.display())),
        }
    }

    /// `pack_invalid`: the pack file `file` fails at `line` over `reference`, for the reason
    /// `pack.<why>` gives, filled from `vars`.
    pub fn pack_invalid(
        file: &str,
        line: u32,
        reference: &str,
        why: &str,
        vars: &[(&str, &dyn std::fmt::Display)],
    ) -> AppError {
        let why = text::get(&format!("pack.{why}"), vars);
        AppError::new(ErrorCode::PackInvalid, &[("file", &file), ("line", &line), ("why", &why)])
            .details(json!({"file": file, "line": line, "reference": reference}))
    }

    pub fn internal(what: impl std::fmt::Display) -> AppError {
        AppError::new(ErrorCode::Internal, &[("what", &what)]).details(json!({"report": null}))
    }

    /// The git on `PATH` is older than the adapter's floor, or no git answers: the fix names the
    /// floor (DR-78).
    pub fn git_too_old(e: &planroom_extract::git::TooOld) -> AppError {
        let minimum = planroom_extract::git::TooOld::minimum();
        let fix = text::get("hint.git_too_old", &[("minimum", &minimum)]);
        let error = match &e.found {
            Some(found) => AppError::new(ErrorCode::GitTooOld, &[("found", found), ("minimum", &minimum)]),
            None => AppError::new(ErrorCode::GitTooOld, &[("minimum", &minimum)])
                .with_message(text::get("error.git_missing", &[("minimum", &minimum)])),
        };
        error.details(json!({"found": e.found, "minimum": minimum, "fix": fix})).with_hint(fix)
    }

    /// git failed in the repo at `path`; the hint is the command that shows why: git with the
    /// repo's own git directory named, so it never reports a repo above it, or `ls -l` of a place
    /// git needs as a directory or a regular file that is neither, which git would block on.
    pub fn git(repo: &str, path: &std::path::Path, e: &GitError) -> AppError {
        let first = e.stderr.lines().next().unwrap_or_default().to_owned();
        let hint = match &e.refusal {
            Some(planroom_extract::git::Refusal::NotAFile(place)) => {
                let place = text::shell_word(&place.display().to_string());
                text::get("hint.git_failed_not_a_file", &[("path", &place)])
            }
            _ => text::get("hint.git_failed", &[("path", &text::shell_word(&path.display().to_string()))]),
        };
        AppError::new(ErrorCode::GitFailed, &[("repo", &repo), ("what", &first)])
            .details(json!({"argv": e.argv, "stderr": e.stderr}))
            .with_hint(hint)
    }

    /// The error another engine process reported in its envelope, such as the `serve` that
    /// `open` starts, passed on as this command's.
    pub fn from_object(error: ErrorObject) -> AppError {
        AppError(Box::new(Failure {
            code: error.code,
            message: error.message,
            hint: error.hint,
            details: error.details,
        }))
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
            StoreError::NewerFile { file, found, supported } => {
                return AppError::newer_file(file, found, supported);
            }
            StoreError::IdempotencyMismatch { idem, .. } => AppError::new(e.code(), &[("idem", idem)]),
            StoreError::Busy => AppError::new(e.code(), &[]),
            // Only an engine older than the one that recorded the set is refused, and a person
            // clears it by updating: a release from the plugin, a prerelease by building it.
            StoreError::PacksMismatch { recorded, .. } => {
                let version = recorded.as_deref().and_then(|r| r.split('+').next()).unwrap_or_default();
                let hint =
                    if version.contains('-') { "hint.packs_mismatch_dev" } else { "hint.packs_mismatch" };
                AppError::new(e.code(), &[]).with_hint(text::get(hint, &[]))
            }
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
