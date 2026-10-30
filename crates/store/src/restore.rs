//! The restore (architecture.md, Store schema; DR-53, DR-73): reading the event backups and the
//! store copies under `DATA_ROOT/backups/`, the pre-restore copy, and the put-back of a copy into
//! the live store in one transaction, so the app layer above never touches SQL or backup bytes.
//!
//! - Restored and replayed events are written with `target: None`. An export's `Event` has no
//!   `target`; the column is read only for `scan` events (`read.rs`'s `active_scans` and
//!   `active_snapshots`), and a restore skips every `scan` event.
//! - The put-back sets the person-authored edits the copy doesn't hold aside as a replay set,
//!   `replay-<secs>.ndjson` beside the copy, named in `meta.replay_set`. The way out of a replay
//!   that keeps failing is `doctor --restore` of the pre-restore copy, which opens no store.
//! - The replay runs before the pack guard is installed: its events passed the guard when they
//!   were first written.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write as _};
use std::ops::ControlFlow;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use planroom_contract::format::{self, FormatError, ReadError};
use planroom_contract::{Actor, Event, ExportHeader, Seq, Timestamp};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{Map, Value, json};

use crate::backup::{backups_area, dir_of};
use crate::migrate::{KEPT_COPIES, private_dir, prune_copies, vacuum_copy};
use crate::pointer::{sync_dir, valid_workspace_id};
use crate::read::{Snapshot, decode_rows};
use crate::{BUSY_TIMEOUT, STORE_VERSION, Store, StoreError, fs_check};

/// The longest line a backup may hold; a longer one is refused, never read whole.
pub const MAX_LINE: usize = 4 * 1024 * 1024;
/// The most of a backup's first line [`backups`] reads for its header.
const HEADER_CAP: u64 = 64 * 1024;
/// The event backup's file name in its directory.
const EVENTS: &str = "events.ndjson";
/// The name a pre-restore copy starts with.
const PRE_RESTORE: &str = "store-pre-restore-";
/// The name a migration copy starts with.
const MIGRATION_COPY: &str = "store-v";

/// Whether `id` can name a backup directory.
pub fn valid_id(id: &str) -> bool {
    valid_workspace_id(id)
}

/// What [`read_export`] found in an export or a backup besides its events.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub header: ExportHeader,
    /// How many of the events read were person-authored.
    pub persons: u64,
    /// The `at` of the last of them.
    pub last_person_at: Option<Timestamp>,
    /// Whether the last line had no newline, from a process killed partway through it, and was
    /// left out.
    pub torn: bool,
}

/// Read an event export or backup from `r`: its header line, then each event, handed to `each`
/// in file order, never held here; `ControlFlow::Break` stops the read. A last line with no
/// newline is left out and sets `torn`. A header in a newer format is `NewerFile`, whose `file`
/// is empty for the caller to name; a line over [`MAX_LINE`], or one that isn't an event, is `Io`.
pub fn read_export(
    r: impl BufRead,
    mut each: impl FnMut(Event) -> ControlFlow<()>,
) -> Result<Summary, StoreError> {
    let mut r = r;
    let mut line = Vec::new();
    let header = match next_line(&mut r, &mut line)? {
        Line::Whole => parse_header(&line)?,
        Line::Torn | Line::End => return Err(StoreError::Io("the backup has no header line".into())),
    };
    let mut summary = Summary { header, persons: 0, last_person_at: None, torn: false };
    loop {
        match next_line(&mut r, &mut line)? {
            Line::End => break,
            Line::Torn => {
                summary.torn = true;
                break;
            }
            Line::Whole => {}
        }
        let event: Event = serde_json::from_slice(&line)
            .map_err(|e| StoreError::Io(format!("a backup line isn't an event: {e}")))?;
        if matches!(event.actor, Actor::Person { .. }) {
            summary.persons += 1;
            summary.last_person_at = Some(event.at.clone());
        }
        if each(event).is_break() {
            break;
        }
    }
    Ok(summary)
}

enum Line {
    /// A line ended by a newline, which `buf` holds without it.
    Whole,
    /// A last line with no newline.
    Torn,
    End,
}

fn next_line(r: &mut impl BufRead, buf: &mut Vec<u8>) -> Result<Line, StoreError> {
    buf.clear();
    let read = r
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', buf)
        .map_err(|e| StoreError::Io(format!("reading a backup: {e}")))?;
    if read == 0 {
        return Ok(Line::End);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        return Ok(Line::Whole);
    }
    if buf.len() > MAX_LINE {
        return Err(StoreError::Io("a backup line over 4 MiB".into()));
    }
    Ok(Line::Torn)
}

fn parse_header(line: &[u8]) -> Result<ExportHeader, StoreError> {
    let text =
        std::str::from_utf8(line).map_err(|_| StoreError::Io("the backup's header isn't UTF-8".into()))?;
    format::read_str::<ExportHeader>(text).map_err(|e| match e {
        ReadError::Format(FormatError::Newer { found, supported }) => {
            StoreError::NewerFile { file: PathBuf::new(), found, supported }
        }
        other => StoreError::Io(format!("not an event backup: {other}")),
    })
}

/// One workspace's event backup, by its header.
#[derive(Debug, Clone, PartialEq)]
pub struct BackupHeader {
    /// `DATA_ROOT/backups/<workspace_id>`.
    pub dir: PathBuf,
    /// `events.ndjson` in `dir`.
    pub file: PathBuf,
    pub header: ExportHeader,
}

/// Every event backup under `data_root`, by directory name: each directory directly in the
/// backups area, not a link, whose name is a workspace ID and whose `events.ndjson` starts with a
/// header line. Only that first line is read; anything unreadable is skipped.
pub fn backups(data_root: &Path) -> Vec<BackupHeader> {
    let Ok(entries) = fs::read_dir(backups_area(data_root)) else { return Vec::new() };
    let mut found: Vec<BackupHeader> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let dir = dir_of(data_root, &name)?;
            let file = dir.join(EVENTS);
            let header = first_header(&file)?;
            Some(BackupHeader { dir, file, header })
        })
        .collect();
    found.sort_by(|a, b| a.dir.cmp(&b.dir));
    found
}

/// Open the backup file `file` to read it, as `backup_path` would take it: a regular file, not a
/// link, whose directory isn't a link either, so a file outside the backups area never stands in
/// for one. The file opened must be the one checked, or a link swapped in between is refused too.
pub fn open_backup(file: &Path) -> io::Result<File> {
    let not_a_backup =
        || io::Error::new(io::ErrorKind::InvalidInput, "not a regular file in a backup directory");
    let dir = file.parent().ok_or_else(not_a_backup)?;
    if !fs::symlink_metadata(dir)?.file_type().is_dir() {
        return Err(not_a_backup());
    }
    let checked = fs::symlink_metadata(file)?;
    if !checked.file_type().is_file() {
        return Err(not_a_backup());
    }
    let opened = File::open(file)?;
    let meta = opened.metadata()?;
    if (meta.dev(), meta.ino()) != (checked.dev(), checked.ino()) {
        return Err(not_a_backup());
    }
    Ok(opened)
}

fn first_header(file: &Path) -> Option<ExportHeader> {
    let mut line = Vec::new();
    BufReader::new(open_backup(file).ok()?.take(HEADER_CAP)).read_until(b'\n', &mut line).ok()?;
    if line.pop() != Some(b'\n') {
        return None;
    }
    format::read_str::<ExportHeader>(std::str::from_utf8(&line).ok()?).ok()
}

/// What a backup path names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupKind {
    /// A workspace's event backup, `events.ndjson`.
    Events,
    /// A copy of the store: a migration's `store-vN.db` or a restore's `store-pre-restore-N.db`.
    Copy,
}

/// Why [`backup_path`] refused a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRefused {
    /// No file there, in a valid backup directory; `closest` is the newest store copy in it.
    Missing { closest: Option<String> },
    /// Anything that isn't a regular file directly in `DATA_ROOT/backups/<workspace_id>/` with a
    /// backup's name.
    NotABackup,
}

/// Check that `given` is a backup under `data_root`: a regular file, not a link, directly in
/// `backups/<workspace_id>/` once both are made canonical, named `events.ndjson`, `store-vN.db` or
/// `store-pre-restore-N.db`. Returns the workspace ID and what the file is.
pub fn backup_path(
    data_root: &Path,
    given: &Path,
) -> Result<Result<(String, BackupKind), PathRefused>, StoreError> {
    let root = fs::canonicalize(data_root).map_err(|e| io_error(data_root, e))?;
    let refused = Ok(Err(PathRefused::NotABackup));
    let (Some(parent), Some(name)) = (given.parent(), given.file_name().and_then(OsStr::to_str)) else {
        return refused;
    };
    let parent = if parent.as_os_str().is_empty() { Path::new(".") } else { parent };
    let Ok(dir) = fs::canonicalize(parent) else { return refused };
    let Some(id) = dir.file_name().and_then(OsStr::to_str) else { return refused };
    if !valid_id(id) || dir.parent() != Some(backups_area(&root).as_path()) {
        return refused;
    }
    match fs::symlink_metadata(given) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(Err(PathRefused::Missing { closest: newest_copy(&dir) }));
        }
        Ok(meta) if meta.file_type().is_file() => {}
        _ => return refused,
    }
    let kind = if name == EVENTS {
        BackupKind::Events
    } else if copy_number(name).is_some() {
        BackupKind::Copy
    } else {
        return refused;
    };
    Ok(Ok((id.to_owned(), kind)))
}

/// The `N` of a store copy's name, `store-vN.db` or `store-pre-restore-N.db`.
fn copy_number(name: &str) -> Option<u64> {
    numbered(name, MIGRATION_COPY).or_else(|| numbered(name, PRE_RESTORE))
}

/// The `N` of `<prefix>N.db`.
fn numbered(name: &str, prefix: &str) -> Option<u64> {
    let n = name.strip_prefix(prefix)?.strip_suffix(".db")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

/// The store copy in `dir` with the highest number in its name.
fn newest_copy(dir: &Path) -> Option<String> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            Some((copy_number(&name)?, e.path()))
        })
        .max_by_key(|c| c.0)
        .map(|(_, path)| path.display().to_string())
}

/// A connection to the database at `path` that can only read it.
fn read_only(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(conn)
}

/// One `meta` value of the database at `path`, read on a read-only connection of its own.
pub fn read_meta(path: &Path, key: &str) -> Result<Option<String>, StoreError> {
    let conn = read_only(path)?;
    Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

impl Snapshot<'_> {
    /// Whether the store holds any person-authored event.
    pub fn person_authored(&self) -> Result<bool, StoreError> {
        Ok(self
            .tx()
            .query_row(
                "SELECT 1 FROM events WHERE json_extract(actor, '$.kind') = 'person' LIMIT 1",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Whether the store holds the event with this ID.
    pub fn holds_event(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self.tx().query_row("SELECT 1 FROM events WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some())
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Copy the store at `store_path`, with `VACUUM INTO`, checked like a migration's copy, to
/// `DATA_ROOT/backups/<workspace_id>/store-pre-restore-<secs>.db`, keeping the last three; so
/// restoring that copy undoes the restore.
pub fn pre_restore_copy(store_path: &Path, workspace_id: &str) -> Result<PathBuf, StoreError> {
    let env = fs_check::environment().ok_or_else(|| {
        StoreError::Io(format!(
            "{}: a restore needs the data root for its pre-restore copy, and this process has none",
            store_path.display()
        ))
    })?;
    pre_restore_copy_in(&env.data_root, store_path, workspace_id)
}

/// [`pre_restore_copy`] under `data_root`.
pub(crate) fn pre_restore_copy_in(
    data_root: &Path,
    store_path: &Path,
    workspace_id: &str,
) -> Result<PathBuf, StoreError> {
    let dir = dir_of(data_root, workspace_id).ok_or_else(|| {
        StoreError::Io(format!("{}: its workspace ID can't name a backup directory", store_path.display()))
    })?;
    private_dir(&dir)?;
    // vacuum_copy renames over a file already there, so a second restore in the same second
    // takes the next free name.
    let mut secs = now_secs();
    while dir.join(format!("{PRE_RESTORE}{secs}.db")).exists() {
        secs += 1;
    }
    let copy = vacuum_copy(store_path, &dir, &format!("{PRE_RESTORE}{secs}.db"))?;
    prune_copies(&dir, PRE_RESTORE, KEPT_COPIES);
    Ok(copy)
}

/// The newest pre-restore copy of `workspace_id`'s store whose last `seq` is below `seq`: the
/// copy a restore committed at `seq` wrote first. Reads at most the kept copies.
pub fn pre_restore_before(data_root: &Path, workspace_id: &str, seq: Seq) -> Option<PathBuf> {
    let dir = dir_of(data_root, workspace_id)?;
    let mut copies: Vec<(u64, PathBuf)> = fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| Some((numbered(e.file_name().to_str()?, PRE_RESTORE)?, e.path())))
        .collect();
    copies.sort_by_key(|c| std::cmp::Reverse(c.0));
    copies.into_iter().take(KEPT_COPIES).map(|c| c.1).find(|path| {
        read_only(path)
            .and_then(|conn| {
                Ok(conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |r| r.get::<_, i64>(0))?)
            })
            .is_ok_and(|head| (head as Seq) < seq)
    })
}

/// `meta.spent_tokens`, parsed, with the digests expired by `now` dropped.
pub(crate) fn merge_spent(held: Option<&str>, now: u64) -> Result<Map<String, Value>, StoreError> {
    let mut spent: Map<String, Value> = match held {
        Some(text) => serde_json::from_str(text)?,
        None => Map::new(),
    };
    spent.retain(|_, until| until.as_u64().is_some_and(|u| u > now));
    Ok(spent)
}

/// The edits a put-back set aside, as `meta.replay_set` names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySet {
    /// The replay file's name, in the backup directory.
    pub file: String,
    /// The restored copy's last `seq`, the head both timelines share: each replayed edit's base.
    pub base: Seq,
    /// The restored copy's store version; the set waits for a migration above it.
    pub store_version: u32,
    /// How many events the file holds after its header.
    pub count: u64,
}

impl ReplaySet {
    /// The JSON `meta.replay_set` holds.
    pub fn to_json(&self) -> String {
        json!({"file": self.file, "base": self.base, "store_version": self.store_version, "count": self.count})
            .to_string()
    }

    /// Parse `meta.replay_set`; `None` when it isn't one.
    pub fn parse(text: &str) -> Option<ReplaySet> {
        let v: Value = serde_json::from_str(text).ok()?;
        Some(ReplaySet {
            file: v.get("file")?.as_str()?.to_owned(),
            base: v.get("base")?.as_u64()?,
            store_version: u32::try_from(v.get("store_version")?.as_u64()?).ok()?,
            count: v.get("count")?.as_u64()?,
        })
    }
}

/// What a replay of a set did: the events it wrote, and those it skipped with nothing written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayCount {
    pub replayed: u64,
    pub skipped: u64,
}

/// Writes a set's events back into the store: the set, and its file, which the store resolves
/// from its data root and workspace ID so the app never builds a path under the data root.
pub type ReplayHook = fn(&mut Store, &ReplaySet, &Path) -> Result<ReplayCount, StoreError>;

static REPLAY: OnceLock<ReplayHook> = OnceLock::new();

/// Register the replay the open runs when it finds a store migrated past its replay set. Only the
/// first call counts; it returns whether this one did.
pub fn register_replay(hook: ReplayHook) -> bool {
    REPLAY.set(hook).is_ok()
}

/// At an open: when `meta.replay_set` names a set made on an older store version than the store
/// now has, replay it through the registered hook, then clear the key. A hook's error is returned,
/// so the open fails and the set stays for the next one. A set that doesn't parse, or names no
/// plain file name, is left for a person to see; with no hook or no data root nothing happens.
pub(crate) fn replay_if_set(store: &mut Store) -> Result<(), StoreError> {
    let Some(text) = store.meta("replay_set")? else { return Ok(()) };
    let Some(set) = ReplaySet::parse(&text) else { return Ok(()) };
    if Path::new(&set.file).file_name() != Some(OsStr::new(&set.file)) {
        return Ok(());
    }
    let version: u32 = store.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version <= set.store_version {
        return Ok(());
    }
    let (Some(hook), Some(env)) = (REPLAY.get(), fs_check::environment()) else { return Ok(()) };
    let Some(dir) = store.meta("workspace_id")?.and_then(|id| dir_of(&env.data_root, &id)) else {
        return Ok(());
    };
    hook(store, &set, &dir.join(&set.file))?;
    let tx = store.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    crate::commit::check_version(&tx)?;
    // Only the set this open replayed: a restore that set another since keeps its own.
    tx.execute("DELETE FROM meta WHERE key = 'replay_set' AND value = ?1", [&text])?;
    tx.commit()?;
    Ok(())
}

/// What [`put_back`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum PutBack {
    Done(PutBackReport),
    /// The person token's digest was already spent: nothing changed.
    TokenSpent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PutBackReport {
    /// The restored copy's last `seq`.
    pub seq: Seq,
    /// The copy of the store as it was, written first.
    pub pre_restore: PathBuf,
    /// How many person-authored events were set aside.
    pub set_aside: u64,
    /// The replay file and its set, when any were.
    pub replay: Option<(PathBuf, ReplaySet)>,
    /// The engine version that last wrote the live store, `meta.engine_version`.
    pub made_on: Option<String>,
    /// The restored store's version.
    pub user_version: u32,
}

/// A person token the put-back spends in its own transaction, as [`crate::Store::spend_token`].
#[derive(Debug, Clone, Copy)]
pub struct Spend<'a> {
    pub digest: &'a str,
    pub now: u64,
    pub expires: u64,
}

/// The `meta` keys the live store keeps across a put-back: the person's key and the spent tokens
/// (so a token spent before is never good again), and a temporary host's backup ref.
const KEPT_META: &[&str] =
    &["person_pubkey", "person_pubkey_registered_at", "person_pubkey_surface", "backup_ref"];
/// The tables a put-back keeps from the live store, when the copy's schema of them is the same:
/// who is here now, and who holds a duty.
const KEPT_TABLES: &[&str] = &["presence", "leases"];

/// The version-1 columns of every person-authored event of `main` that `copy` doesn't hold, in
/// `seq` order, leaving out a card restore's events: those at or below the first accepted review
/// of a restore card above the copy's head, which the card restore committed with them. The ID
/// rule, not "above the copy's head", keeps an edit made after an earlier restore when the copy is
/// that restore's pre-restore copy.
const SET_ASIDE: &str = "
    WITH head AS (SELECT COALESCE(MAX(seq), 0) AS seq FROM copy.events),
    cut AS (
        SELECT COALESCE((
            SELECT r.seq FROM main.events r, head
            WHERE r.seq > head.seq AND r.kind = 'review' AND r.status <> 'conflict'
              AND json_extract(r.body, '$.verdict') = 'accept'
              AND EXISTS (
                  SELECT 1 FROM main.events c
                  WHERE c.id = json_extract(r.body, '$.of') AND c.kind = 'card'
                    AND json_extract(c.body, '$.card_kind') = 'restore')
            ORDER BY r.seq LIMIT 1), 0) AS seq)
    SELECT seq, id, idem, request_hash, at, actor, client, project, cause, status, body
    FROM main.events
    WHERE json_extract(actor, '$.kind') = 'person'
      AND id NOT IN (SELECT id FROM copy.events)
      AND seq > (SELECT seq FROM cut)
    ORDER BY seq";

/// The events the put-back of `copy`, attached as `copy`, sets aside. A store whose events this
/// build can't read is `NewerFile` naming `live`.
fn set_aside(conn: &Connection, live: &Path) -> Result<Vec<Event>, StoreError> {
    let read = (|| {
        let mut stmt = conn.prepare(SET_ASIDE)?;
        decode_rows(&mut stmt)
    })();
    read.map_err(|_| {
        let found: u32 = conn.pragma_query_value(Some("main"), "user_version", |r| r.get(0)).unwrap_or(0);
        StoreError::NewerFile {
            file: live.to_path_buf(),
            found: found.to_string(),
            supported: STORE_VERSION.to_string(),
        }
    })
}

/// A `file:` URI for `path` that opens it read-only.
fn read_only_uri(path: &Path) -> String {
    let mut uri = String::from("file:");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri.push_str("?mode=ro");
    uri
}

fn attach(conn: &Connection, copy: &Path) -> Result<(), StoreError> {
    conn.execute("ATTACH DATABASE ?1 AS copy", [read_only_uri(copy)])?;
    Ok(())
}

/// How many person-authored events a put-back of `copy` into `live` would set aside, read only:
/// the confirmation's count.
pub fn count_set_aside(live: &Path, copy: &Path) -> Result<u64, StoreError> {
    let conn = read_only(live)?;
    attach(&conn, copy)?;
    Ok(set_aside(&conn, live)?.len() as u64)
}

/// How many person-authored events the store at `path` holds, read on a read-only connection of
/// its own, so a store no engine opens can still be counted.
pub fn person_events(path: &Path) -> Result<u64, StoreError> {
    let conn = read_only(path)?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE json_extract(actor, '$.kind') = 'person'",
        [],
        |r| r.get(0),
    )?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// Put the store copy `copy` back in place of the live store at `live`, in one transaction, after
/// a pre-restore copy of it, as [`put_back_in`] says; the data root comes from the registered
/// environment.
pub fn put_back(live: &Path, copy: &Path, spend: Option<Spend<'_>>) -> Result<PutBack, StoreError> {
    let env = fs_check::environment().ok_or_else(|| {
        StoreError::Io(format!(
            "{}: a restore needs the data root for its pre-restore copy, and this process has none",
            live.display()
        ))
    })?;
    put_back_in(&env.data_root, live, copy, spend)
}

/// [`put_back`] under `data_root`. The copy must pass `PRAGMA integrity_check`, belong to the same
/// workspace and be no newer than this build. Then, in one IMMEDIATE transaction on a connection
/// of its own: spend `spend`'s digest (already spent: nothing changes), write the pre-restore
/// copy, set the person-authored edits the copy lacks aside in `replay-<secs>.ndjson`, rebuild
/// every table from the copy but `presence` and `leases` (kept while their schema is the copy's,
/// each lease's epoch bumped so a duty mid-run loses it), carry the person's key, the spent
/// tokens and the backup ref over, and set `meta.replay_set` and the copy's `user_version`. Any
/// failure rolls it all back. The result is never opened with `Store::open` here.
pub(crate) fn put_back_in(
    data_root: &Path,
    live: &Path,
    copy: &Path,
    spend: Option<Spend<'_>>,
) -> Result<PutBack, StoreError> {
    let copy_version = check_copy(live, copy)?;
    let workspace_id = read_meta(live, "workspace_id")?
        .ok_or_else(|| StoreError::Io(format!("{}: the store names no workspace", live.display())))?;

    let mut conn = Connection::open_with_flags(
        live,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI,
    )?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    attach(&conn, copy)?;
    let mut replay_file = None;
    let done = (|| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.pragma_update(None, "defer_foreign_keys", "ON")?;
        let outcome =
            put_back_tx(&tx, data_root, live, &workspace_id, copy_version, spend, &mut replay_file)?;
        if matches!(outcome, PutBack::Done(_)) {
            tx.commit()?;
        }
        Ok(outcome)
    })();
    let _ = conn.execute("DETACH DATABASE copy", []);
    if done.is_err()
        && let Some(file) = replay_file
    {
        let _ = fs::remove_file(file);
    }
    done
}

/// Why [`copy_fits`] refused a store copy: a person picked the wrong file, not a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyRefused {
    /// It can't be read as a store, or `PRAGMA integrity_check` finds it damaged.
    Damaged,
    /// Its `meta.workspace_id` is missing or isn't the live store's.
    OtherWorkspace,
}

/// Check the store copy `copy` against the live store at `live`, each on a read-only connection
/// of its own: the copy must pass `PRAGMA integrity_check` and belong to the same workspace, or
/// the refusal says why; a copy newer than this build is `NewerFile`. Its `user_version`.
pub fn copy_fits(live: &Path, copy: &Path) -> Result<Result<u32, CopyRefused>, StoreError> {
    let read = || -> Result<(String, Option<String>, u32), StoreError> {
        let conn = read_only(copy)?;
        let verdict: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        let id = conn
            .query_row("SELECT value FROM meta WHERE key = 'workspace_id'", [], |r| r.get(0))
            .optional()?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok((verdict, id, version))
    };
    let Ok((verdict, id, version)) = read() else { return Ok(Err(CopyRefused::Damaged)) };
    if verdict != "ok" {
        return Ok(Err(CopyRefused::Damaged));
    }
    let live_id = read_meta(live, "workspace_id")?;
    if id.is_none() || id != live_id {
        return Ok(Err(CopyRefused::OtherWorkspace));
    }
    if version > STORE_VERSION {
        return Err(StoreError::NewerFile {
            file: copy.to_path_buf(),
            found: version.to_string(),
            supported: STORE_VERSION.to_string(),
        });
    }
    Ok(Ok(version))
}

/// [`copy_fits`], with a refusal as an error: the put-back's own check, after the app's.
fn check_copy(live: &Path, copy: &Path) -> Result<u32, StoreError> {
    copy_fits(live, copy)?.map_err(|why| {
        let what = match why {
            CopyRefused::Damaged => "the copy can't be read as a store or failed its integrity check",
            CopyRefused::OtherWorkspace => "the copy belongs to another workspace",
        };
        StoreError::Io(format!("{}: {what}", copy.display()))
    })
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn meta_value(tx: &Transaction<'_>, schema: &str, key: &str) -> Result<Option<String>, StoreError> {
    Ok(tx
        .query_row(&format!("SELECT value FROM {schema}.meta WHERE key = ?1"), [key], |r| r.get(0))
        .optional()?)
}

fn set_meta(tx: &Transaction<'_>, key: &str, value: Option<&str>) -> Result<(), StoreError> {
    match value {
        Some(value) => tx.execute(
            "INSERT INTO main.meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?,
        None => tx.execute("DELETE FROM main.meta WHERE key = ?1", [key])?,
    };
    Ok(())
}

/// The user objects of `schema`'s `sqlite_master`, in creation order: type, name, table, SQL.
fn objects(tx: &Transaction<'_>, schema: &str) -> Result<Vec<(String, String, String, String)>, StoreError> {
    let mut stmt = tx.prepare(&format!(
        "SELECT type, name, tbl_name, sql FROM {schema}.sqlite_master
         WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL ORDER BY rowid"
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn put_back_tx(
    tx: &Transaction<'_>,
    data_root: &Path,
    live: &Path,
    workspace_id: &str,
    copy_version: u32,
    spend: Option<Spend<'_>>,
    replay_file: &mut Option<PathBuf>,
) -> Result<PutBack, StoreError> {
    let now = spend.map_or_else(now_secs, |s| s.now);
    let live_spent = merge_spent(meta_value(tx, "main", "spent_tokens")?.as_deref(), now)?;
    if let Some(spend) = spend
        && live_spent.contains_key(spend.digest)
    {
        return Ok(PutBack::TokenSpent);
    }

    let pre_restore = pre_restore_copy_in(data_root, live, workspace_id)?;

    let events = set_aside(tx, live)?;
    let head: i64 = tx.query_row("SELECT COALESCE(MAX(seq), 0) FROM copy.events", [], |r| r.get(0))?;
    let head = head as Seq;
    let replay = if events.is_empty() {
        None
    } else {
        let dir =
            dir_of(data_root, workspace_id).ok_or_else(|| StoreError::Io("no backup directory".into()))?;
        let (path, file) = new_replay_file(&dir)?;
        *replay_file = Some(path.clone());
        write_replay(file, &path, workspace_id, copy_version, &events)?;
        sync_dir(Some(&dir));
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default().to_owned();
        let set =
            ReplaySet { file: name, base: head, store_version: copy_version, count: events.len() as u64 };
        Some((path, set))
    };

    // What the live store keeps, read before its tables go.
    let made_on = meta_value(tx, "main", "engine_version")?;
    let kept_meta: Vec<(&str, Option<String>)> =
        KEPT_META.iter().map(|k| Ok((*k, meta_value(tx, "main", k)?))).collect::<Result<_, StoreError>>()?;

    // Rebuild the schema from the copy's.
    let live_objects = objects(tx, "main")?;
    let copy_objects = objects(tx, "copy")?;
    let sql_of = |objs: &[(String, String, String, String)], name: &str| {
        objs.iter().find(|o| o.0 == "table" && o.1 == name).map(|o| o.3.clone())
    };
    let kept: Vec<&str> = KEPT_TABLES
        .iter()
        .copied()
        .filter(|t| {
            sql_of(&live_objects, t).is_some() && sql_of(&live_objects, t) == sql_of(&copy_objects, t)
        })
        .collect();
    let is_kept = |table: &str| kept.contains(&table);
    for kind in ["trigger", "view", "index"] {
        for (_, name, _, _) in live_objects.iter().filter(|o| o.0 == kind && !is_kept(&o.2)) {
            tx.execute_batch(&format!("DROP {} main.{}", kind.to_uppercase(), quoted(name)))?;
        }
    }
    for (_, name, _, _) in live_objects.iter().rev().filter(|o| o.0 == "table" && !is_kept(&o.1)) {
        tx.execute_batch(&format!("DROP TABLE main.{}", quoted(name)))?;
    }
    for (kind, name, table, sql) in &copy_objects {
        if is_kept(table) {
            continue;
        }
        tx.execute_batch(sql)?;
        if kind == "table" && !KEPT_TABLES.contains(&name.as_str()) && name != "meta" {
            let name = quoted(name);
            tx.execute_batch(&format!("INSERT INTO main.{name} SELECT * FROM copy.{name}"))?;
        }
    }
    if is_kept("leases") {
        tx.execute("UPDATE main.leases SET epoch = epoch + 1", [])?;
    }

    // The next seq follows the copy's last.
    tx.execute("DELETE FROM main.sqlite_sequence WHERE name = 'events'", [])?;
    tx.execute(
        "INSERT INTO main.sqlite_sequence (name, seq) SELECT name, seq FROM copy.sqlite_sequence WHERE name = 'events'",
        [],
    )?;

    // meta: the copy's, with the live person's key, the spent tokens of both and the backup ref.
    tx.execute("INSERT INTO main.meta SELECT * FROM copy.meta", [])?;
    let mut spent = merge_spent(meta_value(tx, "main", "spent_tokens")?.as_deref(), now)?;
    spent.extend(live_spent);
    if let Some(spend) = spend {
        spent.insert(spend.digest.to_owned(), spend.expires.into());
    }
    let spent = (!spent.is_empty()).then(|| Value::Object(spent).to_string());
    set_meta(tx, "spent_tokens", spent.as_deref())?;
    for (key, value) in &kept_meta {
        set_meta(tx, key, value.as_deref())?;
    }
    set_meta(tx, "replay_set", replay.as_ref().map(|r| r.1.to_json()).as_deref())?;
    tx.pragma_update(None, "user_version", copy_version)?;

    Ok(PutBack::Done(PutBackReport {
        seq: head,
        pre_restore,
        set_aside: events.len() as u64,
        replay,
        made_on,
        user_version: copy_version,
    }))
}

/// `replay-<secs>.ndjson` in `dir`, created new with mode 0600, at the next free second.
fn new_replay_file(dir: &Path) -> Result<(PathBuf, File), StoreError> {
    let mut secs = now_secs();
    loop {
        let path = dir.join(format!("replay-{secs}.ndjson"));
        match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => secs += 1,
            Err(e) => return Err(io_error(&path, e)),
        }
    }
}

/// The replay file: an event export's header line, then each event, synced.
fn write_replay(
    file: File,
    path: &Path,
    workspace_id: &str,
    store_version: u32,
    events: &[Event],
) -> Result<(), StoreError> {
    let fail = |e: io::Error| io_error(path, e);
    let mut out = io::BufWriter::new(&file);
    let header = ExportHeader::new(workspace_id.to_owned(), store_version, None);
    serde_json::to_writer(&mut out, &header)?;
    out.write_all(b"\n").map_err(fail)?;
    for event in events {
        serde_json::to_writer(&mut out, event)?;
        out.write_all(b"\n").map_err(fail)?;
    }
    out.flush().map_err(fail)?;
    drop(out);
    file.sync_all().map_err(fail)
}

fn io_error(path: &Path, e: io::Error) -> StoreError {
    StoreError::Io(format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::ops::ControlFlow;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    use planroom_contract::{
        Actor, Client, EventBody, EventId, ExportHeader, PresenceRow, Seq, Status, Surface, Timestamp,
    };
    use rusqlite::{Connection, OpenFlags};
    use serde_json::json;

    use super::*;
    use crate::{GC_LEASE, Init, NewEvent, STORE_VERSION, Store, StoreError, Write};

    const ID: &str = "ws-restore";

    /// A store in `<tmp>/ws/.planroom`, its path, and a data root beside it.
    fn setup(tmp: &Path) -> (Store, PathBuf, PathBuf) {
        let state = tmp.join("ws/.planroom");
        fs::create_dir_all(&state).unwrap();
        let init = Init {
            workspace_id: ID.into(),
            engine_version: "0.1.0".into(),
            created_at: "2026-10-29T00:00:00Z".into(),
        };
        let path = state.join("store.db");
        (Store::open(&path, &init).unwrap(), path, tmp.join("data"))
    }

    fn person() -> Actor {
        Actor::Person { id: "person:ann".into(), name: None }
    }

    fn new_event(id: &str, actor: Actor, body: EventBody) -> NewEvent {
        NewEvent {
            id: EventId(id.into()),
            at: Timestamp(format!("2026-10-29T00:00:0{}Z", id.len() % 10)),
            actor,
            client: Client { surface: Surface::Test, id: "test".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body,
        }
    }

    fn version(label: &str) -> EventBody {
        EventBody::Version { project: None, label: label.into() }
    }

    fn commit(store: &mut Store, idem: &str, events: Vec<NewEvent>) -> Vec<Seq> {
        let write = Write { idem: idem.into(), request_hash: "h".into(), events };
        store.commit(write, |_| json!({})).unwrap().seqs
    }

    /// One person-authored event with the ID and key `id`, committed; its `seq`.
    fn edit(store: &mut Store, id: &str) -> Seq {
        commit(store, id, vec![new_event(id, person(), version(id))])[0]
    }

    /// A `VACUUM INTO` copy of the store at `src`, as a migration writes one.
    fn vacuum(src: &Path, dest: &Path) {
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        let reader = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        reader.execute("VACUUM INTO ?1", [dest.to_str().unwrap()]).unwrap();
    }

    /// Each row's first column, as text.
    fn sql(path: &Path, query: &str) -> Vec<String> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = conn.prepare(query).unwrap();
        stmt.query_map([], |r| {
            Ok(match r.get::<_, rusqlite::types::Value>(0)? {
                rusqlite::types::Value::Integer(n) => n.to_string(),
                rusqlite::types::Value::Text(s) => s,
                rusqlite::types::Value::Null => String::new(),
                other => format!("{other:?}"),
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    }

    fn ids(path: &Path) -> Vec<String> {
        sql(path, "SELECT id FROM events ORDER BY seq")
    }

    fn header_line(workspace_id: &str) -> String {
        serde_json::to_string(&ExportHeader::new(workspace_id.into(), STORE_VERSION, None)).unwrap()
    }

    fn pre_restore_copies(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with("store-pre-restore-") && n.ends_with(".db"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn only_workspace_id_directories_are_read() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let area = data.join("backups");
        for dir in ["x..y", "ok_id", "goodid"] {
            fs::create_dir_all(area.join(dir)).unwrap();
        }
        fs::write(area.join("x..y/events.ndjson"), format!("{}\n", header_line("x..y"))).unwrap();
        fs::write(area.join("ok_id/events.ndjson"), "not a header\n").unwrap();
        fs::write(area.join("ledger-v1.db"), "").unwrap();
        fs::write(area.join("goodid/events.ndjson"), format!("{}\n{{\"seq\":1}}\n", header_line("goodid")))
            .unwrap();
        symlink(area.join("goodid"), area.join("linked")).unwrap();

        let found = backups(&data);
        let names: Vec<_> = found.iter().map(|b| b.dir.file_name().unwrap().to_str().unwrap()).collect();
        assert_eq!(names, ["goodid"]);
        assert_eq!(found[0].header.workspace_id, "goodid");
        assert_eq!(found[0].file, area.join("goodid/events.ndjson"));
        assert!(backups(&tmp.path().join("nowhere")).is_empty(), "no backups area is no backup");
    }

    #[test]
    fn a_linked_event_backup_is_never_read() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        let area = data.join("backups");
        let outside = tmp.path().join("outside/events.ndjson");
        fs::create_dir_all(outside.parent().unwrap()).unwrap();
        fs::write(&outside, format!("{}\n", header_line("planted"))).unwrap();
        fs::create_dir_all(area.join("planted")).unwrap();
        symlink(&outside, area.join("planted/events.ndjson")).unwrap();

        assert!(backups(&data).is_empty(), "a linked events.ndjson is no backup");
        assert!(open_backup(&area.join("planted/events.ndjson")).is_err());
        assert!(open_backup(&outside).is_ok(), "a regular file opens");
    }

    #[test]
    fn a_torn_last_line_is_left_out() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, _, _) = setup(tmp.path());
        for id in ["e1", "e22", "e333"] {
            edit(&mut store, id);
        }
        let events = store.events_after(0, 10).unwrap();
        let lines: Vec<String> = events.iter().map(|e| serde_json::to_string(e).unwrap()).collect();
        let text = format!("{}\n{}\n{}\n{}", header_line(ID), lines[0], lines[1], lines[2]);

        let mut seen = Vec::new();
        let summary = read_export(text.as_bytes(), |e| {
            seen.push(e.id.0);
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(seen, ["e1", "e22"]);
        assert_eq!(summary.persons, 2);
        assert!(summary.torn);
        assert_eq!(summary.last_person_at, Some(events[1].at.clone()));
        assert_eq!(summary.header.workspace_id, ID);

        let mut seen = 0;
        let whole = format!("{text}\n");
        let summary = read_export(whole.as_bytes(), |_| {
            seen += 1;
            ControlFlow::Break(())
        })
        .unwrap();
        assert_eq!((seen, summary.torn), (1, false), "a break stops the read");

        let long = format!("{}\n{}\n", header_line(ID), "x".repeat(MAX_LINE + 5));
        assert!(matches!(
            read_export(long.as_bytes(), |_| ControlFlow::Continue(())),
            Err(StoreError::Io(_))
        ));
    }

    #[test]
    fn a_link_or_a_stray_file_is_no_backup_path() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real-data");
        let dir = real.join("backups/wsid");
        fs::create_dir_all(dir.join("redacted")).unwrap();
        fs::create_dir_all(real.join("backups/other")).unwrap();
        for name in [
            "events.ndjson",
            "store-v1.db",
            "store-pre-restore-5.db",
            "events-1700000000.ndjson",
            "notes.txt",
            "redacted/events-000001.ndjson",
        ] {
            fs::write(dir.join(name), "").unwrap();
        }
        fs::write(real.join("backups/ledger-v1.db"), "").unwrap();
        symlink(dir.join("events.ndjson"), real.join("backups/other/events.ndjson")).unwrap();
        fs::create_dir_all(tmp.path().join("elsewhere/backups/wsid")).unwrap();
        fs::write(tmp.path().join("elsewhere/backups/wsid/events.ndjson"), "").unwrap();
        // The data root is reached through a link, as a home directory on another disk can be.
        let data = tmp.path().join("data");
        symlink(&real, &data).unwrap();
        let path = |rel: &str| data.join("backups").join(rel);

        let refused = |given: &Path| match backup_path(&data, given).unwrap() {
            Err(PathRefused::NotABackup) => true,
            other => panic!("{} wasn't refused: {other:?}", given.display()),
        };
        refused(&path("other/events.ndjson"));
        refused(&path("wsid/redacted/events-000001.ndjson"));
        refused(&path("wsid/events-1700000000.ndjson"));
        refused(&path("ledger-v1.db"));
        refused(&path("wsid/notes.txt"));
        refused(&path("wsid"));
        refused(&tmp.path().join("elsewhere/backups/wsid/events.ndjson"));

        let accepted = |rel: &str| backup_path(&data, &path(rel)).unwrap().unwrap();
        assert_eq!(accepted("wsid/events.ndjson"), ("wsid".to_owned(), BackupKind::Events));
        assert_eq!(accepted("wsid/store-v1.db"), ("wsid".to_owned(), BackupKind::Copy));
        assert_eq!(accepted("wsid/store-pre-restore-5.db"), ("wsid".to_owned(), BackupKind::Copy));

        match backup_path(&data, &path("wsid/store-v9.db")).unwrap() {
            Err(PathRefused::Missing { closest: Some(closest) }) => {
                assert!(closest.ends_with("store-pre-restore-5.db"), "{closest}")
            }
            other => panic!("a missing copy: {other:?}"),
        }
    }

    #[test]
    fn a_put_back_keeps_the_live_key_tokens_presence_and_leases() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, live, data) = setup(tmp.path());
        store.meta_insert_once("person_pubkey", "old-key").unwrap();
        for id in ["e1", "e2", "e3"] {
            edit(&mut store, id);
        }
        let copy = data.join("backups").join(ID).join("store-v1.db");
        vacuum(&live, &copy);
        for id in ["e4", "e5"] {
            edit(&mut store, id);
        }
        assert!(store.meta_swap("person_pubkey", Some("old-key"), "new-key", &[]).unwrap());
        assert!(store.spend_token("old-digest", 100, 10_000).unwrap());
        let row = PresenceRow {
            actor: person(),
            surface: Surface::Test,
            client_id: "live-client".into(),
            doing: None,
            focus: None,
            seen_at: Timestamp("2026-10-29T00:00:00Z".into()),
        };
        store.touch_presence(&row).unwrap();
        let lease = store.take_lease(GC_LEASE, "gc-holder").unwrap().unwrap();
        drop(store);
        Connection::open(&copy)
            .unwrap()
            .execute(
                "INSERT INTO presence (actor, surface, client_id, seen_at)
                 VALUES ('{\"kind\":\"person\",\"id\":\"person:bob\",\"name\":null}', 'test', 'copy-client', 'x')",
                [],
            )
            .unwrap();
        let copy_sequence = sql(&copy, "SELECT seq FROM sqlite_sequence WHERE name = 'events'");

        assert_eq!(count_set_aside(&live, &copy).unwrap(), 2, "the confirmation's count");
        let spend = Spend { digest: "new-digest", now: 200, expires: 20_000 };
        let PutBack::Done(report) = put_back_in(&data, &live, &copy, Some(spend)).unwrap() else {
            panic!("the token was fresh")
        };

        assert_eq!(ids(&live), ["e1", "e2", "e3"]);
        assert_eq!((report.seq, report.set_aside, report.user_version), (3, 2, STORE_VERSION));
        assert_eq!(report.made_on.as_deref(), Some("0.1.0"));
        let (file, set) = report.replay.clone().unwrap();
        let text = fs::read_to_string(&file).unwrap();
        let mut lines = text.lines();
        let header = planroom_contract::format::read_str::<ExportHeader>(lines.next().unwrap()).unwrap();
        assert_eq!((header.workspace_id.as_str(), header.store_version), (ID, STORE_VERSION));
        let replayed: Vec<String> =
            lines.map(|l| serde_json::from_str::<planroom_contract::Event>(l).unwrap().id.0).collect();
        assert_eq!(replayed, ["e4", "e5"]);
        assert_eq!(set.file, file.file_name().unwrap().to_str().unwrap());
        assert_eq!((set.base, set.count, set.store_version), (3, 2, STORE_VERSION));
        let held = sql(&live, "SELECT value FROM meta WHERE key = 'replay_set'");
        assert_eq!(ReplaySet::parse(&held[0]), Some(set));

        assert_eq!(sql(&live, "SELECT value FROM meta WHERE key = 'person_pubkey'"), ["new-key"]);
        let spent: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&sql(&live, "SELECT value FROM meta WHERE key = 'spent_tokens'")[0])
                .unwrap();
        assert!(spent.contains_key("old-digest") && spent.contains_key("new-digest"), "{spent:?}");
        assert_eq!(sql(&live, "SELECT value FROM meta WHERE key = 'workspace_id'"), [ID]);
        assert_eq!(sql(&live, "SELECT client_id FROM presence"), ["live-client"]);
        assert_eq!(sql(&live, "SELECT epoch FROM leases"), [(lease.epoch + 1).to_string()]);
        assert_eq!(sql(&live, "SELECT seq FROM sqlite_sequence WHERE name = 'events'"), copy_sequence);
        assert!(report.pre_restore.is_file());
        assert_eq!(
            ids(&report.pre_restore),
            ["e1", "e2", "e3", "e4", "e5"],
            "the copy is the store it started from"
        );
        assert_eq!(sql(&live, "PRAGMA integrity_check"), ["ok"]);

        let again = Spend { digest: "new-digest", now: 300, expires: 20_000 };
        assert!(matches!(put_back_in(&data, &live, &copy, Some(again)).unwrap(), PutBack::TokenSpent));
        assert_eq!(ids(&live), ["e1", "e2", "e3"]);
        assert_eq!(
            pre_restore_copies(&data.join("backups").join(ID)).len(),
            1,
            "a spent token copies nothing"
        );

        // The restored store opens, and its next event follows the copy's last.
        let init = Init { workspace_id: ID.into(), engine_version: "0.1.0".into(), created_at: "x".into() };
        let mut store = Store::open(&live, &init).unwrap();
        assert_eq!(edit(&mut store, "e6"), 4);
    }

    #[test]
    fn a_copy_newer_than_the_engine_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, live, data) = setup(tmp.path());
        edit(&mut store, "e1");
        let copy = data.join("backups").join(ID).join("store-v1.db");
        vacuum(&live, &copy);
        edit(&mut store, "e2");
        drop(store);
        Connection::open(&copy).unwrap().pragma_update(None, "user_version", STORE_VERSION + 1).unwrap();

        match put_back_in(&data, &live, &copy, None) {
            Err(StoreError::NewerFile { file, found, supported }) => {
                assert_eq!(file, copy);
                assert_eq!((found, supported), ((STORE_VERSION + 1).to_string(), STORE_VERSION.to_string()));
            }
            other => panic!("a newer copy: {:?}", other.map(|_| ())),
        }
        assert_eq!(ids(&live), ["e1", "e2"]);
        assert!(pre_restore_copies(&data.join("backups").join(ID)).is_empty());
    }

    #[test]
    fn restoring_the_pre_restore_copy_sets_aside_an_edit_made_after_the_restore() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, live, data) = setup(tmp.path());
        edit(&mut store, "e1");
        let copy = data.join("backups").join(ID).join("store-v1.db");
        vacuum(&live, &copy);
        edit(&mut store, "e2");
        edit(&mut store, "e3");
        drop(store);
        let PutBack::Done(first) = put_back_in(&data, &live, &copy, None).unwrap() else { unreachable!() };
        assert_eq!((ids(&live), first.set_aside), (vec!["e1".to_owned()], 2));

        // An edit after the restore takes a seq at or below the pre-restore copy's head.
        let init = Init { workspace_id: ID.into(), engine_version: "0.1.0".into(), created_at: "x".into() };
        let mut store = Store::open(&live, &init).unwrap();
        assert_eq!(edit(&mut store, "e4"), 2);
        drop(store);

        let PutBack::Done(second) = put_back_in(&data, &live, &first.pre_restore, None).unwrap() else {
            unreachable!()
        };
        assert_eq!(ids(&live), ["e1", "e2", "e3"]);
        assert_eq!(second.set_aside, 1);
        let (file, set) = second.replay.unwrap();
        assert_eq!((set.base, set.count), (3, 1));
        let text = fs::read_to_string(file).unwrap();
        assert_eq!(
            text.lines()
                .skip(1)
                .map(|l| serde_json::from_str::<planroom_contract::Event>(l).unwrap().id.0)
                .collect::<Vec<_>>(),
            ["e4"]
        );
        assert_eq!(
            pre_restore_copies(&data.join("backups").join(ID)).len(),
            2,
            "each restore keeps its own copy"
        );
        assert_eq!(
            pre_restore_before(&data, ID, 3).map(|p| ids(&p)),
            Some(vec!["e1".to_owned(), "e4".to_owned()]),
            "the copy taken before the restore at seq 3"
        );
    }

    #[test]
    fn a_card_restore_s_events_are_never_set_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, live, data) = setup(tmp.path());
        let card: EventBody = serde_json::from_value(json!({
            "kind": "card", "card_kind": "restore", "subject": "old-ws",
            "question": "card.restore_question", "why": "card.restore_why", "stakes": "person",
            "choices": [], "default": "reject", "dealt_by": "scan"
        }))
        .unwrap();
        commit(&mut store, "deal", vec![new_event("card", Actor::System { part: "scan".into() }, card)]);
        let copy = data.join("backups").join(ID).join("store-v1.db");
        vacuum(&live, &copy);
        // The accept writes the backup's events, then the card's review, in one commit.
        let review =
            EventBody::Review { of: EventId("card".into()), verdict: planroom_contract::Verdict::Accept };
        commit(
            &mut store,
            "accept",
            vec![
                new_event("r1", person(), version("r1")),
                new_event("r2", person(), version("r2")),
                new_event("review", person(), review),
            ],
        );
        edit(&mut store, "after");
        drop(store);

        let PutBack::Done(report) = put_back_in(&data, &live, &copy, None).unwrap() else { unreachable!() };
        assert_eq!(ids(&live), ["card"]);
        let (file, _) = report.replay.unwrap();
        let text = fs::read_to_string(file).unwrap();
        let set: Vec<String> = text
            .lines()
            .skip(1)
            .map(|l| serde_json::from_str::<planroom_contract::Event>(l).unwrap().id.0)
            .collect();
        assert_eq!(set, ["after"]);
    }

    #[test]
    fn spent_tokens_merge_drops_the_expired() {
        let merged = merge_spent(Some(r#"{"a": 5, "b": 50}"#), 10).unwrap();
        assert_eq!(merged.keys().collect::<Vec<_>>(), ["b"]);
        assert!(merge_spent(None, 10).unwrap().is_empty());
    }
}
