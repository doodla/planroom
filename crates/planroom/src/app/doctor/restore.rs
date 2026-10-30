//! The restore (architecture.md, Store schema; DR-53, DR-73): finding the backup a workspace whose
//! state directory was lost can be restored from, and the card a scan deals to offer it. A backup
//! is tied to a workspace by the state directory its header names; on a temporary host, where the
//! data root is lost with the container, the redacted copy under `refs/planroom/backup/` stands in.
//!
//! A person's accept of the card reads the backup again ([`read_source`]) and writes its events
//! into the new store in the review's own transaction, each under its original ID and keys and
//! with its stored status. They are written with no `target`: the export's events carry none, the
//! column is read only for `scan` events, and a restore writes no `scan`; rebuilding each
//! writer's rule (an accept's copy takes the reviewed event's ID, an undo its first undo key)
//! would need the fold.
//!
//! `doctor --restore BACKUP` ([`run`]) is a person's own way to the same restore from an event
//! backup, and the way back from any restore, or down to an older engine, from a store copy.
//!
//! A copy's put-back sets aside the person edits the copy doesn't hold, as a replay set. The first
//! open that finds the store migrated past that set replays it ([`replay`], through the store's
//! registered hook), before `app::hook` installs the pack guard, since its events passed the guard
//! when they were first written. Each event keeps its ID, actor and keys, so a second replay is a
//! no-op, and is written by a writer instance of its own, so an edit made after the restore
//! conflicts with it whoever made it (DR-73). The way out of a replay that keeps failing is
//! `doctor --restore` of the pre-restore copy, which opens no store.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use p256::ecdsa::VerifyingKey;
use planroom_contract::{
    Card, CardKind, Choice, Client, Command, CommandName, CommandSpec, DoctorReq, ErrorCode, Event,
    EventBody, EventId, RestoreKind, RestoreReport, Seq, Stakes, Surface, Timestamp, Vocabulary,
};
use planroom_extract::git::Git;
use planroom_kernel::cas::{self, Authority, Proposed};
use planroom_kernel::fold::Writer;
use planroom_store::restore::{BackupKind, CopyRefused, PathRefused, PutBack, ReplayCount, ReplaySet, Spend};
use planroom_store::{Dealt, NewEvent, Planned, Store, StoreError};
use serde_json::json;

use crate::app::actor::{self, Acting, Bound, Caller, Refused, Terminal, TerminalPath};
use crate::app::cards::{self, RESTORE};
use crate::app::error::AppError;
use crate::app::{Reply, data, review, workspace};
use crate::packs::{self, state};
use crate::{clock, live, text};

/// The most segments a backup ref may hold before it is left alone.
pub const MAX_SEGMENTS: usize = 1_000;
/// The largest segment a backup ref may hold; a bigger one is left alone, unread.
pub const MAX_SEGMENT_BYTES: u64 = 64 << 20;
/// The most a backup ref may hold in all.
pub const MAX_REF_BYTES: u64 = 256 << 20;
/// The characters of a timestamp that make its date.
const DATE_CHARS: usize = 10;

/// Where a backup was found.
#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    /// `DATA_ROOT/backups/<workspace_id>/events.ndjson`.
    Backup { file: PathBuf },
    /// `refs/planroom/backup/<workspace_id>` in one repo of the workspace.
    Ref { repo: PathBuf, namespace: String, reference: String },
}

/// A backup a workspace can be restored from: whose it is, what it holds and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub workspace_id: String,
    /// How many person-authored events it holds.
    pub persons: u64,
    /// The `at` of the last of them.
    pub last_person_at: Timestamp,
    pub origin: Origin,
}

/// The backup to offer for the workspace whose root is `root` and whose state directory is
/// `state_dir`, which may be missing: of the event backups in the data root whose header names
/// this state directory and holds person-authored events, the one with the latest such edit; only
/// when there is none, on a temporary host, the same of the backup refs in the workspace's repos.
/// A backup of the workspace `current_id` itself is never one. Anything unreadable is skipped,
/// silently: a backup that cannot be read is no reason to fail a scan.
pub fn source_for(root: &Path, state_dir: &Path, current_id: Option<&str>) -> Option<Source> {
    let data = data::root().ok()?;
    let key = planroom_store::backup::state_key(state_dir)?;
    let mut best: Option<Source> = None;
    for found in planroom_store::restore::backups(&data) {
        let id = &found.header.workspace_id;
        let named = found.dir.file_name().and_then(|n| n.to_str());
        if found.header.state_dir.as_deref() != Some(key.as_str())
            || Some(id.as_str()) == current_id
            || named != Some(id.as_str())
        {
            continue;
        }
        let Ok(file) = planroom_store::restore::open_backup(&found.file) else { continue };
        match planroom_store::restore::read_export(BufReader::new(file), |_| ControlFlow::Continue(())) {
            Ok(summary) => {
                let Some(last) = summary.last_person_at.filter(|_| summary.persons > 0) else { continue };
                better(
                    &mut best,
                    Source {
                        workspace_id: id.clone(),
                        persons: summary.persons,
                        last_person_at: last,
                        origin: Origin::Backup { file: found.file.clone() },
                    },
                );
            }
            Err(_) => continue,
        }
    }
    if best.is_none() && crate::app::backup::temporary_host() {
        best = from_refs(root, current_id);
    }
    best
}

/// `candidate` in place of `best` when it holds the later edit, or the greater ID on a tie.
fn better(best: &mut Option<Source>, candidate: Source) {
    let beats = |held: &Source| {
        (&candidate.last_person_at.0, &candidate.workspace_id) > (&held.last_person_at.0, &held.workspace_id)
    };
    if best.as_ref().is_none_or(beats) {
        *best = Some(candidate);
    }
}

/// The best backup ref of the workspace at `root`'s repos.
fn from_refs(root: &Path, current_id: Option<&str>) -> Option<Source> {
    let ws = workspace::discover(root).ok()?;
    let mut best = None;
    for repo in &ws.repos {
        let git = Git::new(&repo.path);
        let Ok(refs) = git.backup_refs() else { continue };
        for (reference, _) in refs {
            let Some(id) = reference.strip_prefix("refs/planroom/backup/") else { continue };
            if !planroom_store::restore::valid_id(id) || Some(id) == current_id {
                continue;
            }
            if let Some((persons, last)) = read_ref(&git, &reference, id) {
                better(
                    &mut best,
                    Source {
                        workspace_id: id.to_owned(),
                        persons,
                        last_person_at: last,
                        origin: Origin::Ref {
                            repo: repo.path.clone(),
                            namespace: repo.name.clone(),
                            reference: reference.clone(),
                        },
                    },
                );
            }
        }
    }
    best
}

/// The person-authored edits a backup ref holds and the time of the last, or `None` for a ref that
/// is too big, isn't a backup of `id`, or can't be read. Sizes are checked before any blob is read.
fn read_ref(git: &Git, reference: &str, id: &str) -> Option<(u64, Timestamp)> {
    let reader = ref_reader(git, reference)?;
    let summary = planroom_store::restore::read_export(reader, |_| ControlFlow::Continue(())).ok()?;
    if summary.header.workspace_id != id || summary.persons == 0 {
        return None;
    }
    Some((summary.persons, summary.last_person_at?))
}

/// A backup ref's segments as one stream, or `None` for a ref that can't be listed or is too
/// big. Sizes are checked before any blob is read.
fn ref_reader<'a>(git: &'a Git, reference: &str) -> Option<Segments<'a>> {
    let segments = git.backup_segments(reference).ok()?;
    let total: u64 = segments.iter().map(|(_, _, size)| *size).sum();
    if segments.is_empty()
        || segments.len() > MAX_SEGMENTS
        || segments.iter().any(|(_, _, size)| *size > MAX_SEGMENT_BYTES)
        || total > MAX_REF_BYTES
    {
        return None;
    }
    Some(Segments::new(git, segments.into_iter().map(|(_, oid, _)| oid).collect()))
}

/// A backup ref's segments read as one stream, each blob fetched when the one before it is used
/// up and dropped after, so memory holds one segment at a time.
struct Segments<'a> {
    git: &'a Git,
    remaining: VecDeque<String>,
    current: Cursor<Vec<u8>>,
}

impl<'a> Segments<'a> {
    fn new(git: &'a Git, oids: VecDeque<String>) -> Self {
        Segments { git, remaining: oids, current: Cursor::new(Vec::new()) }
    }
}

impl BufRead for Segments<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        while self.current.position() as usize >= self.current.get_ref().len() {
            let Some(oid) = self.remaining.pop_front() else { break };
            let blob = self.git.blob(&oid).map_err(io::Error::other)?;
            self.current = Cursor::new(blob);
        }
        self.current.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.current.consume(amt);
    }
}

impl Read for Segments<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = {
            let available = self.fill_buf()?;
            let n = available.len().min(buf.len());
            buf[..n].copy_from_slice(&available[..n]);
            n
        };
        self.consume(n);
        Ok(n)
    }
}

/// The restore card for `source`: its subject is the backup's workspace; Restore is the
/// preselected pick, but the default stays Start empty, since a restore writes the whole store and
/// a kind's default is the choice that leaves the result as it is.
pub fn card_of(source: &Source) -> Card {
    let date: String = source.last_person_at.0.chars().take(DATE_CHARS).collect();
    let mut vars = std::collections::BTreeMap::from([
        ("edits".to_owned(), source.persons.to_string()),
        ("date".to_owned(), date),
    ]);
    let why = match &source.origin {
        Origin::Backup { .. } => {
            vars.insert("source".into(), "backup".into());
            "card.restore_why"
        }
        Origin::Ref { namespace, reference, .. } => {
            vars.insert("source".into(), "ref".into());
            vars.insert("repo".into(), namespace.clone());
            vars.insert("ref".into(), reference.clone());
            "card.restore_why_ref"
        }
    };
    Card {
        kind: CardKind(RESTORE.into()),
        subject: source.workspace_id.clone(),
        question: "card.restore_question".into(),
        why: why.into(),
        vars,
        stakes: Stakes::Person,
        choices: vec![
            Choice { id: "accept".into(), label: "card.restore_accept".into(), preselected: true },
            Choice { id: "reject".into(), label: "card.restore_reject".into(), preselected: false },
        ],
        default: "reject".into(),
        dealt_by: CommandName("scan".into()),
    }
}

/// What a person's accept of a restore card writes: the backup's events, read again at the pick.
#[derive(Debug, Clone)]
pub struct Restoring {
    /// The restore card.
    pub card: EventId,
    /// The backup's workspace, the card's subject.
    pub subject: String,
    /// Every event of the backup but its scans, in `seq` order, one for each key, each keeping
    /// its ID, keys, author, time and stored status.
    pub events: Vec<Dealt>,
    /// How many of them are person-authored.
    pub edits: u64,
    /// The date of the last of them.
    pub date: String,
}

/// Read again the backup that the restore card `card`, the event `id`, offers: the data root's
/// `backups/<subject>/events.ndjson`, or, for one dealt from a ref, that ref in the workspace's
/// repo the card names, under the same size caps; the header must name the card's subject. A
/// backup that is gone or can't be read is `not_found`, hinted with the reject of the card at
/// `head`.
pub fn read_source(id: &EventId, card: &Card, root: &Path, head: Seq) -> Result<Restoring, AppError> {
    let gone = |what: String| {
        AppError::new(ErrorCode::NotFound, &[("what", &"backup")])
            .with_message(text::get("review.restore_gone", &[("what", &what)]))
            .details(json!({"closest": []}))
            .with_hint(reject_line(id, head))
    };
    let subject = &card.subject;
    let mut read: Vec<Event> = Vec::new();
    let keep = |event: Event| {
        if !matches!(event.body, EventBody::Scan { .. }) {
            read.push(event);
        }
        ControlFlow::Continue(())
    };
    let summary = match card.vars.get("source").map(String::as_str) {
        Some("backup") => {
            let data = data::root()?;
            let file = planroom_store::backup::dir_of(&data, subject)
                .ok_or_else(|| gone(subject.clone()))?
                .join("events.ndjson");
            let opened = planroom_store::restore::open_backup(&file)
                .map_err(|e| gone(format!("{}: {e}", file.display())))?;
            planroom_store::restore::read_export(BufReader::new(opened), keep)
                .map_err(|e| gone(format!("{}: {e}", file.display())))?
        }
        Some("ref") => {
            let (Some(repo), Some(reference)) = (card.vars.get("repo"), card.vars.get("ref")) else {
                return Err(gone(subject.clone()));
            };
            let named = format!("{reference} in {repo}");
            let ws = workspace::discover(root).map_err(|_| gone(named.clone()))?;
            let found = ws.repos.iter().find(|r| &r.name == repo).ok_or_else(|| gone(named.clone()))?;
            let git = Git::new(&found.path);
            let reader = ref_reader(&git, reference).ok_or_else(|| gone(named.clone()))?;
            planroom_store::restore::read_export(reader, keep).map_err(|e| gone(format!("{named}: {e}")))?
        }
        _ => return Err(gone(subject.clone())),
    };
    if summary.header.workspace_id != *subject {
        return Err(gone(subject.clone()));
    }
    read.sort_by_key(|e| e.seq);
    let mut seen = std::collections::BTreeSet::new();
    read.retain(|e| seen.insert(e.idem.clone()));
    let persons: Vec<&Event> =
        read.iter().filter(|e| matches!(e.actor, planroom_contract::Actor::Person { .. })).collect();
    let edits = persons.len() as u64;
    let date = persons.last().map(|e| e.at.0.chars().take(DATE_CHARS).collect()).unwrap_or_default();
    let events = read
        .into_iter()
        .map(|e| Dealt {
            idem: e.idem,
            request_hash: e.request_hash,
            event: NewEvent {
                id: e.id,
                at: e.at,
                actor: e.actor,
                client: e.client,
                project: e.project,
                cause: e.cause,
                status: e.status,
                target: None,
                body: e.body,
            },
        })
        .collect();
    Ok(Restoring { card: id.clone(), subject: subject.clone(), events, edits, date })
}

/// The command that rejects the restore card `card` at `head`: what to run when its backup can't
/// be restored.
pub fn reject_line(card: &EventId, head: Seq) -> String {
    format!("planroom review --base {head} {} reject", text::shell_word(&card.0))
}

/// `planroom doctor --restore BACKUP` (DR-53): put the backup at `path`, BACKUP as `given`, back
/// into the workspace at or above `cwd`, as a person only. Where BACKUP may be is checked first,
/// then who the caller is, and nothing is copied or written before both pass.
///
/// - An event backup, `events.ndjson`, does what the accept of the workspace's open restore card
///   for that backup does, and resolves the card in the same commit.
/// - A store copy, `store-v<N>.db` or `store-pre-restore-<secs>.db`, replaces the store in one
///   transaction ([`planroom_store::restore::put_back`]), opening no store through the engine, so
///   it serves even when every open refuses, and sets aside the person's edits the copy lacks.
///
/// Every `person_required` names this exact command line as the way a person runs it.
pub fn run(
    path: &Path,
    given: &str,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Reply<RestoreReport>, AppError> {
    let data = data::root()?;
    let (id, kind) = match planroom_store::restore::backup_path(&data, path)? {
        Ok(found) => found,
        Err(PathRefused::NotABackup) => {
            return Err(AppError::invalid_argument("restore", "argument.restore_backup"));
        }
        Err(PathRefused::Missing { closest }) => {
            let what = format!("backup {}", text::shell_word(given));
            let closest: Vec<String> = closest.into_iter().collect();
            return Err(
                AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest}))
            );
        }
    };
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let store_path = workspace::store_path(&root);
    let live = planroom_store::locate(&store_path)?.ok_or_else(|| AppError::workspace_not_found(&root))?;
    let ask = Asking {
        caller,
        spec: Command::Doctor(DoctorReq::default()).spec(),
        line: format!("planroom doctor --restore {}", text::shell_word(given)),
        now: clock::unix_micros() / 1_000_000,
    };
    // An agent is never a person: refused before anything is read or opened.
    if caller.actor.as_deref().is_some_and(|a| !a.starts_with("person:")) {
        return Err(ask.refused(Refused::NotAPerson));
    }
    let key = planroom_store::restore::read_meta(&live, "person_pubkey")?;
    let keys = actor::keys_from(key.as_deref(), &root, caller, &ask.name(), &|| Ok(None))
        .map_err(|e| ask.naming(e))?;
    let keys: Vec<&VerifyingKey> = keys.iter().collect();
    match kind {
        BackupKind::Copy => put_copy_back(&ask, &live, path, &keys, terminal),
        BackupKind::Events => restore_events(&ask, &root, &store_path, &id, file_name(path), &keys, terminal),
    }
}

/// Who asks for a restore, and how a person runs it.
struct Asking<'a> {
    caller: &'a Caller,
    spec: &'static CommandSpec,
    /// The exact command line a person runs: `planroom doctor --restore <BACKUP>`.
    line: String,
    now: u64,
}

impl Asking<'_> {
    fn name(&self) -> String {
        self.spec.name()
    }

    /// `e`, when it is a `person_required`, naming the exact line.
    fn naming(&self, e: AppError) -> AppError {
        actor::naming_line(e, &self.name(), TerminalPath::Takes, &self.line)
    }

    fn refused(&self, why: Refused) -> AppError {
        self.naming(actor::person_required(&self.name(), why, TerminalPath::Takes))
    }

    /// Resolve the caller as a person, asking `question` at a terminal, and refuse a token whose
    /// digest the live store at `live` already spent, before anything is written.
    fn person(
        &self,
        live: &Path,
        keys: &[&VerifyingKey],
        terminal: &mut dyn Terminal,
        question: &str,
    ) -> Result<Acting, AppError> {
        let acting = actor::person_asking(
            self.caller,
            self.spec,
            &Bound { idem: None, base: None },
            keys,
            self.now,
            TerminalPath::Takes,
            terminal,
            question,
        )
        .map_err(|e| self.naming(e))?;
        if let Some(digest) = &acting.digest {
            let spent = planroom_store::restore::read_meta(live, "spent_tokens")?
                .and_then(|held| {
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&held).ok()
                })
                .is_some_and(|held| held.contains_key(digest));
            if spent {
                return Err(self.refused(Refused::Spent));
            }
        }
        Ok(acting)
    }

    /// When a token is spent: kept past both ends of its window, as every spend keeps it.
    fn expires(&self) -> u64 {
        self.now + 2 * actor::TOKEN_WINDOW_SECS + 1
    }
}

/// The name of the file at `path`.
fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The id of the person this OS user is, as a confirmation names them.
fn person_id() -> String {
    match actor::os_person() {
        planroom_contract::Actor::Person { id, .. } => id,
        _ => String::new(),
    }
}

/// Count what putting `copy` back into the live store at `live` would set aside, read only, and
/// resolve the caller as a person, asking with that count at a terminal; who acts, and the count.
fn confirm_copy(
    ask: &Asking<'_>,
    live: &Path,
    copy: &Path,
    keys: &[&VerifyingKey],
    terminal: &mut dyn Terminal,
) -> Result<(Acting, u64), AppError> {
    let count = planroom_store::restore::count_set_aside(live, copy)?;
    let version = planroom_store::restore::read_meta(live, "engine_version")?.unwrap_or_default();
    let (file, person) = (file_name(copy), person_id());
    let question = if count == 0 {
        text::get("doctor.restore_confirm_none", &[("file", &file), ("person", &person)])
    } else {
        text::get(
            "doctor.restore_confirm",
            &[("file", &file), ("person", &person), ("count", &count), ("version", &version)],
        )
    };
    Ok((ask.person(live, keys, terminal, &question)?, count))
}

/// The copy form: put the store copy `copy` back in place of the live store at `live`.
fn put_copy_back(
    ask: &Asking<'_>,
    live: &Path,
    copy: &Path,
    keys: &[&VerifyingKey],
    terminal: &mut dyn Terminal,
) -> Result<Reply<RestoreReport>, AppError> {
    // Another workspace's copy, or one that fails its check, is the wrong file to pick: refused
    // as usage before anything is asked.
    if let Err(why) = planroom_store::restore::copy_fits(live, copy)? {
        let key = match why {
            CopyRefused::OtherWorkspace => "argument.restore_other_workspace",
            CopyRefused::Damaged => "argument.restore_damaged",
        };
        return Err(AppError::invalid_argument("restore", key));
    }
    let (acting, _) = confirm_copy(ask, live, copy, keys, terminal)?;
    let spend = acting.digest.as_deref().map(|digest| Spend { digest, now: ask.now, expires: ask.expires() });
    let report = match planroom_store::restore::put_back(live, copy, spend)? {
        PutBack::Done(report) => report,
        PutBack::TokenSpent => return Err(ask.refused(Refused::Spent)),
    };
    let from = file_name(copy);
    let version = report.made_on.clone().unwrap_or_default();
    let summary = if report.set_aside == 0 {
        text::get("doctor.restore_done_none", &[("file", &from), ("seq", &report.seq)])
    } else {
        text::get(
            "doctor.restore_done",
            &[("file", &from), ("seq", &report.seq), ("count", &report.set_aside), ("version", &version)],
        )
    };
    let result = RestoreReport {
        summary: summary.clone(),
        restored: RestoreKind::Copy,
        from,
        seq: report.seq,
        events: planroom_store::restore::person_events(live)?,
        pre_restore: Some(file_name(&report.pre_restore)),
        set_aside: u32::try_from(report.set_aside).unwrap_or(u32::MAX),
        replay_set: report.replay.as_ref().map(|(file, _)| file_name(file)),
        made_on: report.made_on,
        card: None,
    };
    Ok(Reply { result, seq: report.seq, warnings: vec![], text: summary })
}

/// The event form: the accept of the open restore card for the backup of the workspace `id`, the
/// file `from`, resolving the card in the restore's own commit.
fn restore_events(
    ask: &Asking<'_>,
    root: &Path,
    store_path: &Path,
    id: &str,
    from: String,
    keys: &[&VerifyingKey],
    terminal: &mut dyn Terminal,
) -> Result<Reply<RestoreReport>, AppError> {
    let mut store = Store::open_existing(store_path)?.ok_or_else(|| AppError::workspace_not_found(root))?;
    crate::app::hook(&mut store);
    let snapshot = store.snapshot()?;
    let head = snapshot.seq();
    let Some(open) = cards::open_of(&snapshot, RESTORE, id)? else {
        let closed = last_restore_card(&snapshot, id)?;
        let authored = snapshot.person_authored()?;
        drop(snapshot);
        if let Some(card) = closed {
            let set = state::for_read(root, store.recorded_packs()?.as_ref())?;
            return Err(review::not_open(&store, &card, set.vocabulary()));
        }
        return Err(if authored {
            AppError::invalid_argument("restore", "doctor.restore_authored").without_hint()
        } else {
            AppError::invalid_argument("restore", "doctor.restore_no_card").with_hint("planroom scan".into())
        });
    };
    drop(snapshot);
    let card = open.event.id.clone();
    // The file given is what is restored, the data root's backup of that workspace, even when
    // the card was dealt from a temporary host's ref.
    let mut given = open.card.clone();
    given.vars.insert("source".into(), "backup".into());
    let restoring = read_source(&card, &given, root, head)?;
    let question = text::get(
        "doctor.restore_confirm_events",
        &[("edits", &restoring.edits), ("date", &restoring.date), ("person", &person_id())],
    );
    let acting = ask.person(store.path(), keys, terminal, &question)?;
    if let Some(digest) = &acting.digest
        && !store.spend_token(digest, ask.now, ask.expires())?
    {
        return Err(ask.refused(Refused::Spent));
    }
    let writer = Writer { actor: acting.actor, client: ask.caller.client() };
    let (written, copy) = review::accept_restore(&mut store, root, writer, &card, &restoring)?;
    let summary = text::get(
        "doctor.restore_events",
        &[("edits", &restoring.edits), ("date", &restoring.date), ("file", &from)],
    );
    let seq = written.reply.seq;
    let result = RestoreReport {
        summary: summary.clone(),
        restored: RestoreKind::Events,
        from,
        seq,
        events: restoring.edits,
        pre_restore: Some(file_name(&copy)),
        set_aside: 0,
        replay_set: None,
        made_on: None,
        card: Some(card.0),
    };
    Ok(Reply { result, seq, warnings: written.reply.warnings, text: summary })
}

/// The last restore card about `subject` that is no longer open, if any.
fn last_restore_card(
    snapshot: &planroom_store::Snapshot<'_>,
    subject: &str,
) -> Result<Option<EventId>, AppError> {
    Ok(snapshot
        .card_events()?
        .into_iter()
        .rev()
        .find(|e| matches!(&e.body, EventBody::Card(c) if c.kind.0 == RESTORE && c.subject == subject))
        .map(|e| e.id))
}

/// Why one replayed event wrote nothing.
enum Replay {
    /// Skipped and counted: its ID is held already, it undoes or reviews an event the store
    /// doesn't hold, or its check closes a loop or finds it superseded.
    Skip,
    Store(StoreError),
}

impl From<StoreError> for Replay {
    fn from(e: StoreError) -> Replay {
        Replay::Store(e)
    }
}

/// Replay the set a copy restore set aside, from its file, into `store`: one commit per event under
/// its original keys, each edit and undo checked against the head with the copy's last `seq` as
/// its base, so a write to the same field after the restore makes it a `conflict`. Only a store
/// error stops it, and the set stays for the next open; the counts go to stderr alone.
pub fn replay(store: &mut Store, set: &ReplaySet, file: &Path) -> Result<ReplayCount, StoreError> {
    // The workspace's recorded pack set, resolved before any write's transaction opens; the
    // built-ins when the state directory names no root or the set doesn't resolve.
    let recorded = workspace::root_of_state_dir(store.state_dir()).and_then(|root| {
        let packs = store.recorded_packs().ok()?;
        state::for_read(&root, packs.as_ref()).ok()
    });
    let vocabulary = match &recorded {
        Some(set) => set.vocabulary(),
        None => packs::vocabulary().map_err(|_| StoreError::Io("the built-in packs don't load".into()))?,
    };
    let read = File::open(file).map_err(|e| StoreError::Io(format!("{}: {e}", file.display())))?;
    let mut events = Vec::new();
    planroom_store::restore::read_export(BufReader::new(read), |event| {
        events.push(event);
        ControlFlow::Continue(())
    })?;
    let client = Client { surface: Surface::Cli, id: format!("restore:{}", set.file) };
    let mut count = ReplayCount::default();
    for event in events {
        match replay_one(store, set.base, vocabulary, &client, event) {
            Ok(true) => count.replayed += 1,
            // A retry finds the key and writes nothing; a key a later write took is left to it.
            Ok(false) | Err(Replay::Skip | Replay::Store(StoreError::IdempotencyMismatch { .. })) => {
                count.skipped += 1;
            }
            Err(Replay::Store(e)) => return Err(e),
        }
    }
    // The counts alone, never an event's content; this build logs to stderr, which `--json`
    // leaves free.
    eprintln!(
        "planroom: replayed a restore's set-aside edits: {} written, {} skipped",
        count.replayed, count.skipped
    );
    Ok(count)
}

/// One replayed event; whether it was written.
fn replay_one(
    store: &mut Store,
    base: Seq,
    vocabulary: &Vocabulary,
    client: &Client,
    event: Event,
) -> Result<bool, Replay> {
    let Event { id, idem, request_hash, at, actor, project, cause, status, body, .. } = event;
    let proposed = Proposed {
        writer: Writer { actor: actor.clone(), client: client.clone() },
        authority: Authority::of(&actor),
        base,
    };
    let committed = store.commit_keyed(
        &idem,
        &request_hash,
        |snapshot| {
            if snapshot.holds_event(&id.0)? {
                return Err(Replay::Skip);
            }
            let (status, body) = match body {
                EventBody::Edit(mut edit) => {
                    edit.base = base;
                    let folded = live::folded(snapshot, vocabulary)?;
                    let status = cas::check_edit(&folded, &proposed, &edit).status().ok_or(Replay::Skip)?;
                    (status, EventBody::Edit(edit))
                }
                EventBody::Undo { of } => {
                    if !snapshot.holds_event(&of.0)? {
                        return Err(Replay::Skip);
                    }
                    let folded = live::folded(snapshot, vocabulary)?;
                    let status = cas::check_undo(&folded, &proposed, &of).status().ok_or(Replay::Skip)?;
                    (status, EventBody::Undo { of })
                }
                EventBody::Review { of, verdict } => {
                    if !snapshot.holds_event(&of.0)? {
                        return Err(Replay::Skip);
                    }
                    (status, EventBody::Review { of, verdict })
                }
                body => (status, body),
            };
            Ok(vec![Planned::Command(NewEvent {
                id,
                at,
                actor: actor.clone(),
                client: client.clone(),
                project,
                cause,
                status,
                target: None,
                body,
            })])
        },
        |seqs| json!({ "seq": seqs[0], "replayed": true }),
    )?;
    Ok(!committed.replayed)
}

#[cfg(test)]
mod tests {
    use planroom_contract::{Client, Surface, Timestamp};
    use planroom_store::{Init, Store, Write};

    use super::*;

    /// A terminal that records each question and answers yes.
    #[derive(Default)]
    struct Recording(Vec<String>);

    impl Terminal for Recording {
        fn present(&mut self) -> bool {
            true
        }

        fn confirm(&mut self, question: &str) -> Option<bool> {
            self.0.push(question.to_owned());
            Some(true)
        }
    }

    fn person_event(store: &mut Store, n: usize) {
        let write = Write {
            idem: format!("w{n}"),
            request_hash: "h".into(),
            events: vec![NewEvent {
                id: EventId(format!("e{n}")),
                at: Timestamp("2026-10-30T00:00:00Z".into()),
                actor: planroom_contract::Actor::Person { id: "ada".into(), name: None },
                client: Client { surface: Surface::Test, id: "test".into() },
                project: None,
                cause: None,
                status: planroom_contract::Status::Applied,
                target: None,
                body: EventBody::Version { project: None, label: format!("v{n}") },
            }],
        };
        store.commit(write, |_| json!({})).unwrap();
    }

    #[test]
    fn the_confirmation_counts_the_set_aside_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let live = tmp.path().join("store.db");
        let init = Init {
            workspace_id: "ws-confirm".into(),
            engine_version: "0.2.0".into(),
            created_at: "2026-10-30T00:00:00Z".into(),
        };
        let mut store = Store::open(&live, &init).unwrap();
        for n in 1..=3 {
            person_event(&mut store, n);
        }
        drop(store);
        let copy = tmp.path().join("store-v1.db");
        std::fs::copy(&live, &copy).unwrap();
        let mut store = Store::open(&live, &init).unwrap();
        for n in 4..=5 {
            person_event(&mut store, n);
        }
        drop(store);

        let mut terminal = Recording::default();
        let caller = Caller::default();
        let ask = Asking {
            caller: &caller,
            spec: Command::Doctor(DoctorReq::default()).spec(),
            line: "planroom doctor --restore store-v1.db".into(),
            now: 0,
        };
        let (acting, count) = confirm_copy(&ask, &live, &copy, &[], &mut terminal).unwrap();
        assert_eq!(count, 2);
        assert_eq!(acting.digest, None, "a confirmation spends no token");
        assert_eq!(terminal.0.len(), 1, "{:?}", terminal.0);
        assert!(terminal.0[0].contains("set aside 2 edits"), "{:?}", terminal.0);
        assert!(terminal.0[0].starts_with("Restore store-v1.db as "), "{:?}", terminal.0);
    }
}
