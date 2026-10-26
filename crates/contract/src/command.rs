//! The command catalogue: one `Command` variant per leaf, each with its `CommandSpec`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::{CommandName, Handle};
use crate::errors::ErrorCode;
use crate::model::{Level, NodeId, Seq};
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

/// Everything the surfaces need to know about one command leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub errors: &'static [ErrorCode],
    pub undo: Undo,
    pub tool: Option<Tool>,
    /// Reachable as `/room ...`.
    pub room: bool,
}

impl CommandSpec {
    /// The command name: the path joined with spaces.
    pub fn name(&self) -> String {
        self.path.join(" ")
    }
}

/// One variant per catalogue leaf; the request travels as `args`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", content = "args", rename_all = "snake_case")]
pub enum Command {
    Map(MapReq),
    Focus(FocusReq),
    Scan(ScanReq),
    Watch(WatchReq),
    Schema(SchemaReq),
    Version(VersionReq),
}

const MAP: CommandSpec = CommandSpec {
    path: &["map"],
    idea: Idea::Levels,
    summary: "The map at a level through a lens.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
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
};

const FOCUS: CommandSpec = CommandSpec {
    path: &["focus"],
    idea: Idea::Levels,
    summary: "Where you are, the one open decision and the next action.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_000,
    needs_base: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: Some(Tool::RoomQuery),
    room: true,
};

const SCAN: CommandSpec = CommandSpec {
    path: &["scan"],
    idea: Idea::Setup,
    summary: "Discover, extract and link the workspace's repos.",
    actor: ActorRule::Any,
    class: Class::Change,
    budget_tokens: 1_500,
    needs_base: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::WorkspaceNotFound,
        ErrorCode::PackInvalid,
        ErrorCode::GitFailed,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::NotUndoable("a scan records what the repos hold; scan again instead"),
    tool: Some(Tool::RoomChange),
    room: true,
};

const WATCH: CommandSpec = CommandSpec {
    path: &["watch"],
    idea: Idea::Setup,
    summary: "Stream the workspace's committed events as NDJSON frames.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 0,
    needs_base: false,
    errors: &[
        ErrorCode::WorkspaceNotFound,
        ErrorCode::StoreBusy,
        ErrorCode::SchemaMismatch,
        ErrorCode::Internal,
    ],
    undo: Undo::Query,
    tool: None,
    room: false,
};

const SCHEMA: CommandSpec = CommandSpec {
    path: &["schema"],
    idea: Idea::Setup,
    summary: "The contract schema's summary, or the command catalogue with --commands.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    errors: &[ErrorCode::InvalidArgument, ErrorCode::Internal],
    undo: Undo::Query,
    tool: None,
    room: false,
};

const VERSION: CommandSpec = CommandSpec {
    path: &["version"],
    idea: Idea::Setup,
    summary: "This engine's version, target, schema version and catalogue hash.",
    actor: ActorRule::Any,
    class: Class::Read,
    budget_tokens: 1_500,
    needs_base: false,
    errors: &[ErrorCode::Internal],
    undo: Undo::Query,
    tool: None,
    room: false,
};

impl Command {
    /// Every leaf's spec, in catalogue order.
    pub const SPECS: &'static [CommandSpec] = &[MAP, FOCUS, SCAN, WATCH, SCHEMA, VERSION];

    pub const fn spec(&self) -> &'static CommandSpec {
        match self {
            Command::Map(_) => &MAP,
            Command::Focus(_) => &FOCUS,
            Command::Scan(_) => &SCAN,
            Command::Watch(_) => &WATCH,
            Command::Schema(_) => &SCHEMA,
            Command::Version(_) => &VERSION,
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
        }
    }
}

/// Flattened into every write request.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct WriteCommon {
    /// The `seq` of the view you acted on; required when the command writes user-authored state.
    #[arg(long)]
    pub base: Option<Seq>,
    /// The idempotency key; derived from the request when absent.
    #[arg(long)]
    pub idem: Option<String>,
    /// The project the write belongs to.
    #[arg(long)]
    pub project: Option<NodeId>,
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

/// `planroom map [--level L] [--lens X] [--focus ID] [--detail summary|graph]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct MapReq {
    /// The level to show; the top level when absent.
    #[arg(long, value_enum)]
    pub level: Option<Level>,
    /// The lens; `all` lifts every relation kind.
    #[arg(long)]
    pub lens: Option<String>,
    /// A node ID, an ID suffix or a name to centre on.
    #[arg(long)]
    pub focus: Option<String>,
    #[arg(long, value_enum, default_value_t)]
    #[serde(default)]
    pub detail: Detail,
}

/// `planroom focus [--session ID]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct FocusReq {
    /// The Claude Code session whose focus to show.
    #[arg(long)]
    pub session: Option<String>,
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

/// `planroom watch [--since SEQ]`: one `StreamFrame` per line on stdout until stopped.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct WatchReq {
    /// Stream the events after this `seq`; the store's latest when absent.
    #[arg(long)]
    pub since: Option<Seq>,
}

/// `planroom schema [--commands]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct SchemaReq {
    /// List the command catalogue instead of the schema's summary.
    #[arg(long, conflicts_with = "command")]
    #[serde(default)]
    pub commands: bool,
    /// One leaf's request and result types, with the definitions they reference.
    #[arg(long, value_name = "LEAF")]
    #[serde(default)]
    pub command: Option<String>,
}

/// `planroom version`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct VersionReq {}

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
}

/// What `schema --commands` returns: every leaf, summary first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandList {
    pub summary: String,
    pub items: Vec<LeafListing>,
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
    fn a_command_travels_as_its_name_and_args() {
        let cmd = Command::Map(MapReq { level: Some(Level::Container), ..MapReq::default() });
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
        assert_eq!(map.level, Some(Level::Container));
        assert_eq!(map.detail, Detail::Graph);
        let scan: ScanReq = parse(&["ws", "--base", "12", "--project", "plan:checkout:retries"]);
        assert_eq!(scan.dir.as_deref(), Some("ws"));
        assert_eq!(scan.common.base, Some(12));
        let focus: FocusReq = parse(&["--session", "s1"]);
        assert_eq!(focus.session.as_deref(), Some("s1"));
    }
}
