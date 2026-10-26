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

/// A registered error code, with its category, retryability, meaning and `details` fields.
pub struct ErrorSpec {
    pub code: ErrorCode,
    pub category: Category,
    pub retryable: bool,
    pub meaning: &'static str,
    /// The keys of the error's `details` object.
    pub details: &'static [&'static str],
}

macro_rules! error_codes {
    ($($variant:ident = $code:literal, $category:ident, $retryable:literal, $meaning:literal, [$($detail:literal),*];)*) => {
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
                        retryable: $retryable,
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
    InvalidArgument = "invalid_argument", Usage, false, "bad flags or request", ["field"];
    AmbiguousRef = "ambiguous_ref", Usage, false, "an ID suffix or name matched several records", ["candidates"];
    WorkspaceNotFound = "workspace_not_found", NotFound, false, "no repos at the path", ["path"];
    NotFound = "not_found", NotFound, false, "unknown ID, question or file", ["closest"];
    StoreBusy = "store_busy", Unavailable, true, "still locked after the 5 s busy timeout", [];
    SchemaMismatch = "schema_mismatch", Unavailable, false,
        "the store or a file is from a newer version, or this process is older than the store",
        ["found", "supported"];
    PackInvalid = "pack_invalid", Failed, false, "a pack failed validation", ["file", "line", "reference"];
    GitFailed = "git_failed", Failed, false, "git exited non-zero; stderr redacted", ["argv", "stderr"];
    Internal = "internal", Internal, false, "a bug", ["report"];
}

impl ErrorCode {
    pub const fn category(self) -> Category {
        self.spec().category
    }

    pub const fn exit_code(self) -> i32 {
        self.category().exit_code()
    }

    pub const fn retryable(self) -> bool {
        self.spec().retryable
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
    PartialParse = "partial_parse", "a file parsed with error nodes; its facts are weak";
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
