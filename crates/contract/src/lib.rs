//! Every public Planroom type, the error and warning registries and the command catalogue.
//!
//! The Rust types here are the one source of the public contract. `cargo xtask gen` turns them
//! into `schema/planroom.v1.schema.json` (what Planroom emits), `schema/planroom.v1.input.schema.json`
//! (what Planroom accepts) and the TypeScript types in `packages/contract/src/generated/`.

pub mod card;
pub mod command;
pub mod decision;
pub mod envelope;
pub mod errors;
pub mod event;
pub mod format;
pub mod model;
mod open_set;
pub mod pack;
pub mod remote;
pub mod requests;
pub mod resolver;
pub mod schema;
pub mod stream;
pub mod view;
pub mod vocab;

pub use card::{Card, CardKind};
pub use command::{
    ActorRule, Class, CodeReq, Command, CommandList, CommandSpec, Detail, FocusReq, Idea, ImportReq,
    LayoutPut, LeafListing, LeafSchema, MapReq, PersonInitReq, PersonKey, ScanReq, ScanResult, SchemaReq,
    SchemaSummary, ServeInfo, ServeReq, SessionReq, SessionToken, ShareReq, Tool, Undo, VersionInfo,
    VersionReq, WatchReq, WriteCommon,
};
pub use decision::{
    Band, BoolCriteria, DecisionBudget, DecisionKind, DecisionRecord, Effect, EffectOp, OptionsFrom, Outcome,
    Policy, Predicate, Question, Role, SliceSpec, SubjectKind,
};
pub use envelope::{
    CommandName, Cursor, DirtyRepo, Empty, EmptyReason, Envelope, ErrorObject, Handle, Meta, ModelRequestRef,
    ModelUse, SCHEMA_VERSION, Size, ToolError, ToolMeta, ToolResult, Usage, Warning,
};
pub use errors::{Category, ErrorCode, Retry, WarningCode};
pub use event::{
    Actor, Cause, Client, Edit, Event, EventBody, EventId, EventKind, HandoffState, HandoffStep, Op, Rename,
    ScanStats, Status, Surface, Target, Timestamp, Verdict,
};
pub use model::{
    CONTAINS_MAX_CHARS, Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Hint, HintRef, Kind, Layer, Level,
    Node, NodeId, PartialFile, Provenance, Ref, Seq, Tier,
};
pub use pack::{
    ActionDecl, ArgDecl, CacheTtl, EmitTemplate, ExtractorDecl, Flow, HintDecl, ImageRole, ItemType,
    KindDecl, LanguageDecl, LensDecl, LinkerDecl, ModelTier, PackFile, PromptDecl, PromptRole, QuestionDecl,
    RelationDecl, RuleDecl, RuleType, Shape, ShapeStep, StageDecl, SuggestionDecl,
};
pub use remote::{
    Ack, AckStatus, Answer, DbBatch, DbOp, DbWrite, ImportCounts, ImportLine, ImportResult, ImportedRow,
    InboxHead, InboxRow, LiveHead, PublishArgs, SampleUse, ShareResult, SharedFile, Snapshot,
};
pub use requests::{
    AskItem, AskReq, AskResult, EditOp, EditReq, FindItem, FindReq, FindResult, IntentNextReq, LintReq,
    ProposeReq, ReviewReq,
};
pub use resolver::{ResolverStatus, ResolverStep, UnavailableReason};
pub use stream::StreamFrame;
pub use view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, EngineState, EngineStatus, Focus, LastSeq,
    LayoutKey, LiftedEdge, MapNode, MapView, MiniBlock, MiniEdge, MiniMap, PaneModel, PaneState, Position,
    PresenceRow, Progress, ScanState, Stakes, Tone, TurnSeq,
};
pub use vocab::{KindVocab, LensVocab, RelationVocab, Vocabulary};
