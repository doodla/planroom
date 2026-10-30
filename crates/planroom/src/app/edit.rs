//! `planroom edit OP ...`: one hand edit, or the undo of a whole command, from the CLI or over
//! HTTP (architecture.md, Writes; DR-3, DR-5, DR-7, DR-8, DR-61, DR-73).
//!
//! Each op becomes edits of the three operations (`put`, `set`, `retract`) on stable IDs, or one
//! `undo` event. They are planned inside the write's own IMMEDIATE transaction
//! ([`Store::commit_planned`]): the store folds as the commit will find it, resolves the targets,
//! and the kernel's compare-and-set ([`cas`]) says how the events are stored. A parent change that
//! would close a loop is refused with `invalid_argument` naming it; a field another writer instance
//! set after `base` stores the write as a conflict and returns `stale_write`; a person's write
//! applies, and an agent's or a model's is a ghost until a person accepts it. An undo reverses
//! every event of the command that wrote the event it names, and is a write like any other.
//!
//! Every write carries `base`. Its key is the caller's `--idem`, or one derived from the actor, the
//! op and the base (DR-8), and its request hash covers the op, the base, the project, the actor and
//! the client, never a transport field (DR-73). A command's events are keyed `KEY#1`, `KEY#2` and
//! so on, and the whole reply is stored on the first, so a retry returns it whole.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use planroom_contract::{
    Actor, Client, CommandName, CommandSpec, Edge, Edit, EditOp, EditReq, EditResult, ErrorCode, Event,
    EventBody, EventId, Handle, Kind, Layer, Node, NodeId, Op, Seq, Status, Surface, Target, Verdict,
    Vocabulary,
};
use planroom_kernel::cas::{self, Authority, Check, Proposed};
use planroom_kernel::fold::{Folded, HIDDEN, NAME, PARENT, POS_PREFIX, Writer, actor_key};
use planroom_kernel::keys;
use planroom_kernel::ladder::Ladder;
use planroom_store::{NewEvent, Planned, Snapshot, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::actor::{self, Bound, Caller, Terminal};
use crate::app::error::AppError;
use crate::app::{Reply, cards, new_event_id, workspace};
use crate::packs::state;
use crate::{clock, live, text};

/// The kind of a group made by hand (architecture.md, IDs and aliases).
const GROUP_KIND: &str = "kernel.group";
/// IDs a refusal names at most.
const CANDIDATES: usize = 5;
/// The most bytes of `yours` a `stale_write` carries: a compact three-way diff, at most 1 KB with
/// `theirs` (DR-7), so the error keeps to the leaf's budget.
const YOURS_MAX_BYTES: usize = 600;
/// The most bytes of `theirs` a `stale_write` carries.
pub(crate) const THEIRS_MAX_BYTES: usize = 400;
/// The most bytes of changed IDs a reply lists; `more_targets` counts the rest.
pub(crate) const TARGETS_MAX_BYTES: usize = 500;
/// The most characters of a name or an ID a reply's line quotes.
const QUOTE_CHARS: usize = 80;

/// A write's reply, and whether it replays an earlier commit of the same key: `meta.replayed`,
/// and `meta.replayed_from` when the key was derived, so the caller knows to pass a new `--idem`
/// to write it again (DR-8).
#[derive(Debug, Clone)]
pub struct Written<T> {
    pub reply: Reply<T>,
    pub replayed: bool,
    pub replayed_from: Option<Seq>,
}

/// `planroom edit` from the CLI: the actor the actor rules resolve (`actor::writer`): the agent
/// `PLANROOM_ACTOR` names, a person with a valid `PLANROOM_PERSON_TOKEN`, or a person who confirms
/// the edit on `terminal`; with none of them, `actor_required`.
pub fn run(
    req: &EditReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Written<EditResult>, AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);
    let base = req.common.base.ok_or_else(base_required)?;
    let writer = cli_writer(req, caller, &mut store, &root, base, terminal)?;
    write(req, &mut store, &root, writer, base)
}

/// `POST /api/v1/edit` on `serve`: a signed-in tab writes as the person who serves it, from the
/// client the request names, so a flush of another tab's pending edit keeps the ID of the tab that
/// made it, never the flusher's (DR-73). `idem_header` is the `Idempotency-Key` header, which a
/// body's `idem` overrides.
pub fn run_http(
    req: &EditReq,
    root: &Path,
    idem_header: Option<&str>,
) -> Result<Written<EditResult>, AppError> {
    let mut store = Store::open_existing(&workspace::store_path(root))?
        .ok_or_else(|| AppError::workspace_not_found(root))?;
    crate::app::hook(&mut store);
    let base = req.common.base.ok_or_else(base_required)?;
    let mut req = req.clone();
    if req.common.idem.is_none() {
        req.common.idem = idem_header.map(str::to_owned);
    }
    // Each tab is its own writer instance, so two tabs of one person conflict like two people
    // (DR-5): a write over HTTP names its tab, never a shared default.
    let client = req
        .common
        .client
        .clone()
        .ok_or_else(|| AppError::invalid_argument("client", "edit.client_required"))?;
    let writer =
        Writer { actor: actor::os_person(), client: Client { surface: Surface::Canvas, id: client } };
    write(&req, &mut store, root, writer, base)
}

fn base_required() -> AppError {
    AppError::new(ErrorCode::BaseRequired, &[("command", &"edit")])
}

/// The exact command line that runs `req` at `base` from a terminal, each operand quoted for a
/// POSIX shell, and `--` before the operands when one starts with `-`.
fn terminal_line(req: &EditReq, base: Seq) -> String {
    let mut flags = vec!["--base".to_owned(), base.to_string()];
    if let Some(project) = &req.common.project {
        flags.extend(["--project".to_owned(), project.to_string()]);
    }
    let (op, operands, options): (&str, Vec<&str>, Vec<(&str, String)>) = match &req.op {
        EditOp::Move { id, parent } => ("move", vec![id, parent], vec![]),
        EditOp::Group { name, ids } => {
            ("group", std::iter::once(name).chain(ids).map(String::as_str).collect(), vec![])
        }
        EditOp::Rename { id, name } => ("rename", vec![id, name], vec![]),
        EditOp::Hide { id } => ("hide", vec![id], vec![]),
        EditOp::Pin { id, view, x, y } => {
            ("pin", vec![id], vec![("--view", view.clone()), ("--x", x.to_string()), ("--y", y.to_string())])
        }
        EditOp::Relate { from, to, kind } => ("relate", vec![from, to], vec![("--kind", kind.clone())]),
        EditOp::Unrelate { from, to, kind } => ("unrelate", vec![from, to], vec![("--kind", kind.clone())]),
        EditOp::Forbid { from, to, kind } => {
            ("forbid", vec![from, to], kind.iter().map(|k| ("--kind", k.clone())).collect())
        }
        EditOp::Undo { event } => ("undo", vec![event], vec![]),
    };
    let mut words = vec!["planroom".to_owned(), "edit".to_owned()];
    words.extend(flags);
    words.push(op.to_owned());
    for (flag, value) in options {
        words.extend([flag.to_owned(), text::shell_word(&value)]);
    }
    if operands.iter().any(|o| o.starts_with('-')) {
        words.push("--".to_owned());
    }
    words.extend(operands.into_iter().map(text::shell_word));
    words.join(" ")
}

/// The CLI's writer instance, resolved by the actor rules (DR-4, DR-60): a person's token is checked
/// against the registered key and spent; an agent writes as `PLANROOM_ACTOR` names it, from its own
/// ID unless it names another client, so two calls of one agent are one writer instance and never
/// conflict with each other; and an agent is never a person, whatever else its call carries.
fn cli_writer(
    req: &EditReq,
    caller: &Caller,
    store: &mut Store,
    root: &Path,
    base: Seq,
    terminal: &mut dyn Terminal,
) -> Result<Writer, AppError> {
    let spec = planroom_contract::Command::Edit(req.clone()).spec();
    // A refusal names this very edit as the line a person runs from a terminal.
    let line = terminal_line(req, base);
    writer_for(spec, &req.common, &line, caller, store, root, Some(base), terminal)
}

/// The CLI's writer instance for a write of `spec` whose common fields are `common`, resolved by
/// the actor rules as [`cli_writer`] resolves an edit's: a refusal names `line`, the exact command
/// a person runs from a terminal, and a token binds the request's `idem` and `base`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn writer_for(
    spec: &'static CommandSpec,
    common: &planroom_contract::WriteCommon,
    line: &str,
    caller: &Caller,
    store: &mut Store,
    root: &Path,
    base: Option<Seq>,
    terminal: &mut dyn Terminal,
) -> Result<Writer, AppError> {
    let name = spec.name();
    let keys = actor::registered_keys(store, root, caller, &name)
        .map_err(|e| actor::naming_line(e, &name, actor::TerminalPath::Takes, line))?;
    let now = clock::unix_micros() / 1_000_000;
    let bound = Bound { idem: common.idem.as_deref(), base };
    let path = actor::terminal_path(&name, !keys.is_empty());
    let acting = actor::writer(caller, spec, &bound, &keys.iter().collect::<Vec<_>>(), now, terminal)
        .map_err(|e| actor::naming_line(e, &name, path, line))?;
    if let Some(digest) = &acting.digest {
        actor::spend(store, digest, now, &name, path)
            .map_err(|e| actor::naming_line(e, &name, path, line))?;
    }
    let default = match (&acting.actor, caller.actor.as_deref()) {
        (Actor::Agent { .. }, Some(named)) => named.to_owned(),
        _ => format!("cli:{}", std::process::id()),
    };
    let id = common.client.clone().or_else(|| caller.client.clone()).unwrap_or(default);
    let surface = if id.starts_with("mod:") { Surface::Mod } else { Surface::Cli };
    Ok(Writer { actor: acting.actor, client: Client { surface, id } })
}

/// What a commit stores as the command's reply on its first event: the result and its line, or the
/// conflict the write was stored as, so a retry with the same key returns the same.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stored {
    Done { result: EditResult, text: String },
    Conflict { name: String, seq: Seq, details: Value },
}

/// What a plan decided, inside the write's transaction.
pub(crate) struct Plan {
    pub(crate) bodies: Vec<(Option<String>, EventBody)>,
    pub(crate) targets: Vec<String>,
    pub(crate) check: Check,
    pub(crate) line: String,
    /// The name a conflict's line gives each target.
    pub(crate) names: BTreeMap<String, String>,
    /// The cards an undo closes, each with its review, which the write keys apart from its own
    /// events ([`crate::app::cards::card_review`]).
    pub(crate) closes: Vec<(EventId, Verdict)>,
    /// The project the events carry, when the plan resolved it; else the request's.
    pub(crate) project: Option<NodeId>,
}

fn write(
    req: &EditReq,
    store: &mut Store,
    root: &Path,
    writer: Writer,
    base: Seq,
) -> Result<Written<EditResult>, AppError> {
    // The set `commit` folds with: a cache hit there, whose warning `commit` carries.
    let set = state::for_read(root, store.recorded_packs()?.as_ref())?;
    let vocabulary = set.vocabulary();
    let args = serde_json::to_value(&req.op).map_err(AppError::internal)?;
    let writing = Writing {
        spec: planroom_contract::Command::Edit(req.clone()).spec(),
        args,
        idem: req.common.idem.as_deref(),
        project: req.common.project.clone(),
        writer: writer.clone(),
        base: Some(base),
    };
    commit(store, root, writing, |snapshot, folded, base| {
        plan(&req.op, snapshot, folded, vocabulary, &writer, base)
    })
}

/// One write a leaf commits through [`commit`]: what it is keyed and hashed by, and who writes it.
pub(crate) struct Writing<'a> {
    pub(crate) spec: &'static CommandSpec,
    /// The request's own arguments, as the request hash and a derived key read them: the parsed
    /// request, never a transport field or a file's path.
    pub(crate) args: Value,
    pub(crate) idem: Option<&'a str>,
    /// The project the request names; a plan that resolves its own project names it instead.
    pub(crate) project: Option<NodeId>,
    pub(crate) writer: Writer,
    /// The `seq` of the view the writer acted on; with none, the commit's own snapshot's, so a
    /// write that needs no base still records the view it was checked against.
    pub(crate) base: Option<Seq>,
}

/// The one write path `edit` and the `intent` writes take: `plan` runs inside the write's
/// own IMMEDIATE transaction, over the fold of the store as the commit finds it, with the base the
/// write is checked at, and its events are stored under one key with the reply on the first (DR-5,
/// DR-8, DR-61, DR-73). A conflict is stored and returns `stale_write`; a retry of the key returns
/// the stored reply.
pub(crate) fn commit(
    store: &mut Store,
    root: &Path,
    writing: Writing<'_>,
    plan: impl FnOnce(&Snapshot<'_>, &Folded, Seq) -> Result<Plan, AppError>,
) -> Result<Written<EditResult>, AppError> {
    // The workspace's recorded pack set, resolved before the write's transaction opens, so its
    // trust check never runs under the write lock.
    let set = state::for_read(root, store.recorded_packs()?.as_ref())?;
    let vocabulary = set.vocabulary();
    let Writing { spec, args, idem, project, writer, base } = writing;
    let who = actor_key(&writer.actor);
    let request = json!({
        "args": args, "base": base, "project": project, "actor": who, "client": writer.client,
    });
    let request_hash = keys::request_hash(&spec.name(), &request);
    let keyed = json!({"args": args, "project": project});
    let (idem, derived) = write_key(spec, idem, &who, &keyed, base)?;
    let at = clock::now();
    let planned: RefCell<Option<(Plan, Vec<EventId>, Seq)>> = RefCell::new(None);
    let committed = store.commit_keyed(
        &idem,
        &request_hash,
        |snapshot| {
            // A base past the head names a version this writer can't have read, and compare-and-set
            // would take it as covering every write before it (DR-73).
            if base.is_some_and(|b| b > snapshot.seq()) {
                return Err(AppError::invalid_argument("base", "edit.base_ahead"));
            }
            let base = base.unwrap_or_else(|| snapshot.seq());
            let folded = live::folded(snapshot, vocabulary)?;
            let plan = plan(snapshot, &folded, base)?;
            let status = match &plan.check {
                Check::Cycle { cycle } => return Err(cycle_refused(cycle)),
                Check::Superseded { .. } => {
                    return Err(AppError::internal("a hand edit is never an automatic write"));
                }
                check => check.status().expect("an applied, ghost or conflict write is stored"),
            };
            let project = plan.project.clone().or_else(|| project.clone());
            let events: Vec<NewEvent> = plan
                .bodies
                .iter()
                .map(|(target, body)| NewEvent {
                    id: new_event_id(),
                    at: at.clone(),
                    actor: writer.actor.clone(),
                    client: writer.client.clone(),
                    project: project.clone(),
                    cause: None,
                    status,
                    target: target.clone(),
                    body: body.clone(),
                })
                .collect();
            let ids = events.iter().map(|e| e.id.clone()).collect();
            let mut events: Vec<Planned> = events.into_iter().map(Planned::Command).collect();
            for (card, verdict) in &plan.closes {
                let review = cards::card_review(card, *verdict, &writer, &idem, &request_hash, &at);
                events.push(Planned::Own(review));
            }
            *planned.borrow_mut() = Some((plan, ids, base));
            Ok(events)
        },
        |seqs| {
            let planned = planned.borrow();
            let (plan, ids, base) = planned.as_ref().expect("the plan ran before the reply");
            serde_json::to_value(stored(plan, ids, seqs, *base)).expect("a stored reply serializes")
        },
    )?;
    let first = *committed.seqs.first().ok_or_else(|| AppError::internal("a commit wrote no event"))?;
    let last = committed.seqs.iter().chain(&committed.own).copied().max().expect("one event at least");
    let stored: Stored = serde_json::from_value(committed.reply).map_err(AppError::internal)?;
    match stored {
        Stored::Conflict { name, seq, details } => {
            let event = details["event"].as_str().unwrap_or_default().to_owned();
            Err(AppError::new(ErrorCode::StaleWrite, &[("name", &name), ("seq", &seq)])
                .details(details)
                .with_hint(text::get("hint.stale_write", &[("event", &event)])))
        }
        Stored::Done { result, mut text } => {
            // An undo of a trust card's accept takes the workspace's trust with it (DR-48).
            cards::after_trust_undo(store, &result, root)?;
            let replayed_from = (committed.replayed && derived).then_some(first);
            if replayed_from.is_some() {
                text.push('\n');
                text.push_str(&text::get("edit.replayed", &[]));
            }
            let mut warnings = vec![];
            set.warn(&mut warnings);
            for warning in &warnings {
                text.push('\n');
                text.push_str(&warning.message);
            }
            Ok(Written {
                reply: Reply { result, seq: last, warnings, text },
                replayed: committed.replayed,
                replayed_from,
            })
        }
    }
}

/// The idempotency key a write takes, and whether it was derived (DR-8). The caller's `--idem`
/// comes first; it may not hold `#`, which keys a command's events `KEY#1`, `KEY#2` and so on. With
/// none, a leaf that needs a base derives one from the actor, the leaf, its arguments and the base,
/// which pins the intent, so a retry replays; every other leaf mints a fresh uuid v7 on each
/// invocation, so a repeat runs again instead of replaying the first reply.
pub fn write_key(
    spec: &CommandSpec,
    given: Option<&str>,
    actor: &str,
    args: &Value,
    base: Option<Seq>,
) -> Result<(String, bool), AppError> {
    match given {
        Some(key) if key.is_empty() || key.contains('#') => {
            Err(AppError::invalid_argument("idem", "edit.idem"))
        }
        Some(key) => Ok((key.to_owned(), false)),
        None => match base.filter(|_| spec.needs_base) {
            Some(base) => Ok((keys::derived_idem(actor, &spec.name(), args, base), true)),
            None => Ok((uuid::Uuid::now_v7().to_string(), false)),
        },
    }
}

/// The reply a commit stores for its plan: the result, or the conflict its events were stored as.
fn stored(plan: &Plan, ids: &[EventId], seqs: &[Seq], base: Seq) -> Stored {
    let last = *seqs.last().expect("one event at least");
    if let Check::Conflict { clashes, .. } = &plan.check {
        let theirs = within(
            clashes.iter().map(|c| {
                json!({
                    "target": target_id(&c.target),
                    "field": c.field,
                    "seq": c.theirs.as_ref().map(|r| r.seq),
                    "event": c.theirs.as_ref().map(|r| r.event.0.clone()),
                })
            }),
            THEIRS_MAX_BYTES,
        );
        let first = &clashes[0];
        let id = target_id(&first.target);
        let name = q(&plan.names.get(&id).cloned().unwrap_or(id));
        let seq = first.theirs.as_ref().map_or(base, |r| r.seq);
        let details = json!({
            "event": ids[0].0,
            "base": base,
            "theirs": theirs,
            "yours": yours(&plan.bodies),
            // The person's three-way card arrives with the card dealers; until then a person
            // re-reads and decides, as an agent does.
            "card": null,
        });
        return Stored::Conflict { name, seq, details };
    }
    let status = plan.check.status().unwrap_or(Status::Applied);
    let line = if status == Status::Ghost {
        text::get("edit.ghost", &[("line", &plan.line)])
    } else {
        plan.line.clone()
    };
    let targets = within(plan.targets.iter().cloned(), TARGETS_MAX_BYTES);
    let result = EditResult {
        event: ids[0].clone(),
        count: ids.len() as u32,
        status,
        more_targets: (plan.targets.len() - targets.len()) as u32,
        targets,
        undo: Handle {
            command: CommandName("edit".into()),
            args: vec!["--base".into(), last.to_string(), "undo".into(), ids[0].0.clone()],
            label: text::label("edit.undo_label", &[]),
        },
    };
    Stored::Done { result, text: line }
}

/// The first of `items` whose JSON fits in `max` bytes together, so a reply keeps to its leaf's
/// budget however many targets a command touched.
pub(crate) fn within<T: Serialize>(items: impl Iterator<Item = T>, max: usize) -> Vec<T> {
    let mut used = 0;
    let mut out = Vec::new();
    for item in items {
        used += serde_json::to_string(&item).map_or(max, |s| s.len() + 1);
        if used > max {
            break;
        }
        out.push(item);
    }
    out
}

/// What the write set, as a compact map from target to fields, cut to [`YOURS_MAX_BYTES`].
pub(crate) fn yours(bodies: &[(Option<String>, EventBody)]) -> Value {
    let mut out = serde_json::Map::new();
    for (_, body) in bodies {
        let entry = match body {
            EventBody::Edit(edit) => (target_id(&edit.target), json!({"op": edit.op, "fields": edit.fields})),
            EventBody::Undo { of } => ("undo".to_owned(), json!(of.0)),
            _ => continue,
        };
        let mut next = out.clone();
        next.insert(entry.0, entry.1);
        // An entry that doesn't fit is left out; a smaller one after it may still.
        if Value::Object(next.clone()).to_string().len() > YOURS_MAX_BYTES {
            continue;
        }
        out = next;
    }
    Value::Object(out)
}

pub(crate) fn cycle_refused(cycle: &[NodeId]) -> AppError {
    let ids: Vec<&str> = cycle.iter().map(|n| n.0.as_str()).collect();
    let node = q(ids.first().copied().unwrap_or_default());
    let what =
        text::get("edit.parent_cycle", &[("node", &node), ("cycle", &text::short(&ids.join(" -> "), 300))]);
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({"field": "parent", "cycle": ids}))
}

/// A name or an ID as a line quotes it: its first [`QUOTE_CHARS`] characters, so the line stays
/// within the leaf's budget whatever a repo or a person named.
pub(crate) fn q(text: &str) -> String {
    text::short(text, QUOTE_CHARS)
}

pub(crate) fn target_id(target: &Target) -> String {
    match target {
        Target::Node(id) => id.0.clone(),
        Target::Edge(id) => id.0.clone(),
    }
}

/// Plan `op` over the fold of the store as this write's commit finds it.
pub(crate) fn plan(
    op: &EditOp,
    snapshot: &Snapshot<'_>,
    folded: &Folded,
    vocabulary: &Vocabulary,
    writer: &Writer,
    base: Seq,
) -> Result<Plan, AppError> {
    let proposed = Proposed { writer: writer.clone(), authority: Authority::of(&writer.actor), base };
    let nodes = &folded.model.nodes;
    let mut names = BTreeMap::new();
    let mut named = |node: &Node| {
        names.insert(node.id.0.clone(), node.name.clone());
    };
    let (edits, line): (Vec<Edit>, String) = match op {
        EditOp::Move { id, parent } => {
            let node = resolve(nodes, id)?;
            let to = resolve(nodes, parent)?;
            named(node);
            let edit = set(folded, node, PARENT, json!(to.id.0), base);
            (vec![edit], text::get("edit.moved", &[("id", &q(&node.id.0)), ("parent", &q(&to.id.0))]))
        }
        EditOp::Group { name, ids } => {
            let mut members: Vec<&Node> = Vec::new();
            for id in ids {
                let node = resolve(nodes, id)?;
                if !members.iter().any(|m| m.id == node.id) {
                    members.push(node);
                }
            }
            let slug =
                keys::slug(name).ok_or_else(|| AppError::invalid_argument("name", "edit.group_name"))?;
            let namespace = common_namespace(&members);
            let group = NodeId(format!("{namespace}:group:{slug}"));
            // The coarsest member's level on the ladder; `members` holds every ID the op names.
            let ladder = Ladder::of(vocabulary);
            let level = members
                .iter()
                .map(|m| &m.level)
                .min_by_key(|l| ladder.rank(l))
                .expect("a group names a member");
            let parent = members[0].parent.clone();
            let mut fields = BTreeMap::from([
                ("kind".to_owned(), json!(GROUP_KIND)),
                ("level".to_owned(), json!(level)),
                ("name".to_owned(), json!(name)),
                ("parent".to_owned(), json!(parent)),
            ]);
            fields.retain(|_, v| !v.is_null());
            let put = Edit {
                op: Op::Put,
                target: Target::Node(group.clone()),
                layer: Layer::Actual,
                fields,
                base,
                over: BTreeMap::new(),
            };
            let mut edits = vec![put];
            for member in &members {
                named(member);
                edits.push(set(folded, member, PARENT, json!(group.0), base));
            }
            let line = text::get(
                "edit.grouped",
                &[("count", &members.len()), ("name", &q(name)), ("id", &q(&group.0))],
            );
            (edits, line)
        }
        EditOp::Rename { id, name } => {
            if name.trim().is_empty() {
                return Err(AppError::invalid_argument("name", "edit.rename_empty"));
            }
            let node = resolve(nodes, id)?;
            named(node);
            let edit = set(folded, node, NAME, json!(name), base);
            (vec![edit], text::get("edit.renamed", &[("id", &q(&node.id.0)), ("name", &q(name))]))
        }
        EditOp::Hide { id } => {
            let edit = match folded.model.edges.iter().find(|e| &e.id.0 == id) {
                Some(edge) => Edit {
                    op: Op::Set,
                    target: Target::Edge(edge.id.clone()),
                    layer: edge.layer,
                    fields: BTreeMap::from([(HIDDEN.to_owned(), json!(true))]),
                    base,
                    over: BTreeMap::new(),
                },
                None => {
                    let node = resolve(nodes, id)?;
                    named(node);
                    set(folded, node, HIDDEN, json!(true), base)
                }
            };
            let shown = target_id(&edit.target);
            (vec![edit], text::get("edit.hidden", &[("id", &q(&shown))]))
        }
        EditOp::Pin { id, view, x, y } => {
            if view.trim().is_empty() || !x.is_finite() || !y.is_finite() {
                return Err(AppError::invalid_argument("view", "edit.pin_position"));
            }
            let node = resolve(nodes, id)?;
            named(node);
            let field = format!("{POS_PREFIX}{view}");
            let edit = set(folded, node, &field, json!({"x": x, "y": y}), base);
            let line =
                text::get("edit.pinned", &[("id", &q(&node.id.0)), ("x", x), ("y", y), ("view", &q(view))]);
            (vec![edit], line)
        }
        EditOp::Relate { from, to, kind } | EditOp::Forbid { from, to, kind: Some(kind) } => {
            let rule = if matches!(op, EditOp::Forbid { .. }) { "forbid" } else { "require" };
            let kind = relation(vocabulary, kind)?;
            let (from, to) = (resolve(nodes, from)?, resolve(nodes, to)?);
            let id = keys::edge_id(&kind, &from.id, &to.id);
            let fields = BTreeMap::from([
                ("kind".to_owned(), json!(kind)),
                ("from".to_owned(), json!({"id": from.id})),
                ("to".to_owned(), json!({"id": to.id})),
                ("props".to_owned(), json!({"rule": rule})),
            ]);
            let put = Edit {
                op: Op::Put,
                target: Target::Edge(id),
                layer: Layer::Intended,
                fields,
                base,
                over: BTreeMap::new(),
            };
            let key = if rule == "forbid" { "edit.forbidden" } else { "edit.related" };
            let line = text::get(key, &[("from", &q(&from.id.0)), ("kind", &kind.0), ("to", &q(&to.id.0))]);
            (vec![put], line)
        }
        EditOp::Forbid { kind: None, .. } => {
            return Err(AppError::invalid_argument("kind", "edit.forbid_kind"));
        }
        EditOp::Unrelate { from, to, kind } => {
            let kind = relation(vocabulary, kind)?;
            let (from, to) = (resolve(nodes, from)?, resolve(nodes, to)?);
            let id = keys::edge_id(&kind, &from.id, &to.id);
            let (from_id, to_id) = (q(&from.id.0), q(&to.id.0));
            let vars: [(&str, &dyn std::fmt::Display); 3] =
                [("from", &from_id), ("kind", &kind.0), ("to", &to_id)];
            if !folded.model.edges.iter().any(|e: &Edge| e.id == id && e.layer == Layer::Intended) {
                let what = text::get("edit.missing_relation", &vars);
                return Err(
                    AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": []}))
                );
            }
            let retract = Edit {
                op: Op::Retract,
                target: Target::Edge(id),
                layer: Layer::Intended,
                fields: BTreeMap::new(),
                base,
                over: BTreeMap::new(),
            };
            (vec![retract], text::get("edit.unrelated", &vars))
        }
        EditOp::Undo { event } => return plan_undo(event, snapshot, folded, &proposed),
    };
    let check = cas::check_edits(folded, &proposed, &edits);
    let mut targets: Vec<String> = edits.iter().map(|e| target_id(&e.target)).collect();
    targets.dedup();
    let bodies = edits.into_iter().map(|e| (Some(target_id(&e.target)), EventBody::Edit(e))).collect();
    Ok(Plan { bodies, targets, check, line, names, closes: Vec::new(), project: None })
}

/// An undo of the command that wrote `event`: one `undo` event, checked like any write (DR-61).
fn plan_undo(
    event: &str,
    snapshot: &Snapshot<'_>,
    folded: &Folded,
    proposed: &Proposed,
) -> Result<Plan, AppError> {
    let events = snapshot.events_after(0)?;
    let of = find_event(&events, event)?;
    if matches!(of.body, EventBody::Scan { .. }) {
        return Err(AppError::invalid_argument("event", "edit.undo_scan"));
    }
    // A card is picked, or resolves with its ghosts; undoing it would leave what waits on it on no
    // card, and no later card would count it.
    if matches!(of.body, EventBody::Card(_)) {
        return Err(AppError::invalid_argument("event", "edit.undo_card"));
    }
    if folded.is_undone(&of.id) {
        let what = text::get("edit.already_undone", &[("event", &of.id.0)]);
        return Err(
            AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "event"}))
        );
    }
    let command = folded.command_of(&of.id);
    let check = cas::check_undo(folded, proposed, &of.id);
    let mut targets: Vec<String> =
        folded.undo_keys(&of.id).into_iter().map(|(t, _)| target_id(&t.target())).collect();
    targets.dedup();
    let line = match command.len() {
        0 | 1 => text::get("edit.undone_one", &[("event", &of.id.0)]),
        count => text::get("edit.undone", &[("event", &of.id.0), ("count", &count)]),
    };
    let names = folded
        .model
        .nodes
        .iter()
        .filter(|n| targets.contains(&n.id.0))
        .map(|n| (n.id.0.clone(), n.name.clone()))
        .collect();
    let target = targets.first().cloned();
    let mut bodies = vec![(target, EventBody::Undo { of: of.id.clone() })];
    let mut closes = Vec::new();
    // Once this undo applies, a card is open exactly while a ghost waits on it (DR-89): one left
    // with none waiting closes in the same write, under a key of its own, and a closed one whose
    // ghost waits again reopens, by this command's undo of each review that closed it. A ghost
    // undo applies nothing, so changes no card.
    // An undo that would open a card beside the open one of its kind and subject waits until that
    // one resolves: a dealer has one open card at a time, which its later rows join.
    if check == Check::Applied {
        let cards = crate::app::cards::after_undo(folded, &of.id, &events);
        if let Some((card, open)) = &cards.beside {
            return Err(crate::app::cards::undo_beside(folded, card, open, snapshot.seq()));
        }
        for review in cards.reopen {
            bodies.push((Some(review.0.clone()), EventBody::Undo { of: review }));
        }
        closes = cards.close;
    }
    Ok(Plan { bodies, targets, check, line, names, closes, project: None })
}

/// The event `target` names: its ID, or the one event whose ID ends with it.
pub(crate) fn find_event<'a>(events: &'a [Event], target: &str) -> Result<&'a Event, AppError> {
    if let Some(event) = events.iter().find(|e| e.id.0 == target) {
        return Ok(event);
    }
    let found: Vec<&Event> =
        if target.is_empty() { vec![] } else { events.iter().filter(|e| e.id.0.ends_with(target)).collect() };
    match found.as_slice() {
        [event] => Ok(event),
        [] => {
            let what = text::get("edit.missing_event", &[("target", &q(target))]);
            Err(AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": []})))
        }
        several => {
            let ids: Vec<&str> = several.iter().take(CANDIDATES).map(|e| e.id.0.as_str()).collect();
            Err(ambiguous(target, several.len(), &ids))
        }
    }
}

/// A `set` of one field of `node`, recording the fact's own value under it for scan drift.
fn set(folded: &Folded, node: &Node, field: &str, value: Value, base: Seq) -> Edit {
    let target = Target::Node(node.id.clone());
    let over: BTreeMap<String, Value> =
        folded.fact_value(&target, field).map(|fact| (field.to_owned(), fact)).into_iter().collect();
    Edit {
        op: Op::Set,
        target,
        layer: node.layer,
        fields: BTreeMap::from([(field.to_owned(), value)]),
        base,
        over,
    }
}

/// The node `target` names: the one with that ID, else the one whose ID ends with `:target`, else
/// the one named `target`. None is `not_found` with the closest IDs; several is `ambiguous_ref`
/// with the candidates.
fn resolve<'a>(nodes: &'a [Node], target: &str) -> Result<&'a Node, AppError> {
    if let Some(node) = nodes.iter().find(|n| n.id.0 == target) {
        return Ok(node);
    }
    let suffix = format!(":{target}");
    let by_suffix: Vec<&Node> = nodes.iter().filter(|n| n.id.0.ends_with(&suffix)).collect();
    let found =
        if by_suffix.is_empty() { nodes.iter().filter(|n| n.name == target).collect() } else { by_suffix };
    match found.as_slice() {
        [node] => Ok(node),
        [] => {
            let closest: Vec<&str> = nodes
                .iter()
                .filter(|n| !target.is_empty() && (n.id.0.contains(target) || n.name.contains(target)))
                .take(CANDIDATES)
                .map(|n| n.id.0.as_str())
                .collect();
            let what = text::get("edit.missing_node", &[("target", &q(target))]);
            Err(AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest})))
        }
        several => {
            let ids: Vec<&str> = several.iter().take(CANDIDATES).map(|n| n.id.0.as_str()).collect();
            Err(ambiguous(target, several.len(), &ids))
        }
    }
}

fn ambiguous(target: &str, count: usize, ids: &[&str]) -> AppError {
    let shown: Vec<String> = ids.iter().take(3).map(|id| q(id)).collect();
    AppError::new(
        ErrorCode::AmbiguousRef,
        &[("target", &q(target)), ("count", &count), ("ids", &shown.join(", "))],
    )
    .details(json!({"candidates": ids}))
}

/// The relation kind `kind` names among the loaded packs': the whole kind, or the one whose name
/// after its pack is `kind`.
fn relation(vocabulary: &Vocabulary, kind: &str) -> Result<Kind, AppError> {
    let kinds: Vec<&Kind> = vocabulary.relations.iter().map(|r| &r.kind).collect();
    if let Some(found) = kinds.iter().find(|k| k.0 == kind) {
        return Ok((*found).clone());
    }
    let short: Vec<&&Kind> =
        kinds.iter().filter(|k| k.0.split_once('.').is_some_and(|(_, n)| n == kind)).collect();
    match short.as_slice() {
        [found] => Ok((**found).clone()),
        [] => {
            let known: Vec<&str> = kinds.iter().map(|k| k.0.as_str()).collect();
            let what = text::get("edit.relation_kind", &[("kind", &kind), ("known", &known.join(", "))]);
            Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "kind"})))
        }
        several => {
            let ids: Vec<&str> = several.iter().take(CANDIDATES).map(|k| k.0.as_str()).collect();
            let what = text::get(
                "edit.relation_ambiguous",
                &[("kind", &kind), ("count", &several.len()), ("ids", &ids.join(", "))],
            );
            Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "kind"})))
        }
    }
}

/// The namespace a group of `members` takes: theirs when they share one, else `ws`.
fn common_namespace(members: &[&Node]) -> String {
    let namespace = |n: &Node| n.id.0.split(':').next().unwrap_or_default().to_owned();
    let first = members.first().map(|n| namespace(n)).unwrap_or_default();
    if !first.is_empty() && members.iter().all(|n| namespace(n) == first) { first } else { "ws".to_owned() }
}
