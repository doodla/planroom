//! The error and warning registries: the only place a code is spelled.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::open_set::{self, CODE_PATTERN};

/// An error's category; the exit code and the HTTP status follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Usage,
    NotFound,
    Conflict,
    Permission,
    Budget,
    Unavailable,
    Failed,
    Internal,
}

impl Category {
    /// The process exit code for an error in this category.
    pub const fn exit_code(self) -> i32 {
        match self {
            Category::Usage => 2,
            Category::NotFound => 3,
            Category::Conflict => 4,
            Category::Permission => 5,
            Category::Budget => 6,
            Category::Unavailable => 7,
            Category::Failed => 8,
            Category::Internal => 70,
        }
    }
}

/// When a caller may retry a failed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Retry {
    /// Retrying the same request can't succeed; a person or another command must act first.
    Never,
    /// The same request may succeed at once.
    Now,
    /// Re-read, then retry with the new `seq` as `base`.
    AfterReread,
    /// Wait, then retry; `retry_after_ms` says how long when known.
    Later,
}

/// A registered error code, with its category, retry class, meaning and `details` fields.
pub struct ErrorSpec {
    pub code: ErrorCode,
    pub category: Category,
    pub retry: Retry,
    pub meaning: &'static str,
    /// The keys of the error's `details` object.
    pub details: &'static [&'static str],
}

macro_rules! error_codes {
    ($($variant:ident = $code:literal, $category:ident, $retry:ident, $meaning:literal, [$($detail:literal),*];)*) => {
        /// A stable error code. Older readers map an unknown code to its category.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ErrorCode {
            $($variant,)*
        }

        impl ErrorCode {
            /// Every code, in registry order.
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$variant,)*];

            /// The code's registry entry.
            pub const fn spec(self) -> &'static ErrorSpec {
                match self {
                    $(ErrorCode::$variant => &ErrorSpec {
                        code: ErrorCode::$variant,
                        category: Category::$category,
                        retry: Retry::$retry,
                        meaning: $meaning,
                        details: &[$($detail),*],
                    },)*
                }
            }

            /// The code as it appears on the wire.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(ErrorCode::$variant => $code,)*
                }
            }
        }
    };
}

error_codes! {
    InvalidArgument = "invalid_argument", Usage, Never, "bad flags or request", ["field", "synopsis", "schema"];
    AmbiguousRef = "ambiguous_ref", Usage, Never, "an ID suffix or name matched several records", ["candidates"];
    WorkspaceNotFound = "workspace_not_found", NotFound, Never,
        "no repos at the path, or a read found no workspace store", ["path"];
    NotFound = "not_found", NotFound, Never, "unknown ID, question or file", ["closest"];
    PersonRequired = "person_required", Permission, Never,
        "a person-only command without a valid person token: missing, stale, spent, or for another command",
        ["command", "reason"];
    Unauthorized = "unauthorized", Permission, Never,
        "HTTP: no bearer token, a missing or foreign Origin or Host, or a used or expired login code", [];
    PersonKeyMismatch = "person_key_mismatch", Permission, Never,
        "person init found another key registered; the pane's Use this session's key button runs person init --rekey",
        ["registered_at", "surface"];
    IdempotencyMismatch = "idempotency_mismatch", Conflict, Never,
        "the same idem with a different request", ["idem", "stored_event"];
    StoreBusy = "store_busy", Unavailable, Now, "still locked after the 5 s busy timeout", [];
    PortInUse = "port_in_use", Unavailable, Never,
        "another program holds the workspace's recorded canvas port; serve never moves unasked",
        ["port", "fix"];
    EngineFetching = "engine_fetching", Unavailable, Later,
        "the resolver's envelope to a caller other than the mod: a prefetch held the fetch lock for 20 s",
        ["progress"];
    EngineUnavailable = "engine_unavailable", Unavailable, Never,
        "the resolver's envelope when it can't hand back an engine; details.reason says why and fix names the command",
        ["reason", "fix", "est_minutes", "canvas", "missing"];
    SchemaMismatch = "schema_mismatch", Unavailable, Never,
        "the store or a file is from a newer version, or this process is older than the store",
        ["found", "supported", "writer_version", "file"];
    PackInvalid = "pack_invalid", Failed, Never, "a pack failed validation", ["file", "line", "reference"];
    GitFailed = "git_failed", Failed, Never, "git exited non-zero; stderr redacted", ["argv", "stderr"];
    Internal = "internal", Internal, Never, "a bug", ["report"];
}

impl ErrorCode {
    pub const fn category(self) -> Category {
        self.spec().category
    }

    pub const fn exit_code(self) -> i32 {
        self.category().exit_code()
    }

    pub const fn retry(self) -> Retry {
        self.spec().retry
    }
}

impl JsonSchema for ErrorCode {
    fn schema_name() -> Cow<'static, str> {
        "ErrorCode".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        open_set::schema(
            "A stable error code. Older readers map an unknown code to its category.",
            ErrorCode::ALL.iter().map(|c| c.as_str()),
            CODE_PATTERN,
        )
    }
}

/// How many items a warning's `details` list at most, in their sorted order, beside their
/// `count`: `partial_scan`'s uncovered paths and `partial_parse`'s paths. A reply an agent reads
/// keeps to its leaf's budget, so a warning never lists every path of a large repo.
pub const WARNING_SAMPLE: usize = 10;

macro_rules! warning_codes {
    ($($variant:ident = $code:literal, $meaning:literal;)*) => {
        /// A stable warning code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum WarningCode {
            $($variant,)*
        }

        impl WarningCode {
            /// Every code, in registry order.
            pub const ALL: &'static [WarningCode] = &[$(WarningCode::$variant,)*];

            /// The code as it appears on the wire.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(WarningCode::$variant => $code,)*
                }
            }

            pub const fn meaning(self) -> &'static str {
                match self {
                    $(WarningCode::$variant => $meaning,)*
                }
            }
        }
    };
}

warning_codes! {
    Truncated = "truncated", "the result hit its budget; the summary and first page are kept";
    UnresolvedRef = "unresolved_ref", "an alias or a linker join had zero or several candidates; no edge was made";
    EvidenceDropped = "evidence_dropped", "an evidence item failed verification at ingest";
    PartialParse = "partial_parse",
        "files parsed with error nodes, so their facts are weak; details name the repo, the count and the first 10 paths in sorted order";
    PartialScan = "partial_scan",
        "an extractor reached its file or byte bound, or a partial clone lacks some blobs; details name the extractor, the repo, the count of uncovered files and the first 10 in sorted order";
    WorkspaceNested = "workspace_nested",
        "another workspace's .planroom/ sits inside this one, or this one inside it; details name the other root";
    RepoUnborn = "repo_unborn",
        "a repo has no commit yet, so it maps nothing until its first commit; details name the repo and its branch";
    RepoSkipped = "repo_skipped",
        "git couldn't read a repo, so the scan left it out and its last map stays; details name the repo, the reason (other_owner or git_failed) and the fix";
    ScanSuperseded = "scan_superseded",
        "a repo's HEAD moved on while the scan read it, so its map stays as it was until a scan reads the new HEAD; details name the repo and the commit this scan read";
}

impl JsonSchema for WarningCode {
    fn schema_name() -> Cow<'static, str> {
        "WarningCode".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        open_set::schema("A stable warning code.", WarningCode::ALL.iter().map(|c| c.as_str()), CODE_PATTERN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_code_serializes_as_its_registry_string() {
        for code in ErrorCode::ALL {
            assert_eq!(serde_json::to_value(code).unwrap(), code.as_str());
            assert_eq!(code.spec().code, *code);
        }
    }

    #[test]
    fn every_warning_code_serializes_as_its_registry_string() {
        for code in WarningCode::ALL {
            assert_eq!(serde_json::to_value(code).unwrap(), code.as_str());
        }
    }

    #[test]
    fn exit_codes_follow_the_category() {
        assert_eq!(ErrorCode::InvalidArgument.exit_code(), 2);
        assert_eq!(ErrorCode::NotFound.exit_code(), 3);
        assert_eq!(ErrorCode::StoreBusy.exit_code(), 7);
        assert_eq!(ErrorCode::PackInvalid.exit_code(), 8);
        assert_eq!(ErrorCode::Internal.exit_code(), 70);
    }
}
