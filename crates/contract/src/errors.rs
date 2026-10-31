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
    /// The keys among `details` that hold repo-derived free text, which `kernel::redact` masks as
    /// it masks a marked field; never a handle, an ID, a field name or a command.
    pub redact: &'static [&'static str],
}

macro_rules! error_codes {
    ($($variant:ident = $code:literal, $category:ident, $retry:ident, $meaning:literal, [$($detail:literal),*]
        $(redact [$($masked:literal),*])?;)*) => {
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
                        redact: &[$($($masked),*)?],
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
    InvalidArgument = "invalid_argument", Usage, Never, "bad flags or request", ["field", "synopsis", "schema", "next", "cycle", "lenses", "levels", "ghosts", "count", "card"];
    AmbiguousRef = "ambiguous_ref", Usage, Never, "an ID suffix or name matched several records", ["candidates", "level"];
    WorkspaceNotFound = "workspace_not_found", NotFound, Never,
        "no repos at the path, or a read found no workspace store", ["path"];
    NotFound = "not_found", NotFound, Never, "unknown ID, question or file", ["closest"];
    PersonRequired = "person_required", Permission, Never,
        "a person-only command without a valid person token: missing, stale, spent, or for another command",
        ["command", "reason", "card"];
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
    GitFailed = "git_failed", Failed, Never, "git exited non-zero; stderr redacted", ["argv", "stderr"]
        redact ["stderr"];
    BaseRequired = "base_required", Usage, Never,
        "a write to user-authored state without base; the hint names meta.seq", [];
    ActorRequired = "actor_required", Usage, Never,
        "a write with no PLANROOM_ACTOR, no person token and no terminal; the hint shows PLANROOM_ACTOR=agent:<id>:<role>",
        [];
    StaleWrite = "stale_write", Conflict, AfterReread,
        "a field changed since base; stored as a conflict, never merged",
        ["event", "base", "theirs", "yours", "card"] redact ["theirs", "yours"];
    AlreadyResolved = "already_resolved", Conflict, Never, "the card was already picked", ["by", "at", "pick"];
    HandoffConflict = "handoff_conflict", Conflict, Never,
        "the merge preflight failed; every repo untouched", ["repo", "files"] redact ["repo"];
    ConsentRequired = "consent_required", Permission, Never,
        "a model-only command before the person allowed this provider for the workspace; only the person's pick on the consent card clears it",
        ["provider", "card"];
    PathRefused = "path_refused", Permission, Never,
        "a read named a path outside the workspace's repos and scanned files, a non-person named a credential path, or a repo's git directory belongs to another user, or `intent new` would make a workspace in the home directory or at the filesystem root",
        ["path", "reason"];
    RepoConfigRefused = "repo_config_refused", Permission, Never,
        "a checkout or merge in a workspace repo whose local git config names a command git would run",
        ["repo", "keys", "card"] redact ["repo"];
    BudgetExceeded = "budget_exceeded", Budget, Later,
        "a spend or size guard stopped a model-only command; decisions keep their default instead",
        ["guard", "limit", "spent"];
    ModelUnavailable = "model_unavailable", Unavailable, Later,
        "a model-only command found Jev or Claude unreachable", ["provider"];
    GitTooOld = "git_too_old", Unavailable, Never,
        "the git on PATH is older than 2.34.1, the oldest whose plumbing the adapter uses", ["found", "minimum", "fix"];
    PacksMismatch = "packs_mismatch", Unavailable, Never,
        "this process resolved another pack set than the one the store records; a write is refused",
        ["recorded", "resolved"];
    ReplayMissing = "replay_missing", Unavailable, Never, "strict replay found no record", ["key", "path"];
    ModelReplyInvalid = "model_reply_invalid", Failed, Never,
        "a model's reply couldn't be used: unparsable, cut off, refused, or naming nothing new; nothing was written",
        ["purpose", "key", "reason", "field"];
    ServeLimit = "serve_limit", Unavailable, Later,
        "the local server hit one of its limits: a request still running at 10 s, or a 65th stream client",
        ["limit", "max"];
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
/// `count`: `partial_scan`'s uncovered paths, and `partial_parse`'s and `unparsed_files`' paths.
/// A reply an agent reads keeps to its leaf's budget, so a warning never lists every path of a
/// large repo.
pub const WARNING_SAMPLE: usize = 10;

/// A registered warning code, with its meaning and `details` fields.
pub struct WarningSpec {
    pub code: WarningCode,
    pub meaning: &'static str,
    /// The keys of the warning's `details` object.
    pub details: &'static [&'static str],
    /// The keys among `details` that hold repo-derived free text, which `kernel::redact` masks as
    /// it masks a marked field; never a handle, an ID, a field name or a command.
    pub redact: &'static [&'static str],
}

macro_rules! warning_codes {
    ($($variant:ident = $code:literal, $meaning:literal, [$($detail:literal),*]
        $(redact [$($masked:literal),*])?;)*) => {
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

            /// The code's registry entry.
            pub const fn spec(self) -> &'static WarningSpec {
                match self {
                    $(WarningCode::$variant => &WarningSpec {
                        code: WarningCode::$variant,
                        meaning: $meaning,
                        details: &[$($detail),*],
                        redact: &[$($($masked),*)?],
                    },)*
                }
            }

            pub const fn meaning(self) -> &'static str {
                self.spec().meaning
            }
        }
    };
}

warning_codes! {
    Truncated = "truncated", "the result hit its budget; the summary and first page are kept",
        ["kept", "total", "budget_tokens", "next", "left_out", "codes"];
    UnresolvedRef = "unresolved_ref",
        "an alias had zero or several candidates, or a linker join had two or more; no edge was made. A linker's warning comes from the scan that built the link: details give the count of join hints left unresolved and the first 10 by hint ID, each with its linker, hint, first 3 candidate IDs and candidate_count",
        ["count", "items"];
    EvidenceDropped = "evidence_dropped", "an evidence item failed verification at ingest", ["repo", "count", "items"]
        redact ["repo"];
    PartialParse = "partial_parse",
        "files parsed with error nodes, so their facts are weak; details name the repo, the count and the first 10 paths in sorted order",
        ["repo", "count", "paths"] redact ["repo", "paths"];
    UnparsedFiles = "unparsed_files",
        "files an extractor took no facts from because they pass a nesting or work bound, would hold more heap in facts and kept matches than their size allows, or come at or after the first file in sorted order whose facts would take the repo past its bound; details name the repo, the count, the bound (nesting_limit, work_limit or repo_limit) and the first 10 paths in sorted order",
        ["repo", "count", "bound", "paths"] redact ["repo", "paths"];
    PartialScan = "partial_scan",
        "an extractor reached its file or byte bound, or the git history its commit bound or a shallow clone's boundary, or a partial clone lacks some blobs; details name the extractor, the repo, the count of uncovered files and the first 10 in sorted order, or a `history before <sha>` for each cut",
        ["extractor", "repo", "count", "uncovered"] redact ["repo", "uncovered"];
    WorkspaceNested = "workspace_nested",
        "another workspace's .planroom/ sits inside this one, or this one inside it; details name the other root",
        ["other_root"];
    RepoUnborn = "repo_unborn",
        "a repo has no commit yet, so it maps nothing until its first commit; details name the repo and its branch",
        ["repo", "branch"] redact ["repo"];
    RepoSkipped = "repo_skipped",
        "git couldn't read a repo, or another user owns a place git uses for it, so the scan left it out and its last map stays; details name the repo, the reason (other_owner or git_failed) and the fix, null for other_owner, since Planroom never runs git in another user's repo",
        ["repo", "reason", "fix"] redact ["repo", "fix"];
    ScanSuperseded = "scan_superseded",
        "a repo's HEAD moved on while the scan read it, or the workspace's repos changed and a newer scan mapped it or named it otherwise meanwhile, so its map stays as it was and a scan never moves it back; details name the repo, the commit this scan read, the reason (head_moved or repos_changed) and the commit the map holds (mapped, empty when none)",
        ["repo", "commit", "reason", "mapped"] redact ["repo"];
    Claimed = "claimed", "the target is claimed by someone else", ["by", "until"];
    DeterministicFallback = "deterministic_fallback",
        "no model was reachable, allowed or within budget; deterministic tiers run, low kinds keep their default and cards pre-select it",
        [];
    WorkspaceUntrusted = "workspace_untrusted",
        "the state directory is untrusted, so its config wasn't loaded; details name the trust card",
        ["card"];
    TrustNotRecorded = "trust_not_recorded",
        "a person's pick of the trust card stands, but the workspace's trust couldn't be recorded, so the next scan deals a fresh card; details name the card",
        ["card"];
    PackNotUpdated = "pack_not_updated",
        "a person's accept of the changed-source card stands, but the pack's new copy couldn't be recorded, so the next scan deals a fresh card; details name the card and the pack",
        ["card", "pack"];
    PacksNotLoaded = "packs_not_loaded",
        "a pack the workspace records didn't load from its copy, or doesn't load with this engine, so the reply reads the built-in packs alone; details name the pack and the reason",
        ["pack", "reason"];
    ConfigKeyIgnored = "config_key_ignored",
        "a user-level key in a workspace's config, or api_key in a user-level config that isn't mode 0600",
        ["file", "key"];
    InferredUnconfirmed = "inferred_unconfirmed", "the result includes inferred facts no person confirmed", [];
    CacheRegression = "cache_regression", "an unchanged Claude prefix wrote the cache again inside its TTL", [];
    CacheNotApplied = "cache_not_applied",
        "a marked Claude prefix reported no cache write and no cache read, so it is below the model's minimum", [];
    HiddenCharacters = "hidden_characters",
        "a diff hunk holds bidi controls or zero-width characters, which surfaces show as markers", ["path", "lines"];
    OutsideTouchPoints = "outside_touch_points",
        "a hand-off's diff changes files outside the components its spec's touch points name", ["files"];
    ScanChangedUnderEdit = "scan_changed_under_edit", "a fact under a person's override changed; see Diffs", [];
    StalePage = "stale_page",
        "a page cursor's seq is behind, or its snapshot was pruned so the page folds at the current seq", [];
    ModelRequestPending = "model_request_pending", "the session backend is waiting for the mod", [];
    StoreMigrated = "store_migrated",
        "this command migrated the store; details name the versions and the VACUUM INTO copy", ["from", "to", "backup"];
    TemporaryHost = "temporary_host",
        "this host loses its files with its container, so a redacted copy of the backup is also kept at a local ref",
        ["repo", "ref"] redact ["repo"];
    BackupBehind = "backup_behind",
        "the event backup stops before the last person-authored event; the next person edit tries to catch it up",
        ["backup_seq", "file_seq", "last_authored"];
    BackupRefFailed = "backup_ref_failed",
        "the redacted backup couldn't be copied to its local ref; the next person edit tries again",
        ["repo", "ref"] redact ["repo"];
    ReconcilePaused = "reconcile_paused",
        "a repo's HEAD moved other than forward along its tracked branch, so the map follows it and the reconciler skipped it",
        ["repo", "head", "tracked_branch", "reason"] redact ["repo"];
    DutyFailed = "duty_failed",
        "a background duty failed and hasn't succeeded since; details name the duty, when, and its log under the data root",
        ["duty", "at", "log"];
    UnnamedPaths = "unnamed_paths",
        "files whose paths aren't UTF-8, or hold a newline or a backslash, which no node ID can name, so they aren't mapped; details name the repo, the count and the first 10 paths in sorted order, escaped",
        ["repo", "count", "paths"] redact ["repo", "paths"];
    UnsafeFilesystem = "unsafe_filesystem",
        "the store's place is on a filesystem that can't share a write-ahead log between processes (a network mount, WSL's /mnt, FUSE or a sync folder): a new store moved under the data root with a store.pointer left in the state directory, an existing one stayed, or the data root is on one too and the store stayed; details name the unsafe place, its filesystem and where the store is",
        ["path", "fs", "store"] redact ["path", "store"];
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
            assert_eq!(code.spec().code, *code);
        }
    }

    #[test]
    fn every_masked_details_key_is_one_of_its_codes_details() {
        for spec in ErrorCode::ALL.iter().map(|c| c.spec()) {
            assert!(spec.redact.iter().all(|k| spec.details.contains(k)), "{}", spec.code.as_str());
        }
        for spec in WarningCode::ALL.iter().map(|c| c.spec()) {
            assert!(spec.redact.iter().all(|k| spec.details.contains(k)), "{}", spec.code.as_str());
        }
        // A handle, an ID, a field name or a command is never masked.
        for key in ["next", "field", "closest", "candidates", "fix", "card", "argv"] {
            assert!(ErrorCode::ALL.iter().all(|c| !c.spec().redact.contains(&key)), "{key}");
            let masked = |c: &&WarningCode| c.spec().redact.contains(&key);
            // Save one: repo_skipped's fix is a line that names the repo's path, which is repo text,
            // so it's masked as the repo's name is.
            let allowed: &[WarningCode] = if key == "fix" { &[WarningCode::RepoSkipped] } else { &[] };
            assert!(WarningCode::ALL.iter().filter(masked).all(|c| allowed.contains(c)), "{key}");
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
