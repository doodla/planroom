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
//! - `edit`, `review`, `decide` and `intent propose` arrive in M1 and M2. Until then, with a person
//!   token they are acked `refused`; without one they stay in the inbox, unacked, for a person's
//!   sync, as a tokenless sync leaves every row it can't apply.
//!
//! Import writes each document the session must write under `.planroom/share/sync/<seq>-<µs>/`,
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
    Ack, AckStatus, Answer, Command, CommandName, DbBatch, DbOp, DbWrite, Detail, ErrorCode, Handle,
    ImportCounts, ImportLine, ImportReq, ImportResult, ImportedRow, InboxRow, LiveHead, MapReq, Seq,
};
use planroom_kernel::{redact, tree};
use planroom_store::Store;
use serde::Serialize;
use serde_json::{Value, json};

use crate::app::actor::{self, Bound, Caller};
use crate::app::error::AppError;
use crate::app::share::SNAPSHOT_SEQ_META;
use crate::app::{Reply, data, map, workspace};
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
) -> Result<Reply<ImportResult>, AppError> {
    if req.source != "-" {
        return Err(AppError::invalid_argument("source", "argument.import_stdin"));
    }
    let spec = Command::Import(req.clone()).spec();
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;

    // A token makes this a person's import; a tokenless sync only answers reads (DR-77).
    let person = caller.token.is_some();
    if person {
        let registered = store.meta("person_pubkey")?;
        let keys: Vec<_> = registered.as_deref().and_then(actor::parse_key).into_iter().collect();
        let now = clock::unix_micros() / 1_000_000;
        let digest = actor::check(caller, spec, &Bound::default(), &keys.iter().collect::<Vec<_>>(), now)
            .map_err(|why| actor::person_required(&spec.name(), why))?;
        actor::spend(&mut store, &digest, now, &spec.name())?;
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

    // One viewer's later row never lands before an earlier one (DR-61).
    pending.sort_by(|a, b| {
        (&a.row.instance, a.row.counter, &a.doc_id).cmp(&(&b.row.instance, b.row.counter, &b.doc_id))
    });
    for p in pending {
        outcomes.push(apply(&root, p, person));
    }

    let seq = store.seq()?;
    let snapshot_seq: Seq = store.meta(SNAPSHOT_SEQ_META)?.and_then(|s| s.parse().ok()).unwrap_or(0);
    let head = live_head(&store, seq, snapshot_seq)?;
    let (dir, batch_files) = batch_files(&root, &outcomes, seq, &head, head_version, &ack_versions)?;

    let count = |f: &dyn Fn(&Outcome) -> bool| outcomes.iter().filter(|o| f(o)).count();
    let answered = count(&|o| o.answer.is_some() && o.status == Some(AckStatus::Imported));
    let refused = count(&|o| o.status == Some(AckStatus::Refused));
    let left = count(&|o| o.status.is_none() && o.uid.is_some());
    let summary =
        text::get("import.summary", &[("answered", &answered), ("refused", &refused), ("left", &left)]);
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

/// Apply one valid row: answer a read, refuse what a row may not name, and leave or refuse a
/// change, which arrives in M1 and M2.
fn apply(root: &Path, p: Pending, person: bool) -> Outcome {
    let name = p.row.command.0.clone();
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
    out
}

/// A read request's answer: its result, redacted and within one `db` document, or why it failed.
fn read(root: &Path, command: &str, request: Value) -> Answer {
    let failed = |handle: Handle, e: AppError| Answer {
        format: ANSWER_FORMAT.into(),
        handle,
        seq: 0,
        result: Value::Null,
        error: Some(e.object()),
        extra: Default::default(),
    };
    if command != "map" {
        let handle = Handle { command: CommandName(command.into()), args: vec![], label: command.into() };
        let what = text::get("import.read_not_yet", &[("command", &command)]);
        let e = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
            .details(json!({"field": "command"}));
        return failed(handle, e);
    }
    let req: MapReq = match serde_json::from_value(request) {
        Ok(req) => req,
        Err(e) => {
            let handle = Handle { command: CommandName("map".into()), args: vec![], label: "map".into() };
            let what = text::get("import.bad_request", &[("command", &"map"), ("why", &e)]);
            let e = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "request"}));
            return failed(handle, e);
        }
    };
    let handle = map_handle(&req);
    match map::run(&req, root) {
        Ok(reply) => {
            let mut view = reply.result;
            redact::map_view(&mut view, false);
            let mut answer = Answer {
                format: ANSWER_FORMAT.into(),
                handle: handle.clone(),
                seq: reply.seq,
                result: serde_json::to_value(&view).unwrap_or(Value::Null),
                error: None,
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

/// The handle that asks for a map: `map` with the request's flags.
fn map_handle(req: &MapReq) -> Handle {
    let mut args = Vec::new();
    if let Some(level) = req.level {
        args.extend(["--level".to_owned(), tree::level_name(level).to_owned()]);
    }
    if let Some(lens) = &req.lens {
        args.extend(["--lens".to_owned(), lens.clone()]);
    }
    if req.detail == Detail::Graph {
        args.extend(["--detail".to_owned(), "graph".to_owned()]);
    }
    let level = tree::level_name(req.level.unwrap_or(planroom_contract::Level::System));
    Handle {
        command: CommandName("map".into()),
        args,
        label: text::get("import.map_label", &[("level", &level)]),
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
    let dir = root.join(workspace::DIR).join("share/sync").join(format!("{seq}-{}", clock::unix_micros()));
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
        let req = MapReq {
            level: Some(planroom_contract::Level::Container),
            detail: Detail::Graph,
            ..MapReq::default()
        };
        assert_eq!(map_handle(&req).args, ["--level", "container", "--detail", "graph"]);
    }
}
