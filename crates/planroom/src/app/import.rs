//! `planroom import -`: Artifact inbox rows on stdin, one `ImportLine` per line, as the mod passes
//! them from the `ArtifactData list` result it observed (architecture.md, "Remote mode through the
//! Artifact (S11)", step 5; DR-52, DR-67, DR-77). Import never reads rows from a file.
//!
//! - The author is the path's `<uid>` (`person:artifact:<uid>`), never a field of the row.
//! - Rows apply sorted by `(instance, counter)`.
//! - A read request (`map` in this slice) needs no person: every sync answers it into
//!   `answers/<docId>`, redacted, and acks it `imported`.
//! - A row that isn't valid, or names a command a row may not name, is acked `refused`; it never
//!   fails the import.
//! - An `edit` row no paired device signed, in a person's import, applies nothing on its own
//!   (DR-83). No device can be paired yet, so that is every row, a `sig` or not: its edits are
//!   stored as ghosts under `person:artifact:<uid>`, from the client `artifact:<uid>:<instance>`,
//!   each caused by the one open Apply card, and the row is acked `held`. The first such row deals
//!   the card, written first in that row's own transaction under a key of its own, so an undo of
//!   that row leaves the card for the rows that joined it; later rows join it. A row stale
//!   against its `base` is stored as a conflict and acked `conflict`. Each row is one write keyed
//!   `artifact:<uid>:<docId>`, so a sync that brings it again replays it.
//! - An `edit undo` row, a row a paired device signed, and `review`, `decide` and
//!   `intent propose` rows arrive in later milestones. Until then, with a person token they are
//!   acked `refused`; without one they stay in the inbox, unacked, for a person's sync, as a
//!   tokenless sync leaves every row it can't apply.
//! - An agent (`PLANROOM_ACTOR=agent:...`) is never a person: its import of any such row, or of an
//!   event export, is `person_required` and changes nothing (DR-4, DR-52).
//!
//! Import writes each document the session must write under the state directory's
//! `share/sync/<seq>-<µs>/` (`.planroom/`, or a single repo's `.git/planroom/`),
//! and each `ArtifactData batch` call's writes, which name those documents, to a batch file there,
//! `live/head` last, so the session never retypes a document. The reply names the batch files and
//! counts the rows' outcomes with the first few, so it keeps to its budget for any number of rows.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use planroom_contract::format::{self, FormatError, ReadError};
use planroom_contract::remote::{
    ACK_FORMAT, ANSWER_FORMAT, BATCH_MAX_BYTES, BATCH_MAX_WRITES, DB_DOC_MAX_BYTES, IMPORT_ROWS_SAMPLE,
    LIVE_HEAD_FORMAT,
};
use planroom_contract::{
    Ack, AckStatus, Actor, Answer, Cause, Client, Command, CommandName, DbBatch, DbOp, DbWrite, Detail,
    EditOp, EditReq, ErrorCode, Event, EventBody, Handle, ImportCounts, ImportLine, ImportReq, ImportResult,
    ImportedRow, InboxRow, LiveHead, MapReq, Seq, Status, Surface,
};
use planroom_kernel::cas::{self, Authority, Check, Proposed};
use planroom_kernel::fold::Writer;
use planroom_kernel::keys;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::redact;
use planroom_store::{NewEvent, Planned, Store};
use serde::Serialize;
use serde_json::{Value, json};

use crate::app::actor::{self, Bound, Caller, Refused, Terminal, TerminalPath};
use crate::app::error::AppError;
use crate::app::share::SNAPSHOT_SEQ_META;
use crate::app::{Paged, Reply, cards, data, edit, map, new_event_id, workspace};
use crate::{clock, live, text};

/// The commands a row may name that only read; any sync answers them (DR-77).
const READS: &[&str] = &["map", "show", "find", "diff", "ask"];
/// The commands a row may name that change the workspace (DR-56).
const CHANGES: &[&str] = &["edit", "review", "decide", "intent propose"];
/// Change frames `live/head` carries at most.
const HEAD_CHANGES: u32 = 200;

/// One row read from stdin, with what its path names.
struct Pending {
    path: String,
    uid: String,
    doc_id: String,
    version: Option<u64>,
    row: InboxRow,
}

/// What became of one row.
struct Outcome {
    path: String,
    uid: Option<String>,
    doc_id: Option<String>,
    version: Option<u64>,
    command: Option<CommandName>,
    status: Option<AckStatus>,
    reason: Option<String>,
    answer: Option<Answer>,
}

pub fn run(
    req: &ImportReq,
    cwd: &Path,
    caller: &Caller,
    stdin: &mut dyn BufRead,
    terminal: &mut dyn Terminal,
) -> Result<Reply<ImportResult>, AppError> {
    if req.source != "-" {
        return Err(AppError::invalid_argument("source", "argument.import_stdin"));
    }
    let spec = Command::Import(req.clone()).spec();
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);

    // A token, or a person's claim, makes this a person's import, checked before anything is read;
    // a tokenless sync only answers reads and leaves the rest (DR-77). An agent is never a person,
    // and may import nothing a person must (below).
    let keys = actor::registered_keys(&store, &root, caller, &spec.name())?;
    let keys: Vec<_> = keys.iter().collect();
    let now = clock::unix_micros() / 1_000_000;
    let claims_person = caller.actor.as_deref().is_some_and(|a| a.starts_with("person:"));
    let agent = caller.actor.is_some() && !claims_person;
    let mut person = false;
    if (caller.token.is_some() && !agent) || claims_person {
        let acting =
            actor::person(caller, spec, &Bound::default(), &keys, now, TerminalPath::Takes, terminal)?;
        if let Some(digest) = acting.digest {
            actor::spend(&mut store, &digest, now, &spec.name(), TerminalPath::Takes)?;
        }
        person = true;
    }

    let mut outcomes = Vec::new();
    let mut pending = Vec::new();
    let mut head_version = None;
    let mut ack_versions: BTreeMap<String, u64> = BTreeMap::new();
    let mut text_in = String::new();
    loop {
        text_in.clear();
        let read = stdin.read_line(&mut text_in).map_err(AppError::internal)?;
        if read == 0 {
            break;
        }
        let line = text_in.trim();
        if line.is_empty() {
            continue;
        }
        if is_event_export(line) {
            // An event export replays a whole timeline: a person's alone, never a sync's (DR-52).
            if !person {
                actor::person(caller, spec, &Bound::default(), &keys, now, TerminalPath::Takes, terminal)?;
            }
            return Err(AppError::invalid_argument("source", "import.event_export_not_yet"));
        }
        let parsed: ImportLine = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(e) => {
                outcomes.push(unackable("", text::get("import.not_json", &[("why", &e)])));
                continue;
            }
        };
        let Some(row) = parsed.row else {
            match parsed.path.split('/').collect::<Vec<_>>().as_slice() {
                ["live", "head"] => head_version = parsed.version,
                ["acks", uid] if segment_ok(uid) => {
                    if let Some(v) = parsed.version {
                        ack_versions.insert((*uid).to_owned(), v);
                    }
                }
                _ => outcomes.push(unackable(&parsed.path, not_a_row_path(&parsed.path))),
            }
            continue;
        };
        let Some((uid, doc_id)) = row_path(&parsed.path) else {
            outcomes.push(unackable(&parsed.path, not_a_row_path(&parsed.path)));
            continue;
        };
        let refuse = |reason: String| Outcome {
            path: parsed.path.clone(),
            uid: Some(uid.clone()),
            doc_id: Some(doc_id.clone()),
            version: parsed.version,
            command: None,
            status: Some(AckStatus::Refused),
            reason: Some(reason),
            answer: None,
        };
        match format::read::<InboxRow>(row) {
            Ok(row) if !row.instance.is_empty() => {
                pending.push(Pending { path: parsed.path, uid, doc_id, version: parsed.version, row })
            }
            Ok(_) => outcomes.push(refuse(text::get("import.malformed", &[("why", &"instance")]))),
            Err(ReadError::Format(FormatError::Newer { found, .. })) => {
                outcomes.push(refuse(text::get("import.newer_format", &[("format", &found)])))
            }
            Err(ReadError::Format(FormatError::Other { .. })) => {
                outcomes.push(refuse(text::get("import.malformed", &[("why", &"format")])))
            }
            Err(ReadError::Shape(e)) => outcomes.push(refuse(text::get("import.malformed", &[("why", &e)]))),
        }
    }

    // An agent may have read requests answered, as any sync does, but rows that change the
    // workspace need a person, so its import of one changes nothing at all (DR-52).
    let changes = pending.iter().any(|p| CHANGES.contains(&p.row.command.0.as_str()));
    if agent && changes {
        return Err(actor::person_required(&spec.name(), Refused::NotAPerson, TerminalPath::Takes));
    }
    // A person at a terminal confirms such rows as theirs; without a yes, the import is a sync
    // with no person, which leaves them (DR-4).
    if changes && !person && caller.actor.is_none() && terminal.present() {
        person =
            actor::person(caller, spec, &Bound::default(), &keys, now, TerminalPath::Takes, terminal).is_ok();
    }

    // One viewer's later row never lands before an earlier one (DR-61).
    pending.sort_by(|a, b| {
        (&a.row.instance, a.row.counter, &a.doc_id).cmp(&(&b.row.instance, b.row.counter, &b.doc_id))
    });
    for p in pending {
        outcomes.push(apply(&root, &mut store, p, person)?);
    }

    let seq = store.seq()?;
    let snapshot_seq: Seq = store.meta(SNAPSHOT_SEQ_META)?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let head = live_head(&store, seq, snapshot_seq)?;
    let (dir, batch_files) = batch_files(&root, &outcomes, seq, &head, head_version, &ack_versions)?;

    let count = |f: &dyn Fn(&Outcome) -> bool| outcomes.iter().filter(|o| f(o)).count();
    let answered = count(&|o| o.answer.is_some() && o.status == Some(AckStatus::Imported));
    let refused = count(&|o| o.status == Some(AckStatus::Refused));
    let left = count(&|o| o.status.is_none() && o.uid.is_some());
    let held = count(&|o| matches!(o.status, Some(AckStatus::Held | AckStatus::Conflict)));
    let summary = text::get(
        "import.summary",
        &[("answered", &answered), ("held", &held), ("refused", &refused), ("left", &left)],
    );
    let mut counts = ImportCounts::default();
    for o in &outcomes {
        let slot = match (o.status, &o.uid) {
            (Some(AckStatus::Imported), _) => &mut counts.imported,
            (Some(AckStatus::Held), _) => &mut counts.held,
            (Some(AckStatus::Conflict), _) => &mut counts.conflict,
            (Some(AckStatus::Refused), _) => &mut counts.refused,
            (None, Some(_)) => &mut counts.left,
            (None, None) => &mut counts.skipped,
        };
        *slot += 1;
    }
    let rows = outcomes
        .into_iter()
        .take(IMPORT_ROWS_SAMPLE)
        .map(|o| ImportedRow { path: o.path, command: o.command, status: o.status, reason: o.reason })
        .collect();
    let result = ImportResult {
        summary: summary.clone(),
        person,
        counts,
        rows,
        root: dir.display().to_string(),
        batch_files,
    };
    Ok(Reply { result, seq, warnings: vec![], text: summary })
}

/// Apply one valid row: answer a read, hold an unsigned edit as ghosts on the Apply card, refuse
/// what a row may not name, and leave or refuse any other change, which arrives later.
fn apply(root: &Path, store: &mut Store, p: Pending, person: bool) -> Result<Outcome, AppError> {
    let name = p.row.command.0.clone();
    if person && name == "edit" && !signed(&p.row) {
        let req: Result<EditReq, _> = serde_json::from_value(p.row.request.clone());
        match req {
            Ok(req) if !matches!(req.op, EditOp::Undo { .. }) => return held(store, p, &req),
            Ok(_) => {}
            Err(e) => {
                let mut out = outcome(&p, AckStatus::Refused);
                out.reason = Some(text::get("import.bad_request", &[("command", &name), ("why", &e)]));
                return Ok(out);
            }
        }
    }
    let mut out = Outcome {
        path: p.path,
        uid: Some(p.uid),
        doc_id: Some(p.doc_id),
        version: p.version,
        command: Some(p.row.command.clone()),
        status: Some(AckStatus::Refused),
        reason: None,
        answer: None,
    };
    if READS.contains(&name.as_str()) {
        let answer = read(root, &name, p.row.request);
        out.status = Some(if answer.error.is_none() { AckStatus::Imported } else { AckStatus::Refused });
        out.reason = answer.error.as_ref().map(|e| e.message.clone());
        out.answer = Some(answer);
    } else if CHANGES.contains(&name.as_str()) {
        if person {
            out.reason = Some(text::get("import.not_yet", &[("command", &name)]));
        } else {
            out.status = None;
            out.reason = Some(text::get("import.left", &[]));
        }
    } else {
        out.reason = Some(text::get("import.not_allowed", &[("command", &name)]));
    }
    Ok(out)
}

/// Whether a paired device signed `row`: its `sig` checks against `meta.pairing_key` (DR-67). No
/// device can be paired yet, so no `sig` checks, and a row that carries one, broken or forged, is
/// held on the Apply card like any row no paired device signed (DR-83).
fn signed(_row: &InboxRow) -> bool {
    false
}

/// A row's outcome with `status`, before its reason.
fn outcome(p: &Pending, status: AckStatus) -> Outcome {
    Outcome {
        path: p.path.clone(),
        uid: Some(p.uid.clone()),
        doc_id: Some(p.doc_id.clone()),
        version: p.version,
        command: Some(p.row.command.clone()),
        status: Some(status),
        reason: None,
        answer: None,
    }
}

/// Whether `event` is an inbox row's: a person's, written from the artifact, as `held` writes
/// each row under its viewer (DR-67).
pub(crate) fn is_viewer_row(event: &Event) -> bool {
    matches!(event.actor, Actor::Person { .. }) && event.client.surface == Surface::Artifact
}

/// An unsigned edit row: its edits, planned as the `edit` leaf plans them from the row's `base`,
/// stored as ghosts on the open Apply card, which this row's write deals first when none is open;
/// or, stale against that base, stored as a conflict. A row whose edit can't be planned, such as
/// one naming a block that isn't on the map, is refused with why (DR-83).
fn held(store: &mut Store, p: Pending, req: &EditReq) -> Result<Outcome, AppError> {
    let vocabulary = crate::packs::vocabulary()?;
    let writer = Writer {
        actor: Actor::Person { id: format!("person:artifact:{}", p.uid), name: None },
        client: Client { surface: Surface::Artifact, id: format!("artifact:{}:{}", p.uid, p.row.instance) },
    };
    let base = p.row.base;
    let idem = format!("artifact:{}:{}", p.uid, p.doc_id);
    let request_hash =
        keys::request_hash("import", &serde_json::to_value(&p.row).map_err(AppError::internal)?);
    let at = clock::now();
    let stored = std::cell::Cell::new(Status::Ghost);
    let committed = store.commit_keyed(
        &idem,
        &request_hash,
        |snapshot| {
            if base > snapshot.seq() {
                return Err(AppError::invalid_argument("base", "edit.base_ahead"));
            }
            let folded = live::folded(snapshot, vocabulary)?;
            let plan = edit::plan(&req.op, snapshot, &folded, vocabulary, &writer, base)?;
            let edits: Vec<_> = plan
                .bodies
                .iter()
                .filter_map(|(_, body)| match body {
                    EventBody::Edit(edit) => Some(edit.clone()),
                    _ => None,
                })
                .collect();
            // Whoever's inbox holds it, an unsigned row only proposes: a ghost, never applied.
            let proposed = Proposed { writer: writer.clone(), authority: Authority::Proposal, base };
            let check = cas::check_edits(&folded, &proposed, &edits);
            let (status, card) = match &check {
                Check::Cycle { cycle } => return Err(edit::cycle_refused(cycle)),
                Check::Ghost => (Status::Ghost, Some(cards::apply_card(snapshot, &at)?)),
                Check::Conflict { .. } => (Status::Conflict, None),
                Check::Applied | Check::Superseded { .. } => {
                    return Err(AppError::internal("an unsigned row is never applied"));
                }
            };
            stored.set(status);
            let mut events = Vec::new();
            let cause = card.as_ref().map(|(id, _)| Cause::Event(id.clone()));
            // The card a row deals is written first, under a key of its own, so an undo of this
            // row's ghosts leaves it for the rows that join it.
            if let Some((_, Some(dealt))) = card {
                events.push(Planned::Own(dealt));
            }
            for (target, body) in plan.bodies {
                events.push(Planned::Command(NewEvent {
                    id: new_event_id(),
                    at: at.clone(),
                    actor: writer.actor.clone(),
                    client: writer.client.clone(),
                    project: req.common.project.clone(),
                    cause: cause.clone(),
                    status,
                    target,
                    body,
                }));
            }
            Ok(events)
        },
        |_| json!({"status": stored.get()}),
    );
    let mut out = outcome(&p, AckStatus::Held);
    match committed {
        Ok(committed) => {
            // A replay reads how the row was stored from the reply its first commit kept.
            if committed.reply["status"] == json!(Status::Conflict) {
                out.status = Some(AckStatus::Conflict);
                out.reason = Some(text::get("import.conflict", &[]));
            } else {
                out.reason = Some(text::get("import.held", &[]));
            }
            Ok(out)
        }
        // What the row names, not the store, is wrong: the row is refused, the import goes on. So
        // is a row whose document ID an earlier, different row already used.
        Err(e)
            if e.code.spec().category == planroom_contract::Category::Usage
                || matches!(e.code, ErrorCode::NotFound | ErrorCode::IdempotencyMismatch) =>
        {
            out.status = Some(AckStatus::Refused);
            out.reason = Some(e.message.clone());
            Ok(out)
        }
        Err(e) => Err(e),
    }
}

/// A read request's answer, redacted as every output is: whether it succeeded or failed, its
/// error's and warnings' lines and their repo-text `details` are masked here, its result by its
/// own type where it is made.
fn read(root: &Path, command: &str, request: Value) -> Answer {
    let mut answer = answer(root, command, request);
    redact::answer(&mut answer);
    answer
}

/// A read request's answer: its result, redacted and within one `db` document, or why it failed.
fn answer(root: &Path, command: &str, request: Value) -> Answer {
    let failed = |handle: Handle, e: AppError| Answer {
        format: ANSWER_FORMAT.into(),
        handle,
        seq: 0,
        result: Value::Null,
        error: Some(e.object()),
        next: None,
        warnings: vec![],
        extra: Default::default(),
    };
    if command != "map" {
        let handle = Handle { command: CommandName(command.into()), args: vec![], label: command.into() };
        let what = text::get("import.read_not_yet", &[("command", &command)]);
        let e = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
            .details(json!({"field": "command"}));
        return failed(handle, e);
    }
    let bare = || Handle { command: CommandName("map".into()), args: vec![], label: "map".into() };
    let req: MapReq = match serde_json::from_value(request) {
        Ok(req) => req,
        Err(e) => {
            let handle = bare();
            let what = text::get("import.bad_request", &[("command", &"map"), ("why", &e)]);
            let e = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "request"}));
            return failed(handle, e);
        }
    };
    let vocabulary = match crate::packs::vocabulary() {
        Ok(vocabulary) => vocabulary,
        Err(e) => return failed(bare(), e),
    };
    let handle = map_handle(&req, &Ladder::of(vocabulary));
    // The answer is the map's page, as the CLI gives it: within the leaf's budget, with the cursor
    // of the page after it, which a read request passes back as its `cursor`, and the reply's
    // warnings, such as `stale_page` for a cursor from a map that has moved on.
    match map::run(&req, root) {
        Ok(Paged { reply, next, .. }) => {
            let mut view = reply.result;
            redact::map_view(&mut view, false);
            let mut answer = Answer {
                format: ANSWER_FORMAT.into(),
                handle: handle.clone(),
                seq: reply.seq,
                result: serde_json::to_value(&view).unwrap_or(Value::Null),
                error: None,
                next,
                warnings: reply.warnings,
                extra: Default::default(),
            };
            if size(&answer) > DB_DOC_MAX_BYTES {
                // Evidence is a `show` away; drop it before giving up on the view.
                view.edges.iter_mut().for_each(|e| e.evidence_sample.clear());
                answer.result = serde_json::to_value(&view).unwrap_or(Value::Null);
            }
            if size(&answer) > DB_DOC_MAX_BYTES {
                let what = text::get("import.too_large", &[]);
                let e = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                    .details(json!({"field": "request"}));
                return failed(handle, e);
            }
            answer
        }
        Err(e) => failed(handle, e),
    }
}

/// The handle that asks for a map: `map` with the request's flags, labelled with the level it
/// shows, the ladder's top when it names none.
fn map_handle(req: &MapReq, ladder: &Ladder<'_>) -> Handle {
    let mut args = Vec::new();
    if let Some(level) = &req.level {
        args.extend(["--level".to_owned(), level.as_str().to_owned()]);
    }
    if let Some(lens) = &req.lens {
        args.extend(["--lens".to_owned(), lens.clone()]);
    }
    if req.detail == Detail::Graph {
        args.extend(["--detail".to_owned(), "graph".to_owned()]);
    }
    if let Some(cursor) = &req.cursor {
        args.extend(["--cursor".to_owned(), cursor.0.clone()]);
    }
    let level = map::label_of(ladder, req.level.as_ref().unwrap_or_else(|| ladder.top()));
    Handle {
        command: CommandName("map".into()),
        args,
        label: text::label("import.map_label", &[("level", &level)]),
    }
}

/// The next `live/head`: the change frames since the snapshot, newest last, within one document.
fn live_head(store: &Store, seq: Seq, snapshot_seq: Seq) -> Result<LiveHead, AppError> {
    let mut changes: Vec<_> = Vec::new();
    let mut after = snapshot_seq.max(seq.saturating_sub(u64::from(HEAD_CHANGES)));
    loop {
        let events = store.events_after(after, HEAD_CHANGES)?;
        let Some(last) = events.last() else { break };
        after = last.seq;
        changes.extend(events.iter().map(live::frame));
    }
    let mut head = LiveHead {
        format: LIVE_HEAD_FORMAT.into(),
        seq,
        snapshot_seq,
        cards: vec![],
        changes,
        conflicts: vec![],
        extra: Default::default(),
    };
    while size(&head) > DB_DOC_MAX_BYTES && !head.changes.is_empty() {
        head.changes.remove(0);
    }
    Ok(head)
}

/// Write each document to its file, and group the writes into `ArtifactData batch` calls of at
/// most 50 writes and 1 MiB: answers, then acks, then the deletes of every acked row, then
/// `live/head`, last of all. Each call's writes go to `batch-<n>.json`; returns the sync's
/// directory and those files' paths relative to it, in order.
fn batch_files(
    root: &Path,
    outcomes: &[Outcome],
    seq: Seq,
    head: &LiveHead,
    head_version: Option<u64>,
    ack_versions: &BTreeMap<String, u64>,
) -> Result<(PathBuf, Vec<String>), AppError> {
    let dir = workspace::state_dir(root).join("share/sync").join(format!("{seq}-{}", clock::unix_micros()));
    data::ensure_dir(&dir)?;
    let mut writes: Vec<(DbWrite, usize)> = Vec::new();
    for o in outcomes {
        if let (Some(answer), Some(doc_id)) = (&o.answer, &o.doc_id) {
            let (path, bytes) = write_doc(&dir, "answers", doc_id, answer)?;
            writes.push((set("answers", doc_id, &path, None), bytes));
        }
    }
    let mut acks: BTreeMap<&str, Ack> = BTreeMap::new();
    for o in outcomes {
        if let (Some(uid), Some(doc_id), Some(status)) = (&o.uid, &o.doc_id, o.status) {
            acks.entry(uid)
                .or_insert_with(|| Ack {
                    format: ACK_FORMAT.into(),
                    imported_through: seq,
                    rows: BTreeMap::new(),
                    extra: Default::default(),
                })
                .rows
                .insert(doc_id.clone(), status);
        }
    }
    for (uid, ack) in &acks {
        let (path, bytes) = write_doc(&dir, "acks", uid, ack)?;
        // An existing ack merges this import's rows into the ones it lists.
        let write = match ack_versions.get(*uid) {
            Some(v) => DbWrite { op: DbOp::Update, ..set("acks", uid, &path, Some(*v)) },
            None => set("acks", uid, &path, None),
        };
        writes.push((write, bytes));
    }
    for o in outcomes {
        if let (Some(uid), Some(doc_id), Some(_)) = (&o.uid, &o.doc_id, o.status) {
            let delete = DbWrite {
                op: DbOp::Delete,
                collection: format!("inbox/{uid}/events"),
                doc_id: doc_id.clone(),
                file_path: None,
                if_version: o.version,
            };
            writes.push((delete, 64));
        }
    }
    let (path, bytes) = write_doc(&dir, "live", "head", head)?;
    writes.push((set("live", "head", &path, head_version), bytes));

    let mut batches: Vec<DbBatch> = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0;
    for (write, bytes) in writes {
        if !current.is_empty()
            && (current.len() == BATCH_MAX_WRITES || current_bytes + bytes > BATCH_MAX_BYTES)
        {
            batches.push(DbBatch { writes: std::mem::take(&mut current) });
            current_bytes = 0;
        }
        current_bytes += bytes;
        current.push(write);
    }
    batches.push(DbBatch { writes: current });
    let mut files = Vec::with_capacity(batches.len());
    for (i, batch) in batches.iter().enumerate() {
        let name = format!("batch-{}.json", i + 1);
        let bytes = serde_json::to_vec(batch).map_err(AppError::internal)?;
        data::write_private(&dir.join(&name), &bytes)?;
        files.push(name);
    }
    Ok((dir, files))
}

fn set(collection: &str, doc_id: &str, path: &Path, if_version: Option<u64>) -> DbWrite {
    DbWrite {
        op: DbOp::Set,
        collection: collection.into(),
        doc_id: doc_id.into(),
        file_path: Some(path.display().to_string()),
        if_version,
    }
}

/// Write one document to its batch file, `DIR/<collection>/<docId>.json`.
fn write_doc<T: Serialize>(
    dir: &Path,
    collection: &str,
    doc_id: &str,
    doc: &T,
) -> Result<(PathBuf, usize), AppError> {
    let bytes = serde_json::to_vec(doc).map_err(AppError::internal)?;
    let path = dir.join(collection).join(format!("{doc_id}.json"));
    if let Some(parent) = path.parent() {
        data::ensure_dir(parent)?;
    }
    data::write_private(&path, &bytes)?;
    Ok((path, bytes.len()))
}

fn size<T: Serialize>(doc: &T) -> usize {
    serde_json::to_vec(doc).map_or(usize::MAX, |b| b.len())
}

/// Whether a line on stdin is an event export's header or event, `planroom.event/v<n>`, rather
/// than an inbox row.
fn is_event_export(line: &str) -> bool {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| v.get("format").and_then(Value::as_str).map(|f| f.starts_with("planroom.event/")))
        .unwrap_or(false)
}

/// `inbox/<uid>/events/<docId>`, each segment in the `db` path grammar.
fn row_path(path: &str) -> Option<(String, String)> {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        ["inbox", uid, "events", doc_id] if segment_ok(uid) && segment_ok(doc_id) => {
            Some(((*uid).to_owned(), (*doc_id).to_owned()))
        }
        _ => None,
    }
}

/// One `db` path segment: letters, digits and `_ - . ~ : @ +`, never `.` or `..` alone, at most
/// 200 bytes.
fn segment_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s != "."
        && s != ".."
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.~:@+".contains(&b))
}

fn not_a_row_path(path: &str) -> String {
    text::get("import.not_a_row_path", &[("path", &path)])
}

/// A line that names no row this import can ack: reported, never written back.
fn unackable(path: &str, reason: String) -> Outcome {
    Outcome {
        path: path.to_owned(),
        uid: None,
        doc_id: None,
        version: None,
        command: None,
        status: None,
        reason: Some(reason),
        answer: None,
    }
}

#[cfg(test)]
mod tests {
    use planroom_contract::Vocabulary;

    use super::*;

    #[test]
    fn a_row_path_is_inbox_uid_events_doc() {
        assert_eq!(row_path("inbox/u_1/events/d1"), Some(("u_1".into(), "d1".into())));
        assert_eq!(row_path("inbox/u_1/probe/latest"), None);
        assert_eq!(row_path("inbox/../events/d1"), None);
        assert_eq!(row_path("inbox/u 1/events/d1"), None);
        assert_eq!(row_path("live/head"), None);
    }

    #[test]
    fn a_map_handle_names_its_flags() {
        let req = MapReq { level: Some("container".into()), detail: Detail::Graph, ..MapReq::default() };
        assert_eq!(
            map_handle(&req, &Ladder::of(&Vocabulary::default())).args,
            ["--level", "container", "--detail", "graph"]
        );
    }
}
