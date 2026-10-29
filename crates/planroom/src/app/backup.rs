//! A temporary host's redacted backup ref (DR-53, DR-75; architecture.md, Temporary hosts).
//!
//! A cloud session, a Codespace or a dev container loses the state directory and the data root
//! together when its container goes. On such a host, after each append to the event backup, the
//! store runs [`copy_to_ref`]: it keeps a redacted copy of the backup file in `redacted/` beside
//! it, in segments of [`SEGMENT_LINES`] lines, and commits those segments as one parentless commit
//! to `refs/planroom/backup/<workspace_id>` in one workspace repo. Nothing here pushes the ref.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use planroom_contract::{Event, ExportHeader, Seq};
use planroom_extract::git::{self, Git, GitError};
use planroom_kernel::redact;
use planroom_store::{BUSY_TIMEOUT, Backup, META_REPOS, Notice, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::workspace;

/// The lines of one segment of the redacted copy, the first segment's header line included. An
/// append rewrites only the tail segment, so it hashes at most this many lines.
pub const SEGMENT_LINES: usize = 1000;

/// The variables that mark a temporary host: a cloud session, a Codespace, a dev container.
const VARIABLES: [&str; 3] = ["CLAUDE_CODE_REMOTE", "CODESPACES", "REMOTE_CONTAINERS"];
/// The redacted copy's directory, beside the backup file.
const REDACTED: &str = "redacted";
/// The lock every step on the copy and the ref runs under, beside the backup file.
const LOCK: &str = "redacted.lock";
/// `{seq, commit}` of the copy's last successful ref update, in the copy's directory.
const COMMITTED: &str = "committed";
/// The wait between tries for the lock.
const STEP: Duration = Duration::from_millis(50);
/// The bytes a read back from a segment's end starts with, doubled until it holds a whole line.
const TAIL_READ: u64 = 4096;

/// Whether this process runs on a host that loses its files with its container: any of
/// `CLAUDE_CODE_REMOTE`, `CODESPACES` or `REMOTE_CONTAINERS` set and non-empty.
pub fn temporary_host() -> bool {
    VARIABLES.iter().any(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()))
}

/// The store's copy step on a temporary host ([`planroom_store::register_copy`]): bring the
/// redacted copy of `backup`'s file up to date and commit it to the backup ref. The repo is the
/// one `meta.backup_ref` names, fixed at the first write so the ref never moves between repos:
/// the workspace root when it is a repo, else the first recorded repo by namespace. `None` when
/// there is nothing to do: a state directory that names no workspace root, or no recorded repo
/// yet. Otherwise the `temporary_host` notice, or `backup_ref_failed`, which the next
/// person-authored commit retries.
pub fn copy_to_ref(store: &mut Store, backup: &Backup) -> Option<Notice> {
    let root = workspace::root_of_state_dir(&backup.state_dir)?;
    let repos: BTreeMap<String, String> = store
        .meta(META_REPOS)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let (key, recorded) = match store.meta("backup_ref").ok().flatten() {
        Some(key) => (key, true),
        None => {
            let key = if git::is_repo(&root) {
                String::new()
            } else {
                repos.iter().min_by_key(|(_, ns)| ns.as_str())?.0.clone()
            };
            match store.record_backup_ref(&key) {
                Ok(held) => (held, true),
                Err(_) => (key, false),
            }
        }
    };
    let reference = format!("refs/planroom/backup/{}", backup.workspace_id);
    let repo = repos.get(&key).cloned().unwrap_or_else(|| match key.as_str() {
        "" => workspace::escape(root.file_name().map_or(&b""[..], OsStr::as_bytes)),
        key => key.to_owned(),
    });
    let path = recorded.then(|| repo_path(&root, &key)).flatten().filter(|path| git::is_repo(path));
    match path.map(|path| write(&path, backup, &reference, SEGMENT_LINES)) {
        Some(Ok(_)) => Some(Notice::TemporaryHost { repo, reference }),
        _ => Some(Notice::BackupRefFailed { repo, reference }),
    }
}

/// The repo `key` names under `root`: the root for `""`, else the root joined with the key
/// unescaped, when that stays below the root.
fn repo_path(root: &Path, key: &str) -> Option<PathBuf> {
    if key.is_empty() {
        return Some(root.to_path_buf());
    }
    let bytes = workspace::unescape(key)?;
    let rel = Path::new(OsStr::from_bytes(&bytes));
    rel.components().all(|c| matches!(c, Component::Normal(_))).then(|| root.join(rel))
}

/// Why the copy or the ref wasn't written; the notice carries none of it.
#[derive(Debug)]
#[allow(dead_code, reason = "the cause shows in a test's failure message")]
enum Failed {
    Io(io::Error),
    Git(GitError),
    Store(StoreError),
    /// A line of the backup file that isn't an event, or a lock past its bound.
    Content(&'static str),
}

impl From<io::Error> for Failed {
    fn from(e: io::Error) -> Failed {
        Failed::Io(e)
    }
}

impl From<GitError> for Failed {
    fn from(e: GitError) -> Failed {
        Failed::Git(e)
    }
}

impl From<StoreError> for Failed {
    fn from(e: StoreError) -> Failed {
        Failed::Store(e)
    }
}

impl From<serde_json::Error> for Failed {
    fn from(_: serde_json::Error) -> Failed {
        Failed::Content("a line that isn't JSON")
    }
}

/// The copy's last successful ref update.
#[derive(Debug, Serialize, Deserialize)]
struct Committed {
    seq: Seq,
    commit: String,
}

/// Just the `seq` of an event line.
#[derive(Deserialize)]
struct SeqOnly {
    seq: Seq,
}

/// Where the copy's tail stands.
struct Tail {
    /// The tail segment's number, from 1.
    number: u32,
    /// Its lines.
    lines: usize,
    /// The copy's mark: the `seq` of its last line, 0 when it holds only the header.
    mark: Seq,
}

/// Under the copy's lock, bring the redacted copy of `backup`'s file up to date, with `segment`
/// lines a segment, and commit it to `reference` in the repo at `repo`; the new commit's ID.
fn write(repo: &Path, backup: &Backup, reference: &str, segment: usize) -> Result<String, Failed> {
    let _lock = lock(&backup.dir.join(LOCK))?;
    let dir = backup.dir.join(REDACTED);
    let mark = match tail(&dir, backup)? {
        Some(tail) => append(&dir, backup, tail, segment)?,
        None => rebuild(backup, segment)?,
    };
    let committed: Option<Committed> =
        fs::read(dir.join(COMMITTED)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let kept = match &committed {
        Some(c) => reusable(&dir, c.seq)?,
        None => BTreeSet::new(),
    };
    let reuse = committed.as_ref().map(|c| (c.commit.as_str(), &kept));
    let commit = Git::new(repo).write_backup_ref(reference, &dir, reuse)?;
    let tmp = dir.join(format!("{COMMITTED}.{}.tmp", unique()));
    fs::write(&tmp, serde_json::to_vec(&Committed { seq: mark, commit: commit.clone() })?)?;
    fs::rename(&tmp, dir.join(COMMITTED))?;
    Ok(commit)
}

/// The copy's tail, when this append can extend it; `None` when the copy must be rebuilt whole:
/// a new file (`after` 0, as after a rotation), no copy yet, a header that differs from the
/// file's once masked, a torn or unreadable tail, a mark below `after` (the copy missed lines), or a mark
/// above the file's last `seq` (the copy holds lines the file doesn't).
fn tail(dir: &Path, backup: &Backup) -> Result<Option<Tail>, Failed> {
    if backup.after == 0 {
        return Ok(None);
    }
    let names = segments(dir)?;
    let (Some(first), Some(last)) = (names.first(), names.last()) else { return Ok(None) };
    if first_line(&dir.join(first))? != header(&first_line(&backup.file)?)? {
        return Ok(None);
    }
    let text = fs::read(dir.join(last))?;
    let Some(body) = text.strip_suffix(b"\n") else { return Ok(None) };
    let lines: Vec<&[u8]> = body.split(|b| *b == b'\n').collect();
    let mark = match (names.len(), lines.len()) {
        (1, 1) => 0,
        _ => match lines.last().and_then(|line| seq_of(line)) {
            Some(seq) => seq,
            None => return Ok(None),
        },
    };
    if mark < backup.after || mark > planroom_store::file_seq(&backup.file)? {
        return Ok(None);
    }
    Ok(number(last).map(|number| Tail { number, lines: lines.len(), mark }))
}

/// Append the file's complete lines from `backup.offset` above the copy's mark, redacted, to the
/// tail segment, opening a new one as each fills; the copy's new mark.
fn append(dir: &Path, backup: &Backup, mut tail: Tail, segment: usize) -> Result<Seq, Failed> {
    let mut file = File::open(&backup.file)?;
    file.seek(SeekFrom::Start(backup.offset))?;
    let mut reader = BufReader::new(file);
    let mut out = BufWriter::new(open_segment(dir, tail.number, false)?);
    let mut line = Vec::new();
    while next_line(&mut reader, &mut line)? {
        let seq = seq_of(&line).ok_or(Failed::Content("an event line with no seq"))?;
        if seq <= tail.mark {
            continue;
        }
        if tail.lines >= segment {
            out.flush()?;
            tail.number += 1;
            tail.lines = 0;
            out = BufWriter::new(open_segment(dir, tail.number, true)?);
        }
        out.write_all(&redacted(&line)?)?;
        out.write_all(b"\n")?;
        tail.lines += 1;
        tail.mark = seq;
    }
    out.flush()?;
    Ok(tail.mark)
}

/// Write the whole copy into a directory of its own, then swap it in for the old one; its mark.
fn rebuild(backup: &Backup, segment: usize) -> Result<Seq, Failed> {
    let tmp = backup.dir.join(format!("{REDACTED}.{}.tmp", unique()));
    DirBuilder::new().mode(0o700).create(&tmp)?;
    let written = rebuild_into(&tmp, backup, segment).and_then(|mark| {
        let live = backup.dir.join(REDACTED);
        let old = backup.dir.join(format!("{REDACTED}.{}.old", unique()));
        match fs::rename(&live, &old) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        fs::rename(&tmp, &live)?;
        let _ = fs::remove_dir_all(&old);
        Ok(mark)
    });
    if written.is_err() {
        let _ = fs::remove_dir_all(&tmp);
    }
    written
}

/// The file's header line, then every complete event line, each redacted, into `dir`.
fn rebuild_into(dir: &Path, backup: &Backup, segment: usize) -> Result<Seq, Failed> {
    let mut reader = BufReader::new(File::open(&backup.file)?);
    let mut line = Vec::new();
    if !next_line(&mut reader, &mut line)? {
        return Err(Failed::Content("a backup file with no header"));
    }
    let (mut number, mut lines, mut mark) = (1, 1, 0);
    let mut out = BufWriter::new(open_segment(dir, number, true)?);
    out.write_all(&header(&line)?)?;
    out.write_all(b"\n")?;
    while next_line(&mut reader, &mut line)? {
        let seq = seq_of(&line).ok_or(Failed::Content("an event line with no seq"))?;
        if lines >= segment {
            out.flush()?;
            number += 1;
            lines = 0;
            out = BufWriter::new(open_segment(dir, number, true)?);
        }
        out.write_all(&redacted(&line)?)?;
        out.write_all(b"\n")?;
        lines += 1;
        mark = seq;
    }
    out.flush()?;
    Ok(mark)
}

/// The segments whose every line is at or below `seq`, which a ref update at `seq` committed as
/// they are now. Segments run in `seq` order, so the first one past `seq` ends the list.
fn reusable(dir: &Path, seq: Seq) -> Result<BTreeSet<String>, Failed> {
    let mut kept = BTreeSet::new();
    for name in segments(dir)? {
        match last_line(&dir.join(&name))? {
            Some(line) if seq_of(&line).is_some_and(|last| last <= seq) => {
                kept.insert(name);
            }
            _ => break,
        }
    }
    Ok(kept)
}

/// The copy's segment names, in order; none when the directory is missing.
fn segments(dir: &Path) -> Result<Vec<String>, Failed> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut names = Vec::new();
    for entry in entries {
        if let Ok(name) = entry?.file_name().into_string()
            && git::is_segment(&name)
        {
            names.push(name);
        }
    }
    names.sort_unstable();
    Ok(names)
}

fn segment_name(number: u32) -> String {
    format!("events-{number:06}.ndjson")
}

/// The number in a segment's name.
fn number(name: &str) -> Option<u32> {
    name.strip_prefix("events-")?.strip_suffix(".ndjson")?.parse().ok()
}

/// Segment `number` in `dir`, mode 0600: created anew, or opened to append to.
fn open_segment(dir: &Path, number: u32, new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.mode(0o600);
    if new {
        options.write(true).create_new(true);
    } else {
        options.append(true);
    }
    options.open(dir.join(segment_name(number)))
}

/// The next complete line from `reader` into `line`, without its newline; false at the end, or
/// at a torn last line with no newline after it.
fn next_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<bool> {
    line.clear();
    reader.read_until(b'\n', line)?;
    if line.pop() == Some(b'\n') {
        return Ok(true);
    }
    line.clear();
    Ok(false)
}

/// `path`'s first line, without its newline; empty for an empty file.
fn first_line(path: &Path) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    BufReader::new(File::open(path)?).read_until(b'\n', &mut line)?;
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    Ok(line)
}

/// `path`'s last complete line, read back from its end; `None` when it holds none.
fn last_line(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut want = TAIL_READ;
    loop {
        let start = len.saturating_sub(want);
        let mut bytes = Vec::with_capacity((len - start) as usize);
        file.seek(SeekFrom::Start(start))?;
        (&mut file).take(len - start).read_to_end(&mut bytes)?;
        let Some(body) = bytes.strip_suffix(b"\n") else { return Ok(None) };
        match body.iter().rposition(|b| *b == b'\n') {
            Some(at) => return Ok(Some(body[at + 1..].to_vec())),
            None if start == 0 => return Ok(Some(body.to_vec())),
            None => want = want.saturating_mul(2),
        }
    }
}

fn seq_of(line: &[u8]) -> Option<Seq> {
    serde_json::from_slice::<SeqOnly>(line).ok().map(|s| s.seq)
}

/// An event line as the copy holds it: through [`redact::value_with_open`], which masks every
/// marked field and every value under the event's open fields, whatever `PLANROOM_RAW` says.
fn redacted(line: &[u8]) -> Result<Vec<u8>, Failed> {
    let mut value: Value = serde_json::from_slice(line)?;
    redact::value_with_open::<Event>(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

/// The header line as the copy holds it: through [`redact::value_with_open`], which masks its
/// marked `state_dir`, repo-derived path text, and every value in its open `extra`, leaving an ID
/// such as `workspace_id` whole. Keys keep their order, so a header with nothing to mask is
/// unchanged.
fn header(line: &[u8]) -> Result<Vec<u8>, Failed> {
    let mut value: Value = serde_json::from_slice(line)?;
    redact::value_with_open::<ExportHeader>(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

/// Take the exclusive lock on `path`, created with mode 0600, retrying every 50 ms up to
/// [`BUSY_TIMEOUT`]. Held until the returned file is dropped.
fn lock(path: &Path) -> Result<File, Failed> {
    let file = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(path)?;
    let deadline = Instant::now() + BUSY_TIMEOUT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(STEP),
            Err(TryLockError::WouldBlock) => return Err(Failed::Content("the copy's lock stayed held")),
            Err(TryLockError::Error(e)) => return Err(e.into()),
        }
    }
}

/// A name no other process or call picks at the same time.
fn unique() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use planroom_store::Backup;

    use super::{REDACTED, write};
    use crate::app::workspace::root_of_state_dir;

    /// Plain `git ARGS` in `dir`; its stdout.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    /// The repo's loose objects, as `git count-objects -v` counts them.
    fn loose(repo: &Path) -> u64 {
        let out = git(repo, &["count-objects", "-v"]);
        out.lines().find_map(|l| l.strip_prefix("count: ")).unwrap().parse().unwrap()
    }

    /// The blob each name in `reference`'s tree points to.
    fn blobs(repo: &Path, reference: &str) -> Vec<(String, String)> {
        git(repo, &["ls-tree", reference])
            .lines()
            .map(|l| {
                let (meta, name) = l.split_once('\t').unwrap();
                (name.to_owned(), meta.rsplit(' ').next().unwrap().to_owned())
            })
            .collect()
    }

    fn event(seq: u64) -> String {
        format!(
            "{{\"seq\":{seq},\"id\":\"ev-{seq}\",\"body\":{{\"kind\":\"edit\",\"fields\":{{\"name\":\"STRIPE_SECRET_KEY=sk_live_51PlantedUnitKey{seq:04}\"}}}}}}\n"
        )
    }

    /// Append `seq`'s event to the backup file; the [`Backup`] that append reports.
    fn append(file: &Path, dir: &Path, after: u64, seq: u64) -> Backup {
        use std::io::Write as _;
        let offset = std::fs::metadata(file).unwrap().len();
        let mut f = std::fs::OpenOptions::new().append(true).open(file).unwrap();
        f.write_all(event(seq).as_bytes()).unwrap();
        Backup {
            state_dir: dir.join("state"),
            dir: dir.to_path_buf(),
            file: file.to_path_buf(),
            workspace_id: "w".into(),
            offset,
            after,
            through: seq,
        }
    }

    #[test]
    fn only_the_tail_segment_is_hashed() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        let dir = tmp.path().join("backup");
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("events.ndjson");
        let header = "{\"format\":\"planroom.event-export/v1\",\"workspace_id\":\"w\"}\n";
        let first: String = std::iter::once(header.to_owned()).chain((1..=4).map(event)).collect();
        std::fs::write(&file, &first).unwrap();
        let reference = "refs/planroom/backup/w";
        let backup = Backup {
            state_dir: dir.join("state"),
            dir: dir.clone(),
            file: file.clone(),
            workspace_id: "w".into(),
            offset: header.len() as u64,
            after: 0,
            through: 4,
        };
        let one = write(&repo, &backup, reference, 3).unwrap();
        let tree = blobs(&repo, reference);
        let names: Vec<&str> = tree.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["events-000001.ndjson", "events-000002.ndjson"]);
        let copy = dir.join(REDACTED);
        let text = std::fs::read_to_string(copy.join("events-000001.ndjson")).unwrap();
        assert!(text.starts_with(header), "the header is the file's: {text}");
        assert!(!text.contains("sk_live_51PlantedUnitKey") && text.contains("[redacted]"), "{text}");

        // A first segment that differs on disk from what the ref holds shows whether it is read
        // again: a reused segment keeps the old tree's blob.
        let seg = copy.join("events-000001.ndjson");
        let changed = std::fs::read_to_string(&seg).unwrap().replacen("\n{", "\n{ ", 1);
        std::fs::write(&seg, changed).unwrap();
        let before = loose(&repo);
        let two = write(&repo, &append(&file, &dir, 4, 5), reference, 3).unwrap();
        assert_ne!(one, two);
        let after = blobs(&repo, reference);
        assert_eq!(after[0], tree[0], "the first segment keeps its blob");
        assert_ne!(after[1], tree[1], "the tail segment is hashed again");
        assert_eq!(loose(&repo), before + 3, "one blob, one tree and one commit");
        let object = git(&repo, &["cat-file", "-p", &two]);
        assert!(!object.contains("\nparent "), "{object}");

        // A full tail opens the next segment, and the full one is kept as it is.
        let before = loose(&repo);
        write(&repo, &append(&file, &dir, 5, 6), reference, 3).unwrap();
        let third = blobs(&repo, reference);
        assert_eq!(third.len(), 3, "{third:?}");
        assert_eq!(third[..2], after[..], "both full segments keep their blobs");
        assert_eq!(loose(&repo), before + 3, "one blob, one tree and one commit");
        let lines: usize =
            third.iter().map(|(n, _)| std::fs::read_to_string(copy.join(n)).unwrap().lines().count()).sum();
        assert_eq!(lines, std::fs::read_to_string(&file).unwrap().lines().count());
    }

    /// The header is masked as every other line is: its `state_dir` is repo-derived path text,
    /// and its `extra` an open value. A later append still extends the copy, since the copy's
    /// header is compared with the file's once both are masked.
    #[test]
    fn the_copys_header_is_redacted() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        let dir = tmp.path().join("backup");
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("events.ndjson");
        let header = "{\"format\":\"planroom.event/v1\",\"workspace_id\":\"w\",\"store_version\":3,\
            \"state_dir\":\"/home/STRIPE_SECRET_KEY=sk_live_51PlantedHeaderKey0000/ws/.planroom\",\
            \"note\":\"STRIPE_SECRET_KEY=sk_live_51PlantedExtraKey00000\"}\n";
        let first: String = std::iter::once(header.to_owned()).chain((1..=4).map(event)).collect();
        std::fs::write(&file, &first).unwrap();
        let reference = "refs/planroom/backup/w";
        let backup = Backup {
            state_dir: dir.join("state"),
            dir: dir.clone(),
            file: file.clone(),
            workspace_id: "w".into(),
            offset: header.len() as u64,
            after: 0,
            through: 4,
        };
        write(&repo, &backup, reference, 3).unwrap();
        let committed = git(&repo, &["cat-file", "-p", &format!("{reference}:events-000001.ndjson")]);
        let line = committed.lines().next().unwrap();
        assert!(!line.contains("sk_live_51Planted"), "the ref's header holds no secret: {line}");
        assert!(line.contains("[redacted]"), "{line}");
        let masked: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(masked["workspace_id"], "w", "an ID stays whole: {line}");
        assert_eq!(masked["store_version"], 3);

        // A mark on the first segment shows whether the next append rebuilt the copy whole.
        let seg = dir.join(REDACTED).join("events-000001.ndjson");
        let marked = std::fs::read_to_string(&seg).unwrap().replacen("\n{", "\n{ ", 1);
        std::fs::write(&seg, &marked).unwrap();
        write(&repo, &append(&file, &dir, 4, 5), reference, 3).unwrap();
        assert_eq!(std::fs::read_to_string(&seg).unwrap(), marked, "the append extends the copy");
    }

    #[test]
    fn a_state_dir_names_its_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(root_of_state_dir(&root.join("shop/.planroom")), Some(root.join("shop")));
        assert_eq!(root_of_state_dir(&root.join("app/.git/planroom")), Some(root.join("app")));

        // A linked worktree's git directory names its `.git` file in `gitdir`.
        let gitdir = root.join("app/.git/worktrees/feature");
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::write(gitdir.join("gitdir"), format!("{}\n", root.join("feature/.git").display())).unwrap();
        assert_eq!(root_of_state_dir(&gitdir.join("planroom")), Some(root.join("feature")));

        // A submodule's git directory has no `gitdir` file, and other names are no state directory.
        assert_eq!(root_of_state_dir(&root.join("app/.git/modules/lib/planroom")), None);
        assert_eq!(root_of_state_dir(&root.join("app/state")), None);
        assert_eq!(root_of_state_dir(&PathBuf::from("/")), None);
    }
}
