//! Every public Planroom type, the error and warning registries and the command catalogue.
//!
//! The Rust types here are the one source of the public contract. `cargo xtask gen` turns them
//! into `schema/planroom.v1.schema.json` (what Planroom emits), `schema/planroom.v1.input.schema.json`
//! (what Planroom accepts) and the TypeScript types in `packages/contract/src/generated/`.

pub mod card;
pub mod command;
pub mod config;
pub mod decision;
pub mod envelope;
pub mod errors;
pub mod event;
pub mod format;
pub mod model;
pub mod models;
mod open_set;
pub mod pack;
pub mod plan;
pub mod remote;
pub mod requests;
pub mod resolver;
pub mod schema;
pub mod store;
pub mod stream;
pub mod view;
pub mod vocab;

pub use card::{Card, CardKind};
pub use command::PickerName;
pub use command::{
    ActorRule, Class, CodeReq, Command, CommandList, CommandSpec, DefSchema, Detail, DoctorReq, DrainReq,
    FocusDetail, FocusReq, Idea, ImportReq, LayoutPut, LeafFlag, LeafListing, LeafSchema, MapReq, OpenReq,
    OpenResult, PackAddReq, PackCheck, PackRemoveReq, PackReport, PackSetResult, PersonInitReq, PersonKey,
    ScanReq, ScanResult, SchemaReq, SchemaSummary, ServeInfo, ServeReq, SessionReq, SessionToken, ShareReq,
    Tool, Undo, VersionInfo, VersionReq, WatchReq, WriteCommon,
};
pub use command::{
    DoctorBackup, DoctorGit, DoctorMigrations, DoctorPacks, DoctorReport, DoctorResult, DoctorStore,
};
pub use config::Config;
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
    Actor, Cause, Client, Edit, Event, EventBody, EventId, EventKind, ExportHeader, HandoffState,
    HandoffStep, Op, Rename, ScanStats, Status, Surface, Target, Timestamp, Verdict,
};
pub use model::{
    CONTAINS_MAX_CHARS, Edge, EdgeId, Evidence, FACTS_FORMAT, FactBatch, Hint, HintRef, Kind, Layer, LevelId,
    Node, NodeId, PartialFile, PartialReason, Provenance, Ref, Seq, Tier,
};
pub use models::{Measured, REPLAY_FORMAT, RecordedRequest, RecordedResponse, ReplayRecord};
pub use pack::{
    ActionDecl, ArgDecl, CacheTtl, EmitTemplate, ExtractorDecl, Flow, HintDecl, ImageRole, ItemType,
    KindDecl, LanguageDecl, LensDecl, LevelDecl, LinkerDecl, ModelTier, PREDICATE_PARAMS, PackFile,
    PackRecord, Param, ParamNames, PromptDecl, PromptRole, QuestionDecl, RelationDecl, RuleDecl, RuleType,
    Shape, ShapeStep, StageDecl, SuggestionDecl,
};
pub use plan::{
    DraftItem, DraftProp, DraftReply, DraftValue, SliceItem, SliceTarget, StageSlice, draft_reply_schema,
};
pub use plan::{
    IntentListReq, IntentListResult, IntentNewReq, IntentVersionReq, LintFinding, LintResult, PlanItem,
    PlanProject, PlanProposal, PlanVersion, ProposedItem,
};
pub use remote::{
    Ack, AckStatus, Answer, DbBatch, DbOp, DbWrite, ImportCounts, ImportLine, ImportResult, ImportedRow,
    InboxHead, InboxRow, LiveHead, PublishArgs, SampleUse, ShareResult, SharedFile, Snapshot,
};
pub use requests::{
    AskItem, AskReq, AskResult, EditOp, EditReq, EditResult, FindItem, FindReq, FindResult, IntentNextReq,
    LintReq, ProposeReq, ReviewReq, ReviewResult,
};
pub use resolver::{ResolverStatus, ResolverStep, UnavailableReason};
pub use store::{STORE_POINTER_FILE, StorePointer};
pub use stream::StreamFrame;
pub use view::{
    BatchInfo, Choice, Crumb, DecisionCard, DiffLine, DriftLine, EngineState, EngineStatus, Focus,
    FocusResult, LastSeq, LayoutKey, LiftedEdge, MapNode, MapView, MiniBlock, MiniEdge, MiniMap, OutboxEntry,
    PaneModel, PaneState, PickEstimate, Picker, PickerEntry, Position, PresenceRow, Progress, ScanState,
    Stakes, Tone, TurnSeq, Yarn,
};
pub use vocab::{KindVocab, LensVocab, LevelVocab, RelationVocab, Vocabulary};
