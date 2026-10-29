//! `planroom pack add DIR` and `planroom pack remove ID`: a person adds or removes an out-of-tree
//! pack, and the workspace's config and its recorded pack set follow (architecture.md, The pack
//! set is workspace state; DR-62).
//!
//! - Both are person-only: a press, a typed `/room` or a `/dev/tty` confirmation. An agent gets
//!   `person_required` before anything is read or copied.
//! - An engine older than the one that recorded the set gets `packs_mismatch` before anything is
//!   read or copied, and the record's write checks again under the lock.
//! - The new list starts from the config's `packs` in a trusted workspace, since a hand edit of
//!   the config counts only once `pack add` or `pack remove` runs, and from the record's sources
//!   in an untrusted one, whose config isn't loaded (DR-48). An entry the record already holds
//!   keeps its copy without its source being read, so another pack's edited source waits for its
//!   card; any other entry is read, validated, compiled and copied.
//! - Every read and copy happens before the lock. The record is then rebuilt from the record the
//!   lock finds, and the config is written from it in the same transaction, so a concurrent
//!   `pack add` or `pack remove` is never lost.
//! - Each one's undo is the other (architecture.md, Undo classes): no event is written, and no
//!   copy is deleted.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use planroom_contract::{
    Command, CommandSpec, ErrorCode, EventBody, PackAddReq, PackRecord, PackRemoveReq, PackSetResult,
    Verdict, WriteCommon,
};
use planroom_store::{PackVerdict, RecordedPacks, Store, StoreError};
use serde_json::json;
use toml::de::DeTable;

use crate::app::actor::{Caller, Terminal};
use crate::app::error::AppError;
use crate::app::trust::{self, Trust};
use crate::app::{Reply, cards, edit, workspace};
use crate::packs::state::{self, BUILTIN};
use crate::packs::{Source, load};
use crate::{clock, config, text};

/// The most characters of a pack's source a reply quotes, so a source up to `PATH_MAX` keeps the
/// reply within its leaf's budget.
const SOURCE_CHARS: usize = 80;

/// One entry of the list a command records: its record, and its source when this command read it
/// to copy it, rather than keeping a copy the record already holds.
struct Entry {
    record: PackRecord,
    read: Option<Source>,
}

/// `planroom pack add DIR`.
pub fn add(
    req: &PackAddReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Reply<PackSetResult>, AppError> {
    let spec = Command::PackAdd(req.clone()).spec();
    let (root, mut store) = open(cwd)?;
    person(spec, &req.common, &line("pack add", &req.dir), caller, &mut store, &root, terminal)?;
    refuse_older(&store)?;
    let state_dir = workspace::state_dir(&root);
    let trusted = trust::at(&root) == Trust::Trusted;
    let read = read_records(&store)?;
    let mut entries = entries(&root, trusted, &read)?;
    // DIR is the person's, placed from where they ran the command.
    let dir = state::place(cwd, &req.dir)?;
    let source = state::read_named(&dir)?;
    let record = load::parse(source.clone()).map_err(|e| state::checked_by(e, &dir))?.record();
    let replaced = entries.iter().position(|e| e.record.id == record.id);
    let unchanged = replaced.is_some_and(|i| entries[i].record == record);
    let key = match (replaced, unchanged) {
        (_, true) => "pack.unchanged",
        (Some(_), false) => "pack.replaced",
        (None, false) => "pack.added",
    };
    if !unchanged {
        let entry = Entry { record: record.clone(), read: Some(source) };
        match replaced {
            Some(i) => entries[i] = entry,
            None => entries.push(entry),
        }
    }
    let after = commit(&mut store, &state_dir, entries, &read)?;
    let vars: [(&str, &dyn std::fmt::Display); 4] = [
        ("id", &record.id),
        ("version", &record.version),
        ("source", &text::short(&record.source, SOURCE_CHARS)),
        ("copy", &cards::short_hash(&record.sha256)),
    ];
    reply(&store, trusted, text::get(key, &vars), &record, after)
}

/// `planroom pack remove ID`.
pub fn remove(
    req: &PackRemoveReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Reply<PackSetResult>, AppError> {
    let spec = Command::PackRemove(req.clone()).spec();
    let (root, mut store) = open(cwd)?;
    person(spec, &req.common, &line("pack remove", &req.id), caller, &mut store, &root, terminal)?;
    refuse_older(&store)?;
    let id = req.id.trim();
    if builtin_ids()?.iter().any(|b| b == id) {
        let what = text::get("argument.pack_builtin", &[("id", &id)]);
        return Err(
            AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "id"}))
        );
    }
    let state_dir = workspace::state_dir(&root);
    let trusted = trust::at(&root) == Trust::Trusted;
    let read = read_records(&store)?;
    let mut entries = entries(&root, trusted, &read)?;
    let removed =
        entries.iter().map(|e| &e.record).chain(&read).find(|r| r.id == id).cloned().ok_or_else(|| {
            let what = text::get("argument.pack_id", &[("id", &id)]);
            AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"id": id}))
        })?;
    entries.retain(|e| e.record.id != id);
    let after = commit(&mut store, &state_dir, entries, &read)?;
    let line = text::get("pack.removed", &[("id", &id)]);
    reply(&store, trusted, line, &removed, after)
}

/// The workspace `cwd` is in and its store, opened to write: `workspace_not_found` before the
/// first scan.
fn open(cwd: &Path) -> Result<(PathBuf, Store), AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);
    Ok((root, store))
}

/// The caller as a person, or `person_required` naming `line`, the exact command a person runs
/// from a terminal; a token is spent here.
fn person(
    spec: &'static CommandSpec,
    common: &WriteCommon,
    line: &str,
    caller: &Caller,
    store: &mut Store,
    root: &Path,
    terminal: &mut dyn Terminal,
) -> Result<(), AppError> {
    edit::writer_for(spec, common, line, caller, store, root, common.base, terminal).map(drop)
}

/// The exact line that runs `path` on `operand` from a terminal, the operand quoted for a POSIX
/// shell, after `--` when it starts with `-`.
fn line(path: &str, operand: &str) -> String {
    let dashes = if operand.starts_with('-') { "-- " } else { "" };
    format!("planroom {path} {dashes}{}", text::shell_word(operand))
}

/// `packs_mismatch` when this engine is older than the one that recorded the set, read before
/// anything is read or copied; the record's write checks again under the lock.
fn refuse_older(store: &Store) -> Result<(), AppError> {
    match store.pack_verdict()? {
        PackVerdict::Refuse { recorded, resolved } => {
            Err(StoreError::PacksMismatch { recorded, resolved }.into())
        }
        PackVerdict::Keep | PackVerdict::Record(_) => Ok(()),
    }
}

fn builtin_ids() -> Result<Vec<String>, AppError> {
    Ok(state::builtin_records()?.iter().map(|r| r.id.clone()).collect())
}

/// The record's out-of-tree packs, as this command read them before the lock.
fn read_records(store: &Store) -> Result<Vec<PackRecord>, AppError> {
    Ok(store.recorded_packs()?.map(|r| state::out_of_tree(&r.packs)).unwrap_or_default())
}

/// The list a command starts from. In a trusted workspace, the config's `packs`, each entry
/// placed from the root and matched to a record by its canonical path: a matched entry keeps its
/// recorded copy unread, and any other is read to be validated and copied, or `pack_invalid`
/// naming it. A record whose source the config no longer lists is dropped. In an untrusted
/// workspace, whose config isn't loaded, the record's own sources.
fn entries(root: &Path, trusted: bool, read: &[PackRecord]) -> Result<Vec<Entry>, AppError> {
    if !trusted {
        return Ok(read.iter().map(|r| Entry { record: r.clone(), read: None }).collect());
    }
    let listed = config::for_workspace(&workspace::state_dir(root), true)?.config.packs;
    let mut entries = Vec::with_capacity(listed.len());
    for entry in &listed {
        let placed = root.join(entry);
        let canonical = fs::canonicalize(&placed).unwrap_or(placed);
        let key = canonical.display().to_string();
        if let Some(record) = read.iter().find(|r| r.source == key) {
            entries.push(Entry { record: record.clone(), read: None });
            continue;
        }
        let dir = state::place(root, entry)?;
        let source = state::read_named(&dir)?;
        let record = load::parse(source.clone()).map_err(|e| state::checked_by(e, &dir))?.record();
        entries.push(Entry { record, read: Some(source) });
    }
    Ok(entries)
}

/// The record and the config written from `entries`: every pack read here validated with the
/// rest, compiled and copied first, all before the lock; then, under it, the list rebuilt on the
/// record the lock finds. A matched entry takes the record as found, or as read when a concurrent
/// `pack remove` dropped it, since its copy still exists; a record that appeared since the read
/// is kept, appended; one present at the read that the list drops stays dropped. Returns the
/// record afterwards and whether its out-of-tree packs changed.
fn commit(
    store: &mut Store,
    state_dir: &Path,
    entries: Vec<Entry>,
    read: &[PackRecord],
) -> Result<(RecordedPacks, bool), AppError> {
    if entries.iter().any(|e| e.read.is_some()) {
        let mut sources = Vec::with_capacity(entries.len());
        for entry in &entries {
            sources.push(match &entry.read {
                Some(source) => source.clone(),
                None => state::load_copy(state_dir, &entry.record)?,
            });
        }
        state::validated(sources)?;
        for source in entries.iter().filter_map(|e| e.read.as_ref()) {
            state::copy(state_dir, source)?;
        }
    }
    let builtins = state::builtin_records()?;
    let config_file = state_dir.join(config::FILE);
    let at = clock::now().0;
    let mut changed = false;
    let after = store.record_packs(|current: Option<&RecordedPacks>| {
        let found = current.map(|r| state::out_of_tree(&r.packs)).unwrap_or_default();
        let mut list: Vec<PackRecord> = entries
            .iter()
            .map(|e| match e.read {
                Some(_) => e.record.clone(),
                None => found
                    .iter()
                    .find(|r| r.id == e.record.id && r.source == e.record.source)
                    .cloned()
                    .unwrap_or_else(|| e.record.clone()),
            })
            .collect();
        for record in &found {
            if !read.contains(record) && !list.iter().any(|r| r.id == record.id) {
                list.push(record.clone());
            }
        }
        changed = found != list;
        // Written inside the record's transaction: if the commit then fails, the config is ahead
        // of the record, which counts for nothing until the next `pack add` or `pack remove`
        // rebuilds both from it.
        write_config(&config_file, &list)?;
        let mut set = builtins.to_vec();
        set.extend(list);
        Ok::<_, AppError>(Some(state::record_of(set, Some(at.clone()))))
    })?;
    let after = after.ok_or_else(|| AppError::internal("the pack record wasn't written"))?;
    Ok((after, changed))
}

/// Write the config's top-level `packs` from `list`, each source's canonical absolute path in
/// record order, keeping every other byte of the file: the value is spliced in where it was, or
/// put on line 1. A private temporary file, renamed into place.
fn write_config(path: &Path, list: &[PackRecord]) -> Result<(), AppError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(AppError::internal(format!("{}: {e}", path.display()))),
    };
    let quoted: Vec<String> = list
        .iter()
        .map(|r| serde_json::to_string(&r.source))
        .collect::<Result<_, _>>()
        .map_err(AppError::internal)?;
    let value = format!("[{}]", quoted.join(", "));
    let root = DeTable::parse(&text).map_err(|e| {
        let line = e.span().map_or(1, |s| text[..s.start.min(text.len())].matches('\n').count() + 1);
        let shown = path.display().to_string();
        let what = text::get("argument.config_syntax", &[("file", &shown), ("line", &line)]);
        AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "config"}))
    })?;
    let span = root.get_ref().iter().find(|(k, _)| k.get_ref().as_ref() == "packs").map(|(_, v)| v.span());
    let spliced = match span {
        Some(span) => format!("{}{value}{}", &text[..span.start], &text[span.end..]),
        None => format!("packs = {value}\n{text}"),
    };
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&tmp);
    let io = |e: std::io::Error| AppError::internal(format!("{}: {e}", path.display()));
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(io)?;
    file.write_all(spliced.as_bytes()).and_then(|()| file.sync_all()).map_err(io)?;
    fs::rename(&tmp, path).map_err(io)
}

/// The reply: `line`, and in an untrusted workspace that its packs load once it's trusted, with
/// `workspace_untrusted` while its trust card is open.
fn reply(
    store: &Store,
    trusted: bool,
    mut line: String,
    pack: &PackRecord,
    (after, changed): (RecordedPacks, bool),
) -> Result<Reply<PackSetResult>, AppError> {
    let mut warnings = Vec::new();
    if !trusted {
        line.push(' ');
        line.push_str(&text::get("pack.untrusted", &[]));
        let snapshot = store.snapshot()?;
        if let Some(open) = cards::open_trust(&snapshot)? {
            warnings.push(cards::untrusted_warning(&open.event.id, &open.card));
        }
    }
    let loaded = after.packs.iter().filter(|r| r.source != BUILTIN).count() as u32;
    let result = PackSetResult {
        summary: line.clone(),
        id: pack.id.clone(),
        version: pack.version,
        copy: pack.sha256.clone(),
        changed,
        loaded,
        packs_hash: after.hash,
    };
    Ok(Reply { result, seq: store.seq()?, warnings, text: line })
}

/// What a person's accept of the changed-source card `id` takes, read before the review commits,
/// as `pack add` would (architecture.md, The pack set is workspace state): the pack's source as
/// it reads now, validated with the rest of the recorded set, compiled and copied. The review then
/// records it ([`cards::picked_pack`]). None for a reject, for anything but a changed-source card,
/// and for a pack no longer recorded. A source that no longer loads refuses the pick with
/// `pack_invalid`, whose hint checks it, and nothing is committed.
pub fn update_for_card(
    store: &Store,
    root: &Path,
    id: &str,
    verdict: Verdict,
) -> Result<Option<cards::PackUpdate>, AppError> {
    if verdict != Verdict::Accept {
        return Ok(None);
    }
    let snapshot = store.snapshot()?;
    let events = snapshot.card_events()?;
    let Ok(event) = edit::find_event(&events, id) else { return Ok(None) };
    let EventBody::Card(card) = &event.body else { return Ok(None) };
    if card.kind.0 != cards::PACK_SOURCE {
        return Ok(None);
    }
    let (card_id, card) = (event.id.clone(), card.clone());
    drop(snapshot);
    let recorded = read_records(store)?;
    let Some(record) = recorded.iter().find(|r| r.id == card.subject).cloned() else { return Ok(None) };
    let state_dir = workspace::state_dir(root);
    let check = |e: AppError| {
        if e.code == ErrorCode::PackInvalid {
            e.with_hint(format!("planroom doctor --pack {}", text::shell_word(&record.source)))
        } else {
            e
        }
    };
    let source = state::read_named(Path::new(&record.source)).map_err(check)?;
    let next = load::parse(source.clone()).map_err(check)?.record();
    if next.id != record.id {
        let shown = record.source.clone();
        let vars: [(&str, &dyn std::fmt::Display); 2] = [("id", &record.id), ("found", &next.id)];
        return Err(check(AppError::pack_invalid(&shown, 1, &next.id, "id_changed", &vars)));
    }
    let mut sources = Vec::with_capacity(recorded.len());
    for other in &recorded {
        sources.push(if other.id == record.id {
            source.clone()
        } else {
            state::load_copy(&state_dir, other)?
        });
    }
    state::validated(sources).map_err(check)?;
    state::copy(&state_dir, &source)?;
    let named = card.vars.get("to").map(String::as_str).unwrap_or_default();
    let newer = named.is_empty() || !next.sha256.starts_with(named);
    Ok(Some(cards::PackUpdate { card: card_id, record: next, newer }))
}
