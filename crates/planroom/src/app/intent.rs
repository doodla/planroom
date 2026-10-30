//! `planroom intent new | propose | lint | version | list` (architecture.md, "Planning: stages,
//! ghosts and lint (R5)"; DR-4, DR-5, DR-23, DR-45, DR-66).
//!
//! - `intent new TEXT` puts a project, `plan:<slug>:project`; `intent propose --file F|-` writes a
//!   batch of plan items and what they touch; `intent version LABEL` tags the project's plan. The
//!   three write through `edit`'s one write path ([`edit::commit`]), so each is planned inside its
//!   own IMMEDIATE transaction under compare-and-set, keyed and replayed like an edit. A person's
//!   write applies; an agent's is a ghost until a person accepts each item with `review`.
//!   `version` is a person's alone.
//! - A proposal is read from stdin or from a file the workspace holds ([`confine::proposal`]), at
//!   most 64 KiB, and checked against the plan pack's stages before any store is read: its bounds,
//!   its keys, each item's type and each prop against its type's schema. Its parents, touch
//!   targets and project are resolved inside the write's transaction, so they see the commit's
//!   snapshot; a missing one writes nothing.
//! - `intent lint` runs every loaded rule over a project's items in one read transaction, and
//!   `intent list` lists every project and the named or only project's items, ghosts with the
//!   exact review that accepts them. Each keeps to its leaf's budget.
//!
//! A project is named by its full ID, its slug alone or its exact name, among projects only, ghost
//! ones included; with none named, the only project is used.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use planroom_contract::{
    Actor, Command, CommandName, Cursor, EditResult, Empty, EmptyReason, ErrorCode, EventBody, EventId,
    Handle, IntentListReq, IntentListResult, IntentNewReq, IntentNextReq, IntentVersionReq, LevelId,
    LintFinding, LintReq, LintResult, ModelTier, ModelUse, Node, NodeId, PlanItem, PlanProject, PlanProposal,
    PlanVersion, ProposeReq, RuleDecl, Seq, StageDecl, Status, Usage, Vocabulary, Warning, WarningCode,
};
use planroom_kernel::cas::{self, Authority, Check, Proposed};
use planroom_kernel::fold::{Folded, Writer};
use planroom_kernel::ladder::Ladder;
use planroom_kernel::plan::{self, Item, Project};
use planroom_kernel::query::lookup::{self, Lookup};
use planroom_store::{Snapshot, Store};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};

use crate::app::actor::{Caller, Terminal};
use crate::app::edit::{self, Plan, Writing, Written, q};
use crate::app::error::AppError;
use crate::app::{Paged, Reply, confine, workspace};
use crate::packs::{Loaded, state};
use crate::{live, text};

/// A project's text, at most.
const TEXT_MAX: usize = 200;
/// A version label, at most.
const LABEL_MAX: usize = 40;
/// A proposal's bytes, at most.
const FILE_MAX: usize = 64 * 1024;
/// A proposal's items, at most.
const ITEMS_MAX: usize = 50;
/// An item's name, in characters, at most.
const NAME_MAX: usize = 200;
/// A prop's text, in characters, at most.
const STRING_MAX: usize = 2_000;
/// A list prop's entries, at most.
const LIST_MAX: usize = 20;
/// The blocks one item touches, at most.
const TOUCHES_MAX: usize = 10;
/// IDs a refusal names at most.
const CANDIDATES: usize = 5;
/// The review handles one item carries at most.
const REVIEWS_MAX: usize = 3;
/// What the envelope around a result takes, in tokens: its fields, `meta` and the summary again.
const ENVELOPE_TOKENS: usize = 60;
/// What a `truncated` warning takes, in tokens, with its counts and handle.
const WARNING_TOKENS: usize = 160;
/// Characters of a lint finding's value kept.
const VALUE_CHARS: usize = 80;
/// A finding's values, at most.
const VALUES_MAX: usize = 20;
/// Characters of the last entry's ID a listing cursor keeps.
const MARK_CHARS: usize = 32;

// ---------------------------------------------------------------------------------------------
// What the packs declare

/// The loaded stages, in order: the plan pack's, the one pack that declares any.
pub(crate) fn stages(loaded: &Loaded) -> Vec<StageDecl> {
    loaded.packs.iter().flat_map(|p| p.file.stages.iter().cloned()).collect()
}

/// Each item type's prop schema: the union of every stage's declaration of it, which the loader
/// keeps free of a prop declared two ways.
pub(crate) fn item_types(stages: &[StageDecl]) -> BTreeMap<String, Map<String, Value>> {
    let mut out: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for stage in stages {
        for item in &stage.items {
            let props = out.entry(item.id.clone()).or_default();
            if let Some(declared) = item.props.get("properties").and_then(Value::as_object) {
                for (name, schema) in declared {
                    props.entry(name.clone()).or_insert_with(|| schema.clone());
                }
            }
        }
    }
    out
}

/// Every loaded rule a plan's items or its project are the subject of, with its index in load
/// order, kept by `keep`.
fn plan_rules(loaded: &Loaded, keep: impl Fn(&RuleDecl) -> bool) -> Vec<&RuleDecl> {
    loaded
        .packs
        .iter()
        .flat_map(|p| &p.file.rules)
        .filter(|r| (r.subject == plan::ITEM_KIND || r.subject == plan::PROJECT_KIND) && keep(r))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The current project

/// The project `given` names, by full ID, slug or exact name, among `projects`; with nothing
/// given, the only one, `None` with none, and `ambiguous_ref` with several.
pub(crate) fn current<'a>(
    projects: &'a [Project],
    given: Option<&str>,
) -> Result<Option<&'a Project>, AppError> {
    let Some(given) = given.map(str::trim) else {
        return match projects {
            [] => Ok(None),
            [only] => Ok(Some(only)),
            several => Err(ambiguous("--project", several.iter().map(|p| &p.id).collect())),
        };
    };
    if let Some(found) = projects.iter().find(|p| p.id.0 == given || p.id == plan::project_id(given)) {
        return Ok(Some(found));
    }
    let named: Vec<&Project> = projects.iter().filter(|p| p.name == given).collect();
    match named.as_slice() {
        [one] => Ok(Some(one)),
        [] => {
            let what = text::get("intent.project_missing", &[("project", &q(given))]);
            let closest: Vec<&str> = projects.iter().take(CANDIDATES).map(|p| p.id.0.as_str()).collect();
            Err(AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest})))
        }
        several => Err(ambiguous(given, several.iter().map(|p| &p.id).collect())),
    }
}

/// `ambiguous_ref` for `target`, naming the first few of `ids` at the intent level.
fn ambiguous(target: &str, ids: Vec<&NodeId>) -> AppError {
    let candidates: Vec<&str> = ids.iter().take(CANDIDATES).map(|id| id.0.as_str()).collect();
    let shown: Vec<String> = candidates.iter().take(3).map(|id| q(id)).collect();
    AppError::new(
        ErrorCode::AmbiguousRef,
        &[("target", &q(target)), ("count", &ids.len()), ("ids", &shown.join(", "))],
    )
    .details(json!({"candidates": candidates, "level": LevelId::INTENT}))
}

/// `invalid_argument` for a person's write onto `id`, which waits as the ghost `event`: its hint
/// is the exact review that accepts it, at `seq`.
fn waits(field: &str, key: &str, id: &NodeId, event: &EventId, seq: Seq) -> AppError {
    let what = text::get(key, &[("id", &q(&id.0))]);
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({"field": field, "ghosts": [event.0], "count": 1}))
        .with_hint(format!("planroom review --base {seq} {} accept", text::shell_word(&event.0)))
}

/// No project to write into: the usage, as text, and never a hint that names it.
fn no_project() -> AppError {
    AppError::invalid_argument("project", "intent.no_project")
}

// ---------------------------------------------------------------------------------------------
// Writes

/// The store at the workspace `cwd` is in, hooked to the live loop, to write.
fn open_write(cwd: &Path) -> Result<(PathBuf, Store), AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);
    Ok((root, store))
}

fn proposed(writer: &Writer, base: Seq) -> Proposed {
    Proposed { writer: writer.clone(), authority: Authority::of(&writer.actor), base }
}

/// The exact line that runs a write from a terminal: its path, its flags, then its operand quoted
/// for a POSIX shell, after `--` when it starts with `-`.
fn line(
    path: &str,
    base: Option<Seq>,
    project: Option<&NodeId>,
    flags: &[(&str, &str)],
    operand: Option<&str>,
) -> String {
    let mut words = vec!["planroom".to_owned(), path.to_owned()];
    if let Some(base) = base {
        words.extend(["--base".to_owned(), base.to_string()]);
    }
    if let Some(project) = project {
        words.extend(["--project".to_owned(), text::shell_word(&project.0)]);
    }
    for (flag, value) in flags {
        words.extend([(*flag).to_owned(), text::shell_word(value)]);
    }
    if let Some(operand) = operand {
        if operand.starts_with('-') {
            words.push("--".to_owned());
        }
        words.push(text::shell_word(operand));
    }
    words.join(" ")
}

/// `planroom intent new TEXT`: one `put` of the project, a person's applied, an agent's a ghost.
pub fn new(
    req: &IntentNewReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Written<EditResult>, AppError> {
    let name = req.text.trim().to_owned();
    if name.is_empty() {
        return Err(AppError::invalid_argument("text", "intent.text_empty"));
    }
    if name.chars().count() > TEXT_MAX {
        return Err(AppError::invalid_argument("text", "intent.text_long"));
    }
    let (root, mut store) = open_write(cwd)?;
    let spec = Command::IntentNew(req.clone()).spec();
    let line = line("intent new", req.common.base, req.common.project.as_ref(), &[], Some(&req.text));
    let writer =
        edit::writer_for(spec, &req.common, &line, caller, &mut store, &root, req.common.base, terminal)?;
    let id = plan::project_id(&plan::project_slug(&name));
    let writing = Writing {
        spec,
        args: json!({"text": req.text}),
        idem: req.common.idem.as_deref(),
        project: Some(id.clone()),
        writer: writer.clone(),
        base: req.common.base,
    };
    edit::commit(&mut store, &root, writing, |_, folded, base| {
        if plan::projects(folded).iter().any(|p| p.id == id) {
            let what = text::get("intent.exists", &[("id", &id.0)]);
            return Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "text"})));
        }
        let put = plan::project_put(&id, &name, base);
        let check = cas::check_edits(folded, &proposed(&writer, base), std::slice::from_ref(&put));
        Ok(Plan {
            bodies: vec![(Some(id.0.clone()), EventBody::Edit(put))],
            targets: vec![id.0.clone()],
            check,
            line: text::get("intent.created", &[("name", &q(&name)), ("id", &id.0)]),
            names: BTreeMap::from([(id.0.clone(), name.clone())]),
            closes: vec![],
            project: Some(id.clone()),
        })
    })
}

/// `planroom intent version LABEL`: a person's `version` tag on the project, applied.
pub fn version(
    req: &IntentVersionReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Written<EditResult>, AppError> {
    let label = req.label.trim().to_owned();
    if label.is_empty() {
        return Err(AppError::invalid_argument("label", "intent.label_empty"));
    }
    if label.chars().count() > LABEL_MAX {
        return Err(AppError::invalid_argument("label", "intent.label_long"));
    }
    // A label is a handle a caller passes back, never masked, so it may hold nothing redaction
    // would mask.
    if planroom_kernel::redact::text(&label) != label {
        return Err(AppError::invalid_argument("label", "intent.label_secret"));
    }
    let (root, mut store) = open_write(cwd)?;
    let spec = Command::IntentVersion(req.clone()).spec();
    let line = line("intent version", req.common.base, req.common.project.as_ref(), &[], Some(&req.label));
    let writer =
        edit::writer_for(spec, &req.common, &line, caller, &mut store, &root, req.common.base, terminal)?;
    let writing = Writing {
        spec,
        args: json!({"label": label}),
        idem: req.common.idem.as_deref(),
        project: req.common.project.clone(),
        writer,
        base: req.common.base,
    };
    edit::commit(&mut store, &root, writing, |snapshot, folded, _| {
        let projects = plan::projects(folded);
        let project = match &req.common.project {
            Some(id) => current(&projects, Some(&id.0))?,
            None => current(&projects, None)?,
        }
        .ok_or_else(no_project)?;
        if !project.live {
            let ghost = project.pending.first().cloned().unwrap_or_else(|| EventId(String::new()));
            return Err(waits("project", "intent.project_ghost", &project.id, &ghost, snapshot.seq()));
        }
        if folded.versions.iter().any(|v| v.project.as_ref() == Some(&project.id) && v.label == label) {
            let vars: &[(&str, &dyn std::fmt::Display)] =
                &[("project", &q(&project.name)), ("label", &label)];
            return Err(AppError::new(
                ErrorCode::InvalidArgument,
                &[("what", &text::get("intent.label_taken", vars))],
            )
            .details(json!({"field": "label"})));
        }
        let body = EventBody::Version { project: Some(project.id.clone()), label: label.clone() };
        Ok(Plan {
            bodies: vec![(Some(project.id.0.clone()), body)],
            targets: vec![project.id.0.clone()],
            check: Check::Applied,
            line: text::get("intent.versioned", &[("project", &q(&project.name)), ("label", &label)]),
            names: BTreeMap::from([(project.id.0.clone(), project.name.clone())]),
            closes: vec![],
            project: Some(project.id.clone()),
        })
    })
}

/// `planroom intent propose --file F|-`: a batch of plan items and what they touch.
pub fn propose(
    req: &ProposeReq,
    cwd: &Path,
    caller: &Caller,
    stdin: &mut dyn Read,
    terminal: &mut dyn Terminal,
) -> Result<Written<EditResult>, AppError> {
    let (root, mut store) = open_write(cwd)?;
    let base = req
        .common
        .base
        .ok_or_else(|| AppError::new(ErrorCode::BaseRequired, &[("command", &"intent propose")]))?;
    let spec = Command::IntentPropose(req.clone()).spec();
    let stdin_file = req.file == "-";
    // `--file -` stays `--file -`: the proposal comes on the line's stdin, as `import -`'s rows do.
    let line =
        line("intent propose", Some(base), req.common.project.as_ref(), &[("--file", &req.file)], None);
    let writer = edit::writer_for(spec, &req.common, &line, caller, &mut store, &root, Some(base), terminal)
        .map_err(|e| {
            if e.code == ErrorCode::PersonRequired && stdin_file {
                e.with_message(text::get("intent.person_stdin", &[]))
            } else {
                e
            }
        })?;
    let bytes = if stdin_file {
        bounded(stdin)?
    } else {
        let path = confine::proposal(&req.file, cwd, matches!(writer.actor, Actor::Person { .. }))?;
        read_file(&path)?
    };
    let proposal = parse(&bytes)?;
    let set = state::for_read(&root, store.recorded_packs()?.as_ref())?;
    let stages = stages(&set);
    check_items(&proposal, &stages)?;
    // The key and the request hash read the proposal itself, never the file it came in, so two
    // proposals at one base never share a derived key and a retry of one replays.
    let writing = Writing {
        spec,
        args: serde_json::to_value(&proposal).map_err(AppError::internal)?,
        idem: req.common.idem.as_deref(),
        project: req.common.project.clone(),
        writer: writer.clone(),
        base: Some(base),
    };
    edit::commit(&mut store, &root, writing, |snapshot, folded, base| {
        plan_proposal(
            &proposal,
            req.common.project.as_ref(),
            snapshot,
            folded,
            &writer,
            base,
            set.vocabulary(),
        )
    })
    .map_err(|e| cycle_field(e, &proposal))
}

/// What `intent next` returns: its write, and the model call it made, which `meta.models` reports.
/// A retry of a stored key replays the write and calls no model.
#[derive(Debug, Clone)]
pub struct Drafted {
    pub written: Written<EditResult>,
    pub models: Option<ModelUse>,
}

/// `planroom intent next [--stage S] [--project P]`: the engine asks Claude to draft a plan stage,
/// and the reply's items wait as the model's ghosts whoever runs it, until a person accepts each
/// with `review` (architecture.md, "Planning: stages, ghosts and lint (R5)"; DR-23, DR-89).
///
/// The live gate is read first, as `scan` reads it. In M1 `claude.backend = auto` and `gateway`
/// both reach the gateway adapter, whose gate answers from a recording, records a live call under
/// `PLANROOM_LIVE_GATEWAY=1` in record mode, or refuses it; `session` arrives with the session
/// backend, and `off` (or `models = "off"`) turns every call off. The stage is `--stage` or the
/// next one the plan lacks; the project is `--project` or the only one, a ghost one included.
///
/// The read and the call happen before the write's transaction, and the reply's items are checked
/// as a proposal is, then planned inside it against the commit's own snapshot, as `intent propose`
/// plans them. A reply that can't be used is `model_reply_invalid` and writes nothing. A key the
/// store already holds replays its stored reply, or meets `idempotency_mismatch`, before any call.
/// Once the stage and the project are known, a model error's hint is the `ask stage` that drafts
/// the stage in chat instead.
pub fn next(
    req: &IntentNextReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Drafted, AppError> {
    let env = crate::app::models::live_env()?;
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let trusted = crate::app::trust::at(&root) == crate::app::trust::Trust::Trusted;
    let loaded_config = crate::config::for_workspace(&workspace::state_dir(&root), trusted)?;
    let config = loaded_config.config;
    backend(&config)?;
    let (root, mut store) = open_write(cwd)?;
    let spec = Command::IntentNext(req.clone()).spec();
    let stage_flag = req.stage.as_deref().map(|s| ("--stage", s));
    let line = line("intent next", req.common.base, req.common.project.as_ref(), stage_flag.as_slice(), None);
    let caller_writer =
        edit::writer_for(spec, &req.common, &line, caller, &mut store, &root, req.common.base, terminal)?;
    let set = state::for_read(&root, store.recorded_packs()?.as_ref())?;
    let model = match crate::app::draft::stage_tier(&set)? {
        ModelTier::Cheap => config.claude.model_cheap.clone(),
        ModelTier::Capable => config.claude.model_capable.clone(),
    };
    // The items are the model's proposal whoever asked for them, written from the caller's client.
    let writer = Writer { actor: Actor::Model { id: model }, client: caller_writer.client };
    let writing = || Writing {
        spec,
        args: json!({"stage": req.stage, "project": req.common.project}),
        idem: req.common.idem.as_deref(),
        project: req.common.project.clone(),
        writer: writer.clone(),
        base: req.common.base,
    };
    let snapshot = store.snapshot()?;
    if let Some(idem) = req.common.idem.as_deref()
        && snapshot.holds_key(idem)?
    {
        drop(snapshot);
        let written = edit::commit(&mut store, &root, writing(), |_, _, _| {
            Err(AppError::internal("a held key replays"))
        })?;
        return Ok(Drafted { written, models: None });
    }
    let folded = live::folded(&snapshot, set.vocabulary())?;
    drop(snapshot);
    let spender = crate::app::models::Spender {
        data_root: crate::app::data::root()?,
        workspace_id: store.meta("workspace_id")?,
    };
    let project_flag = req.common.project.as_ref().map(|p| p.0.as_str());
    let drafting = crate::app::draft::drafting(&set, &folded, req.stage.as_deref(), project_flag)?;
    let stage = drafting.stage.id.clone();
    let project = drafting.project.clone();
    // Once the stage and the project are known, a model refusal names the chat route instead.
    let ask_stage =
        format!("planroom ask stage {} {}", text::shell_word(&stage), text::shell_word(&project.id.0));
    let hinted = |e: AppError| match e.code {
        ErrorCode::BudgetExceeded
        | ErrorCode::ModelUnavailable
        | ErrorCode::ReplayMissing
        | ErrorCode::ModelReplyInvalid => e.with_hint(ask_stage.clone()),
        _ => e,
    };
    let call = &drafting.call;
    let key = planroom_models::claude::build(&config.claude, call).key;
    let reply = crate::app::models::ask_claude(call, &env, &config, &spender).map_err(hinted)?;
    let (proposal, dropped) =
        crate::app::draft::proposal(&reply, &call.purpose, &key, &folded, &project, set.vocabulary())
            .map_err(hinted)?;
    let invalid = |e: AppError| {
        if e.code != ErrorCode::InvalidArgument {
            return e;
        }
        let field = e.details.get("field").and_then(Value::as_str).map(str::to_owned);
        hinted(crate::app::draft::unusable(
            &call.purpose,
            &key,
            crate::app::draft::Unusable::InvalidItems,
            field.as_deref(),
        ))
    };
    check_items(&proposal, &stages(&set)).map_err(invalid)?;
    let count = proposal.items.len();
    let nothing_new = |e: AppError| {
        // The only `items` refusal a plan makes once its items are checked: nothing differs.
        if e.code == ErrorCode::InvalidArgument && e.details.get("field") == Some(&json!("items")) {
            let why = crate::app::draft::Unusable::NothingNew;
            hinted(crate::app::draft::unusable(&call.purpose, &key, why, Some("items")))
        } else {
            e
        }
    };
    let mut written = edit::commit(&mut store, &root, writing(), |snapshot, folded, base| {
        let mut plan = plan_proposal(&proposal, None, snapshot, folded, &writer, base, set.vocabulary())
            .map_err(nothing_new)?;
        let shown: Vec<String> = plan.targets.iter().take(3).map(|id| q(id)).collect();
        let key = if count == 1 { "intent.drafted_one" } else { "intent.drafted" };
        let vars: &[(&str, &dyn std::fmt::Display)] = &[
            ("count", &count),
            ("stage", &stage),
            ("project", &q(&project.name)),
            ("ids", &shown.join(", ")),
        ];
        plan.line = text::get(key, vars);
        if dropped > 0 {
            plan.line.push(' ');
            let key = if dropped == 1 { "intent.targets_dropped_one" } else { "intent.targets_dropped" };
            plan.line.push_str(&text::get(key, &[("count", &dropped)]));
        }
        Ok(plan)
    })
    .map_err(|e| {
        // A parent cycle the reply's items would close: the reply can't be used as it stands.
        if e.code == ErrorCode::InvalidArgument && e.details.get("field") == Some(&json!("parent")) {
            invalid(cycle_field(e, &proposal))
        } else {
            e
        }
    })?;
    written.reply.warnings.extend(loaded_config.warnings);
    if crate::app::draft::cache_not_applied(&reply) {
        written.reply.warnings.push(Warning {
            code: WarningCode::CacheNotApplied,
            message: text::get("warning.cache_not_applied", &[]),
            details: json!({"purpose": call.purpose}),
        });
    }
    let usage = Usage {
        calls: 1,
        cache_hits: u32::from(reply.usage.cache_read > 0),
        cost_usd: reply.cost_usd.unwrap_or(0.0),
    };
    Ok(Drafted { written, models: Some(ModelUse { jev: Usage::default(), claude: usage }) })
}

/// The configured backend may reach the gateway adapter: `off` (or `models = "off"`) turns every
/// call off, and `session` arrives with the session backend, so both are `model_unavailable`.
fn backend(config: &crate::config::Config) -> Result<(), AppError> {
    let unavailable = |key: &str| {
        AppError::new(ErrorCode::ModelUnavailable, &[("provider", &"claude")])
            .details(json!({"provider": "claude"}))
            .with_message(text::get(key, &[]))
    };
    match (config.models.as_str(), config.claude.backend.as_str()) {
        ("off", _) | (_, "off") => Err(unavailable("models.off")),
        (_, "session") => Err(unavailable("models.session_later")),
        _ => Ok(()),
    }
}

/// A parent cycle names the request field it came from: the first of the proposal's items, in
/// its order, that the cycle runs through, since an item can keep a parent the proposal doesn't name.
fn cycle_field(e: AppError, proposal: &PlanProposal) -> AppError {
    if e.code != ErrorCode::InvalidArgument || e.details.get("field") != Some(&json!("parent")) {
        return e;
    }
    let Some(cycle) = e.details.get("cycle").and_then(Value::as_array) else { return e };
    let on_cycle =
        |key: &str| cycle.iter().filter_map(Value::as_str).any(|id| key_of(&NodeId(id.to_owned())) == key);
    let Some(i) = proposal.items.iter().position(|item| on_cycle(&item.key)) else { return e };
    let mut details = e.details.clone();
    details["field"] = json!(format!("items[{i}].parent"));
    e.details(details)
}

/// At most [`FILE_MAX`] bytes from `input`; more is `invalid_argument`.
fn bounded(input: &mut dyn Read) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    input.take(FILE_MAX as u64 + 1).read_to_end(&mut bytes).map_err(AppError::internal)?;
    if bytes.len() > FILE_MAX {
        return Err(AppError::invalid_argument("file", "intent.file_large"));
    }
    Ok(bytes)
}

fn read_file(path: &Path) -> Result<Vec<u8>, AppError> {
    let unreadable = || AppError::invalid_argument("file", "intent.file_unreadable");
    // A plain file only: opening a FIFO or a device would wait or read without end.
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(unreadable());
    }
    let mut file = std::fs::File::open(path).map_err(|_| unreadable())?;
    bounded(&mut file)
}

/// The proposal in `bytes`, or `invalid_argument` naming the line, the column and the JSON path
/// where it stops fitting, never quoting it.
fn parse(bytes: &[u8]) -> Result<PlanProposal, AppError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| {
        let what = text::get("intent.parse", &[("line", &e.line()), ("column", &e.column())]);
        AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
            .details(json!({"field": "file", "line": e.line(), "column": e.column(), "path": "$"}))
    })?;
    let typed = serde_json::from_slice::<PlanProposal>(bytes);
    let misfit = shape(&value).or_else(|| typed.is_err().then(|| ("$".to_owned(), "intent.shape_object")));
    let Some((path, why)) = misfit else {
        return typed.map_err(AppError::internal);
    };
    let (line, column) = typed.as_ref().err().map_or((1, 1), |e| (e.line(), e.column()));
    let why = text::get(why, &[]);
    let vars: &[(&str, &dyn std::fmt::Display)] =
        &[("path", &path), ("line", &line), ("column", &column), ("why", &why)];
    Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get("intent.shape", vars))])
        .details(json!({"field": "file", "line": line, "column": column, "path": path})))
}

/// Where `value` first stops fitting [`PlanProposal`], and why: its JSON path and the line's key.
fn shape(value: &Value) -> Option<(String, &'static str)> {
    fn keys(
        obj: &Map<String, Value>,
        at: &str,
        allowed: &[&str],
        required: &[&str],
    ) -> Option<(String, &'static str)> {
        if obj.keys().any(|k| !allowed.contains(&k.as_str())) {
            return Some((at.to_owned(), "intent.shape_unknown"));
        }
        required.iter().find(|k| !obj.contains_key(**k)).map(|_| (at.to_owned(), "intent.shape_missing"))
    }
    let text = |v: Option<&Value>| v.is_none_or(|v| v.is_string() || v.is_null());
    let Some(top) = value.as_object() else { return Some(("$".to_owned(), "intent.shape_object")) };
    if let Some(bad) = keys(top, "$", &["project", "stage", "items"], &["items"]) {
        return Some(bad);
    }
    for key in ["project", "stage"] {
        if !text(top.get(key)) {
            return Some((key.to_owned(), "intent.shape_string"));
        }
    }
    let Some(items) = top["items"].as_array() else {
        return Some(("items".to_owned(), "intent.shape_array"));
    };
    for (i, item) in items.iter().enumerate() {
        let at = format!("items[{i}]");
        let Some(obj) = item.as_object() else { return Some((at, "intent.shape_object")) };
        let allowed = ["key", "type", "name", "parent", "props", "touches"];
        if let Some(bad) = keys(obj, &at, &allowed, &["key", "type", "name"]) {
            return Some(bad);
        }
        for key in ["key", "type", "name"] {
            if !obj[key].is_string() {
                return Some((format!("{at}.{key}"), "intent.shape_string"));
            }
        }
        if !text(obj.get("parent")) {
            return Some((format!("{at}.parent"), "intent.shape_string"));
        }
        if obj.get("props").is_some_and(|p| !p.is_object()) {
            return Some((format!("{at}.props"), "intent.shape_object"));
        }
        if obj.get("touches").is_some_and(|t| !t.as_array().is_some_and(|t| t.iter().all(Value::is_string))) {
            return Some((format!("{at}.touches"), "intent.shape_strings"));
        }
    }
    None
}

fn invalid(field: &str, key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> AppError {
    AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get(key, vars))])
        .details(json!({"field": field}))
}

/// Check a proposal against the loaded stages, before any store is read: its bounds, its keys,
/// each item's type, in the named stage when it names one, and each prop against its type's
/// schema. A prop the schema doesn't declare is named by its item, never by its own key.
fn check_items(proposal: &PlanProposal, stages: &[StageDecl]) -> Result<(), AppError> {
    let types = item_types(stages);
    let all_types: Vec<&str> = types.keys().map(String::as_str).collect();
    let stage = match &proposal.stage {
        Some(id) => Some(stages.iter().find(|s| &s.id == id).ok_or_else(|| {
            let ids: Vec<&str> = stages.iter().map(|s| s.id.as_str()).collect();
            invalid("stage", "intent.stage_unknown", &[("stage", &q(id)), ("stages", &ids.join(", "))])
        })?),
        None => None,
    };
    if proposal.items.len() > ITEMS_MAX {
        return Err(AppError::invalid_argument("items", "intent.too_many"));
    }
    let mut keys = BTreeSet::new();
    for (i, item) in proposal.items.iter().enumerate() {
        let at = format!("items[{i}]");
        if !plan::key_ok(&item.key) {
            return Err(AppError::invalid_argument(&format!("{at}.key"), "intent.key"));
        }
        if !keys.insert(item.key.as_str()) {
            return Err(invalid(&format!("{at}.key"), "intent.key_twice", &[("key", &item.key)]));
        }
        let name = item.name.trim();
        if name.is_empty() || name.chars().count() > NAME_MAX {
            return Err(AppError::invalid_argument(&format!("{at}.name"), "intent.name"));
        }
        let Some(props) = types.get(&item.item_type) else {
            let vars: &[(&str, &dyn std::fmt::Display)] =
                &[("type", &q(&item.item_type)), ("types", &all_types.join(", "))];
            return Err(invalid(&format!("{at}.type"), "intent.type_unknown", vars));
        };
        if let Some(stage) = stage
            && !stage.items.iter().any(|t| t.id == item.item_type)
        {
            let writes: Vec<&str> = stage.items.iter().map(|t| t.id.as_str()).collect();
            let vars: &[(&str, &dyn std::fmt::Display)] =
                &[("stage", &stage.id), ("types", &writes.join(", ")), ("type", &item.item_type)];
            // A stage that only lints writes no item type, so its line says that, not a list.
            let key =
                if writes.is_empty() { "intent.stage_writes_nothing" } else { "intent.type_not_in_stage" };
            return Err(invalid(&format!("{at}.type"), key, vars));
        }
        let schema = json!({"type": "object", "properties": props});
        for (prop, value) in &item.props {
            if prop == plan::TYPE {
                return Err(AppError::invalid_argument(&format!("{at}.props.{prop}"), "intent.prop_type"));
            }
            if !props.contains_key(prop) {
                let declared: Vec<&str> = props.keys().map(String::as_str).collect();
                let vars: &[(&str, &dyn std::fmt::Display)] =
                    &[("type", &item.item_type), ("props", &declared.join(", "))];
                return Err(invalid(&format!("{at}.props"), "intent.prop_unknown", vars));
            }
            let field = format!("{at}.props.{prop}");
            if too_long(value) {
                return Err(AppError::invalid_argument(&field, "intent.prop_long"));
            }
            if let Some(keyword) = planroom_extract::declarative::breaks(&schema, prop, value) {
                let vars: &[(&str, &dyn std::fmt::Display)] =
                    &[("prop", prop), ("type", &item.item_type), ("keyword", &keyword)];
                return Err(invalid(&field, "intent.prop_breaks", vars));
            }
        }
        if item.touches.len() > TOUCHES_MAX {
            return Err(AppError::invalid_argument(&format!("{at}.touches"), "intent.touches_many"));
        }
    }
    Ok(())
}

/// Whether a prop's value passes the text or list bounds, at any depth.
fn too_long(value: &Value) -> bool {
    match value {
        Value::String(s) => s.chars().count() > STRING_MAX,
        Value::Array(items) => items.len() > LIST_MAX || items.iter().any(too_long),
        Value::Object(map) => map.len() > LIST_MAX || map.values().any(too_long),
        _ => false,
    }
}

/// An item's key: its ID after the project's prefix.
pub(crate) fn key_of(id: &NodeId) -> &str {
    id.0.rsplit_once(':').map_or(id.0.as_str(), |(_, key)| key)
}

/// Plan a proposal inside the write's transaction: its project, each item's parent and touch
/// targets resolved against the commit's own snapshot, then the edits and their compare-and-set.
fn plan_proposal(
    proposal: &PlanProposal,
    flag: Option<&NodeId>,
    snapshot: &Snapshot<'_>,
    folded: &Folded,
    writer: &Writer,
    base: Seq,
    vocabulary: &Vocabulary,
) -> Result<Plan, AppError> {
    let projects = plan::projects(folded);
    let project = match (&proposal.project, flag) {
        (Some(named), flag) => {
            let found = current(&projects, Some(named))?.ok_or_else(no_project)?;
            if let Some(flag) = flag.filter(|f| **f != found.id) {
                let vars: &[(&str, &dyn std::fmt::Display)] =
                    &[("named", &found.id.0), ("flag", &q(&flag.0))];
                return Err(invalid("project", "intent.project_mismatch", vars));
            }
            found
        }
        (None, Some(flag)) => current(&projects, Some(&flag.0))?.ok_or_else(no_project)?,
        (None, None) => current(&projects, None)?.ok_or_else(no_project)?,
    };
    let person = matches!(writer.actor, Actor::Person { .. });
    let seq = snapshot.seq();
    if person && !project.live {
        let ghost = project.pending.first().cloned().unwrap_or_else(|| EventId(String::new()));
        return Err(waits("project", "intent.project_ghost", &project.id, &ghost, seq));
    }
    let slug = plan::slug_of(&project.id).ok_or_else(|| AppError::internal("a project ID with no slug"))?;
    let items = plan::items_of(folded, &project.id);
    let batch: BTreeSet<&str> = proposal.items.iter().map(|i| i.key.as_str()).collect();
    let mut parents = BTreeMap::new();
    for (i, item) in proposal.items.iter().enumerate() {
        let Some(given) = item.parent.as_deref() else { continue };
        let key = if batch.contains(given) {
            given.to_owned()
        } else if let Some(found) = items.iter().find(|it| it.id.0 == given || key_of(&it.id) == given) {
            // An agent may build on another proposal; a person's item goes under what is accepted.
            if !found.live && person {
                let ghost = found.pending.first().cloned().unwrap_or_else(|| EventId(String::new()));
                return Err(waits(
                    &format!("items[{i}].parent"),
                    "intent.parent_ghost",
                    &found.id,
                    &ghost,
                    seq,
                ));
            }
            key_of(&found.id).to_owned()
        } else if given == plan::PROJECT_KEY || given == project.id.0 {
            // Named, so an existing item moves to the project; an absent parent keeps its place.
            parents.insert(item.key.clone(), project.id.clone());
            continue;
        } else {
            let vars: &[(&str, &dyn std::fmt::Display)] =
                &[("parent", &q(given)), ("project", &q(&project.name))];
            let what = text::get("intent.parent_missing", vars);
            let closest: Vec<&str> = items.iter().take(CANDIDATES).map(|it| it.id.0.as_str()).collect();
            return Err(AppError::new(ErrorCode::NotFound, &[("what", &what)])
                .details(json!({"field": format!("items[{i}].parent"), "closest": closest})));
        };
        parents.insert(item.key.clone(), plan::item_id(slug, &key));
    }
    // Touch targets are blocks of the map, never plan items: nothing at a level apart from the
    // ladder.
    let ladder = Ladder::of(vocabulary);
    let pool: Vec<Node> = folded.model.nodes.iter().filter(|n| !ladder.is_apart(&n.level)).cloned().collect();
    let mut touches: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
    for item in &proposal.items {
        for given in &item.touches {
            let id = match lookup::lookup(&pool, given, ladder.finest(), &ladder) {
                Lookup::Found(node) => node.id.clone(),
                Lookup::Missing(closest) => {
                    let what = text::get("intent.touch_missing", &[("target", &q(given))]);
                    let closest: Vec<&str> = closest.iter().map(|n| n.id.0.as_str()).collect();
                    return Err(AppError::new(ErrorCode::NotFound, &[("what", &what)])
                        .details(json!({"closest": closest})));
                }
                Lookup::Ambiguous(several) => {
                    return Err(ambiguous(given, several.iter().map(|n| &n.id).collect()));
                }
            };
            let entry = touches.entry(item.key.clone()).or_default();
            if !entry.contains(&id) {
                entry.push(id);
            }
        }
    }
    let edits = plan::edits_for(proposal, &project.id, &parents, &touches, folded, base);
    if edits.is_empty() {
        return Err(AppError::invalid_argument("items", "intent.nothing_new"));
    }
    let check = cas::check_edits(folded, &proposed(writer, base), &edits);
    let targets: Vec<String> = proposal.items.iter().map(|i| plan::item_id(slug, &i.key).0).collect();
    let names = proposal.items.iter().map(|i| (plan::item_id(slug, &i.key).0, i.name.clone())).collect();
    let shown: Vec<String> = targets.iter().take(3).map(|id| q(id)).collect();
    let key = if proposal.items.len() == 1 { "intent.proposed_one" } else { "intent.proposed" };
    let line = text::get(
        key,
        &[("count", &proposal.items.len()), ("project", &q(&project.name)), ("ids", &shown.join(", "))],
    );
    let bodies = edits.into_iter().map(|e| (Some(edit::target_id(&e.target)), EventBody::Edit(e))).collect();
    Ok(Plan { bodies, targets, check, line, names, closes: vec![], project: Some(project.id.clone()) })
}

// ---------------------------------------------------------------------------------------------
// Reads

/// The root of the workspace `cwd` is in and its store, to read; `None` before the first scan.
fn open_read(cwd: &Path) -> Result<Option<(PathBuf, Store)>, AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    Ok(Store::open_existing(&workspace::store_path(&root))?.map(|store| (root, store)))
}

/// No plan yet: the line gives `intent new`'s usage, and the handle shows how to start one.
pub(crate) fn no_intent() -> Empty {
    Empty {
        reason: EmptyReason("no_intent".into()),
        line: Some(text::get("empty.no_intent_plan", &[])),
        next: Handle {
            command: CommandName("schema".into()),
            args: vec!["--command".into(), "intent new".into()],
            label: text::label("empty.no_intent_next", &[]),
        },
    }
}

/// `ask stage STAGE PROJECT`: a stage's checklist, for the project.
pub(crate) fn stage_handle(stage: &str, project: &NodeId) -> Handle {
    Handle {
        command: CommandName("ask".into()),
        args: vec!["stage".into(), stage.to_owned(), project.0.clone()],
        label: text::label("intent.stage_label", &[]),
    }
}

/// The review that accepts the ghost `event`, at `seq`.
pub(crate) fn review_handle(event: &EventId, seq: Seq) -> Handle {
    Handle {
        command: CommandName("review".into()),
        args: vec!["--base".into(), seq.to_string(), event.0.clone(), "accept".into()],
        label: text::label("intent.review_label", &[]),
    }
}

/// The lint findings of the rules `keep` keeps, over `project`'s items in the model, in rule
/// order, then by subject; each with the stage whose lint runs its rule first.
pub(crate) fn findings(
    loaded: &Loaded,
    folded: &Folded,
    project: &NodeId,
    keep: impl Fn(&RuleDecl) -> bool,
) -> Vec<LintFinding> {
    let stages = stages(loaded);
    let rules = plan_rules(loaded, keep);
    let found = planroom_kernel::lint::lint(&folded.model, &rules, |n| plan::in_project(&n.id, project));
    let names: BTreeMap<&str, &str> =
        folded.model.nodes.iter().map(|n| (n.id.0.as_str(), n.name.as_str())).collect();
    found
        .into_iter()
        .map(|f| {
            let rule = rules[f.rule];
            let stage = stages.iter().find(|s| s.lint.contains(&rule.id)).map(|s| s.id.clone());
            LintFinding {
                rule: rule.id.clone(),
                stage: stage.clone(),
                name: q(names.get(f.subject.0.as_str()).copied().unwrap_or(&f.subject.0)),
                subject: f.subject,
                message: text::short(&rule.message, 200),
                values: f.values.iter().take(VALUES_MAX).map(|v| text::short(v, VALUE_CHARS)).collect(),
                related: f.related.into_iter().take(VALUES_MAX).collect(),
                handles: stage.iter().map(|s| stage_handle(s, project)).collect(),
            }
        })
        .collect()
}

/// The tokens a reply carrying `result` takes as it is sent: the result redacted or raw, whichever
/// is longer, its summary again, which the envelope repeats, and the envelope's own fields.
pub(crate) fn sent_tokens<T: Serialize + DeserializeOwned + Clone>(result: &T, summary: &str) -> usize {
    let mut redacted = result.clone();
    planroom_kernel::redact::output(&mut redacted);
    let bytes = |r: &T| serde_json::to_string(r).map_or(usize::MAX, |s| s.len());
    (bytes(result).max(bytes(&redacted)) + summary.len()).div_ceil(4) + ENVELOPE_TOKENS
}

/// The largest `k` in `0..=n` for which `fits(k)` holds, given it holds for 0 and fails past some
/// point: found by halving.
fn longest(n: usize, fits: impl Fn(usize) -> bool) -> usize {
    let (mut fit, mut over) = (0, n);
    while fit < over {
        let mid = (fit + over).div_ceil(2);
        if fits(mid) { fit = mid } else { over = mid - 1 }
    }
    fit
}

fn budget(command: Command) -> usize {
    command.spec().budget_tokens as usize
}

/// `planroom intent lint [--project P]`: the project's findings, within the leaf's budget.
pub fn lint(req: &LintReq, cwd: &Path) -> Result<Reply<LintResult>, AppError> {
    let empty = |empty: Empty, project: Option<NodeId>, seq: Seq| {
        let summary = empty.line.clone().unwrap_or_default();
        let result = LintResult { project, summary: summary.clone(), findings: vec![], empty: Some(empty) };
        Reply { result, seq, warnings: vec![], text: summary }
    };
    let Some((root, store)) = open_read(cwd)? else { return Ok(empty(no_intent(), None, 0)) };
    let snapshot = store.snapshot()?;
    let seq = snapshot.seq();
    let set = state::for_snapshot(&snapshot, &root)?;
    let empty = |e: Empty, project: Option<NodeId>, seq: Seq| {
        let mut reply = empty(e, project, seq);
        set.warn(&mut reply.warnings);
        reply
    };
    let folded = live::folded(&snapshot, set.vocabulary())?;
    let projects = plan::projects(&folded);
    let Some(project) = current(&projects, req.project.as_deref())? else {
        return Ok(empty(no_intent(), None, seq));
    };
    let all = findings(&set, &folded, &project.id, |_| true);
    if all.is_empty() {
        let none = Empty {
            reason: EmptyReason("no_findings".into()),
            line: Some(text::get("empty.no_findings", &[("project", &q(&project.name))])),
            next: Handle {
                command: CommandName("intent list".into()),
                args: vec!["--project".into(), project.id.0.clone()],
                label: text::label("empty.no_findings_next", &[]),
            },
        };
        return Ok(empty(none, Some(project.id.clone()), seq));
    }
    let total = all.len();
    let result = |found: &[LintFinding]| {
        let rules: Vec<&str> =
            found.iter().map(|f| f.rule.as_str()).collect::<BTreeSet<_>>().into_iter().take(3).collect();
        let key = if total == 1 { "intent.lint_one" } else { "intent.lint" };
        let summary = text::get(
            key,
            &[("project", &q(&project.name)), ("count", &total), ("rules", &rules.join(", "))],
        );
        LintResult { project: Some(project.id.clone()), summary, findings: found.to_vec(), empty: None }
    };
    let budget = budget(Command::IntentLint(LintReq::default()));
    let whole = result(&all);
    let (result, mut warnings) = if sent_tokens(&whole, &whole.summary) <= budget {
        (whole, vec![])
    } else {
        let room = budget.saturating_sub(WARNING_TOKENS);
        let kept = longest(all.len(), |k| {
            let r = result(&all[..k]);
            sent_tokens(&r, &r.summary) <= room
        });
        let warning = Warning {
            code: WarningCode::Truncated,
            message: text::get("intent.lint_cut", &[("kept", &kept), ("total", &total), ("budget", &budget)]),
            details: json!({"kept": kept, "total": total, "budget_tokens": budget}),
        };
        (result(&all[..kept]), vec![warning])
    };
    let mut text = result.summary.clone();
    for f in &result.findings {
        text.push_str(&format!("\n  {} · {} · {}", f.subject.0, f.rule, f.message));
    }
    set.warn(&mut warnings);
    Ok(Reply { result, seq, warnings, text })
}

/// One entry of a listing page: a project or an item.
#[derive(Debug, Clone)]
enum Entry {
    Project(PlanProject),
    Item(PlanItem),
}

impl Entry {
    fn id(&self) -> &str {
        match self {
            Entry::Project(p) => &p.id.0,
            Entry::Item(i) => &i.id.0,
        }
    }

    /// The entry with its lists cut to `max` each and an item's touches to `touches`, so even one
    /// that grew large fits a page.
    fn cut(&self, max: usize, touches: usize) -> Entry {
        match self {
            Entry::Project(p) => {
                let mut p = p.clone();
                let skip = p.versions.len().saturating_sub(max);
                p.versions.drain(..skip);
                Entry::Project(p)
            }
            Entry::Item(i) => {
                let mut i = i.clone();
                i.touches.truncate(max.min(touches));
                i.pending.truncate(max);
                i.handles.truncate(max.min(REVIEWS_MAX));
                Entry::Item(i)
            }
        }
    }
}

/// Where a listing page ends: its last entry's place and the start of its ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    at: usize,
    id: String,
}

fn encode(mark: &Mark) -> Cursor {
    let json = serde_json::to_vec(mark).expect("a mark serializes");
    Cursor(json.iter().map(|b| format!("{b:02x}")).collect())
}

fn decode(cursor: &Cursor) -> Result<Mark, AppError> {
    let unreadable = || AppError::invalid_argument("cursor", "intent.cursor_unreadable");
    let hex = cursor.0.as_bytes();
    if !hex.len().is_multiple_of(2) {
        return Err(unreadable());
    }
    let bytes: Option<Vec<u8>> =
        hex.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect();
    serde_json::from_slice(&bytes.ok_or_else(unreadable)?).map_err(|_| unreadable())
}

/// The place after `mark` among `entries`: after the entry it names, or, when the listing moved,
/// after the first entry whose ID it starts, or else from its place.
fn after(mark: &Mark, entries: &[Entry]) -> usize {
    match entries.get(mark.at) {
        Some(e) if e.id().starts_with(&mark.id) => mark.at + 1,
        _ => entries.iter().position(|e| e.id().starts_with(&mark.id)).map_or(mark.at + 1, |at| at + 1),
    }
}

/// `planroom intent list [--project P] [--cursor C]`: every project, then the named or only
/// project's items, paged within the leaf's budget.
pub fn list(req: &IntentListReq, cwd: &Path) -> Result<Paged<IntentListResult>, AppError> {
    let empty_reply = |empty: Empty, projects: Vec<PlanProject>, seq: Seq| {
        let summary = empty.line.clone().unwrap_or_default();
        let result =
            IntentListResult { summary: summary.clone(), projects, items: vec![], empty: Some(empty) };
        Paged::whole(Reply { result, seq, warnings: vec![], text: summary })
    };
    let Some((root, store)) = open_read(cwd)? else { return Ok(empty_reply(no_intent(), vec![], 0)) };
    let snapshot = store.snapshot()?;
    let seq = snapshot.seq();
    let set = state::for_snapshot(&snapshot, &root)?;
    let empty_reply = |empty: Empty, projects: Vec<PlanProject>, seq: Seq| {
        let mut paged = empty_reply(empty, projects, seq);
        set.warn(&mut paged.reply.warnings);
        paged
    };
    let folded = live::folded(&snapshot, set.vocabulary())?;
    let projects = plan::projects(&folded);
    if projects.is_empty() {
        return Ok(empty_reply(no_intent(), vec![], seq));
    }
    let chosen = match &req.project {
        Some(given) => current(&projects, Some(given))?,
        None if projects.len() == 1 => projects.first(),
        None => None,
    };
    let stages = stages(&set);
    let listed: Vec<PlanProject> = projects.iter().map(|p| plan_project(p, &folded)).collect();
    let mut items: Vec<Item> = chosen.map(|p| plan::items_of(&folded, &p.id)).unwrap_or_default();
    plan::order(&mut items, &stages);
    if let Some(project) = chosen
        && items.is_empty()
    {
        // With no items, the next stage is the first: the stages come from the plan pack.
        let Some(next) = plan::next_stage(&stages, &items).map(|s| s.id.clone()) else {
            return Ok(empty_reply(no_intent(), listed, seq));
        };
        let none = Empty {
            reason: EmptyReason("no_plan_items".into()),
            line: Some(text::get("empty.no_plan_items", &[("project", &q(&project.name)), ("stage", &next)])),
            next: Handle {
                command: CommandName("ask".into()),
                args: vec!["stage".into(), next.clone(), project.id.0.clone()],
                label: text::label("empty.no_plan_items_next", &[]),
            },
        };
        return Ok(empty_reply(none, listed, seq));
    }
    let entries: Vec<Entry> = listed
        .iter()
        .cloned()
        .map(Entry::Project)
        .chain(items.iter().map(|i| Entry::Item(plan_item(i, &stages, seq))))
        .collect();
    let summary = match chosen {
        Some(project) => {
            let waiting = items.iter().filter(|i| !i.pending.is_empty()).count();
            let key = if items.len() == 1 { "intent.list_items_one" } else { "intent.list_items" };
            text::get(key, &[("project", &q(&project.name)), ("items", &items.len()), ("ghosts", &waiting)])
        }
        None => {
            let names: Vec<String> = projects.iter().take(5).map(|p| q(&p.name)).collect();
            text::get("intent.list", &[("count", &projects.len()), ("names", &names.join(", "))])
        }
    };
    let from = match &req.cursor {
        Some(cursor) => after(&decode(cursor)?, &entries).min(entries.len()),
        None => 0,
    };
    let budget = budget(Command::IntentList(IntentListReq::default()));
    // An entry too large for a page alone is cut: its lists to 3 each, then its touches, whose
    // block IDs a repo's paths make as long as they are, to 1 and then none. An item's own ID,
    // name and handles are bounded, so the last cut always fits.
    let fits = |e: &Entry| {
        sent_tokens(&page_of(&summary, std::slice::from_ref(e)), &summary) + WARNING_TOKENS <= budget
    };
    // Each entry with whether it was cut, so the page can say how many of its entries it cut.
    let (rest, cut): (Vec<Entry>, Vec<bool>) = entries[from..]
        .iter()
        .map(|e| {
            if fits(e) {
                return (e.clone(), false);
            }
            let cuts = [e.cut(3, 3), e.cut(3, 1)];
            (cuts.into_iter().find(|c| fits(c)).unwrap_or_else(|| e.cut(3, 0)), true)
        })
        .unzip();
    let whole = page_of(&summary, &rest);
    let (page, kept) = if sent_tokens(&whole, &summary) <= budget {
        (whole, rest.len())
    } else {
        let room = budget.saturating_sub(WARNING_TOKENS);
        let kept =
            longest(rest.len(), |k| sent_tokens(&page_of(&summary, &rest[..k]), &summary) <= room).max(1);
        (page_of(&summary, &rest[..kept]), kept)
    };
    let next = (from + kept < entries.len()).then(|| {
        let last = &entries[from + kept - 1];
        encode(&Mark { at: from + kept - 1, id: last.id().chars().take(MARK_CHARS).collect() })
    });
    // The page says how many of its entries it cut, as `ask`'s listing counts what it leaves out,
    // so a cut item is never read as touching fewer blocks than it does.
    let entries_cut = cut[..kept].iter().filter(|c| **c).count();
    let mut warnings = match &next {
        Some(cursor) => {
            // The project by its resolved ID, never the text the caller named it by: a handle's
            // args are never masked, and a name may hold what redaction masks.
            let mut args = vec![];
            if let Some(project) = chosen.filter(|_| req.project.is_some()) {
                args.extend(["--project".to_owned(), project.id.0.clone()]);
            }
            args.extend(["--cursor".to_owned(), cursor.0.clone()]);
            let handle = Handle {
                command: CommandName("intent list".into()),
                args,
                label: text::label("intent.next_label", &[]),
            };
            let mut details =
                json!({"kept": kept, "total": entries.len(), "budget_tokens": budget, "next": handle});
            if entries_cut > 0 {
                details["entries_cut"] = json!(entries_cut);
            }
            let key = if entries_cut > 0 { "intent.paged_cut" } else { "intent.paged" };
            vec![Warning {
                code: WarningCode::Truncated,
                message: text::get(
                    key,
                    &[
                        ("kept", &kept),
                        ("total", &entries.len()),
                        ("budget", &budget),
                        ("cut", &entries_cut),
                        ("cursor", &cursor.0),
                    ],
                ),
                details,
            }]
        }
        None if entries_cut > 0 => vec![Warning {
            code: WarningCode::Truncated,
            message: text::get(
                "intent.cut",
                &[("total", &entries.len()), ("budget", &budget), ("cut", &entries_cut)],
            ),
            details: json!({"kept": kept, "total": entries.len(), "budget_tokens": budget, "entries_cut": entries_cut}),
        }],
        None => vec![],
    };
    let mut text = page.summary.clone();
    for p in &page.projects {
        text.push_str(&format!("\n  {} · {} · {} items", p.id.0, p.name, p.items));
    }
    for i in &page.items {
        let status = if i.status == Status::Ghost { "ghost" } else { "live" };
        text.push_str(&format!("\n  {} · {} · {} · {status}", i.id.0, i.item_type, i.name));
    }
    set.warn(&mut warnings);
    Ok(Paged { reply: Reply { result: page, seq, warnings, text }, next, dirty: Default::default() })
}

fn page_of(summary: &str, entries: &[Entry]) -> IntentListResult {
    let mut projects = vec![];
    let mut items = vec![];
    for entry in entries {
        match entry {
            Entry::Project(p) => projects.push(p.clone()),
            Entry::Item(i) => items.push(i.clone()),
        }
    }
    IntentListResult { summary: summary.to_owned(), projects, items, empty: None }
}

fn plan_project(project: &Project, folded: &Folded) -> PlanProject {
    let items = plan::items_of(folded, &project.id);
    let ghosts = items.iter().map(|i| i.pending.len()).sum::<usize>() + project.pending.len();
    PlanProject {
        id: project.id.clone(),
        name: q(&project.name),
        status: if project.live { Status::Applied } else { Status::Ghost },
        items: items.iter().filter(|i| i.live).count() as u32,
        ghosts: ghosts as u32,
        versions: folded
            .versions
            .iter()
            .filter(|v| v.project.as_ref() == Some(&project.id))
            .map(|v| PlanVersion { label: v.label.clone(), seq: v.seq })
            .collect(),
    }
}

pub(crate) fn plan_item(item: &Item, stages: &[StageDecl], seq: Seq) -> PlanItem {
    PlanItem {
        id: item.id.clone(),
        name: q(&item.name),
        item_type: item.item_type.clone(),
        stage: plan::stage_of(stages, &item.item_type).map(|s| s.id.clone()),
        status: if item.live { Status::Applied } else { Status::Ghost },
        parent: item.parent.clone(),
        touches: item.touches.clone(),
        pending: item.pending.clone(),
        handles: item.pending.iter().take(REVIEWS_MAX).map(|e| review_handle(e, seq)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proposal_that_misfits_names_where_and_never_quotes_it() {
        let e = parse(br#"{"items": [{"key": "a", "type": "work", "name": "x", "secret": "hunter2"}]}"#)
            .unwrap_err();
        assert_eq!(e.details["path"], "items[0]");
        assert!(!e.message.contains("hunter2") && !e.message.contains("secret"), "{}", e.message);
        let e = parse(b"{\n  \"items\": [hunter2").unwrap_err();
        assert_eq!((e.details["line"].as_u64(), e.details["path"].as_str()), (Some(2), Some("$")));
        assert!(!e.message.contains("hunter2"), "{}", e.message);
        let e = parse(br#"{"items": [{"key": 1, "type": "work", "name": "x"}]}"#).unwrap_err();
        assert_eq!(e.details["path"], "items[0].key");
        assert!(parse(br#"{"project": null, "items": []}"#).is_ok());
    }

    #[test]
    fn every_intent_output_is_masked_by_its_marks() {
        use planroom_kernel::redact::is_listed;
        assert!(is_listed::<IntentListResult>() && is_listed::<LintResult>());
        assert!(is_listed::<PlanItem>() && is_listed::<PlanProject>() && is_listed::<LintFinding>());
    }

    #[test]
    fn a_proposal_over_its_bounds_is_refused_before_it_is_read_whole() {
        let mut big = std::io::repeat(b' ').take(FILE_MAX as u64 * 4);
        assert_eq!(bounded(&mut big).unwrap_err().details["field"], "file");
        assert!(too_long(&json!("x".repeat(STRING_MAX + 1))));
        assert!(too_long(&json!(vec!["a"; LIST_MAX + 1])));
        assert!(!too_long(&json!(["a", "b"])));
    }
}
