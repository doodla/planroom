//! The command catalogue: one `Command` variant per leaf, each with its `CommandSpec`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::{CommandName, Cursor, Empty, Handle};
use crate::errors::ErrorCode;
use crate::model::{LevelId, NodeId, Seq};
use crate::plan::{IntentListReq, IntentNewReq, IntentVersionReq};
use crate::requests::{AskReq, EditReq, FindReq, IntentNextReq, LintReq, ProposeReq, ReviewReq};
use crate::view::MapView;

/// Which of the five ideas (plus setup) a command belongs to; help is grouped by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Idea {
    Levels,
    Lenses,
    Intent,
    Diffs,
    Decisions,
    Setup,
}

/// Who may run a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorRule {
    Any,
    PersonOnly,
    /// Some inputs need a person and others don't; the leaf table names which.
    Mixed,
}

/// A command's permission class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Read,
    Change,
    Handoff,
}

/// How a command's effect is undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Undo {
    Edit,
    Compensate,
    NotUndoable(&'static str),
    /// A read: nothing to undo.
    Query,
}

/// The mod tool a command is reachable through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    RoomQuery,
    RoomChange,
    RoomHandoff,
}

/// Everything the surfaces need to know about one command leaf. Two specs are equal when their
/// paths are: a path names one leaf, and the `args` function pointer has no meaningful equality.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Such as `["handoff", "merge"]`.
    pub path: &'static [&'static str],
    pub idea: Idea,
    /// One line: help, tool description, docs.
    pub summary: &'static str,
    pub actor: ActorRule,
    pub class: Class,
    /// The default output budget.
    pub budget_tokens: u32,
    /// Writes to user-authored state, so `base` is required.
    pub needs_base: bool,
    /// Can make a paid model call, such as `intent next`, `decide --run` or `curate`. No band
    /// Button, `PaneModel.next` or pack suggestion names a leaf that spends (DR-66).
    pub spends: bool,
    pub errors: &'static [ErrorCode],
    pub undo: Undo,
    pub tool: Option<Tool>,
    /// Reachable as `/room ...`.
    pub room: bool,
    /// The request's clap arguments, `<XReq as clap::Args>::augment_args`: the one source of the
    /// CLI's parser, the leaf's synopsis and its flags, so the CLI's usage, the tool descriptions
    /// and `/room`'s typeahead never disagree.
    pub args: fn(clap::Command) -> clap::Command,
}

/// One long flag a leaf takes, as `CommandSpec::flags` lists it for `/room`'s typeahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafFlag {
    /// The subcommand the flag belongs to, such as `relate`; `None` for the leaf's own.
    pub sub: Option<String>,
    /// Such as `--level`.
    pub flag: String,
    /// The value's name, such as `LENS`; `None` for a flag that takes no value.
    pub value_name: Option<String>,
    /// The values the flag accepts, when they are a fixed set.
    pub values: Vec<String>,
}

impl PartialEq for CommandSpec {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for CommandSpec {}

impl CommandSpec {
    /// The command name: the path joined with spaces.
    pub fn name(&self) -> String {
        self.path.join(" ")
    }

    /// The leaf's clap command: its request's arguments under the last word of its path.
    pub fn command(&self) -> clap::Command {
        (self.args)(clap::Command::new(self.path.last().copied().unwrap_or_default()))
    }

    /// The leaf's one-line synopsis from its clap arguments, long flags only, such as
    /// `planroom map [--level LEVEL] [--lens LENS] ... [--json]`.
    /// A leaf whose request has subcommands names them after its path:
    /// `planroom edit move|group|...|undo [--base BASE] ... [--json]`.
    pub fn synopsis(&self) -> String {
        let cmd = self.command();
        let mut words = vec!["planroom".to_owned(), self.name()];
        let subs: Vec<&str> = cmd.get_subcommands().map(clap::Command::get_name).collect();
        if !subs.is_empty() {
            words.push(subs.join("|"));
        }
        words.extend(usage_words(&cmd));
        words.push("[--json]".into());
        words.join(" ")
    }

    /// The leaf's long flags, its own first, then each subcommand's in order; positionals are
    /// left out.
    pub fn flags(&self) -> Vec<LeafFlag> {
        let cmd = self.command();
        let mut flags = long_flags(&cmd, None);
        for sub in cmd.get_subcommands() {
            flags.extend(long_flags(sub, Some(sub.get_name())));
        }
        flags
    }
}

/// Whether an argument is left out of a synopsis: clap's own and the global `--json`.
fn unlisted(arg: &clap::Arg) -> bool {
    matches!(arg.get_id().as_str(), "help" | "version" | "json") || arg.is_hide_set()
}

/// An argument's value as a synopsis shows it: its fixed values joined with `|`, else its value
/// name, else its ID in capitals.
fn value_word(arg: &clap::Arg) -> String {
    let values: Vec<String> = arg.get_possible_values().iter().map(|v| v.get_name().to_owned()).collect();
    if values.is_empty() {
        arg.get_value_names()
            .and_then(|names| names.first())
            .map_or_else(|| arg.get_id().as_str().to_uppercase(), |n| n.as_str().to_owned())
    } else {
        values.join("|")
    }
}

/// The words a command's arguments add to its synopsis, in order: `--flag VALUE`, `--flag` or a
/// positional's value, each in brackets unless it is required.
pub fn usage_words(cmd: &clap::Command) -> Vec<String> {
    cmd.get_arguments()
        .filter(|arg| !unlisted(arg))
        .map(|arg| {
            let word = match arg.get_long() {
                Some(long) if arg.get_action().takes_values() => format!("--{long} {}", value_word(arg)),
                Some(long) => format!("--{long}"),
                None => value_word(arg),
            };
            if arg.is_required_set() { word } else { format!("[{word}]") }
        })
        .collect()
}

fn long_flags(cmd: &clap::Command, sub: Option<&str>) -> Vec<LeafFlag> {
    cmd.get_arguments()
        .filter(|arg| !unlisted(arg))
        .filter_map(|arg| {
            let long = arg.get_long()?;
            let takes = arg.get_action().takes_values();
            let values = if takes {
                arg.get_possible_values().iter().map(|v| v.get_name().to_owned()).collect()
            } else {
                vec![]
            };
            let value_name = takes.then(|| {
                arg.get_value_names()
                    .and_then(|names| names.first())
                    .map_or_else(|| arg.get_id().as_str().to_uppercase(), |n| n.as_str().to_owned())
            });
            Some(LeafFlag { sub: sub.map(str::to_owned), flag: format!("--{long}"), value_name, values })
        })
        .collect()
}

/// One variant per catalogue leaf; the request travels as `args`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", content = "args", rename_all = "snake_case")]
pub enum Command {
    Map(MapReq),
    Find(FindReq),
    Ask(AskReq),
    Focus(FocusReq),
    Scan(ScanReq),
    Watch(WatchReq),
    Schema(SchemaReq),
    Version(VersionReq),
    PersonInit(PersonInitReq),
    Serve(ServeReq),
    Share(ShareReq),
    Import(ImportReq),
    Doctor(DoctorReq),
    Edit(EditReq),
    Open(OpenReq),
    Review(ReviewReq),
    IntentNew(IntentNewReq),
    IntentNext(IntentNextReq),
    IntentPropose(ProposeReq),
    IntentLint(LintReq),
    IntentVersion(IntentVersionReq),
    IntentList(IntentListReq),
    PackAdd(PackAddReq),
    PackRemove(PackRemoveReq),
}

const MAP: CommandSpec = CommandSpec {
    path: &["map"],
    idea: Idea::Levels,
    summary: "The map at a level through a lens.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::NotFound,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomQuery),
    room: true,
    args: <MapReq as clap::Args>::augment_args,
};

const FIND: CommandSpec = CommandSpec {
    path: &["find"],
    idea: Idea::Levels,
    summary: "Search the map's blocks by name or ID, best match first.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 600,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomQuery),
    room: true,
    args: <FindReq as clap::Args>::augment_args,
};

const ASK: CommandSpec = CommandSpec {
    path: &["ask"],
    idea: Idea::Lenses,
    summary: "A question the loaded packs answer; alone, every loaded kind, lens and question.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::NotFound,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PathRefused,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomQuery),
    room: true,
    args: <AskReq as clap::Args>::augment_args,
};

const FOCUS: CommandSpec = CommandSpec {
    path: &["focus"],
    idea: Idea::Levels,
    summary: "Where you are, the one open decision and the next action.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_000,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::AmbiguousRef,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomQuery),
    room: true,
    args: <FocusReq as clap::Args>::augment_args,
};

const SCAN: CommandSpec = CommandSpec {
    path: &["scan"],
    idea: Idea::Setup,
    summary: "Discover, extract and link the workspace's repos.",
    actor: ActorRule::Mixed,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PathRefused,
        ErrorCode::PackInvalid,
        ErrorCode::GitFailed,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <ScanReq as clap::Args>::augment_args,
};

const WATCH: CommandSpec = CommandSpec {
    path: &["watch"],
    idea: Idea::Setup,
    summary: "Stream the workspace's committed events as NDJSON frames.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 0,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <WatchReq as clap::Args>::augment_args,
};

const SCHEMA: CommandSpec = CommandSpec {
    path: &["schema"],
    idea: Idea::Setup,
    summary: "The contract schema's summary, or the command catalogue with --commands.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[ErrorCode::InvalidArgument, ErrorCode::Internal],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <SchemaReq as clap::Args>::augment_args,
};

const VERSION: CommandSpec = CommandSpec {
    path: &["version"],
    idea: Idea::Setup,
    summary: "This engine's version, target, schema version and catalogue hash.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[ErrorCode::Internal],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <VersionReq as clap::Args>::augment_args,
};

const PERSON_INIT: CommandSpec = CommandSpec {
    path: &["person", "init"],
    idea: Idea::Setup,
    summary: "Register this workspace's person key, read from stdin; --rekey replaces it.",
    actor: ActorRule::PersonOnly,
    class: Class::Change,
    budget_tokens: 400,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PersonRequired,
        ErrorCode::PersonKeyMismatch,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <PersonInitReq as clap::Args>::augment_args,
};

const SERVE: CommandSpec = CommandSpec {
    path: &["serve"],
    idea: Idea::Setup,
    summary: "Serve the canvas and the HTTP API on this workspace's port.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 0,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PortInUse,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::ServeLimit,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <ServeReq as clap::Args>::augment_args,
};

const SHARE: CommandSpec = CommandSpec {
    path: &["share"],
    idea: Idea::Levels,
    summary: "Write the canvas, its snapshot and the first live head for an Artifact publish.",
    actor: ActorRule::Any,
    class: Class::Handoff,
    budget_tokens: 800,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::NotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::NotUndoable("a viewer may already have seen what was published"),
    tool: Some(Tool::RoomHandoff),
    room: true,
    args: <ShareReq as clap::Args>::augment_args,
};

const IMPORT: CommandSpec = CommandSpec {
    path: &["import"],
    idea: Idea::Setup,
    summary: "Import Artifact inbox rows from stdin, one {path, row} per line; answers read requests.",
    actor: ActorRule::Mixed,
    class: Class::Change,
    budget_tokens: 800,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PersonRequired,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: None,
    room: false,
    args: <ImportReq as clap::Args>::augment_args,
};

const DOCTOR: CommandSpec = CommandSpec {
    path: &["doctor"],
    idea: Idea::Setup,
    summary: "With no flag, report git, the store, its migrations, the pack set and the event backup; with --pack DIR, check a pack.",
    actor: ActorRule::Mixed,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::PackInvalid,
        ErrorCode::GitTooOld,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: false,
    args: <DoctorReq as clap::Args>::augment_args,
};

const EDIT: CommandSpec = CommandSpec {
    path: &["edit"],
    idea: Idea::Intent,
    summary: "One hand edit: move, group, rename, hide, pin, relate, unrelate or forbid, or undo a command.",
    actor: ActorRule::Any,
    class: Class::Change,
    budget_tokens: 400,
    needs_base: true,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::BaseRequired,
        ErrorCode::ActorRequired,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::NotFound,
        ErrorCode::StaleWrite,
        ErrorCode::IdempotencyMismatch,
        ErrorCode::PersonRequired,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <EditReq as clap::Args>::augment_args,
};

const OPEN: CommandSpec = CommandSpec {
    path: &["open"],
    idea: Idea::Levels,
    summary: "Open the canvas in your browser, signed in with a one-time code it never prints.",
    actor: ActorRule::PersonOnly,
    class: Class::Read,
    budget_tokens: 300,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PersonRequired,
        ErrorCode::PortInUse,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: true,
    args: <OpenReq as clap::Args>::augment_args,
};

const REVIEW: CommandSpec = CommandSpec {
    path: &["review"],
    idea: Idea::Decisions,
    summary: "Accept or reject one ghost or conflict, or pick a card a write dealt.",
    actor: ActorRule::Mixed,
    class: Class::Change,
    budget_tokens: 400,
    needs_base: true,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::BaseRequired,
        ErrorCode::ActorRequired,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::NotFound,
        ErrorCode::StaleWrite,
        ErrorCode::AlreadyResolved,
        ErrorCode::IdempotencyMismatch,
        ErrorCode::PersonRequired,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: None,
    room: false,
    args: <ReviewReq as clap::Args>::augment_args,
};

/// The errors `intent new` may return; `intent propose` and `intent version` add theirs.
const INTENT_NEW_ERRORS: &[ErrorCode] = &[
    ErrorCode::InvalidArgument,
    ErrorCode::ActorRequired,
    ErrorCode::PersonRequired,
    ErrorCode::WorkspaceNotFound,
    ErrorCode::IdempotencyMismatch,
    ErrorCode::PacksMismatch,
    ErrorCode::StoreBusy,
    ErrorCode::SchemaMismatch,
    ErrorCode::Internal,
];

/// The errors `intent lint` and `intent list` may return.
const INTENT_READ_ERRORS: &[ErrorCode] = &[
    ErrorCode::InvalidArgument,
    ErrorCode::AmbiguousRef,
    ErrorCode::NotFound,
    ErrorCode::WorkspaceNotFound,
    ErrorCode::StoreBusy,
    ErrorCode::SchemaMismatch,
    ErrorCode::Internal,
];

const INTENT_NEW: CommandSpec = CommandSpec {
    path: &["intent", "new"],
    idea: Idea::Intent,
    summary: "Start a plan: a new project named by its text.",
    actor: ActorRule::Any,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: INTENT_NEW_ERRORS,
    undo: Undo::Edit,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <IntentNewReq as clap::Args>::augment_args,
};

/// `intent next`: the engine asks Claude to draft a stage, and the reply's items wait as the
/// model's ghosts, whoever runs it. It mints a fresh key on each run, so a repeat drafts again.
const INTENT_NEXT: CommandSpec = CommandSpec {
    path: &["intent", "next"],
    idea: Idea::Intent,
    summary: "Draft a plan stage's items with Claude; they wait as ghosts for a person.",
    actor: ActorRule::Any,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: false,
    spends: true,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::NotFound,
        ErrorCode::ActorRequired,
        ErrorCode::PersonRequired,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::IdempotencyMismatch,
        ErrorCode::StaleWrite,
        ErrorCode::PacksMismatch,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::BudgetExceeded,
        ErrorCode::ModelUnavailable,
        ErrorCode::ReplayMissing,
        ErrorCode::ModelReplyInvalid,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <IntentNextReq as clap::Args>::augment_args,
};

const INTENT_PROPOSE: CommandSpec = CommandSpec {
    path: &["intent", "propose"],
    idea: Idea::Intent,
    summary: "Propose plan items from a file or stdin; an agent's wait as ghosts for a person.",
    actor: ActorRule::Any,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: true,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::AmbiguousRef,
        ErrorCode::BaseRequired,
        ErrorCode::ActorRequired,
        ErrorCode::PersonRequired,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::NotFound,
        ErrorCode::PathRefused,
        ErrorCode::StaleWrite,
        ErrorCode::IdempotencyMismatch,
        ErrorCode::PacksMismatch,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <ProposeReq as clap::Args>::augment_args,
};

const INTENT_LINT: CommandSpec = CommandSpec {
    path: &["intent", "lint"],
    idea: Idea::Intent,
    summary: "The plan's lint findings: what each stage still lacks.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: INTENT_READ_ERRORS,
    undo: Undo::Query,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <LintReq as clap::Args>::augment_args,
};

const INTENT_VERSION: CommandSpec = CommandSpec {
    path: &["intent", "version"],
    idea: Idea::Intent,
    summary: "Tag the plan as it stands with a version label; a person's alone.",
    actor: ActorRule::PersonOnly,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::AmbiguousRef,
        ErrorCode::ActorRequired,
        ErrorCode::PersonRequired,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::IdempotencyMismatch,
        ErrorCode::PacksMismatch,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Edit,
    tool: None,
    room: true,
    args: <IntentVersionReq as clap::Args>::augment_args,
};

const INTENT_LIST: CommandSpec = CommandSpec {
    path: &["intent", "list"],
    idea: Idea::Intent,
    summary: "Every project, and the items of the one named, each ghost with its review.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    spends: false,
    errors: INTENT_READ_ERRORS,
    undo: Undo::Query,
    tool: Some(Tool::RoomChange),
    room: true,
    args: <IntentListReq as clap::Args>::augment_args,
};

/// The errors `pack add` and `pack remove` may return.
const PACK_SET_ERRORS: &[ErrorCode] = &[
    ErrorCode::InvalidArgument,
    ErrorCode::WorkspaceNotFound,
    ErrorCode::PersonRequired,
    ErrorCode::NotFound,
    ErrorCode::PackInvalid,
    ErrorCode::PacksMismatch,
    ErrorCode::StoreBusy,
    ErrorCode::SchemaMismatch,
    ErrorCode::Internal,
];

const PACK_ADD: CommandSpec = CommandSpec {
    path: &["pack", "add"],
    idea: Idea::Setup,
    summary: "Add an out-of-tree pack: validate it, copy it into the state directory and record it; a person's alone.",
    actor: ActorRule::PersonOnly,
    class: Class::Change,
    budget_tokens: 400,
    needs_base: false,
    spends: false,
    errors: PACK_SET_ERRORS,
    undo: Undo::Compensate,
    tool: None,
    room: true,
    args: <PackAddReq as clap::Args>::augment_args,
};

const PACK_REMOVE: CommandSpec = CommandSpec {
    path: &["pack", "remove"],
    idea: Idea::Setup,
    summary: "Remove an out-of-tree pack from the workspace's pack set; a person's alone.",
    actor: ActorRule::PersonOnly,
    class: Class::Change,
    budget_tokens: 400,
    needs_base: false,
    spends: false,
    errors: PACK_SET_ERRORS,
    undo: Undo::Compensate,
    tool: None,
    room: true,
    args: <PackRemoveReq as clap::Args>::augment_args,
};

impl Command {
    /// Every leaf's spec, in catalogue order.
    pub const SPECS: &'static [CommandSpec] = &[
        MAP,
        FIND,
        ASK,
        FOCUS,
        SCAN,
        WATCH,
        SCHEMA,
        VERSION,
        PERSON_INIT,
        SERVE,
        SHARE,
        IMPORT,
        DOCTOR,
        EDIT,
        OPEN,
        REVIEW,
        INTENT_NEW,
        INTENT_NEXT,
        INTENT_PROPOSE,
        INTENT_LINT,
        INTENT_VERSION,
        INTENT_LIST,
        PACK_ADD,
        PACK_REMOVE,
    ];

    pub const fn spec(&self) -> &'static CommandSpec {
        match self {
            Command::Map(_) => &MAP,
            Command::Find(_) => &FIND,
            Command::Ask(_) => &ASK,
            Command::Focus(_) => &FOCUS,
            Command::Scan(_) => &SCAN,
            Command::Watch(_) => &WATCH,
            Command::Schema(_) => &SCHEMA,
            Command::Version(_) => &VERSION,
            Command::PersonInit(_) => &PERSON_INIT,
            Command::Serve(_) => &SERVE,
            Command::Share(_) => &SHARE,
            Command::Import(_) => &IMPORT,
            Command::Doctor(_) => &DOCTOR,
            Command::Edit(_) => &EDIT,
            Command::Open(_) => &OPEN,
            Command::Review(_) => &REVIEW,
            Command::IntentNew(_) => &INTENT_NEW,
            Command::IntentNext(_) => &INTENT_NEXT,
            Command::IntentPropose(_) => &INTENT_PROPOSE,
            Command::IntentLint(_) => &INTENT_LINT,
            Command::IntentVersion(_) => &INTENT_VERSION,
            Command::IntentList(_) => &INTENT_LIST,
            Command::PackAdd(_) => &PACK_ADD,
            Command::PackRemove(_) => &PACK_REMOVE,
        }
    }

    pub fn name(&self) -> CommandName {
        CommandName(self.spec().name())
    }

    /// The catalogue as `schema --commands` lists it, in catalogue order. The catalogue hash is
    /// the sha256 of this list's compact JSON.
    pub fn listing() -> Vec<LeafListing> {
        Command::SPECS.iter().map(LeafListing::of).collect()
    }
}

/// One leaf as `schema --commands` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeafListing {
    /// The command name, its path joined with spaces.
    pub name: String,
    /// `levels`, `lenses`, `intent`, `diffs`, `decisions` or `setup`.
    pub idea: String,
    /// `any` or `person_only`.
    pub actor: String,
    /// `read`, `change` or `handoff`.
    pub class: String,
    /// The default output budget; 0 for a stream.
    pub budget_tokens: u32,
    /// `edit`, `compensate`, `query` or `not_undoable`.
    pub undo: String,
    /// The `schema --command LEAF` handle for the leaf's types.
    pub schema: Handle,
    /// The leaf's one-line synopsis, from its clap arguments: what its `invalid_argument` and its
    /// tool description show, so the catalogue hash covers its flags.
    #[serde(default)]
    pub synopsis: String,
}

impl LeafListing {
    fn of(spec: &CommandSpec) -> LeafListing {
        let idea = match spec.idea {
            Idea::Levels => "levels",
            Idea::Lenses => "lenses",
            Idea::Intent => "intent",
            Idea::Diffs => "diffs",
            Idea::Decisions => "decisions",
            Idea::Setup => "setup",
        };
        let actor = match spec.actor {
            ActorRule::Any => "any",
            ActorRule::PersonOnly => "person_only",
            ActorRule::Mixed => "mixed",
        };
        let class = match spec.class {
            Class::Read => "read",
            Class::Change => "change",
            Class::Handoff => "handoff",
        };
        let undo = match spec.undo {
            Undo::Edit => "edit",
            Undo::Compensate => "compensate",
            Undo::Query => "query",
            Undo::NotUndoable(_) => "not_undoable",
        };
        LeafListing {
            name: spec.name(),
            idea: idea.into(),
            actor: actor.into(),
            class: class.into(),
            budget_tokens: spec.budget_tokens,
            undo: undo.into(),
            schema: Handle {
                command: CommandName("schema".into()),
                args: vec!["--command".into(), spec.name()],
                label: spec.summary.into(),
            },
            synopsis: spec.synopsis(),
        }
    }
}

/// Flattened into every write request. Its flags are global, so a leaf with subcommands, such as
/// `edit`, reads them after its subcommand as well as before it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct WriteCommon {
    /// The `seq` of the view you acted on; required when the command writes user-authored state.
    #[arg(long, global = true)]
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub base: Option<Seq>,
    /// The idempotency key; derived from the request when absent.
    #[arg(long, global = true)]
    pub idem: Option<String>,
    /// The project the write belongs to.
    #[arg(long, global = true)]
    pub project: Option<NodeId>,
    /// The client ID of the writer instance, such as a canvas tab. A flush of a tab's pending
    /// edits sends the ID of the tab that made them, never the flusher's (DR-73); absent, it is an
    /// agent's own ID from `PLANROOM_ACTOR`, or `cli:<pid>` for a person at a terminal.
    #[arg(long, global = true)]
    #[serde(default)]
    pub client: Option<String>,
}

/// How much `map` returns.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    /// Counts, top nodes and lifted edges (for agents).
    #[default]
    Summary,
    /// Every node and edge with positions and evidence (for the canvas).
    Graph,
}

/// `planroom map [--level L] [--lens X] [--focus ID] [--detail summary|graph] [--cursor C]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct MapReq {
    /// The level to show, an ID `ask` lists; the top level when absent.
    #[arg(long, value_name = "LEVEL", value_parser = <LevelId as std::str::FromStr>::from_str)]
    pub level: Option<LevelId>,
    /// The lens; `all` lifts every relation kind.
    #[arg(long)]
    pub lens: Option<String>,
    /// A node ID, an ID suffix or a name to centre on.
    #[arg(long)]
    pub focus: Option<String>,
    #[arg(long, value_enum, default_value_t)]
    #[serde(default)]
    pub detail: Detail,
    /// The cursor `meta.next` gave, to read the page after it.
    #[arg(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

/// How much `focus` returns.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum FocusDetail {
    /// The whole pane, its mini-map included.
    #[default]
    Full,
    /// The pane without its mini-map (for the tools).
    Summary,
}

/// Which picker `focus --picker` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum PickerName {
    /// The lenses the workspace's packs declare.
    Lens,
    /// The workspace's plan projects.
    Project,
    /// The plan stages of a project.
    Stage,
}

/// `planroom focus [--session ID] [--detail full|summary] [--picker NAME [--project P]]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct FocusReq {
    /// The Claude Code session whose focus to show.
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, value_enum, default_value_t)]
    #[serde(default)]
    pub detail: FocusDetail,
    /// Return this picker in place of the pane.
    #[arg(long, value_enum)]
    #[serde(default)]
    pub picker: Option<PickerName>,
    /// The project a stage picker is for.
    #[arg(long, value_name = "P", requires = "picker")]
    #[serde(default)]
    pub project: Option<String>,
}

/// `planroom scan [DIR]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ScanReq {
    /// The workspace directory; the current directory when absent.
    pub dir: Option<String>,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom watch [--since SEQ] [--session ID]`: one `StreamFrame` per line on stdout until
/// stopped.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct WatchReq {
    /// Stream the events after this `seq`; the store's latest when absent.
    #[arg(long)]
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub since: Option<Seq>,
    /// The Claude Code session this stream serves, as the mod starts it.
    #[arg(long)]
    pub session: Option<String>,
}

/// `planroom schema [--commands [--cursor C] | --command LEAF | --def NAME]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct SchemaReq {
    /// List the command catalogue instead of the schema's summary.
    #[arg(long, conflicts_with_all = ["command", "def"])]
    #[serde(default)]
    pub commands: bool,
    /// One leaf's request and result types, with the definitions they reference.
    #[arg(long, value_name = "LEAF", conflicts_with = "def")]
    #[serde(default)]
    pub command: Option<String>,
    /// One type's definition, with the definitions it references.
    #[arg(long, value_name = "NAME")]
    #[serde(default)]
    pub def: Option<String>,
    /// With --commands: the page after the one whose `meta.next` gave this cursor.
    #[arg(long, value_name = "CURSOR", requires = "commands")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Cursor>,
}

/// `planroom version`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct VersionReq {}

/// `planroom person init [--rekey]`: the public key arrives on stdin as the base64url of its
/// uncompressed SEC1 point (`crypto.subtle.exportKey("raw")`), and the call carries a
/// `PLANROOM_PERSON_TOKEN` signed by the registered key or by the key on stdin (DR-72, DR-75).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct PersonInitReq {
    /// Replace the registered key with the one on stdin.
    #[arg(long)]
    #[serde(default)]
    pub rekey: bool,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// What `person init` returns: the registered key's fingerprint and when and from where it was
/// registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PersonKey {
    pub summary: String,
    /// The first 16 hex characters of the sha256 of the key's SEC1 bytes.
    pub fingerprint: String,
    pub registered_at: crate::event::Timestamp,
    pub surface: crate::event::Surface,
    /// This call replaced another key.
    pub rekeyed: bool,
}

/// `planroom serve [--detach] [--port N]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ServeReq {
    /// Run in the background, in a session of its own, and return at once.
    #[arg(long)]
    #[serde(default)]
    pub detach: bool,
    /// Move this workspace's canvas to port N and record it; without it, serve uses the recorded port.
    #[arg(long)]
    #[serde(default)]
    pub port: Option<u16>,
}

/// `planroom open`: the canvas in the browser, through a login page under the data root, so the
/// one-time code is in no argv and no output (DR-29, DR-64). Person only, from a mod press; a
/// `/dev/tty` confirmation serves only where no person key is registered (DR-75, DR-81).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct OpenReq {}

/// What `open` returns: whether a browser was opened. Where none can be seen (no display, an SSH
/// session, or an opener that fails), nothing is opened, and `empty.next` names `share` (DR-29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenResult {
    pub opened: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<Empty>,
}

/// `planroom share [--out DIR] [--include-snippets] [--include-presence]`: the canvas built for an
/// Artifact, `DIR/data/snapshot.json` and the first `live/head`, with the publish's arguments.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ShareReq {
    /// The directory to write the bundle into; the state directory's `share/` when absent: `.planroom/share`,
    /// or `.git/planroom/share` for a workspace that is one repo.
    #[arg(long, value_name = "DIR")]
    #[serde(default)]
    pub out: Option<String>,
    /// Keep each evidence line's text in the snapshot (redacted); left out by default.
    #[arg(long)]
    #[serde(default)]
    pub include_snippets: bool,
    /// Include who is working on what; left out by default.
    #[arg(long)]
    #[serde(default)]
    pub include_presence: bool,
}

/// `planroom import -`: Artifact inbox rows on stdin, one `ImportLine` per line, from the mod,
/// which read them from the `ArtifactData list` result it observed (DR-52, DR-67).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ImportReq {
    /// `-`: the rows come only on stdin, never from a file.
    #[arg(value_name = "SOURCE")]
    pub source: String,
}

/// `planroom doctor [--pack DIR [--update]]`: with no flag, the general report of git, the
/// store, its migrations, the pack set and the event backup; with `--pack`, one pack's conformance
/// kit (DR-10). `--restore` joins them later.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct DoctorReq {
    /// The pack directory to check: validation against the built-in packs, every extractor over
    /// its `tests/inputs/`, the comparison with `tests/expected.jsonl` and a second run.
    #[arg(long, value_name = "DIR")]
    #[serde(default)]
    pub pack: Option<String>,
    /// Rewrite the pack's `tests/expected.jsonl` from this run instead of comparing with it.
    #[arg(long, requires = "pack")]
    #[serde(default)]
    pub update: bool,
}

/// What `doctor --pack DIR` returns: the pack, what it declares, and each conformance check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackReport {
    pub summary: String,
    pub id: String,
    pub version: u32,
    /// `builtin` when the directory holds a built-in pack's ID, else `out_of_tree`.
    pub origin: String,
    /// The lowercase hex sha256 over the pack's files, as the store's pack record names it.
    pub sha256: String,
    /// The pack's kinds, relations, extractors, linkers and lenses, qualified.
    pub kinds: Vec<String>,
    pub relations: Vec<String>,
    pub extractors: Vec<String>,
    pub linkers: Vec<String>,
    pub lenses: Vec<String>,
    /// Each check the kit passed, in order. The first check that fails fails the command with
    /// `pack_invalid`, naming the file, the line and the reference.
    pub checks: Vec<PackCheck>,
}

/// One conformance check `doctor --pack` passed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackCheck {
    /// `validate`, `expected`, `determinism`, `evidence` or `evals`.
    pub check: String,
    /// One engine-authored line on what it found.
    pub line: String,
}

/// What `doctor` returns: with `--pack DIR`, that pack's conformance report; with no flag, the
/// general report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DoctorResult {
    Pack(PackReport),
    General(DoctorReport),
}

/// What `doctor` with no flag returns: git, the store, its migrations, the pack set and the event
/// backup (DR-53, DR-89). Outside a workspace, or before its first scan, `store`, `migrations` and
/// `backup` are null. A path longer than 160 characters shows as `…` and its last 159.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorReport {
    pub summary: String,
    pub git: DoctorGit,
    pub store: Option<DoctorStore>,
    pub migrations: Option<DoctorMigrations>,
    pub packs: DoctorPacks,
    pub backup: Option<DoctorBackup>,
}

/// The git on `PATH`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorGit {
    /// Its version, such as `2.43.0`.
    pub version: String,
    /// The oldest the engine runs on, `2.34.1`.
    pub minimum: String,
}

/// The workspace's store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorStore {
    /// The store file.
    #[schemars(extend("x-redact" = true))]
    pub path: String,
    pub workspace_id: String,
    /// The store's head.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub seq: Seq,
    /// How many events it holds.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub events: u64,
    /// The engine that wrote the store last; none for a store with no record of it.
    pub engine_version: Option<String>,
}

/// The store's version and the copies its migrations kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorMigrations {
    /// The store's version.
    pub store_version: u32,
    /// The version this engine writes.
    pub supported: u32,
    /// The `store-vN.db` copies under `DATA_ROOT/backups/<workspace_id>/`, by file name, newest
    /// first, at most 3.
    #[schemars(extend("x-redact" = true))]
    pub copies: Vec<String>,
}

/// The pack set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorPacks {
    /// The recorded set, in load order, or this engine's built-ins when none is recorded; at
    /// most 6.
    pub packs: Vec<crate::PackRecord>,
    /// How many more packs the set holds than `packs` lists.
    pub more: u32,
    /// The recorded set's hash; none before a set is recorded.
    pub recorded_hash: Option<String>,
    /// The hash of this engine's set: its built-ins, then the record's out-of-tree packs.
    pub hash: String,
    /// Whether `recorded_hash` is `hash`; none before a set is recorded.
    pub matches: Option<bool>,
}

/// How far the event backup has come (DR-53).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorBackup {
    /// `DATA_ROOT/backups/<workspace_id>/events.ndjson`, or `DATA_ROOT/backups` when the
    /// workspace ID names no directory there, so no backup can exist.
    #[schemars(extend("x-redact" = true))]
    pub file: String,
    /// The store's mark: the `seq` the last append brought the file to.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub backup_seq: Seq,
    /// The last `seq` the file holds of this store: its last whole line's, or 0 when the
    /// write-through would rename it aside (its header names another workspace, `store_version`
    /// or state directory, or that line is above the store's head) or no backup can exist.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub file_seq: Seq,
    /// The `seq` of the last person-authored event.
    #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]
    pub last_authored: Seq,
    /// Whether the smaller of `backup_seq` and `file_seq` is below `last_authored`, which the
    /// `backup_behind` warning reports.
    pub behind: bool,
    /// Whether this host loses its files with its container, so each backup also goes to a
    /// redacted ref in one workspace repo.
    pub temporary_host: bool,
    /// That repo's path from the workspace root, once the first copy fixed it.
    #[schemars(extend("x-redact" = true))]
    pub backup_ref: Option<String>,
}

/// What `serve` returns once it listens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServeInfo {
    pub summary: String,
    /// `http://127.0.0.1:PORT/`.
    pub url: String,
    pub port: u16,
    /// The serving process.
    pub pid: u32,
    /// The login page under the data root that opens the canvas with a one-time code, mode 0600;
    /// the code itself is in no output (DR-29, DR-64).
    pub login_page: String,
}

/// `PUT /api/v1/layout`: an auto-layout to cache, last writer wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutPut {
    /// The view the positions belong to, such as `map:system:all`.
    pub view_key: String,
    /// The hash of the node and edge IDs the layout was computed for.
    pub structure_hash: String,
    pub positions: std::collections::BTreeMap<NodeId, crate::view::Position>,
}

/// `POST /api/v1/session`: the one-time code from the login page's URL fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionReq {
    pub code: String,
}

/// `POST /api/v1/code`: a login code to register, by its sha256, sent with the control token from
/// the server's record under the data root, `DATA_ROOT/serve/<port>.json`, so the code itself
/// never reaches the server's memory or any argv (DR-29, DR-64).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CodeReq {
    /// The lowercase hex sha256 of the code.
    pub code_sha256: String,
}

/// `POST /api/v1/drain`: the control token from the server's record under the data root,
/// `DATA_ROOT/serve/<port>.json`, which asks that `serve` to finish its in-flight requests and
/// exit, so a newer engine can take the workspace's port (DR-58, DR-69).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DrainReq {
    pub token: String,
}

/// The tab's bearer token, kept in `sessionStorage` and sent as `Authorization: Bearer`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionToken {
    pub token: String,
}

/// What `version` returns; the mod and the resolver match an engine by its catalogue hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VersionInfo {
    pub version: String,
    /// The commit the binary was built from, when the build recorded one.
    pub git_commit: Option<String>,
    /// The target triple the binary was built for.
    pub target: String,
    pub schema_version: u32,
    /// sha256 of the catalogue as `schema --commands` lists it.
    pub catalogue_hash: String,
}

/// What `schema` returns: the schema's ID and version, and handles to read further.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaSummary {
    pub summary: String,
    /// The serialize view's `$id`.
    pub id: String,
    pub schema_version: u32,
    pub handles: Vec<Handle>,
}

/// What `schema --command LEAF` returns: the leaf's request and result type names, and every
/// definition they reference, keyed by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LeafSchema {
    pub summary: String,
    pub request: String,
    pub result: String,
    pub defs: serde_json::Map<String, serde_json::Value>,
    /// A `schema --def NAME` handle for each type `defs` references but leaves out to keep to the
    /// budget; empty when every referenced type is there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handles: Vec<Handle>,
}

/// What `schema --def NAME` returns: the type's name, and its definition with every definition it
/// references, keyed by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DefSchema {
    pub summary: String,
    pub name: String,
    pub defs: serde_json::Map<String, serde_json::Value>,
    /// A `schema --def NAME` handle for each type `defs` references but leaves out to keep to the
    /// budget; empty when every referenced type is there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handles: Vec<Handle>,
}

/// What `schema --commands` returns: one page of the catalogue's leaves in order, each whole,
/// summary first; `meta.next` is the cursor of the page after, and the pages join to every leaf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandList {
    pub summary: String,
    pub items: Vec<LeafListing>,
}

/// `planroom pack add DIR`: validate the pack in `DIR`, copy it into the state directory's
/// `packs/<sha256>/` and record it in the workspace's pack set and config (DR-62). Person only.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct PackAddReq {
    /// The pack's directory, from the current directory.
    #[arg(value_name = "DIR")]
    pub dir: String,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom pack remove ID`: drop the out-of-tree pack `ID` from the workspace's pack set and
/// config (DR-62). Person only.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct PackRemoveReq {
    /// The pack's ID.
    #[arg(value_name = "ID")]
    pub id: String,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// What `pack add` and `pack remove` return: the pack and its copy, whether the record changed,
/// and the workspace's pack set afterwards. It carries no path: the summary quotes the source,
/// shortened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackSetResult {
    pub summary: String,
    /// The pack's ID.
    pub id: String,
    pub version: u32,
    /// The sha256 of the copy the pack was added or removed with, which names it in the state
    /// directory's `packs/`.
    pub copy: String,
    /// Whether the workspace's recorded pack set changed.
    pub changed: bool,
    /// The out-of-tree packs recorded afterwards.
    pub loaded: u32,
    /// The recorded set's hash afterwards.
    pub packs_hash: String,
}

/// What `scan` returns: the map at the top level with the default lens, and the next action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScanResult {
    pub map: MapView,
    pub next: Handle,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};

    fn parse<T: Args + FromArgMatches>(args: &[&str]) -> T {
        let cmd = T::augment_args(clap::Command::new("t").no_binary_name(true));
        cmd.clone().debug_assert();
        T::from_arg_matches(&cmd.try_get_matches_from(args).unwrap()).unwrap()
    }

    #[test]
    fn each_leaf_has_one_spec_with_a_unique_name() {
        let names: Vec<String> = Command::SPECS.iter().map(CommandSpec::name).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        assert_eq!(Command::Map(MapReq::default()).name().0, "map");
        assert_eq!(Command::Focus(FocusReq::default()).name().0, "focus");
        assert_eq!(Command::Scan(ScanReq::default()).name().0, "scan");
    }

    #[test]
    fn only_a_leaf_that_can_make_a_paid_model_call_spends() {
        // The leaves contracts.md's Command catalogue names as spending; a leaf joins this list
        // only when it can make a paid model call.
        const PAID: &[&str] = &["intent next", "decide run", "curate"];
        for spec in Command::SPECS {
            assert_eq!(spec.spends, PAID.contains(&spec.name().as_str()), "{}", spec.name());
        }
    }

    #[test]
    fn a_command_travels_as_its_name_and_args() {
        let cmd = Command::Map(MapReq { level: Some(LevelId::from("container")), ..MapReq::default() });
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"command": "map", "args": {"level": "container", "lens": null, "focus": null, "detail": "summary"}})
        );
        let back: Command =
            serde_json::from_value(serde_json::json!({"command": "map", "args": {}})).unwrap();
        assert_eq!(back, Command::Map(MapReq::default()));
    }

    #[test]
    fn requests_parse_from_cli_flags() {
        let map: MapReq = parse(&["--level", "container", "--detail", "graph"]);
        assert_eq!(map.level, Some(LevelId::from("container")));
        assert_eq!(map.detail, Detail::Graph);
        let scan: ScanReq = parse(&["ws", "--base", "12", "--project", "plan:checkout:retries"]);
        assert_eq!(scan.dir.as_deref(), Some("ws"));
        assert_eq!(scan.common.base, Some(12));
        let focus: FocusReq = parse(&["--session", "s1"]);
        assert_eq!(focus.session.as_deref(), Some("s1"));
    }

    /// A leaf's own flags parse after its subcommand as well as before it, so its synopsis
    /// (`edit move|...|undo [--project PROJECT] ...`) runs as written.
    #[test]
    fn a_leafs_own_flags_parse_after_its_subcommand() {
        let after: EditReq =
            parse(&["move", "a", "b", "--base", "4", "--project", "plan:x:y", "--idem", "k"]);
        let before: EditReq =
            parse(&["--base", "4", "--project", "plan:x:y", "--idem", "k", "move", "a", "b"]);
        assert_eq!(after, before);
        assert_eq!(after.common.base, Some(4));
        assert_eq!(after.common.project.as_ref().map(|p| p.0.as_str()), Some("plan:x:y"));
        assert_eq!(after.common.idem.as_deref(), Some("k"));
    }
}
