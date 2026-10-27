//! Store errors, each mapped to one registered `ErrorCode`.

use std::fmt;
use std::path::PathBuf;

use planroom_contract::{ErrorCode, Seq};
use serde_json::{Value, json};

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// Still locked after the busy timeout.
    Busy,
    /// The store is newer than this build.
    SchemaMismatch { found: u32, supported: u32, writer_version: Option<String> },
    /// A file beside the store, such as `store.pointer`, is in a newer format than this build
    /// reads: `schema_mismatch` naming the file.
    NewerFile { file: PathBuf, found: String, supported: String },
    /// The same idem with a different request hash.
    IdempotencyMismatch { idem: String, stored_event: String, stored_seq: Seq },
    /// A file the store needs couldn't be created or read.
    Io(String),
    /// Any other SQLite failure, or a stored row that doesn't parse.
    Sqlite(String),
}

impl StoreError {
    /// The registered code a caller reports.
    pub fn code(&self) -> ErrorCode {
        match self {
            StoreError::Busy => ErrorCode::StoreBusy,
            StoreError::SchemaMismatch { .. } | StoreError::NewerFile { .. } => ErrorCode::SchemaMismatch,
            StoreError::IdempotencyMismatch { .. } => ErrorCode::IdempotencyMismatch,
            StoreError::Io(_) | StoreError::Sqlite(_) => ErrorCode::Internal,
        }
    }

    /// The code's `details`, as the registry names them.
    pub fn details(&self) -> Value {
        match self {
            StoreError::Busy => json!({}),
            StoreError::SchemaMismatch { found, supported, writer_version } => {
                json!({"found": found, "supported": supported, "writer_version": writer_version})
            }
            StoreError::NewerFile { file, found, supported } => json!({
                "found": found, "supported": supported, "writer_version": null, "file": file.display().to_string()
            }),
            StoreError::IdempotencyMismatch { idem, stored_event, .. } => {
                json!({"idem": idem, "stored_event": stored_event})
            }
            StoreError::Io(_) | StoreError::Sqlite(_) => json!({}),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Busy => f.write_str("the store is still locked after the busy timeout"),
            StoreError::SchemaMismatch { found, supported, .. } => {
                write!(f, "the store is version {found}; this build reads up to {supported}")
            }
            StoreError::NewerFile { file, found, supported } => {
                write!(f, "{} is {found}; this build reads up to {supported}", file.display())
            }
            StoreError::IdempotencyMismatch { idem, .. } => {
                write!(f, "idem {idem} was used for another request")
            }
            StoreError::Io(e) | StoreError::Sqlite(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::SqliteFailure(inner, _)
                if matches!(
                    inner.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
            {
                StoreError::Busy
            }
            _ => StoreError::Sqlite(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Sqlite(format!("a stored row doesn't parse: {e}"))
    }
}
