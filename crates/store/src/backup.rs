//! The automatic event backup (architecture.md, Store schema; DR-53): every person-authored
//! commit appends the events above the backup file's own last `seq` to
//! `DATA_ROOT/backups/<workspace_id>/events.ndjson`, export's format, under a lock of its own.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use planroom_contract::format;
use planroom_contract::{ExportHeader, Seq};
use rusqlite::TransactionBehavior;

use crate::commit::check_version;
use crate::fs_check::{self, Notice, notify};
use crate::migrate::{narrow, private_dir};
use crate::pointer::{sync_dir, valid_workspace_id};
use crate::{BUSY_TIMEOUT, STORE_VERSION, Store, StoreError};

/// The area under the data root that holds each workspace's backup directory.
const BACKUPS: &str = "backups";
/// The backup file, export's format.
const FILE: &str = "events.ndjson";
/// The lock beside it, never renamed, so a process waiting on it never holds a file renamed aside.
const LOCK: &str = "events.ndjson.lock";
/// Events read from the store per query while appending.
const BATCH: u32 = 500;
/// How much of the file one read back from its end takes.
const CHUNK: usize = 64 * 1024;
/// The wait between tries for the lock.
const STEP: Duration = Duration::from_millis(50);

/// One append to the backup file, owned, with no borrow of the store: what the copy step a
/// temporary host registers ([`register_copy`]) reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// The directory of the path the store was opened at.
    pub state_dir: PathBuf,
    /// `DATA_ROOT/backups/<workspace_id>`.
    pub dir: PathBuf,
    /// `events.ndjson` in `dir`.
    pub file: PathBuf,
    pub workspace_id: String,
    /// The file's byte length before this append's event lines.
    pub offset: u64,
    /// The file's last `seq` before this append.
    pub after: Seq,
    /// The file's last `seq` after it.
    pub through: Seq,
}

/// How far the backup has come, read in one read transaction ([`crate::Snapshot::backup_mark`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupMark {
    /// `meta.backup_seq`, 0 when no backup was written.
    pub backup_seq: Seq,
    /// The highest `seq` of a person-authored event, 0 when there is none.
    pub last_authored: Seq,
    /// `meta.backup_ref`, the repo key a temporary host's copy goes to.
    pub backup_ref: Option<String>,
}

/// The step that runs after each append, once the backup lock is released.
pub type Copy = fn(&mut Store, &Backup) -> Option<Notice>;

static COPY: OnceLock<Copy> = OnceLock::new();

/// Register the copy step. Only the first call counts; it returns whether this one did.
pub fn register_copy(copy: Copy) -> bool {
    COPY.set(copy).is_ok()
}

/// A key for the state directory `dir`: its parent made absolute, joined with its own name, so
/// it holds while `dir` itself is missing. `None` for a path with no name, or whose parent
/// doesn't exist.
pub fn state_key(dir: &Path) -> Option<String> {
    let name = dir.file_name()?;
    let parent = match dir.parent()? {
        p if p.as_os_str().is_empty() => Path::new("."),
        p => p,
    };
    Some(fs::canonicalize(parent).ok()?.join(name).to_string_lossy().into_owned())
}

/// `DATA_ROOT/backups`, the area that holds each workspace's backup directory.
pub fn backups_area(data_root: &Path) -> PathBuf {
    data_root.join(BACKUPS)
}

/// The directory under `data_root` that holds `workspace_id`'s backup file and its migration
/// copies; `None` for an ID that can't name one, for which the write-through writes nothing and
/// says the backup is behind.
pub fn dir_of(data_root: &Path, workspace_id: &str) -> Option<PathBuf> {
    valid_workspace_id(workspace_id).then(|| backups_area(data_root).join(workspace_id))
}

/// The `seq` of the backup file's last complete line: 0 for a missing or empty file, or one
/// that holds only its header. A torn last line, with no newline after it, doesn't count.
pub fn file_seq(path: &Path) -> Result<Seq, StoreError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(io_error(path, e)),
    };
    let ends = line_ends(&mut file).map_err(|e| io_error(path, e))?;
    last_seq(&mut file, ends).map_err(|e| io_error(path, e))
}

/// The last `seq` the backup file at `path` holds of the store with this `workspace_id`,
/// `store_version`, state directory `state_dir` and head `head`, by the write-through's own test:
/// its header names the same workspace, store version and state directory, and its last complete
/// line is at or below the head. 0 for a file the write-through would rename aside, and for a
/// missing, empty or unreadable one.
pub fn held_seq(path: &Path, workspace_id: &str, store_version: u32, state_dir: &Path, head: Seq) -> Seq {
    let Ok(mut file) = File::open(path) else { return 0 };
    let Ok(ends) = line_ends(&mut file) else { return 0 };
    kept(&mut file, ends, &wanted(workspace_id, store_version, state_dir), head).unwrap_or(0)
}

/// The header a backup file of the store with this `workspace_id`, `store_version` and state
/// directory `state_dir` starts with.
fn wanted(workspace_id: &str, store_version: u32, state_dir: &Path) -> ExportHeader {
    ExportHeader::new(workspace_id.to_owned(), store_version, state_key(state_dir))
}

/// The file's last `seq`, given [`line_ends`], when the write-through keeps the file: its header
/// is `wanted` ([`same_header`]) and that `seq` is at or below `head`. `None` for a file it sets
/// aside.
fn kept(file: &mut File, ends: (Option<u64>, Option<u64>), wanted: &ExportHeader, head: Seq) -> Option<Seq> {
    let last = last_seq(file, ends).ok().filter(|seq| *seq <= head)?;
    header(file).is_some_and(|found| same_header(&found, wanted)).then_some(last)
}

/// The ends of the file's last two complete lines, the offsets of their newlines, found by
/// reading back from the end in [`CHUNK`]s: `(before the last, last)`.
fn line_ends(file: &mut File) -> io::Result<(Option<u64>, Option<u64>)> {
    let mut end = file.metadata()?.len();
    let mut found: Vec<u64> = Vec::with_capacity(2);
    let mut buf = vec![0u8; CHUNK];
    while end > 0 && found.len() < 2 {
        let start = end.saturating_sub(CHUNK as u64);
        let chunk = &mut buf[..(end - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(chunk)?;
        for (i, byte) in chunk.iter().enumerate().rev() {
            if *byte == b'\n' {
                found.push(start + i as u64);
                if found.len() == 2 {
                    break;
                }
            }
        }
        end = start;
    }
    Ok((found.get(1).copied(), found.first().copied()))
}

/// The `seq` of the last complete line after the header, given [`line_ends`]; 0 when the header
/// is the only complete line. A line that holds no `seq` is an error.
fn last_seq(file: &mut File, ends: (Option<u64>, Option<u64>)) -> io::Result<Seq> {
    let (Some(before), Some(last)) = ends else { return Ok(0) };
    let mut line = vec![0u8; (last - before - 1) as usize];
    file.seek(SeekFrom::Start(before + 1))?;
    file.read_exact(&mut line)?;
    let seq = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&line)
        .ok()
        .and_then(|line| line.get("seq").and_then(serde_json::Value::as_u64));
    seq.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "its last line holds no event seq"))
}

/// Append the events above the file's last `seq` to the backup under `data_root`, then run the
/// registered copy step. Never with a transaction open. A failure, or a workspace ID that names no
/// backup directory, so no backup can exist, leaves a `backup_behind` notice and the commit that
/// called it stays a success; once the cause is gone, the next person-authored commit catches up.
pub(crate) fn write_through(store: &mut Store, data_root: &Path, last_authored: Seq) {
    match append(store, data_root) {
        Ok(Some(backup)) => {
            if let Some(copy) = COPY.get()
                && let Some(notice) = copy(store, &backup)
            {
                notify(notice);
            }
        }
        Ok(None) | Err(_) => {
            let backup_seq =
                store.meta("backup_seq").ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(0);
            let file_seq = held(store, data_root).unwrap_or(0);
            notify(Notice::BackupBehind { backup_seq, file_seq, last_authored });
        }
    }
}

/// [`held_seq`] for the store's own backup file under `data_root`; `None` when its workspace ID
/// names no directory there, or the store can't be read.
fn held(store: &Store, data_root: &Path) -> Option<Seq> {
    let workspace_id = store.meta("workspace_id").ok()??;
    let dir = dir_of(data_root, &workspace_id)?;
    let head = store.seq().ok()?;
    Some(held_seq(&dir.join(FILE), &workspace_id, store_version(store).ok()?, store.state_dir(), head))
}

/// The write-through after a person-authored commit: a no-op while no data root is registered,
/// as a store test runs.
pub(crate) fn hook(store: &mut Store, last_authored: Seq) {
    if let Some(env) = fs_check::environment() {
        write_through(store, &env.data_root, last_authored);
    }
}

/// Under the backup lock: bring the file up to the store's head and set the mark. `None` when the
/// workspace ID can't name a directory under the data root, so nothing is written.
fn append(store: &mut Store, data_root: &Path) -> Result<Option<Backup>, StoreError> {
    let Some(workspace_id) = store.meta("workspace_id")? else { return Ok(None) };
    let Some(dir) = dir_of(data_root, &workspace_id) else { return Ok(None) };
    private_dir(&dir)?;
    let lock = lock(&dir.join(LOCK))?;
    let path = dir.join(FILE);
    let head = store.seq()?;
    let wanted = wanted(&workspace_id, store_version(store)?, store.state_dir());

    let mut file = open_log(&path)?;
    let fail = |e: io::Error| io_error(&path, e);
    let ends = line_ends(&mut file).map_err(fail)?;
    let complete = ends.1.map_or(0, |last| last + 1);
    if file.metadata().map_err(fail)?.len() > complete {
        // A torn last line, from a process killed partway through it.
        file.set_len(complete).map_err(fail)?;
    }
    let mut after = 0;
    if complete > 0 {
        if let Some(last) = kept(&mut file, ends, &wanted, head) {
            after = last;
        } else {
            drop(file);
            set_aside(&dir, &path)?;
            file = open_log(&path)?;
        }
    }
    let mut offset = file.metadata().map_err(fail)?.len();
    if offset == 0 {
        let mut line = serde_json::to_vec(&wanted)?;
        line.push(b'\n');
        file.write_all(&line).map_err(fail)?;
        file.sync_all().map_err(fail)?;
        sync_dir(Some(&dir));
        offset = line.len() as u64;
    }

    let mut through = after;
    let mut out = BufWriter::new(&file);
    while through < head {
        let batch = store.events_after(through, BATCH)?;
        if batch.is_empty() {
            break;
        }
        for event in batch.iter().take_while(|e| e.seq <= head) {
            serde_json::to_writer(&mut out, event)?;
            out.write_all(b"\n").map_err(fail)?;
            through = event.seq;
        }
        if batch.last().is_some_and(|e| e.seq > head) {
            break;
        }
    }
    out.flush().map_err(fail)?;
    drop(out);
    file.sync_all().map_err(fail)?;
    set_mark(store, through)?;
    drop(lock);
    Ok(Some(Backup {
        state_dir: store.state_dir().to_path_buf(),
        dir,
        file: path,
        workspace_id,
        offset,
        after,
        through,
    }))
}

/// The store version the store records, which every header names.
fn store_version(store: &Store) -> Result<u32, StoreError> {
    Ok(store.meta("store_version")?.and_then(|v| v.parse().ok()).unwrap_or(STORE_VERSION))
}

/// Whether a file's header still tells the truth about the store: the same format, workspace,
/// store version and state directory.
fn same_header(found: &ExportHeader, wanted: &ExportHeader) -> bool {
    found.format == wanted.format
        && found.workspace_id == wanted.workspace_id
        && found.store_version == wanted.store_version
        && found.state_dir.is_some()
        && found.state_dir == wanted.state_dir
}

/// The file's first line as a header; `None` when it isn't one, or is longer than a header can be.
fn header(file: &mut File) -> Option<ExportHeader> {
    file.seek(SeekFrom::Start(0)).ok()?;
    let mut line = Vec::new();
    BufReader::new(Read::take(&*file, CHUNK as u64)).read_until(b'\n', &mut line).ok()?;
    if line.pop() != Some(b'\n') {
        return None;
    }
    format::read_str::<ExportHeader>(std::str::from_utf8(&line).ok()?).ok()
}

/// Open the backup file to read and append, creating it with mode 0600.
fn open_log(path: &Path) -> Result<File, StoreError> {
    let file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| io_error(path, e))?;
    narrow(path, 0o600);
    Ok(file)
}

/// Rename the file aside to `events-<unix seconds>.ndjson`, with `-<n>` when that name is taken.
/// Old files stay where they are.
fn set_aside(dir: &Path, path: &Path) -> Result<(), StoreError> {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut aside = dir.join(format!("events-{secs}.ndjson"));
    let mut n = 1;
    while aside.exists() {
        aside = dir.join(format!("events-{secs}-{n}.ndjson"));
        n += 1;
    }
    fs::rename(path, &aside).map_err(|e| io_error(path, e))?;
    sync_dir(Some(dir));
    Ok(())
}

/// Take the exclusive lock on `path`, created with mode 0600 and never renamed, retrying every
/// 50 ms up to [`BUSY_TIMEOUT`]. Held until the returned file is dropped.
pub(crate) fn lock(path: &Path) -> Result<File, StoreError> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| io_error(path, e))?;
    let deadline = Instant::now() + BUSY_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(STEP),
            Err(TryLockError::WouldBlock) => return Err(StoreError::Busy),
            Err(TryLockError::Error(e)) => return Err(io_error(path, e)),
        }
    }
}

/// Set `meta.backup_seq` in a short IMMEDIATE transaction of its own. Bookkeeping, not an
/// authored write, so it takes the store-version check alone, never the pack guard.
fn set_mark(store: &mut Store, seq: Seq) -> Result<(), StoreError> {
    let tx = store.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    check_version(&tx)?;
    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('backup_seq', ?1)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [seq.to_string()],
    )?;
    tx.commit()?;
    Ok(())
}

fn io_error(path: &Path, e: io::Error) -> StoreError {
    StoreError::Io(format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use planroom_contract::{Actor, Client, EventBody, EventId, ExportHeader, Status, Surface, Timestamp};
    use serde_json::{Value, json};

    use super::*;
    use crate::{Init, NewEvent, STORE_VERSION, Write};

    /// A store in `<tmp>/ws/.planroom`, and a data root beside it.
    fn setup(tmp: &Path) -> (Store, PathBuf) {
        let state = tmp.join("ws/.planroom");
        fs::create_dir_all(&state).unwrap();
        let init = Init {
            workspace_id: "ws-backup".into(),
            engine_version: "0.1.0".into(),
            created_at: "2026-10-29T00:00:00Z".into(),
        };
        (Store::open(&state.join("store.db"), &init).unwrap(), tmp.join("data"))
    }

    /// One person-authored event, committed; its `seq`.
    fn edit(store: &mut Store, n: usize) -> Seq {
        let write = Write {
            idem: format!("w{n}"),
            request_hash: "h".into(),
            events: vec![NewEvent {
                id: EventId(format!("e{n}")),
                at: Timestamp("2026-10-29T00:00:00Z".into()),
                actor: Actor::Person { id: "person:ann".into(), name: None },
                client: Client { surface: Surface::Test, id: "test".into() },
                project: None,
                cause: None,
                status: Status::Applied,
                target: None,
                body: EventBody::Version { project: None, label: format!("v{n}") },
            }],
        };
        store.commit(write, |_| json!({})).unwrap().seqs[0]
    }

    fn file(data: &Path) -> PathBuf {
        data.join("backups/ws-backup/events.ndjson")
    }

    /// The file's header and the `seq` of every line after it.
    fn lines(path: &Path) -> (ExportHeader, Vec<Seq>) {
        let text = fs::read_to_string(path).unwrap();
        assert!(text.ends_with('\n'), "every line is whole: {text:?}");
        let mut lines = text.lines();
        let header = planroom_contract::format::read_str::<ExportHeader>(lines.next().unwrap()).unwrap();
        let seqs = lines
            .map(|l| {
                serde_json::from_str::<planroom_contract::Event>(l).unwrap_or_else(|e| panic!("{e}: {l}")).seq
            })
            .collect();
        (header, seqs)
    }

    fn backup_seq(store: &Store) -> Option<String> {
        store.meta("backup_seq").unwrap()
    }

    #[test]
    fn the_state_key_holds_without_the_state_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&ws, &link).unwrap();
        let state = ws.join(".planroom");
        let missing = state_key(&state).unwrap();
        assert_eq!(missing, ws.canonicalize().unwrap().join(".planroom").to_string_lossy());
        assert_eq!(state_key(&link.join(".planroom")).unwrap(), missing, "a symlinked parent resolves");
        fs::create_dir(&state).unwrap();
        assert_eq!(state_key(&state).unwrap(), missing, "the key doesn't change once the directory exists");
        assert_eq!(state_key(Path::new("/")), None, "a path with no name has no key");
    }

    #[test]
    fn file_seq_reads_the_last_complete_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.ndjson");
        assert_eq!(file_seq(&path).unwrap(), 0, "a missing file");
        fs::write(&path, "").unwrap();
        assert_eq!(file_seq(&path).unwrap(), 0, "an empty file");
        let header = serde_json::to_string(&ExportHeader::new("ws".into(), STORE_VERSION, None)).unwrap();
        fs::write(&path, format!("{header}\n")).unwrap();
        assert_eq!(file_seq(&path).unwrap(), 0, "a header alone");
        // A line longer than one 64 KiB read, so the scan back crosses chunks.
        let long = "x".repeat(200 * 1024);
        let text = format!("{header}\n{{\"seq\":4,\"pad\":\"{long}\"}}\n{{\"pad\":\"{long}\",\"seq\":17}}\n");
        fs::write(&path, &text).unwrap();
        assert_eq!(file_seq(&path).unwrap(), 17);
        fs::write(&path, format!("{text}{{\"seq\":18,\"pa")).unwrap();
        assert_eq!(file_seq(&path).unwrap(), 17, "a torn last line doesn't count");
    }

    #[test]
    fn a_torn_last_line_is_cut_before_the_next_append() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, data) = setup(tmp.path());
        edit(&mut store, 1);
        let two = edit(&mut store, 2);
        write_through(&mut store, &data, two);
        let (_, seqs) = lines(&file(&data));
        assert_eq!(seqs, [1, 2]);
        let mut text = fs::read_to_string(file(&data)).unwrap();
        text.push_str("{\"seq\":3,\"id\":\"e");
        fs::write(file(&data), &text).unwrap();
        let three = edit(&mut store, 3);
        write_through(&mut store, &data, three);
        let (header, seqs) = lines(&file(&data));
        assert_eq!(seqs, [1, 2, 3], "the torn line is gone and every event is there once");
        assert_eq!(header.workspace_id, "ws-backup");
        assert_eq!(backup_seq(&store).as_deref(), Some("3"));
        assert!(crate::take_notices().iter().all(|n| !matches!(n, Notice::BackupBehind { .. })));
    }

    /// A file whose header no longer tells the truth about the store is renamed aside whole, and
    /// a new file holds every event once.
    fn rotates(rewrite: impl Fn(&mut Value, &mut String)) {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, data) = setup(tmp.path());
        let one = edit(&mut store, 1);
        write_through(&mut store, &data, one);
        let old = fs::read_to_string(file(&data)).unwrap();
        let (first, rest) = old.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(first).unwrap();
        let mut rest = rest.to_owned();
        rewrite(&mut header, &mut rest);
        let planted = format!("{header}\n{rest}");
        fs::write(file(&data), &planted).unwrap();
        let two = edit(&mut store, 2);
        write_through(&mut store, &data, two);

        let dir = data.join("backups/ws-backup");
        let aside: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                name.starts_with("events-") && name.ends_with(".ndjson")
            })
            .collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        let name = aside[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name["events-".len()..name.len() - ".ndjson".len()].parse::<u64>().is_ok(), "{name}");
        assert_eq!(fs::read_to_string(&aside[0]).unwrap(), planted, "the old file is kept as it was");
        let (header, seqs) = lines(&file(&data));
        assert_eq!(seqs, [1, 2]);
        assert_eq!(header.store_version, STORE_VERSION);
        assert_eq!(header.workspace_id, "ws-backup");
        assert_eq!(header.state_dir, state_key(store.state_dir()));
        assert_eq!(backup_seq(&store).as_deref(), Some("2"));
    }

    #[test]
    fn a_header_for_another_store_version_starts_a_new_file() {
        rotates(|h, _| h["store_version"] = json!(STORE_VERSION + 1));
    }

    #[test]
    fn a_last_seq_above_the_head_starts_a_new_file() {
        // A line past the store's head, as a file from another store with the same header leaves.
        rotates(|_, rest| {
            let mut line: Value = serde_json::from_str(rest.lines().next().unwrap()).unwrap();
            line["seq"] = json!(9);
            rest.push_str(&format!("{line}\n"));
        });
    }

    /// What the file holds of this store, by the write-through's own test: a file it would rename
    /// aside holds nothing.
    #[test]
    fn held_seq_counts_only_a_file_the_write_through_keeps() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut store, data) = setup(tmp.path());
        let path = file(&data);
        let held = |store: &Store| {
            held_seq(&path, "ws-backup", STORE_VERSION, store.state_dir(), store.seq().unwrap())
        };
        assert_eq!(held(&store), 0, "a missing file");
        let one = edit(&mut store, 1);
        write_through(&mut store, &data, one);
        assert_eq!(held(&store), one);
        assert_eq!(
            held_seq(&path, "ws-other", STORE_VERSION, store.state_dir(), one),
            0,
            "another workspace"
        );
        assert_eq!(
            held_seq(&path, "ws-backup", STORE_VERSION + 1, store.state_dir(), one),
            0,
            "another version"
        );
        assert_eq!(held_seq(&path, "ws-backup", STORE_VERSION, Path::new("/elsewhere/.planroom"), one), 0);
        assert_eq!(
            held_seq(&path, "ws-backup", STORE_VERSION, store.state_dir(), one - 1),
            0,
            "above the head"
        );

        let kept = fs::read_to_string(&path).unwrap();
        let (first, rest) = kept.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(first).unwrap();
        header["state_dir"] = json!("/elsewhere/.planroom");
        fs::write(&path, format!("{header}\n{rest}")).unwrap();
        assert_eq!(file_seq(&path).unwrap(), one, "the file's last line is still the head");
        assert_eq!(held(&store), 0, "but it holds nothing of this store");
    }

    #[test]
    fn a_header_for_another_state_directory_starts_a_new_file() {
        rotates(|h, _| h["state_dir"] = json!("/elsewhere/.planroom"));
        rotates(|h, _| {
            h.as_object_mut().unwrap().remove("state_dir");
        });
    }
}
