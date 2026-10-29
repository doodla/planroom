//! The git CLI adapter (DR-16): every call runs with hooks, fsmonitor and optional locks off, a
//! cleared environment and `LC_ALL=C`, so nothing from a scanned repo runs. Every call is local,
//! and every one is a read but the temporary host's backup ref ([`Git::write_backup_ref`]), whose
//! plumbing writes objects and one ref under `refs/planroom/` and never a working tree, index or
//! branch. No transport may run, each refused by name whatever the repo's own config allows: a
//! partial clone's missing object is never fetched, so the `uploadpack`, `sshCommand` or `ext::`
//! command a repo's own config names never runs either, on any git version. Every call names the
//! repo's git directory with `GIT_DIR`, as the engine resolved it after its owner check, so git
//! never discovers a repo by searching the directories above a working tree.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

/// git exited non-zero, or couldn't start, or the adapter refused to run it in a repo another
/// user owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    pub argv: Vec<String>,
    pub stderr: String,
    /// Why the adapter refused before git ran, when it did.
    pub refusal: Option<Refusal>,
}

/// Why the adapter refused to run git in a repo, before git ran there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Another user, `uid`, owns `place`, a place git uses for the repo (DR-78).
    Owned { place: PathBuf, uid: u32, what: OwnedPlace },
    /// A place git needs as a directory or a regular file is neither, such as a FIFO, which
    /// would block git: a `commondir` file, the common directory it names, the git directory a
    /// `.git` file names, one of the [`FIXED_FILES`] in the git directory or common directory, or
    /// the `info/alternates` file of an object directory git reads.
    NotAFile(PathBuf),
}

/// Which place git uses for a repo another user owns ([`Refusal::Owned`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedPlace {
    /// The working tree, or its `.git`.
    Tree,
    /// The git directory a `.git` file names, or the common directory a `commondir` file there
    /// names.
    GitDir,
    /// An object directory git reads: the `objects` directory of the git directory or the common
    /// directory, or one an `objects/info/alternates` file names, at any depth git follows.
    Objects,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "git {} failed: {}", self.argv.join(" "), self.stderr.trim())
    }
}

impl std::error::Error for GitError {}

impl GitError {
    /// git failed with `stderr`.
    pub fn new(argv: &[&str], stderr: String) -> GitError {
        GitError { argv: argv.iter().map(|a| (*a).to_owned()).collect(), stderr, refusal: None }
    }

    /// Another user owns the repo: the adapter refused it before git ran, or git did
    /// (`safe.directory`), which it can still do in a repo it finds itself, as an older adapter's
    /// call could.
    pub fn is_other_owner(&self) -> bool {
        matches!(self.refusal, Some(Refusal::Owned { .. })) || self.stderr.contains("dubious ownership")
    }
}

/// The files git opens at fixed paths in a git directory or a common directory. One that exists
/// and isn't a regular file, such as a FIFO, which would block git, refuses the repo before git
/// runs ([`Refusal::NotAFile`]); one that resolves to the null device is read as empty, as git
/// reads it. A FIFO git opens anywhere else (a working tree's `.gitignore`, a loose ref or
/// object, a pack, a file the repo's config names) isn't checked. Whoever can write in the
/// directory it sits in can plant one there. Every place git uses for the repo, its object
/// directories and their alternates included, is the user's own, since a repo another user owns
/// any of is refused before git runs; so that's the user, root, and, in a repo shared with a
/// group (`core.sharedRepository`, whose directories the group can write), any member of that
/// group, whose FIFO still blocks the scan.
pub const FIXED_FILES: [&str; 9] = [
    "HEAD",
    "config",
    "config.worktree",
    "packed-refs",
    "index",
    "shallow",
    "objects/info/alternates",
    "info/exclude",
    "info/attributes",
];

/// The first of [`FIXED_FILES`] in `git_dir`, then in `common`, that git would block on or can't
/// use ([`unreadable`]). It reads only metadata.
fn not_a_regular_fixed_file(git_dir: &Path, common: Option<&Path>) -> Option<PathBuf> {
    let dirs = std::iter::once(git_dir).chain(common.filter(|c| *c != git_dir));
    dirs.flat_map(|dir| FIXED_FILES.iter().map(move |file| dir.join(file))).find(|path| unreadable(path))
}

/// Whether `path` exists, followed as git's `open` follows a symlink, and is neither a regular
/// file nor the null device, which git reads as empty: a FIFO, a socket, a directory or another
/// device, which git would block on or can't use. It reads only metadata.
fn unreadable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| !m.is_file() && !is_null_device(&m))
}

/// Whether `meta` is the null device's: a character device with `/dev/null`'s device number,
/// however the path to it is spelled. A FIFO or a file named like it is not.
fn is_null_device(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let char_device = |m: &std::fs::Metadata| m.file_type().is_char_device();
    let Ok(null) = std::fs::metadata("/dev/null") else { return false };
    char_device(meta) && char_device(&null) && meta.rdev() == null.rdev()
}

/// The effective user ID the engine registered at start-up ([`set_euid`]).
static EUID: OnceLock<u32> = OnceLock::new();

/// Register the process's effective user ID, once, at start-up. The engine reads it (this crate
/// has no `libc`) and passes it here, so every [`Git`] call first compares it with the repo's
/// owner, and the guarantee doesn't rest on git's version or a backport of its own ownership check
/// (DR-78). With none registered, as in this crate's own tests, no call is refused.
pub fn set_euid(uid: u32) {
    let _ = EUID.set(uid);
}

/// Where git keeps the repo whose working tree is at `dir`, as the engine resolves it before any
/// git call: the git directory every call pins with `GIT_DIR`, and the places git uses there.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Places {
    /// The git directory: `.git` itself, or the directory a `.git` file names. `Err` says why git
    /// can't use it: the `.git` is neither a directory nor a regular file that names one, the
    /// directory it names isn't one, or the git directory's `commondir` isn't a regular file,
    /// which git would block on or fail to read, or names something that isn't a directory, or
    /// one of the [`FIXED_FILES`] there or in the common directory, or the `info/alternates` of
    /// an object directory git reads, exists and is neither a regular file nor the null device.
    git_dir: Result<PathBuf, Unusable>,
    /// Every place git reads for this repo, whose owner counts: the working tree, its `.git`, the
    /// git directory a `.git` file names, and the common directory a `commondir` file there
    /// names, as a linked worktree's does.
    used: Vec<PathBuf>,
    /// The object directories git reads for this repo that exist, whose owners count as well:
    /// the `objects` of the git directory and the common directory, and each directory their
    /// `info/alternates` names, as git follows them ([`object_dirs`]).
    objects: Vec<PathBuf>,
}

/// Whether `path` is a regular file, following a symlink as git's `open` does. A FIFO, a device
/// or a socket is not, so nothing here opens one, which could block.
fn is_regular(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// `O_NONBLOCK`, which this crate spells itself since its allow-list has no `libc`: opening a FIFO
/// with it returns at once, where a plain open would wait for a writer. A regular file reads as
/// ever.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const O_NONBLOCK: i32 = 0x0004;
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NONBLOCK: i32 = 0o4000;

/// The first 64 KiB of `file` when, opened, it's a regular file; `None` when it isn't, or can't
/// be read. It opens with `O_NONBLOCK` and checks what it opened, so a FIFO, even one swapped in
/// after a regular-file check, never blocks it.
fn read_regular(file: &Path) -> Option<Vec<u8>> {
    read_regular_up_to(file, 64 * 1024)
}

/// The first `limit` bytes of `file`, as [`read_regular`] reads it.
fn read_regular_up_to(file: &Path, limit: u64) -> Option<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt;
    let opened = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(file).ok()?;
    if !opened.metadata().ok()?.is_file() {
        return None;
    }
    let mut text = Vec::new();
    opened.take(limit).read_to_end(&mut text).ok()?;
    Some(text)
}

/// The path a `.git` file (`gitdir: PATH`) or a `commondir` file names, read as git reads it: the
/// whole file, here up to 64 KiB, less its trailing line breaks, relative to `base` when it isn't
/// absolute. `None` when the file is missing, isn't a regular file, lacks `prefix` or names
/// nothing.
fn named_path(file: &Path, prefix: &[u8], base: &Path) -> Option<PathBuf> {
    if !is_regular(file) {
        return None;
    }
    let text = read_regular(file)?;
    let mut named = text.strip_prefix(prefix)?;
    while let Some(rest) = named.strip_suffix(b"\n").or_else(|| named.strip_suffix(b"\r")) {
        named = rest;
    }
    (!named.is_empty()).then(|| base.join(std::ffi::OsStr::from_bytes(named)))
}

/// The owner of `place` when it's a user other than `euid`; `None` when it's `euid`'s or doesn't
/// exist, which git itself then reports.
fn foreign_uid(place: &Path, euid: u32) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(place).ok().map(|m| m.uid()).filter(|uid| *uid != euid)
}

/// The [`Places`] of the working tree at `dir`, or the first of them that a user other than
/// `euid` owns, with that owner. It compares each place's owner as it finds it, before it reads
/// anything there: the working tree's and its `.git`'s before the `.git` file, and the owner of
/// the git directory that file names before that directory's `commondir`. So it never opens a
/// file in a place another user owns, and it opens only regular files. With no `euid`, as in
/// this crate's own tests, no owner counts. It reads metadata, a `.git` file and a `commondir`
/// file, then the object directories and their `info/alternates` files ([`object_dirs`]), then,
/// with every place the user's, the metadata of the [`FIXED_FILES`], and never runs git.
fn places(dir: &Path, euid: Option<u32>) -> Result<Places, Foreign> {
    let mut used = Vec::new();
    let mut admit = |place: PathBuf, what: OwnedPlace| -> Result<(), Foreign> {
        if let Some(uid) = euid.and_then(|euid| foreign_uid(&place, euid)) {
            return Err(Foreign { place, uid, what });
        }
        used.push(place);
        Ok(())
    };
    // Something that exists at `path` but isn't a directory, which git needs there.
    let not_a_dir = |path: &Path| std::fs::metadata(path).is_ok_and(|m| !m.is_dir());
    let dot_git = dir.join(".git");
    admit(dir.to_path_buf(), OwnedPlace::Tree)?;
    admit(dot_git.clone(), OwnedPlace::Tree)?;
    let mut git_dir = if dot_git.is_dir() {
        Ok(dot_git)
    } else {
        match named_path(&dot_git, b"gitdir: ", dir) {
            Some(named) => {
                admit(named.clone(), OwnedPlace::GitDir)?;
                if not_a_dir(&named) {
                    let why = format!("{} isn't a directory", named.display());
                    Err(Unusable { why, place: Some(named) })
                } else {
                    Ok(named)
                }
            }
            None => {
                Err(Unusable { why: format!("{} names no git directory", dot_git.display()), place: None })
            }
        }
    };
    let mut common_dir = None;
    if let Ok(git) = &git_dir {
        let common = git.join("commondir");
        if std::fs::symlink_metadata(&common).is_ok() {
            if !is_regular(&common) {
                let why = format!("{} isn't a regular file", common.display());
                git_dir = Err(Unusable { why, place: Some(common) });
            } else if let Some(named) = named_path(&common, b"", git) {
                admit(named.clone(), OwnedPlace::GitDir)?;
                if not_a_dir(&named) {
                    let why =
                        format!("{} names {}, which isn't a directory", common.display(), named.display());
                    git_dir = Err(Unusable { why, place: Some(named) });
                } else {
                    common_dir = Some(named);
                }
            }
        }
    }
    // The object directories git reads, each compared before anything in it is read, so a FIFO
    // another user plants in an object store a repo borrows never blocks git either.
    let mut objects = Vec::new();
    if let Ok(git) = &git_dir {
        let roots = std::iter::once(git).chain(common_dir.iter()).map(|d| d.join("objects")).collect();
        match object_dirs(roots, euid)? {
            Ok(found) => objects = found,
            Err(unusable) => git_dir = Err(unusable),
        }
    }
    // Every place is this user's now, so a FIFO at a file git opens at a fixed path is theirs.
    if let Ok(git) = &git_dir
        && let Some(file) = not_a_regular_fixed_file(git, common_dir.as_deref())
    {
        git_dir = Err(not_a_file(file));
    }
    Ok(Places { git_dir, used, objects })
}

/// Why git can't use `file`, which isn't a regular file, shown where it sits: its directory as it
/// resolves, without a common directory's `..`, and the file itself not followed, so `ls -l`
/// shows it in the repo, and a symlink with what it names, which is the file to replace, never
/// the device or FIFO it names.
fn not_a_file(file: PathBuf) -> Unusable {
    let resolved = file
        .parent()
        .zip(file.file_name())
        .and_then(|(dir, name)| std::fs::canonicalize(dir).ok().map(|dir| dir.join(name)));
    let file = resolved.unwrap_or(file);
    let why = format!("{} isn't a regular file", file.display());
    Unusable { why, place: Some(file) }
}

/// How deep git follows `info/alternates`: the repo's own object directories' files are read at
/// depth 0, and what a file read deeper than this names is ignored, as git ignores it. git still
/// opens that deepest file, at depth 6, so the check looks at it too.
const ALTERNATES_DEPTH: usize = 5;
/// The most object directories the owner check follows for one repo; a repo that names more
/// fails as `git_failed`, so the check's work stays bounded.
const MAX_OBJECT_DIRS: usize = 1000;
/// The most `info/alternates` bytes the owner check reads for one repo, all its files together; a
/// repo whose files hold more fails as `git_failed`, since a part left unread could name another
/// user's directory. With [`alternate_lines`] linear in what it reads, this bounds each call's
/// work.
const MAX_ALTERNATES_BYTES: u64 = 1024 * 1024;

/// The object directories git reads from `roots` (the git directory's and the common
/// directory's `objects`) that exist, each followed through its `info/alternates` as git
/// follows it ([`alternate_lines`], [`ALTERNATES_DEPTH`]). It compares each one's owner with
/// `euid` before it reads anything there, so it never reads a file in another user's object
/// directory: the first another user owns is `Err`. A missing one, or one that isn't a
/// directory, git skips and reports itself. An `info/alternates` that exists and is neither a
/// regular file nor the null device, which git would block on, makes the git directory
/// unusable, and so do files over [`MAX_ALTERNATES_BYTES`] in all, or more than
/// [`MAX_OBJECT_DIRS`] directories. A directory reached twice is read once, at the shallowest
/// depth.
fn object_dirs(roots: Vec<PathBuf>, euid: Option<u32>) -> Result<Result<Vec<PathBuf>, Unusable>, Foreign> {
    use std::os::unix::fs::MetadataExt;
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> = roots.into_iter().map(|r| (r, 0)).collect();
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    let mut budget = MAX_ALTERNATES_BYTES;
    while let Some((dir, depth)) = queue.pop_front() {
        if !seen.insert(dir.as_os_str().as_bytes().to_vec()) {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&dir) else { continue };
        if !meta.is_dir() {
            continue;
        }
        if let Some(euid) = euid
            && meta.uid() != euid
        {
            return Err(Foreign { place: dir, uid: meta.uid(), what: OwnedPlace::Objects });
        }
        found.push(dir.clone());
        if found.len() > MAX_OBJECT_DIRS {
            let why = format!("{} names more than {MAX_OBJECT_DIRS} object directories", dir.display());
            return Ok(Err(Unusable { why, place: None }));
        }
        // git opens this directory's file even one level past the deepest it follows, and only
        // then ignores what it names, so a FIFO there would block it all the same.
        let file = dir.join("info/alternates");
        if unreadable(&file) {
            return Ok(Err(not_a_file(file)));
        }
        if depth > ALTERNATES_DEPTH {
            continue;
        }
        let Some(text) = read_regular_up_to(&file, budget + 1) else { continue };
        if text.len() as u64 > budget {
            let why =
                format!("{} takes the alternates read past {MAX_ALTERNATES_BYTES} bytes", file.display());
            return Ok(Err(Unusable { why, place: None }));
        }
        budget -= text.len() as u64;
        // A relative path is relative to this directory as it resolves, as git reads it.
        let base = std::fs::canonicalize(&dir).unwrap_or(dir);
        queue.extend(alternate_lines(&base, &text).into_iter().map(|alt| (alt, depth + 1)));
    }
    Ok(Ok(found))
}

/// The directories an `info/alternates` file holding `text` names, each made absolute against
/// `base`, the objects directory it sits in, then resolved both ways git resolves one: by name,
/// `..` and all, as a git before 2.39 does, where a `..` that climbs above the root names
/// nothing; and, when it exists, as `realpath` resolves it, following each symlink before the
/// `..` after it, as git from 2.39 on does. The entries are git's own reading first: an entry per
/// line, a line that starts with `#` a comment, one that starts with `"` C-quoted (where a quoted
/// entry may run past a line break) unless its quoting is broken. Then, so that no reading git
/// might take is left out, each line read alone, with and without a trailing carriage return.
/// Empty entries name nothing, and each directory comes once, in the order first named. Its work
/// is linear in `text`: a `stat` per distinct entry, and a `realpath` only for one that exists.
fn alternate_lines(base: &Path, text: &[u8]) -> Vec<PathBuf> {
    use std::borrow::Cow;
    let line_end = |s: &[u8]| s.iter().position(|b| *b == b'\n').unwrap_or(s.len());
    let mut entries: Vec<Cow<'_, [u8]>> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let end = if rest[0] == b'#' {
            line_end(rest)
        } else if let Some((entry, end)) = unquote_c(rest) {
            entries.push(Cow::Owned(entry));
            end
        } else {
            let end = line_end(rest);
            entries.push(Cow::Borrowed(&rest[..end]));
            end
        };
        // As git does, the byte after an entry is its separator, whatever it is.
        rest = rest.get(end + 1..).unwrap_or_default();
    }
    for line in text.split(|b| *b == b'\n').filter(|line| line.first() != Some(&b'#')) {
        entries.extend(unquote_c(line).map(|(entry, _)| Cow::Owned(entry)));
        entries.push(Cow::Borrowed(line));
        entries.extend(line.strip_suffix(b"\r").map(Cow::Borrowed));
    }
    let mut tried = std::collections::HashSet::new();
    let mut named = std::collections::HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    for entry in entries.iter().filter(|e| !e.is_empty()) {
        if !tried.insert(entry) {
            continue;
        }
        let path = Path::new(std::ffi::OsStr::from_bytes(entry));
        let joined = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
        // The kernel resolves a path as `realpath` does, so a `stat` of the path as joined finds
        // the directory a git from 2.39 on reads; only one that exists is worth resolving.
        let real = std::fs::metadata(&joined).ok().and_then(|_| std::fs::canonicalize(&joined).ok());
        for dir in normalized(&joined).into_iter().chain(real) {
            if named.insert(dir.as_os_str().as_bytes().to_vec()) {
                out.push(dir);
            }
        }
    }
    out
}

/// `path` with `.` and `..` resolved by name, as a git before 2.39 resolves an alternate with
/// `normalize_path_copy`; `None` when a `..` climbs above the root.
fn normalized(path: &Path) -> Option<PathBuf> {
    // On the bytes, since every path here is absolute: an empty `out` is the root.
    let mut out: Vec<u8> = Vec::with_capacity(path.as_os_str().len());
    for part in path.as_os_str().as_bytes().split(|b| *b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => out.truncate(out.iter().rposition(|b| *b == b'/')?),
            name => {
                out.push(b'/');
                out.extend_from_slice(name);
            }
        }
    }
    if out.is_empty() {
        out.push(b'/');
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

/// The C-quoted string at the start of `text`, as git's `unquote_c_style` reads it, with the
/// offset just past its closing quote; `None` when `text` doesn't start with `"` or its quoting is
/// broken: no closing quote, or an escape git doesn't know.
fn unquote_c(text: &[u8]) -> Option<(Vec<u8>, usize)> {
    let mut out = Vec::new();
    let mut i = 1;
    if text.first() != Some(&b'"') {
        return None;
    }
    loop {
        match *text.get(i)? {
            b'"' => return Some((out, i + 1)),
            b'\\' => {
                let escaped = *text.get(i + 1)?;
                i += 2;
                let byte = match escaped {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b'f' => 0x0c,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'v' => 0x0b,
                    b'\\' | b'"' => escaped,
                    b'0'..=b'3' => {
                        let digit =
                            |b: Option<&u8>| b.filter(|b| (b'0'..=b'7').contains(*b)).map(|b| b - b'0');
                        let (two, three) = (digit(text.get(i))?, digit(text.get(i + 1))?);
                        i += 2;
                        ((escaped - b'0') << 6) | (two << 3) | three
                    }
                    _ => return None,
                };
                out.push(byte);
            }
            0 => return None,
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
}

/// A place git uses for a repo that a user other than the engine's owns.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Foreign {
    place: PathBuf,
    uid: u32,
    what: OwnedPlace,
}

/// Why git can't use a repo's git directory, found before git runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Unusable {
    why: String,
    /// The place git needs as a directory or a regular file that is neither ([`Refusal::NotAFile`]);
    /// `None` when there's nothing there to show, as for a `.git` that names no git directory.
    place: Option<PathBuf>,
}

/// The first place git uses for the working tree at `dir` ([`Places`]) that a user other than
/// `euid` owns, with that owner. `None` when each is `euid`'s or doesn't exist, which git itself
/// then reports. No directory above the working tree counts: every call pins its git directory,
/// so git never looks there.
pub fn foreign_owner(dir: &Path, euid: u32) -> Option<(PathBuf, u32)> {
    places(dir, Some(euid)).err().map(|f| (f.place, f.uid))
}

/// The refusal every [`Git`] call in the working tree at `dir` meets when another user owns a
/// place git uses for it, compared with the effective user ID the engine registered
/// ([`set_euid`]); `None` when none is registered, or every place is that user's.
pub fn owner_refusal(dir: &Path) -> Option<Refusal> {
    let euid = *EUID.get()?;
    places(dir, Some(euid)).err().map(|Foreign { place, uid, what }| Refusal::Owned { place, uid, what })
}

/// The [`Places`] of the working tree at `dir`, compared with the effective user ID the engine
/// registered ([`set_euid`]), or the refusal a git call there for `args` meets: another user owns
/// a place git uses for it ([`Refusal::Owned`]).
fn checked_places(dir: &Path, args: &[&str]) -> Result<Places, GitError> {
    let euid = EUID.get().copied();
    places(dir, euid).map_err(|Foreign { place, uid, what }| {
        let euid = euid.unwrap_or(uid);
        let stderr = format!("user {uid}, not this process's user {euid}, owns {}", place.display());
        GitError { refusal: Some(Refusal::Owned { place, uid, what }), ..GitError::new(args, stderr) }
    })
}

/// The git directory git may use for the working tree at `dir`, after the owner check
/// ([`checked_places`]), or why it can't: another user owns a place git uses, or a place git needs
/// as a directory or a regular file is neither ([`Refusal::NotAFile`]), or there is none.
fn usable_git_dir(dir: &Path, args: &[&str]) -> Result<PathBuf, GitError> {
    checked_places(dir, args)?.git_dir.map_err(|Unusable { why, place }| GitError {
        refusal: place.map(Refusal::NotAFile),
        ..GitError::new(args, why)
    })
}

/// The most bytes of `.gitmodules` the adapter reads; a larger one fails as `git_failed`.
const MAX_GITMODULES_BYTES: u64 = 1024 * 1024;

/// The `path` each `submodule` section of a `.gitmodules` file holding `text` names, read as git
/// reads a config file: section and key names in any case, a subsection in quotes (or after a
/// `.`), a value with `"` quoting, the `\\`, `\"`, `\t`, `\b` and `\n` escapes and a `\` that
/// continues the line, and a `#` or `;` comment outside quotes. A path git ignores, one that starts
/// with `-`, is left out. `None` when git couldn't parse the file, which then names no submodule.
///
/// Hand-written: git's config syntax is small, no allowed crate reads it, and the adapter never
/// asks git, since `git config --file` on a working tree's file is git reading the repo's own
/// input to answer a question this reads directly.
fn gitmodules_paths(text: &[u8]) -> Option<Vec<Vec<u8>>> {
    // git reads `\r\n` as `\n`, and the end of the text as one more `\n`.
    let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(text);
    let mut bytes = Vec::with_capacity(text.len() + 1);
    let mut k = 0;
    while k < text.len() {
        if text[k] == b'\r' && text.get(k + 1) == Some(&b'\n') {
            k += 1;
        }
        bytes.push(text[k]);
        k += 1;
    }
    bytes.push(b'\n');
    let key_char = |c: u8| c.is_ascii_alphanumeric() || c == b'-';
    let space = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let mut i = 0;
    let mut in_submodule = false;
    let mut out = Vec::new();
    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        if space(c) {
            continue;
        }
        if c == b'#' || c == b';' {
            while bytes.get(i).is_some_and(|b| *b != b'\n') {
                i += 1;
            }
            continue;
        }
        if c == b'[' {
            let start = i;
            while bytes.get(i).is_some_and(|b| key_char(*b) || *b == b'.') {
                i += 1;
            }
            let name = bytes[start..i].to_ascii_lowercase();
            let (section, dotted) = match name.iter().position(|b| *b == b'.') {
                Some(dot) => (name[..dot].to_vec(), true),
                None => (name, false),
            };
            let mut subsection = dotted;
            match *bytes.get(i)? {
                b']' if !dotted || bytes[i - 1] != b'.' => i += 1,
                c if space(c) && c != b'\n' && !dotted => {
                    while bytes.get(i).is_some_and(|b| space(*b) && *b != b'\n') {
                        i += 1;
                    }
                    if bytes.get(i) != Some(&b'"') {
                        return None;
                    }
                    i += 1;
                    loop {
                        match *bytes.get(i)? {
                            b'\n' => return None,
                            b'"' => break,
                            b'\\' => i += 2,
                            _ => i += 1,
                        }
                    }
                    if bytes.get(i + 1) != Some(&b']') {
                        return None;
                    }
                    i += 2;
                    subsection = true;
                }
                _ => return None,
            }
            in_submodule = section == b"submodule" && subsection;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            return None;
        }
        let start = i - 1;
        while bytes.get(i).is_some_and(|b| key_char(*b)) {
            i += 1;
        }
        let key = bytes[start..i].to_ascii_lowercase();
        while bytes.get(i).is_some_and(|b| *b == b' ' || *b == b'\t') {
            i += 1;
        }
        let value = match *bytes.get(i)? {
            b'\n' => None,
            b'=' => {
                i += 1;
                let (value, end) = config_value(&bytes, i)?;
                i = end;
                Some(value)
            }
            _ => return None,
        };
        if in_submodule
            && key == b"path"
            && let Some(path) = value
            && !path.is_empty()
            && path[0] != b'-'
        {
            out.push(path);
        }
    }
    Some(out)
}

/// The config value that starts at `bytes[i]`, as git's `parse_value` reads it, and the offset
/// just past the line break that ends it; `None` for an unknown escape or an open quote.
fn config_value(bytes: &[u8], mut i: usize) -> Option<(Vec<u8>, usize)> {
    let mut value = Vec::new();
    let (mut quote, mut comment, mut spaces) = (false, false, 0usize);
    loop {
        let c = *bytes.get(i)?;
        i += 1;
        if c == b'\n' {
            return (!quote).then_some((value, i));
        }
        if comment {
            continue;
        }
        if matches!(c, b' ' | b'\t' | b'\r' | 0x0b | 0x0c) && !quote {
            if !value.is_empty() {
                spaces += 1;
            }
            continue;
        }
        if !quote && (c == b';' || c == b'#') {
            comment = true;
            continue;
        }
        value.extend(std::iter::repeat_n(b' ', spaces));
        spaces = 0;
        match c {
            b'\\' => {
                let escaped = *bytes.get(i)?;
                i += 1;
                match escaped {
                    b'\n' => {}
                    b't' => value.push(b'\t'),
                    b'b' => value.push(0x08),
                    b'n' => value.push(b'\n'),
                    b'\\' | b'"' => value.push(escaped),
                    _ => return None,
                }
            }
            b'"' => quote = !quote,
            _ => value.push(c),
        }
    }
}

/// The oldest git the adapter runs: Ubuntu 22.04's, whose plumbing every call here uses [A-21].
pub const MIN_VERSION: (u32, u32, u32) = (2, 34, 1);

/// The git on `PATH` is older than [`MIN_VERSION`], or no git answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooOld {
    /// The version git reported, such as `2.30.0`; `None` when no git answered `--version`.
    pub found: Option<String>,
}

impl TooOld {
    /// The floor, as `2.34.1`.
    pub fn minimum() -> String {
        let (major, minor, patch) = MIN_VERSION;
        format!("{major}.{minor}.{patch}")
    }
}

/// Check the `git` on `PATH` against [`MIN_VERSION`] with one `git --version` in a cleared
/// environment. A version that doesn't parse counts as too old, so a scan never runs on a git it
/// can't vouch for (DR-78).
pub fn check_version() -> Result<(), TooOld> {
    version().map(|_| ())
}

/// The version of the `git` on `PATH`, as `major.minor.patch`, when it is at least
/// [`MIN_VERSION`]; [`check_version`]'s answer otherwise.
pub fn version() -> Result<String, TooOld> {
    let mut cmd = Command::new("git");
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("LC_ALL", "C").arg("--version").stdin(Stdio::null()).stderr(Stdio::null());
    let out = cmd.output().map_err(|_| TooOld { found: None })?;
    if !out.status.success() {
        return Err(TooOld { found: None });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let reported = text.trim();
    let reported = reported.strip_prefix("git version ").unwrap_or(reported);
    match parse_version(reported) {
        Some(version @ (major, minor, patch)) if version >= MIN_VERSION => {
            Ok(format!("{major}.{minor}.{patch}"))
        }
        Some((major, minor, patch)) => Err(TooOld { found: Some(format!("{major}.{minor}.{patch}")) }),
        None => Err(TooOld { found: Some(reported.chars().take(40).collect()) }),
    }
}

/// `2.43.0`, `2.39.3 (Apple Git-146)` or `2.45.1.windows.1` as its first three numbers; a missing
/// patch number reads as 0.
fn parse_version(reported: &str) -> Option<(u32, u32, u32)> {
    let word = reported.split_whitespace().next()?;
    let mut parts = word.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = match parts.next() {
        None => 0,
        Some(p) => p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()?,
    };
    Some((major, minor, patch))
}

/// A repo's checked-out commit and branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub commit: String,
    /// `None` when `HEAD` is detached.
    pub branch: Option<String>,
}

/// One working tree's git.
#[derive(Debug, Clone)]
pub struct Git {
    dir: PathBuf,
}

/// The most segment files one `hash-object` names, so its argv stays bounded.
const HASH_BATCH: usize = 256;

/// Whether `name` is a backup segment's: `events-` and six or more digits, then `.ndjson`.
pub fn is_segment(name: &str) -> bool {
    name.strip_prefix("events-")
        .and_then(|rest| rest.strip_suffix(".ndjson"))
        .is_some_and(|n| n.len() >= 6 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `dir` is the top of a git working tree (a `.git` directory, or the `.git` file of a
/// submodule or linked worktree).
pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Every transport git has: each is refused by name on the command line, since a repo's own
/// `protocol.<name>.allow` beats `protocol.allow` (correctness-r3-3).
pub const TRANSPORTS: &[&str] = &["ext", "fd", "file", "git", "ssh", "http", "https", "ftp", "ftps"];

impl Git {
    /// The git of the working tree at `dir`, made absolute, since every call names its git
    /// directory and working tree from it.
    pub fn new(dir: &Path) -> Git {
        Git { dir: std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf()) }
    }

    /// The git command for `args` in this working tree, refused ([`Refusal`]) when another user
    /// owns a place git uses for it ([`set_euid`], [`Places`]). Every call goes through here, so
    /// none runs git there. It pins the git directory the engine resolved, so git never looks for
    /// a repo itself, above the working tree or anywhere else; a `.git` that names none fails
    /// here, as git fails on one it can't use, and so does a place git needs as a directory or a
    /// regular file that is neither, which git could block on.
    fn command(&self, args: &[&str]) -> Result<Command, GitError> {
        let git_dir = usable_git_dir(&self.dir, args)?;
        let mut cmd = Command::new("git");
        cmd.env_clear();
        for key in ["PATH", "HOME"] {
            if let Some(value) = std::env::var_os(key) {
                cmd.env(key, value);
            }
        }
        cmd.env("LC_ALL", "C").env("GIT_TERMINAL_PROMPT", "0").env("GIT_LFS_SKIP_SMUDGE", "1");
        // The git directory and working tree the owner check read, so git never discovers a repo
        // by looking further up, where a `.git` it can't use would send it, and a repo's own
        // `core.worktree` never moves the working tree elsewhere. With the git directory named,
        // git's own `safe.directory` check doesn't run; the engine's is the one that counts.
        cmd.env("GIT_DIR", &git_dir).env("GIT_WORK_TREE", &self.dir);
        // No graft file: a repo's `info/grafts` would rewrite the ancestry the history reads,
        // which `--no-replace-objects` leaves on, so the history would no longer be the commit's
        // own. An empty file holds no grafts.
        cmd.env("GIT_GRAFT_FILE", "/dev/null");
        // No lazy fetch from a promisor remote. git reads the variable from the 2024 security
        // releases on (2.39.4 and later maintenance lines, 2.45) and in distributions' backports
        // such as Ubuntu's 2.43; an older git ignores it. So no transport may run either:
        // `protocol.allow=never` alone isn't enough, since a repo's own `protocol.<name>.allow`
        // beats it, so each transport is refused by name too, and command-line config beats the
        // repo's for the same key. git then refuses before it runs a configured upload-pack, ssh
        // or ext:: command.
        cmd.env("GIT_NO_LAZY_FETCH", "1");
        #[cfg(test)]
        if tests::AS_OLD_GIT.with(std::cell::Cell::get) {
            cmd.env_remove("GIT_NO_LAZY_FETCH");
        }
        cmd.args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
        ]);
        for name in TRANSPORTS {
            cmd.arg("-c").arg(format!("protocol.{name}.allow=never"));
        }
        cmd.arg("--no-optional-locks");
        cmd.args(args).current_dir(&self.dir).stdin(Stdio::null());
        Ok(cmd)
    }

    fn run(&self, args: &[&str]) -> Result<String, GitError> {
        self.run_bytes(args).map(|out| String::from_utf8_lossy(&out).into_owned())
    }

    fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let fail = |stderr: String| GitError::new(args, stderr);
        let out = self.command(args)?.output().map_err(|e| fail(e.to_string()))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(fail(String::from_utf8_lossy(&out.stderr).into_owned()))
        }
    }

    /// Every entry of `commit`'s tree, recursively, as `git ls-tree -r -z` prints it:
    /// NUL-terminated `<mode> <type> <object>[ <size>]\t<path>` records, each path as raw bytes.
    /// With `sizes` (`-l`) git reads every blob's header, which a partial clone can't do for the
    /// blobs it doesn't hold, so it lists without them.
    pub fn ls_tree(&self, commit: &str, sizes: bool) -> Result<Vec<u8>, GitError> {
        let tree = format!("{commit}^{{tree}}");
        let mut args = vec!["ls-tree", "-r", "-z"];
        if sizes {
            args.push("-l");
        }
        args.extend(["--full-tree", tree.as_str()]);
        self.run_bytes(&args)
    }

    /// Commit the segments in `dir` (each `events-NNNNNN.ndjson`, in name order) as one parentless
    /// commit, and point `reference` at it with a compare-and-set against the value it held;
    /// returns the new commit's ID. Nothing else changes: no working tree, index or branch.
    ///
    /// `reuse` is `(commit, names)`: a segment in `names` keeps the blob the old tree lists for it,
    /// unread, when `reference` still names exactly `commit`, so an append hashes only the segments
    /// it changed. Every other segment is hashed. Each call goes through [`Git::command`], with
    /// signing off on the command line as well as by flag, since a repo's own config could name a
    /// `gpg.program`, and with Planroom's own identity, since the cleared environment has none and
    /// the commit is the engine's, never the person's.
    pub fn write_backup_ref(
        &self,
        reference: &str,
        dir: &Path,
        reuse: Option<(&str, &BTreeSet<String>)>,
    ) -> Result<String, GitError> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map_err(|e| GitError::new(&["write-backup-ref"], e.to_string()))?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| is_segment(name))
            .collect();
        names.sort_unstable();

        let verify = ["rev-parse", "--verify", "-q", reference];
        let out =
            self.backup_command(&verify)?.output().map_err(|e| GitError::new(&verify, e.to_string()))?;
        let old = match out.status.code() {
            Some(0) => Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()),
            Some(1) => None,
            _ => return Err(GitError::new(&verify, String::from_utf8_lossy(&out.stderr).into_owned())),
        };
        let mut blobs = std::collections::BTreeMap::new();
        if let (Some(old), Some((commit, kept))) = (&old, reuse)
            && old == commit
        {
            let listed = self.backup_run(&["ls-tree", old.as_str()], None)?;
            for line in listed.lines() {
                let Some((meta, name)) = line.split_once('\t') else { continue };
                if let Some(oid) = meta.strip_prefix("100644 blob ")
                    && kept.contains(name)
                {
                    blobs.insert(name.to_owned(), oid.to_owned());
                }
            }
        }

        let hashed: Vec<&String> = names.iter().filter(|n| !blobs.contains_key(n.as_str())).collect();
        for batch in hashed.chunks(HASH_BATCH) {
            let paths: Vec<String> =
                batch.iter().map(|n| dir.join(n).to_string_lossy().into_owned()).collect();
            let mut args = vec!["hash-object", "-w", "--no-filters", "--"];
            args.extend(paths.iter().map(String::as_str));
            let out = self.backup_run(&args, None)?;
            let oids: Vec<&str> = out.lines().collect();
            if oids.len() != batch.len() {
                return Err(GitError::new(
                    &args,
                    format!("{} objects for {} files", oids.len(), batch.len()),
                ));
            }
            for (name, oid) in batch.iter().zip(oids) {
                blobs.insert((*name).clone(), oid.to_owned());
            }
        }

        let entries: String =
            names.iter().map(|name| format!("100644 blob {}\t{name}\n", blobs[name])).collect();
        let tree = self.backup_run(&["mktree"], Some(entries.as_bytes()))?.trim().to_owned();
        let commit = self
            .backup_run(
                &["commit-tree", "--no-gpg-sign", tree.as_str(), "-m", "Planroom event backup"],
                None,
            )?
            .trim()
            .to_owned();
        let zero = "0".repeat(commit.len());
        let previous = old.as_deref().unwrap_or(&zero);
        self.backup_run(&["update-ref", reference, commit.as_str(), previous], None)?;
        Ok(commit)
    }

    /// [`Git::command`] for one of [`Git::write_backup_ref`]'s calls: signing off and Planroom's
    /// own identity, on this command alone.
    fn backup_command(&self, args: &[&str]) -> Result<Command, GitError> {
        let args: Vec<&str> = ["-c", "commit.gpgSign=false"].iter().chain(args).copied().collect();
        let mut cmd = self.command(&args)?;
        for (key, value) in [
            ("GIT_AUTHOR_NAME", "Planroom"),
            ("GIT_AUTHOR_EMAIL", "planroom@invalid"),
            ("GIT_COMMITTER_NAME", "Planroom"),
            ("GIT_COMMITTER_EMAIL", "planroom@invalid"),
        ] {
            cmd.env(key, value);
        }
        Ok(cmd)
    }

    /// Run [`Git::backup_command`] with `input` on its stdin; its stdout.
    fn backup_run(&self, args: &[&str], input: Option<&[u8]>) -> Result<String, GitError> {
        let fail = |stderr: String| GitError::new(args, stderr);
        let mut cmd = self.backup_command(args)?;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().map_err(|e| fail(e.to_string()))?;
        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin is piped");
            stdin.write_all(input).map_err(|e| fail(e.to_string()))?;
        }
        let out = child.wait_with_output().map_err(|e| fail(e.to_string()))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(fail(String::from_utf8_lossy(&out.stderr).into_owned()))
        }
    }

    /// This working tree's git directory, as `git rev-parse --absolute-git-dir` names it: `.git`
    /// itself, or the directory a submodule's or linked worktree's `.git` file points to.
    pub fn absolute_git_dir(&self) -> Result<PathBuf, GitError> {
        let out = self.run_bytes(&["rev-parse", "--absolute-git-dir"])?;
        let line = out.strip_suffix(b"\n").unwrap_or(&out);
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(line)))
    }

    /// The commit `HEAD` names and its branch.
    pub fn head(&self) -> Result<Head, GitError> {
        let commit = self.run(&["rev-parse", "--verify", "HEAD^{commit}"])?.trim().to_owned();
        let branch = self.run(&["symbolic-ref", "-q", "--short", "HEAD"]).ok().map(|b| b.trim().to_owned());
        Ok(Head { commit, branch: branch.filter(|b| !b.is_empty()) })
    }

    /// The branch `HEAD` names while it has no commit yet, as after `git init`; `None` when `HEAD`
    /// names a commit, or git can't read the repo at all.
    pub fn unborn_branch(&self) -> Option<String> {
        let branch = self.run(&["symbolic-ref", "-q", "HEAD"]).ok()?.trim().to_owned();
        if branch.is_empty() || self.run(&["rev-parse", "-q", "--verify", &branch]).is_ok() {
            return None;
        }
        Some(branch.strip_prefix("refs/heads/").unwrap_or(&branch).to_owned())
    }

    /// Whether this is a partial clone, whose promisor remote may hold objects that aren't here:
    /// `extensions.partialclone` names one, or a `remote.<name>.promisor` is true. git reads the
    /// latter as a bool, where a key with no value and any nonzero integer are true, so git itself
    /// normalises it.
    pub fn is_partial_clone(&self) -> Result<bool, GitError> {
        if !self.config_matching(r"^extensions\.partialclone$", false)?.is_empty() {
            return Ok(true);
        }
        let promisors = self.config_matching(r"^remote\..*\.promisor$", true)?;
        Ok(promisors.lines().any(|line| line.rsplit_once(' ').is_some_and(|(_, value)| value == "true")))
    }

    /// `git config --get-regexp`'s lines for `pattern`, with each value as a bool when `as_bool`;
    /// empty when no key matches.
    fn config_matching(&self, pattern: &str, as_bool: bool) -> Result<String, GitError> {
        let mut args = vec!["config"];
        if as_bool {
            args.push("--type=bool");
        }
        args.extend(["--get-regexp", pattern]);
        let fail = |stderr: String| GitError::new(&args, stderr);
        let out = self.command(&args)?.output().map_err(|e| fail(e.to_string()))?;
        match out.status.code() {
            Some(0) => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
            // `git config --get-regexp` exits 1 when no key matches; any other failure, a crash
            // included, is git failing.
            Some(1) => Ok(String::new()),
            _ => Err(fail(format!("{}{}", out.status, String::from_utf8_lossy(&out.stderr)))),
        }
    }

    /// The objects of `commit`'s tree that aren't in this clone, as `rev-list --missing=print`
    /// lists them, which never fetches.
    pub fn missing_objects(&self, commit: &str) -> Result<BTreeSet<String>, GitError> {
        let commit = format!("{commit}^{{commit}}");
        let out = self.run(&["rev-list", "--objects", "--missing=print", "--no-walk", &commit])?;
        Ok(out.lines().filter_map(|l| l.strip_prefix('?')).map(|o| o.trim().to_owned()).collect())
    }

    /// The paths of this repo's submodules, relative to this working tree, in index order: each
    /// gitlink of its index whose path its `.gitmodules` names ([`gitmodules_paths`]). Whether
    /// one is checked out is the caller's to see. Nothing runs git in a submodule: the owner
    /// check runs on this repo first, before its `.gitmodules` is read, which is read as a file,
    /// and the gitlinks come from one `git ls-files` here, which reads only this repo's index. A
    /// repo with no `.gitmodules`, or one naming no path, has none, and git doesn't run.
    pub fn submodules(&self) -> Result<Vec<PathBuf>, GitError> {
        let args = ["ls-files", "-s", "-z"];
        checked_places(&self.dir, &args)?;
        let file = self.dir.join(".gitmodules");
        if !is_regular(&file) {
            return Ok(Vec::new());
        }
        let text = read_regular_up_to(&file, MAX_GITMODULES_BYTES + 1).unwrap_or_default();
        if text.len() as u64 > MAX_GITMODULES_BYTES {
            let why = format!("{} is larger than {MAX_GITMODULES_BYTES} bytes", file.display());
            return Err(GitError::new(&args, why));
        }
        let named: BTreeSet<Vec<u8>> = gitmodules_paths(&text).unwrap_or_default().into_iter().collect();
        if named.is_empty() {
            return Ok(Vec::new());
        }
        let gitlinks = self.gitlinks()?;
        Ok(gitlinks.into_iter().filter(|p| named.contains(p.as_os_str().as_bytes())).collect())
    }

    /// The paths of the gitlinks in this repo's index, each once, in index order: `git ls-files
    /// -s -z` entries of mode 160000. It reads only this repo's index, never a submodule.
    fn gitlinks(&self) -> Result<Vec<PathBuf>, GitError> {
        let out = self.run_bytes(&["ls-files", "-s", "-z"])?;
        let mut paths: Vec<PathBuf> = Vec::new();
        // `<mode> <object> <stage>\t<path>`, each NUL-terminated.
        for record in out.split(|b| *b == 0) {
            let Some(tab) = record.iter().position(|b| *b == b'\t') else { continue };
            if !record.starts_with(b"160000 ") {
                continue;
            }
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(&record[tab + 1..]));
            if paths.last() != Some(&path) {
                paths.push(path);
            }
        }
        Ok(paths)
    }

    /// Whether git tracks any file at or below `path`, relative to this directory, in the repo that
    /// holds it (`git ls-files -z -- PATH`). A directory in no repo tracks nothing; any other
    /// failure is an error, which a caller reads as "can't tell".
    ///
    /// Every call pins its git directory, so git never looks above a directory for a repo itself:
    /// the repo that holds this one is found here, the nearest of it and the directories above with
    /// a `.git`, and its own owner check runs before git runs there.
    pub fn tracks_any(&self, path: &Path) -> Result<bool, GitError> {
        let Some(repo) = self.dir.ancestors().find(|d| std::fs::symlink_metadata(d.join(".git")).is_ok())
        else {
            return Ok(false);
        };
        let path = self.dir.strip_prefix(repo).unwrap_or(Path::new("")).join(path);
        let path = path.as_os_str().to_string_lossy();
        let args = ["ls-files", "-z", "--", path.as_ref()];
        match Git::new(repo).run_bytes(&args) {
            Ok(out) => Ok(!out.is_empty()),
            Err(e) if e.stderr.contains("not a git repository") => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// How many files this working tree holds that `HEAD` doesn't: changed, staged, deleted and
    /// untracked, as `git status --porcelain=v1 -z --untracked-files=normal` lists them, with
    /// `.gitignore` honoured (DR-71, DR-85). It never writes the index (`--no-optional-locks`).
    ///
    /// `status` may run a clean filter to compare a file whose stat data changed, so every filter
    /// the repo's config names is turned off on the command line, as command-line config beats
    /// the repo's own. A submodule's working tree isn't looked into (`--ignore-submodules=dirty`):
    /// that would run `status` there under the submodule's own config, whose filters this call
    /// can't see. A submodule the workspace maps is a repo of its own, and its status is its own;
    /// a gitlink that moved still counts here.
    ///
    /// To compare a gitlink, `status` reads the `HEAD` of the submodule checked out at its path,
    /// whatever `--ignore-submodules` says. So it runs only once each such submodule passes its own
    /// owner check, with no FIFO where that check looks ([`Places`]); otherwise this fails with
    /// that submodule's refusal, and git never reads another user's submodule or blocks on one.
    pub fn uncommitted_files(&self) -> Result<u32, GitError> {
        let args =
            ["status", "--porcelain=v1", "-z", "--untracked-files=normal", "--ignore-submodules=dirty"];
        let mut cmd = self.command(&args)?;
        for gitlink in self.gitlinks()? {
            let path = self.dir.join(gitlink);
            // git compares a gitlink with what's checked out only where a directory sits.
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                match usable_git_dir(&path, &args) {
                    Err(e) if e.refusal.is_some() => return Err(e),
                    // A submodule that isn't checked out names no git directory; git reads none.
                    _ => {}
                }
            }
        }
        // Through the environment rather than `-c`, which would split a driver name holding `=`.
        let keys: Vec<String> = self
            .filter_drivers()?
            .iter()
            .flat_map(|name| ["clean", "process"].map(|key| format!("filter.{name}.{key}")))
            .collect();
        cmd.env("GIT_CONFIG_COUNT", keys.len().to_string());
        for (i, key) in keys.iter().enumerate() {
            cmd.env(format!("GIT_CONFIG_KEY_{i}"), key).env(format!("GIT_CONFIG_VALUE_{i}"), "");
        }
        let fail = |stderr: String| GitError::new(&args, stderr);
        let out = cmd.output().map_err(|e| fail(e.to_string()))?;
        if !out.status.success() {
            return Err(fail(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        Ok(count_status(&out.stdout))
    }

    /// The names of the filter drivers git's config names a command for, in any scope.
    fn filter_drivers(&self) -> Result<BTreeSet<String>, GitError> {
        let args = ["config", "-z", "--name-only", "--get-regexp", r"^filter\..*\.(clean|process)$"];
        let fail = |stderr: String| GitError::new(&args, stderr);
        let out = self.command(&args)?.output().map_err(|e| fail(e.to_string()))?;
        match out.status.code() {
            Some(0) => {}
            // No key matches.
            Some(1) => return Ok(BTreeSet::new()),
            _ => return Err(fail(format!("{}{}", out.status, String::from_utf8_lossy(&out.stderr)))),
        }
        let names = out.stdout.split(|b| *b == 0).filter_map(|key| {
            let key = std::str::from_utf8(key).ok()?;
            let rest = key.strip_prefix("filter.")?;
            rest.strip_suffix(".clean").or_else(|| rest.strip_suffix(".process")).map(str::to_owned)
        });
        Ok(names.collect())
    }

    /// Every ref name.
    pub fn refs(&self) -> Result<BTreeSet<String>, GitError> {
        Ok(self.run(&["for-each-ref", "--format=%(refname)"])?.lines().map(str::to_owned).collect())
    }

    /// `commit` and its ancestors, newest first, at most `max` of them, each with its author and
    /// the paths it touched: one `git log --raw -z`, read as it streams, so memory stays bounded
    /// however much the history holds. Each commit keeps at most `keep` paths, and counts them
    /// all in `touched`.
    ///
    /// Nothing a repo configures runs or changes the answer: `--raw` compares trees, never file
    /// contents, so no diff driver, textconv or filter runs and a partial clone fetches no blob;
    /// renames aren't followed; signatures aren't checked, so no `gpg.program` runs; the root
    /// commit's paths always count; and stdout is a pipe, so no pager runs. Merges list no paths.
    /// Neither replace refs nor a graft file apply ([`Git::command`] names no graft file), so the
    /// history is the commit's own, as its snapshot key says. A shallow clone's boundary still
    /// stops it: [`Git::shallow_cut`] names each one, which the scan reports and keys the
    /// snapshot by.
    pub fn log(
        &self,
        commit: &str,
        max: usize,
        keep: usize,
        mut each: impl FnMut(LogCommit),
    ) -> Result<(), GitError> {
        let max = format!("--max-count={max}");
        let args = [
            "--no-replace-objects",
            "-c",
            "log.showRoot=true",
            "-c",
            "log.showSignature=false",
            "log",
            "-z",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "--no-relative",
            "--no-color",
            "--diff-merges=off",
            "--encoding=UTF-8",
            "--format=%x01%H%x00%ae%x00%an",
            &max,
            commit,
            "--",
        ];
        let fail = |stderr: String| GitError::new(&args, stderr);
        let mut child = self
            .command(&args)?
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| fail(e.to_string()))?;
        let stdout = child.stdout.take().expect("piped stdout");
        let read = read_log(BufReader::new(stdout), keep, &mut each);
        if read.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(|e| fail(e.to_string()))?;
        read.map_err(|e| fail(e.to_string()))?;
        if !status.success() {
            return Err(fail(format!("git log exited with {status}")));
        }
        Ok(())
    }

    /// The oldest of the newest `bound` commits of `commit`'s history, in [`Git::log`]'s order,
    /// when older ones exist: where a history read at that bound stops. `None` when it holds all.
    pub fn history_cut(&self, commit: &str, bound: usize) -> Result<Option<String>, GitError> {
        let Some(skip) = bound.checked_sub(1) else { return Ok(None) };
        let skip = format!("--skip={skip}");
        let out = self.run(&["--no-replace-objects", "rev-list", "--max-count=2", &skip, commit, "--"])?;
        let lines: Vec<&str> = out.lines().collect();
        Ok(match lines.as_slice() {
            [last_read, _older] => Some((*last_read).to_owned()),
            _ => None,
        })
    }

    /// The shallow boundaries among the newest `bound` commits of `commit`'s history, in
    /// [`Git::log`]'s order: each a commit this clone holds without its parents, where a history
    /// read stops. Empty in a repo that isn't shallow.
    ///
    /// git takes them from the repo's `shallow` file, which a history read can't turn off without
    /// failing on the parents a real shallow clone doesn't hold. So the read stops there, and the
    /// scan reports the cut instead. A boundary is a commit the read finds with no parents whose
    /// own object names one: git reads each such object with the shallow file turned off
    /// (`GIT_SHALLOW_FILE` empty), and `--no-walk` never looks for the parents themselves.
    pub fn shallow_cut(&self, commit: &str, bound: usize) -> Result<Vec<String>, GitError> {
        if self.run(&["rev-parse", "--is-shallow-repository"])?.trim() != "true" {
            return Ok(Vec::new());
        }
        let max = format!("--max-count={bound}");
        let listing = self.run(&["--no-replace-objects", "rev-list", "--parents", &max, commit, "--"])?;
        // The commits read with no parents, at most `bound`: a root commit, or a boundary.
        let parentless: Vec<&str> = listing.lines().map(str::trim).filter(|l| !l.contains(' ')).collect();
        if parentless.is_empty() {
            return Ok(Vec::new());
        }
        let mut args = vec!["--no-replace-objects", "rev-list", "--parents", "--no-walk"];
        args.extend(&parentless);
        args.push("--");
        let fail = |stderr: String| GitError::new(&args, stderr);
        let mut cmd = self.command(&args)?;
        cmd.env("GIT_SHALLOW_FILE", "");
        let out = cmd.output().map_err(|e| fail(e.to_string()))?;
        if !out.status.success() {
            return Err(fail(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let cut: BTreeSet<&str> =
            text.lines().filter_map(|l| l.split_once(' ').map(|(sha, _)| sha)).collect();
        Ok(parentless.into_iter().filter(|sha| cut.contains(sha)).map(str::to_owned).collect())
    }

    /// What moved between commits `old` and `new`, as `git diff -M --name-status -z` reports it
    /// (DR-74): each file rename, and each directory of `old` whose files all moved into one new
    /// directory, keeping their paths below it. Paths no ID can name, those that aren't UTF-8 or
    /// hold a newline or a backslash, are left out, as the walk leaves them out. External diff
    /// drivers and textconv never run, and the rename limit is fixed, so a repo's config changes
    /// neither what runs nor the answer.
    pub fn renames(&self, old: &str, new: &str) -> Result<Renames, GitError> {
        let out = self.run_bytes(&[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-relative",
            "--no-color",
            "--find-renames",
            "-l1000",
            "--name-status",
            "-z",
            old,
            new,
            "--",
        ])?;
        let files = parse_renames(&out);
        if files.is_empty() {
            return Ok(Renames::default());
        }
        let listing = self.ls_tree(old, false)?;
        let old_files: Vec<&str> = listing
            .split(|b| *b == 0)
            .filter_map(|record| {
                let tab = record.iter().position(|b| *b == b'\t')?;
                let head = std::str::from_utf8(&record[..tab]).ok()?;
                (head.split_whitespace().nth(1) == Some("blob")).then_some(&record[tab + 1..])
            })
            .filter_map(nameable)
            .collect();
        let dirs = moved_dirs(&old_files, &files);
        Ok(Renames { files, dirs })
    }

    /// A `cat-file --batch` reader over this repo's objects.
    pub fn objects(&self) -> Result<Objects, GitError> {
        self.batch(true)
    }

    /// A `cat-file --batch-check` reader: each object's size, never its contents.
    pub fn object_sizes(&self) -> Result<Objects, GitError> {
        self.batch(false)
    }

    fn batch(&self, contents: bool) -> Result<Objects, GitError> {
        let mode = if contents { "--batch" } else { "--batch-check" };
        let fail = |e: std::io::Error| GitError::new(&["cat-file", mode], e.to_string());
        let mut child = self
            .command(&["cat-file", mode])?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(fail)?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Ok(Objects { child, stdin: Some(stdin), stdout, contents })
    }
}

/// What moved between two commits of one repo, as paths from its root, each list sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Renames {
    /// Each renamed file: its old path and its new one.
    pub files: Vec<(String, String)>,
    /// Each directory whose files all moved into one new directory: its old path and the new one.
    pub dirs: Vec<(String, String)>,
}

/// A path as an ID can name it: UTF-8 with no newline and no backslash, as the walk takes it.
fn nameable(path: &[u8]) -> Option<&str> {
    std::str::from_utf8(path).ok().filter(|p| !p.is_empty() && !p.contains('\n') && !p.contains('\\'))
}

/// The renames in `git diff --name-status -z` output: NUL-terminated fields, a status and one
/// path, or for a rename (`R` and its score) or a copy (`C`), a status and two paths.
fn parse_renames(out: &[u8]) -> Vec<(String, String)> {
    let mut fields = out.split(|b| *b == 0);
    let mut renames = Vec::new();
    while let Some(status) = fields.next() {
        if status.is_empty() {
            continue;
        }
        let paths = if matches!(status.first(), Some(b'R' | b'C')) { 2 } else { 1 };
        let (Some(from), to) = (fields.next(), if paths == 2 { fields.next() } else { None }) else { break };
        if status.first() == Some(&b'R')
            && let (Some(from), Some(to)) = (nameable(from), to.and_then(nameable))
        {
            renames.push((from.to_owned(), to.to_owned()));
        }
    }
    renames.sort();
    renames.dedup();
    renames
}

/// Each directory of `old_files` whose every file `renames` moves into one other directory, at the
/// same path below it: `src/cart` when `src/cart/a.ts` and `src/cart/b/c.ts` became
/// `src/basket/a.ts` and `src/basket/b/c.ts`. The new directory is one `old_files` doesn't hold.
/// Its subdirectories are listed too, since their IDs move with it. Linear in the files times
/// their depth.
pub fn moved_dirs(old_files: &[&str], renames: &[(String, String)]) -> Vec<(String, String)> {
    use std::collections::BTreeMap;
    // Each old directory's file count, and whether it was a directory at all.
    let mut total: BTreeMap<&str, usize> = BTreeMap::new();
    for file in old_files {
        let mut dir = *file;
        while let Some((up, _)) = dir.rsplit_once('/') {
            *total.entry(up).or_default() += 1;
            dir = up;
        }
    }
    // For each old directory, the new directory each of its renamed files implies: the new
    // path less the same path below the old directory, or none when the tail differs.
    let mut implied: BTreeMap<&str, (usize, BTreeSet<Option<&str>>)> = BTreeMap::new();
    for (from, to) in renames {
        let mut dir = from.as_str();
        while let Some((up, _)) = dir.rsplit_once('/') {
            let below = &from[up.len()..];
            let target = to.strip_suffix(below).filter(|d| !d.is_empty());
            let entry = implied.entry(up).or_default();
            entry.0 += 1;
            entry.1.insert(target);
            dir = up;
        }
    }
    let mut moved = Vec::new();
    for (dir, (count, targets)) in implied {
        let mut targets = targets.into_iter();
        let (Some(Some(target)), None) = (targets.next(), targets.next()) else { continue };
        if total.get(dir) == Some(&count) && target != dir && !total.contains_key(target) {
            moved.push((dir.to_owned(), target.to_owned()));
        }
    }
    moved
}

/// The files `git status --porcelain=v1 -z` lists: one NUL-terminated `XY PATH` record each, and
/// for a rename or a copy, the path it came from in the record after it, which isn't a file more.
fn count_status(out: &[u8]) -> u32 {
    let mut records = out.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut count = 0;
    while let Some(record) = records.next() {
        count += 1;
        if record.iter().take(2).any(|b| matches!(b, b'R' | b'C')) {
            records.next();
        }
    }
    count
}

/// One commit [`Git::log`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogCommit {
    pub sha: String,
    /// The author's email, as the commit spells it; never stored as a fact.
    pub email: String,
    pub name: String,
    /// The paths it touched, raw, up to the reader's `keep`.
    pub files: Vec<Vec<u8>>,
    /// How many paths it touched, all of them.
    pub touched: usize,
}

/// The longest field [`read_log`] keeps: a longer one, which no path or author line needs, is cut
/// there and the rest skipped, so one field never takes unbounded memory.
const LOG_FIELD_CAP: usize = 4096;

/// The commits of `git log -z --raw --format=%x01%H%x00%ae%x00%an` output. Its NUL-separated
/// fields are a header (`\x01SHA`, then the email and the name), or a raw status (`:MODES SHAS
/// STATUS`), always followed by one path, since renames aren't detected. A field after a newline
/// git puts between a header and its paths starts with that newline. A path is only ever read
/// as the field after a status, so no path, whatever its bytes, reads as a header.
fn read_log(mut out: impl BufRead, keep: usize, each: &mut impl FnMut(LogCommit)) -> std::io::Result<()> {
    let mut field = Vec::new();
    let mut current: Option<LogCommit> = None;
    while next_field(&mut out, &mut field)? {
        let start = field.strip_prefix(b"\n").unwrap_or(&field);
        if let Some(sha) = start.strip_prefix(b"\x01") {
            if let Some(done) = current.take() {
                each(done);
            }
            let sha = String::from_utf8_lossy(sha).into_owned();
            let mut text = || -> std::io::Result<String> {
                next_field(&mut out, &mut field)?;
                Ok(String::from_utf8_lossy(&field).into_owned())
            };
            let email = text()?;
            let name = text()?;
            current = Some(LogCommit { sha, email, name, files: Vec::new(), touched: 0 });
        } else if start.starts_with(b":") {
            if !next_field(&mut out, &mut field)? {
                break;
            }
            if let Some(commit) = current.as_mut() {
                commit.touched += 1;
                if commit.files.len() < keep {
                    commit.files.push(field.clone());
                }
            }
        }
    }
    if let Some(done) = current.take() {
        each(done);
    }
    Ok(())
}

/// The next NUL-terminated field of `out` into `field`, cut at [`LOG_FIELD_CAP`] bytes; false at
/// the end of the output.
fn next_field(out: &mut impl BufRead, field: &mut Vec<u8>) -> std::io::Result<bool> {
    field.clear();
    let read = out.by_ref().take(LOG_FIELD_CAP as u64).read_until(0, field)?;
    if read == 0 {
        return Ok(false);
    }
    if field.last() == Some(&0) {
        field.pop();
        return Ok(true);
    }
    // Cut at the cap: skip the rest of the field.
    loop {
        let (done, used) = {
            let buf = out.fill_buf()?;
            match buf.iter().position(|b| *b == 0) {
                _ if buf.is_empty() => (true, 0),
                Some(i) => (true, i + 1),
                None => (false, buf.len()),
            }
        };
        out.consume(used);
        if done {
            return Ok(true);
        }
    }
}

/// Reads objects by name through one `git cat-file --batch` (or `--batch-check`) process.
pub struct Objects {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// `--batch`, whose replies carry the contents after the header.
    contents: bool,
}

impl Objects {
    /// The object `spec` names, such as `<commit>:<path>` or `<commit>^{commit}`; `None` when it
    /// doesn't exist. A spec holding a newline can't be asked and reads as missing.
    pub fn get(&mut self, spec: &str) -> Result<Option<Vec<u8>>, GitError> {
        assert!(self.contents, "contents come from a --batch reader");
        let Some(size) = self.header(spec)? else { return Ok(None) };
        let size = usize::try_from(size).map_err(|_| self.fail(std::io::Error::other("object too large")))?;
        let mut body = vec![0; size + 1];
        self.stdout.read_exact(&mut body).map_err(|e| self.fail(e))?;
        body.truncate(size);
        Ok(Some(body))
    }

    /// The size of the object `spec` names, from a `--batch-check` reader; `None` when it
    /// doesn't exist.
    pub fn size(&mut self, spec: &str) -> Result<Option<u64>, GitError> {
        assert!(!self.contents, "sizes alone come from a --batch-check reader");
        self.header(spec)
    }

    /// Ask for `spec` and read the reply's header: the object's size, or `None` when it's missing.
    fn header(&mut self, spec: &str) -> Result<Option<u64>, GitError> {
        if spec.contains('\n') {
            return Ok(None);
        }
        let stdin = self.stdin.as_mut().expect("open until drop");
        let sent = writeln!(stdin, "{spec}").and_then(|_| stdin.flush());
        sent.map_err(|e| self.fail(e))?;
        let mut header = String::new();
        self.stdout.read_line(&mut header).map_err(|e| self.fail(e))?;
        let fields: Vec<&str> = header.split_whitespace().collect();
        match fields.as_slice() {
            [_, "missing"] | [_, "ambiguous"] => Ok(None),
            [_, _kind, size] => {
                size.parse().map(Some).map_err(|_| self.fail(std::io::Error::other(header.clone())))
            }
            _ => Err(self.fail(std::io::Error::other(format!("unexpected cat-file reply: {header}")))),
        }
    }

    fn fail(&self, e: std::io::Error) -> GitError {
        let mode = if self.contents { "--batch" } else { "--batch-check" };
        GitError::new(&["cat-file", mode], e.to_string())
    }
}

impl Drop for Objects {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    //! The adapter runs nothing a scanned repo names on any git, including one that predates
    //! `GIT_NO_LAZY_FETCH` (unasked-r3-2, correctness-r3-3).

    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::Git;

    thread_local! {
        /// Run the adapter's git calls without `GIT_NO_LAZY_FETCH`, as a git older than the 2024
        /// security releases runs them: the seam the old-git tests turn on.
        pub(super) static AS_OLD_GIT: Cell<bool> = const { Cell::new(false) };
    }

    /// Plain git, as a person runs it, with a fixed author: the tests' own setup.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.test")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.test")
            .env_remove("GIT_NO_LAZY_FETCH")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    /// A blob-less clone holding no blob at all, of a repo with two files: the clone, its commit
    /// and one blob it lacks.
    fn partial_clone(dir: &Path) -> (PathBuf, String, String) {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("b")).unwrap();
        std::fs::write(src.join("a.txt"), "a\n").unwrap();
        std::fs::write(src.join("b/c.txt"), "c\n").unwrap();
        git(&src, &["init", "-q", "-b", "main"]);
        git(&src, &["add", "."]);
        git(&src, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        git(&src, &["config", "uploadpack.allowFilter", "true"]);
        let url = format!("file://{}", src.display());
        git(dir, &["clone", "-q", "--filter=blob:none", "--no-checkout", &url, "clone"]);
        let clone = dir.join("clone");
        let commit = git(&clone, &["rev-parse", "HEAD"]);
        let blob = git(&clone, &["rev-parse", "HEAD:a.txt"]);
        (clone, commit, blob)
    }

    /// Every adapter call a scan makes that could want a missing blob, as an old git runs them.
    fn scan_like_an_old_git(clone: &Path, commit: &str, blob: &str) {
        AS_OLD_GIT.with(|old| old.set(true));
        let adapter = Git::new(clone);
        // Each may fail; what matters is that none runs a transport.
        let _ = adapter.run(&["cat-file", "-p", blob]);
        let _ = adapter.ls_tree(commit, true);
        if let Ok(mut objects) = adapter.objects() {
            let _ = objects.get(blob);
        }
        if let Ok(mut sizes) = adapter.object_sizes() {
            let _ = sizes.size(blob);
        }
        let _ = crate::walk::walk(&adapter, commit, &crate::walk::Exclude::none());
        AS_OLD_GIT.with(|old| old.set(false));
    }

    /// The control: plain git with only `-c protocol.allow=never` and no `GIT_NO_LAZY_FETCH`, which
    /// runs a live plant.
    fn plain_old_git(clone: &Path, blob: &str) {
        let _ = Command::new("git")
            .args(["-c", "protocol.allow=never", "cat-file", "-p", blob])
            .current_dir(clone)
            .env_remove("GIT_NO_LAZY_FETCH")
            .output()
            .unwrap();
    }

    #[test]
    fn an_ext_remote_the_repo_allows_never_runs_on_an_old_git() {
        let dir = tempfile::tempdir().unwrap();
        let (clone, commit, blob) = partial_clone(dir.path());
        let mark = dir.path().join("PWNED");
        let url = format!("ext::sh -c touch% {}% >&2", mark.display());
        git(&clone, &["config", "remote.origin.url", &url]);
        git(&clone, &["config", "protocol.ext.allow", "always"]);

        scan_like_an_old_git(&clone, &commit, &blob);
        assert!(!mark.exists(), "the repo's ext:: command ran");
        plain_old_git(&clone, &blob);
        assert!(mark.exists(), "the control: a repo-local protocol.ext.allow beats protocol.allow=never");
    }

    #[test]
    fn a_file_remote_the_repo_allows_never_runs_its_upload_pack_on_an_old_git() {
        let dir = tempfile::tempdir().unwrap();
        let (clone, commit, blob) = partial_clone(dir.path());
        let mark = dir.path().join("ran");
        let planted = format!("sh -c 'echo ran >> {}'; git-upload-pack", mark.display());
        git(&clone, &["config", "remote.origin.uploadpack", &planted]);
        git(&clone, &["config", "protocol.file.allow", "always"]);

        scan_like_an_old_git(&clone, &commit, &blob);
        assert!(!mark.exists(), "the repo's upload-pack command ran");
        plain_old_git(&clone, &blob);
        assert!(mark.exists(), "the control: a repo-local protocol.file.allow beats protocol.allow=never");
    }

    #[test]
    fn the_command_line_refuses_each_transport_whatever_the_repo_config_allows() {
        // Command-line config beats the repo's own for the same key, transport by transport.
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main", "r"]);
        let repo = dir.path().join("r");
        let adapter = Git::new(&repo);
        for name in super::TRANSPORTS {
            let key = format!("protocol.{name}.allow");
            git(&repo, &["config", &key, "always"]);
            let got = adapter.run(&["config", "--get", &key]).unwrap();
            assert_eq!(got.trim(), "never", "{key}");
        }
        git(&repo, &["config", "protocol.allow", "always"]);
        assert_eq!(adapter.run(&["config", "--get", "protocol.allow"]).unwrap().trim(), "never");
    }

    #[test]
    fn uncommitted_files_count_changes_renames_and_untracked_files_but_not_ignored_ones() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        std::fs::create_dir_all(&repo).unwrap();
        for (path, text) in [("a.txt", "a\n"), ("b.txt", "b\n"), ("c.txt", "c\n"), (".gitignore", "*.log\n")]
        {
            std::fs::write(repo.join(path), text).unwrap();
        }
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        let adapter = Git::new(&repo);
        assert_eq!(adapter.uncommitted_files().unwrap(), 0);
        std::fs::write(repo.join("a.txt"), "changed\n").unwrap();
        git(&repo, &["mv", "b.txt", "renamed.txt"]);
        std::fs::remove_file(repo.join("c.txt")).unwrap();
        std::fs::write(repo.join("new file.txt"), "n\n").unwrap();
        std::fs::write(repo.join("debug.log"), "ignored\n").unwrap();
        // The change, the rename (one file, though it names two paths), the deletion and the
        // untracked file; never the ignored one.
        assert_eq!(adapter.uncommitted_files().unwrap(), 4);
    }

    #[test]
    fn counting_uncommitted_files_runs_no_filter_the_repo_or_a_submodule_names() {
        let dir = tempfile::tempdir().unwrap();
        let mark = dir.path().join("ran");
        let filter = format!("sh -c 'echo ran >> {}; cat'", mark.display());
        // A submodule whose own config names a filter for its every file.
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(".gitattributes"), "* filter=sub\n").unwrap();
        std::fs::write(sub.join("s.txt"), "s\n").unwrap();
        git(&sub, &["init", "-q", "-b", "main"]);
        git(&sub, &["add", "."]);
        git(&sub, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        // A repo whose config names one too, under a name that holds `=`, and the submodule.
        let top = dir.path().join("top");
        std::fs::create_dir_all(&top).unwrap();
        std::fs::write(top.join(".gitattributes"), "*.txt filter=x=y\n").unwrap();
        std::fs::write(top.join("t.txt"), "t\n").unwrap();
        git(&top, &["init", "-q", "-b", "main"]);
        git(&top, &["add", "."]);
        git(&top, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        let url = sub.display().to_string();
        git(&top, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "sub"]);
        git(&top, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Sub"]);
        git(&top, &["config", "filter.x=y.clean", &filter]);
        git(&top.join("sub"), &["config", "filter.sub.clean", &filter]);
        // Changed files, so status compares their contents.
        std::fs::write(top.join("t.txt"), "changed\n").unwrap();
        std::fs::write(top.join("sub/s.txt"), "changed\n").unwrap();

        assert_eq!(Git::new(&top).uncommitted_files().unwrap(), 1, "t.txt; the submodule counts its own");
        assert_eq!(Git::new(&top.join("sub")).uncommitted_files().unwrap(), 1);
        assert!(!mark.exists(), "a filter the repo's config names ran");
        // The control: plain git status runs both.
        git(&top, &["status", "--porcelain"]);
        assert!(mark.exists(), "the control: plain status runs the filters");
    }

    /// A repo whose `.git/config` ends with `extra`.
    fn repo_with_config(dir: &Path, name: &str, extra: &str) -> PathBuf {
        git(dir, &["init", "-q", "-b", "main", name]);
        let repo = dir.join(name);
        let config = repo.join(".git/config");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}{extra}")).unwrap();
        repo
    }

    #[test]
    fn a_promisor_key_reads_as_git_reads_a_bool() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            ("\tpromisor\n", true),
            ("\tpromisor = true\n", true),
            ("\tpromisor = 1\n", true),
            ("\tpromisor = 2\n", true),
            ("\tpromisor = -1\n", true),
            ("\tpromisor = On\n", true),
            ("\tpromisor = false\n", false),
            ("\tpromisor = 0\n", false),
            ("\tpromisor = no\n", false),
            ("", false),
        ];
        for (i, (value, partial)) in cases.into_iter().enumerate() {
            let extra = format!("[remote \"origin\"]\n\turl = /nowhere\n{value}");
            let repo = repo_with_config(dir.path(), &format!("r{i}"), &extra);
            assert_eq!(Git::new(&repo).is_partial_clone().unwrap(), partial, "{value:?}");
        }
        // extensions.partialclone marks one too. With no value, git itself fails on the repo (2.43
        // crashes), which is never read as "not a partial clone".
        let repo = repo_with_config(dir.path(), "e0", "[extensions]\n\tpartialclone = origin\n");
        assert!(Git::new(&repo).is_partial_clone().unwrap());
        let repo = repo_with_config(dir.path(), "e1", "[extensions]\n\tpartialclone\n");
        let got = Git::new(&repo).is_partial_clone();
        assert!(!matches!(got, Ok(false)), "{got:?}");
    }

    #[test]
    fn a_moved_directory_and_its_files_are_reported_and_a_split_one_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        std::fs::create_dir_all(repo.join("src/cart/sub")).unwrap();
        std::fs::create_dir_all(repo.join("src/order")).unwrap();
        std::fs::write(repo.join("src/cart/a.ts"), "export const a = 1;\n").unwrap();
        std::fs::write(repo.join("src/cart/sub/b.ts"), "export const b = 2;\n").unwrap();
        std::fs::write(repo.join("src/order/c.ts"), "export const c = 3;\n").unwrap();
        std::fs::write(repo.join("src/order/d.ts"), "export const d = 4;\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        let old = git(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["mv", "src/cart", "src/basket"]);
        git(&repo, &["mv", "src/order/c.ts", "src/c.ts"]);
        git(&repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Move"]);
        let new = git(&repo, &["rev-parse", "HEAD"]);
        let renames = Git::new(&repo).renames(&old, &new).unwrap();
        let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert_eq!(
            renames.files,
            [
                pair("src/cart/a.ts", "src/basket/a.ts"),
                pair("src/cart/sub/b.ts", "src/basket/sub/b.ts"),
                pair("src/order/c.ts", "src/c.ts")
            ]
        );
        // src/order kept d.ts, so it didn't move; src/cart and its subdirectory did.
        assert_eq!(renames.dirs, [pair("src/cart", "src/basket"), pair("src/cart/sub", "src/basket/sub")]);
        assert_eq!(Git::new(&repo).renames(&new, &new).unwrap(), super::Renames::default());
    }

    #[test]
    fn the_version_floor_reads_each_platforms_version_line() {
        use super::{MIN_VERSION, parse_version};
        assert_eq!(parse_version("2.43.0"), Some((2, 43, 0)));
        assert_eq!(parse_version("2.39.3 (Apple Git-146)"), Some((2, 39, 3)));
        assert_eq!(parse_version("2.45.1.windows.1"), Some((2, 45, 1)));
        assert_eq!(parse_version("2.34"), Some((2, 34, 0)));
        assert_eq!(parse_version("2.34.1-rc0"), Some((2, 34, 1)));
        assert_eq!(parse_version("unknown"), None);
        assert!(parse_version("2.34.1").unwrap() >= MIN_VERSION);
        assert!(parse_version("2.34.0").unwrap() < MIN_VERSION);
        assert!(parse_version("2.30.9").unwrap() < MIN_VERSION);
        assert!(parse_version("3.0.0").unwrap() >= MIN_VERSION);
        // The git that runs these tests is new enough.
        assert_eq!(super::check_version(), Ok(()));
        // And its version is the one `git --version` reports, as `major.minor.patch`.
        let reported = Command::new("git").arg("--version").output().unwrap().stdout;
        let reported = String::from_utf8(reported).unwrap();
        let (major, minor, patch) =
            parse_version(reported.trim().trim_start_matches("git version ")).unwrap();
        assert_eq!(super::version(), Ok(format!("{major}.{minor}.{patch}")));
    }

    #[test]
    fn a_segment_name_is_events_and_six_digits() {
        assert!(super::is_segment("events-000001.ndjson"));
        assert!(super::is_segment("events-1000000.ndjson"));
        for name in [
            "events.ndjson",
            "events-00001.ndjson",
            "events-00000a.ndjson",
            "committed",
            "events-000001.ndjson.tmp",
        ] {
            assert!(!super::is_segment(name), "{name}");
        }
    }

    #[test]
    fn a_path_that_looks_like_a_header_stays_a_path() {
        // A path field always follows a status field, so no path's bytes read as a header.
        let out = b"\x01aaaa\0a@x\0A\0\n:100644 100644 1 2 M\0\x01bbbb\0:000000 100644 0 3 A\0x/y.txt\0\x01cccc\0b@x\0B\0";
        let mut commits = Vec::new();
        super::read_log(&out[..], 10, &mut |c| commits.push(c)).unwrap();
        let got: Vec<(&str, &str, usize)> =
            commits.iter().map(|c| (c.sha.as_str(), c.email.as_str(), c.touched)).collect();
        assert_eq!(got, [("aaaa", "a@x", 2), ("cccc", "b@x", 0)]);
        assert_eq!(commits[0].files, [b"\x01bbbb".to_vec(), b"x/y.txt".to_vec()]);
        // A commit past `keep` paths counts them all and keeps the first.
        let mut kept = Vec::new();
        super::read_log(&out[..], 1, &mut |c| kept.push(c)).unwrap();
        assert_eq!((kept[0].touched, kept[0].files.len()), (2, 1));
    }

    #[test]
    fn reading_history_runs_no_signature_program_or_pager_the_repo_names() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("a.txt"), "a\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        // A commit that carries a signature, so a log that checks signatures runs gpg.program.
        let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
        let parent = git(&repo, &["rev-parse", "HEAD"]);
        let body = format!(
            "tree {tree}\nparent {parent}\nauthor T <t@example.test> 1700000000 +0000\ncommitter T <t@example.test> 1700000000 +0000\ngpgsig -----BEGIN PGP SIGNATURE-----\n \n x\n -----END PGP SIGNATURE-----\n\nSigned\n"
        );
        let path = dir.path().join("commit.txt");
        std::fs::write(&path, body).unwrap();
        let signed = git(&repo, &["hash-object", "-t", "commit", "-w", path.to_str().unwrap()]);
        git(&repo, &["update-ref", "refs/heads/main", &signed]);
        let mark = dir.path().join("ran");
        // git runs gpg.program as a program, not through the shell, so the plant is a script.
        let script = dir.path().join("plant.sh");
        std::fs::write(&script, format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", mark.display())).unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let plant = script.display().to_string();
        for (key, value) in [
            ("log.showSignature", "true"),
            ("gpg.program", plant.as_str()),
            ("core.pager", plant.as_str()),
            ("log.showRoot", "false"),
            ("diff.external", plant.as_str()),
            ("diff.renames", "copies"),
        ] {
            git(&repo, &["config", key, value]);
        }

        // A replace ref that swaps the first commit for one with no files changes nothing read.
        let empty = git(&repo, &["hash-object", "-t", "tree", "-w", "/dev/null"]);
        let other = git(&repo, &["commit-tree", &empty, "-m", "Other"]);
        git(&repo, &["replace", &parent, &other]);

        let mut commits = Vec::new();
        Git::new(&repo).log(&signed, 10, 10, |c| commits.push(c)).unwrap();
        assert!(!mark.exists(), "a program the repo's config names ran");
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[1].files, [b"a.txt".to_vec()], "the root commit's paths count");
        // The control: plain git log runs the repo's signature program.
        git(&repo, &["log", "-1"]);
        assert!(mark.exists(), "the control: log.showSignature runs gpg.program");
    }

    #[test]
    fn another_users_repo_is_named_before_git_runs() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main", "r"]);
        let repo = dir.path().join("r");
        let me = std::fs::metadata(&repo).unwrap().uid();
        assert_eq!(super::foreign_owner(&repo, me), None);
        assert_eq!(super::foreign_owner(&repo, me.wrapping_add(1)), Some((repo.clone(), me)));
        // A `.git` file names the git directory, which counts as well.
        let linked = dir.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: ../r/.git\n").unwrap();
        assert_eq!(super::foreign_owner(&linked, me), None);
        if me == 0 {
            // As root, a git directory another user owns, behind a working tree root owns.
            let out =
                Command::new("chown").args(["-R", "65534", repo.join(".git").to_str().unwrap()]).output();
            assert!(out.unwrap().status.success());
            assert_eq!(super::foreign_owner(&linked, me), Some((linked.join("../r/.git"), 65534)));
        }
    }

    /// A one-commit repo at `dir`, with its commit.
    fn committed(dir: &Path, file: &str) -> String {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file), "x\n").unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["add", "."]);
        git(dir, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "First"]);
        git(dir, &["rev-parse", "HEAD"])
    }

    #[test]
    fn the_owner_check_reads_exactly_the_places_git_uses() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        // Under a directory whose path holds a `:`, which counts no more than any other above.
        let base = dir.path().join("a:b");
        let main = base.join("main");
        let head = committed(&main, "m.txt");
        let me = std::fs::metadata(&main).unwrap().uid();
        let got = super::places(&main, None).unwrap();
        assert_eq!(got.git_dir, Ok(main.join(".git")));
        assert_eq!(got.used, [main.clone(), main.join(".git")], "nothing above the working tree");

        // A linked worktree: its `.git` file, the git directory it names and that one's
        // `commondir`, the main repo's git directory, where its config and objects are.
        git(&main, &["worktree", "add", "-q", "-b", "topic", "../linked"]);
        let linked = base.join("linked");
        let got = super::places(&linked, None).unwrap();
        let named = PathBuf::from(git(&linked, &["rev-parse", "--absolute-git-dir"]));
        assert_eq!(got.git_dir.as_deref().ok().map(|d| d.canonicalize().unwrap()), Some(named.clone()));
        assert_eq!(got.used.len(), 4, "{:?}", got.used);
        assert_eq!(got.used[..2], [linked.clone(), linked.join(".git")]);
        assert_eq!(got.used[2].canonicalize().unwrap(), named);
        assert_eq!(got.used[3].canonicalize().unwrap(), main.join(".git").canonicalize().unwrap());
        // Its objects are the common directory's.
        let objects: Vec<PathBuf> = got.objects.iter().map(|o| o.canonicalize().unwrap()).collect();
        assert_eq!(objects, [main.join(".git/objects").canonicalize().unwrap()]);
        assert_eq!(Git::new(&linked).head().unwrap().commit, head, "a linked worktree still reads");
        assert_eq!(Git::new(&linked).head().unwrap().branch.as_deref(), Some("topic"));

        // A submodule: its `.git` file names its git directory under the superproject's.
        let sub_src = dir.path().join("sub-src");
        let sub_head = committed(&sub_src, "s.txt");
        let url = sub_src.display().to_string();
        git(&main, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "sub"]);
        let sub = main.join("sub");
        let got = super::places(&sub, None).unwrap();
        assert_eq!(got.used.len(), 3, "{:?}", got.used);
        assert_eq!(
            got.used[2].canonicalize().unwrap(),
            main.join(".git/modules/sub").canonicalize().unwrap()
        );
        assert_eq!(Git::new(&sub).head().unwrap().commit, sub_head, "a submodule still reads");
        assert_eq!(Git::new(&main).submodules().unwrap(), [PathBuf::from("sub")]);
        assert_eq!(Git::new(&main).uncommitted_files().unwrap(), 2, ".gitmodules and the gitlink");

        // A `.git` file that names nothing gives no git directory, and git never runs.
        let blank = base.join("blank");
        std::fs::create_dir_all(&blank).unwrap();
        std::fs::write(blank.join(".git"), "not a gitdir line\n").unwrap();
        assert!(super::places(&blank, None).unwrap().git_dir.is_err());
        let err = Git::new(&blank).head().unwrap_err();
        assert!(!err.is_other_owner() && err.stderr.contains("names no git directory"), "{err:?}");

        if me == 0 {
            // As root: another user's directory above a repo never counts, and another user's
            // common directory behind a linked worktree does.
            let chown = |uid: &str, path: &Path| {
                let out = Command::new("chown").args(["-R", uid, path.to_str().unwrap()]).output();
                assert!(out.unwrap().status.success());
            };
            chown("65534", &base);
            chown("0", &main);
            chown("0", &linked);
            assert_eq!(super::foreign_owner(&main, me), None, "a:b is above the repo");
            assert_eq!(super::foreign_owner(&linked, me), None);
            let common = main.join(".git");
            for entry in ["objects", "refs", "config", "HEAD"] {
                chown("65534", &common.join(entry));
            }
            let out = Command::new("chown").args(["65534", common.to_str().unwrap()]).output();
            assert!(out.unwrap().status.success());
            let (place, uid) = super::foreign_owner(&linked, me).expect("the common directory counts");
            assert_eq!((place.canonicalize().unwrap(), uid), (common.canonicalize().unwrap(), 65534));
            let what = |dir: &Path, euid: u32| super::places(dir, Some(euid)).unwrap_err().what;
            assert_eq!(what(&linked, me), super::OwnedPlace::GitDir, "outside the working tree");
            assert_eq!(what(&main, 1234), super::OwnedPlace::Tree, "the working tree");
        }
    }

    /// `f` on a thread of its own, or a panic when it hasn't returned in 20 seconds.
    fn within_20s<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(20)).expect("still blocked after 20 s")
    }

    #[test]
    fn a_commondir_that_isnt_a_regular_file_is_never_opened() {
        // A FIFO with no writer blocks whoever opens it. The owner check compares the working
        // tree's owner before it reads anything there, so another user's FIFO never blocks a
        // scan; and a `commondir` that isn't a regular file names no git directory, so git, which
        // would block on it too, never runs.
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        committed(&repo, "a.txt");
        let me = std::fs::metadata(&repo).unwrap().uid();
        let fifo = repo.join(".git/commondir");
        let out = Command::new("mkfifo").arg(&fifo).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let r = repo.clone();
        let other = within_20s(move || super::foreign_owner(&r, me.wrapping_add(1)));
        assert_eq!(other, Some((repo.clone(), me)), "the working tree's owner refuses first");
        let r = repo.clone();
        assert_eq!(within_20s(move || super::foreign_owner(&r, me)), None);
        let r = repo.clone();
        let err = within_20s(move || Git::new(&r).head()).unwrap_err();
        assert!(!err.is_other_owner() && err.stderr.contains("commondir"), "{err:?}");
        // The refusal names it, so a fix can show it without opening it.
        assert_eq!(err.refusal, Some(super::Refusal::NotAFile(fifo.clone())));
        // A symlink to one is no different.
        std::fs::remove_file(&fifo).unwrap();
        let elsewhere = dir.path().join("fifo");
        assert!(Command::new("mkfifo").arg(&elsewhere).output().unwrap().status.success());
        std::os::unix::fs::symlink(&elsewhere, &fifo).unwrap();
        let r = repo.clone();
        assert!(within_20s(move || Git::new(&r).head()).is_err());
        // Nor is a `commondir` that names one, or a `.git` file that does.
        std::fs::remove_file(&fifo).unwrap();
        std::fs::write(&fifo, format!("{}\n", elsewhere.display())).unwrap();
        let r = repo.clone();
        let err = within_20s(move || Git::new(&r).head()).unwrap_err();
        assert_eq!(err.refusal, Some(super::Refusal::NotAFile(elsewhere.clone())), "{err:?}");
        let pointed = dir.path().join("pointed");
        std::fs::create_dir_all(&pointed).unwrap();
        std::fs::write(pointed.join(".git"), format!("gitdir: {}\n", elsewhere.display())).unwrap();
        let err = within_20s(move || Git::new(&pointed).head()).unwrap_err();
        assert_eq!(err.refusal, Some(super::Refusal::NotAFile(elsewhere.clone())), "{err:?}");
        // Nor is a `.git` file that's a FIFO, which names no git directory either.
        let linked = dir.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        assert!(Command::new("mkfifo").arg(linked.join(".git")).output().unwrap().status.success());
        let r = linked.clone();
        assert!(within_20s(move || Git::new(&r).head()).is_err());
    }

    #[test]
    fn a_fifo_at_a_file_git_opens_is_refused_before_git_runs() {
        // git opens these files at fixed paths in a git directory or a common directory; a FIFO
        // there would block it, so each is checked before git runs, and the refusal names it.
        let dir = tempfile::tempdir().unwrap();
        let refused = |repo: &Path, place: &Path| {
            // The file git had there, set aside and put back after.
            let kept = PathBuf::from(format!("{}.kept", place.display()));
            let had = std::fs::rename(place, &kept).is_ok();
            std::fs::create_dir_all(place.parent().unwrap()).unwrap();
            assert!(Command::new("mkfifo").arg(place).output().unwrap().status.success());
            let r = repo.to_path_buf();
            let err = within_20s(move || Git::new(&r).head()).unwrap_err();
            let Some(super::Refusal::NotAFile(named)) = &err.refusal else { panic!("{err:?}") };
            assert_eq!(named.canonicalize().unwrap(), place.canonicalize().unwrap(), "{err:?}");
            std::fs::remove_file(place).unwrap();
            if had {
                std::fs::rename(&kept, place).unwrap();
            }
        };
        for (i, file) in super::FIXED_FILES.iter().enumerate() {
            let repo = dir.path().join(format!("r{i}"));
            committed(&repo, "a.txt");
            refused(&repo, &repo.join(".git").join(file));
        }
        // A linked worktree: its own git directory, and the main repo's as its common directory.
        let main = dir.path().join("main");
        committed(&main, "m.txt");
        git(&main, &["worktree", "add", "-q", "-b", "topic", "../wt"]);
        let wt = dir.path().join("wt");
        refused(&wt, &main.join(".git/worktrees/wt/HEAD"));
        refused(&wt, &main.join(".git/config"));
        assert!(Git::new(&wt).head().is_ok(), "with the FIFOs gone, the worktree reads");
    }

    #[test]
    fn opening_a_fifo_to_read_a_named_path_never_blocks() {
        // A `.git` file, a `commondir` or an `info/alternates` may be swapped for a FIFO after
        // the regular-file check: the read still returns at once, with nothing.
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("fifo");
        assert!(Command::new("mkfifo").arg(&fifo).output().unwrap().status.success());
        assert_eq!(within_20s(move || super::read_regular(&fifo)), None);
    }

    #[test]
    fn alternates_lines_are_read_as_git_reads_them() {
        // One path per line: a `#` line is a comment, an empty one names nothing, a line that
        // starts with `"` is C-quoted unless its quoting is broken, and a relative path is
        // relative to the objects directory whose `info/alternates` names it.
        let objects = Path::new("/r/.git/objects");
        let text =
            b"# a comment\n\n/abs/objects\n../../other/objects\n\"/quoted\\tdir/o\\142j\"\n\"broken\nlast/\n";
        let got = super::alternate_lines(objects, text);
        let want: Vec<PathBuf> = [
            "/abs/objects",
            "/r/other/objects",
            "/quoted\tdir/obj",
            "/r/.git/objects/\"broken",
            "/r/.git/objects/last",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(got[..want.len()], want, "as git reads them, in order: {got:?}");
        assert!(!got.iter().any(|p| p.to_string_lossy().contains('#')), "a comment names nothing");
        // A quoted entry may span lines, as git reads it; each line read alone counts as well.
        let got = super::alternate_lines(objects, b"\"/a\nb\"\n/c\r\n");
        for want in ["/a\nb", "/c\r", "/c"] {
            assert!(got.contains(&PathBuf::from(want)), "{want:?} isn't in {got:?}");
        }
        // A `..` above the root names nothing, as in git.
        assert!(super::alternate_lines(Path::new("/o"), b"../../x\n").is_empty());
    }

    #[test]
    fn another_users_alternate_object_directory_is_named_before_git_runs() {
        // A repo the user owns whole can read its objects from another directory through
        // `objects/info/alternates`, as `git clone --shared` leaves it. When another user owns
        // one, the owner check names it, however the line spells it and however deep it nests,
        // and never reads anything there; a missing one is git's to report.
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let theirs = dir.path().join("theirs");
        committed(&theirs, "t.txt");
        let objects = theirs.join(".git/objects").canonicalize().unwrap();
        let mine = dir.path().join("mine");
        git(dir.path(), &["clone", "-q", "--shared", theirs.to_str().unwrap(), mine.to_str().unwrap()]);
        let alternates = mine.join(".git/objects/info/alternates");
        let me = std::fs::metadata(&mine).unwrap().uid();
        assert_eq!(super::foreign_owner(&mine, me), None);
        let got = super::places(&mine, Some(me)).unwrap();
        assert!(got.objects.iter().any(|o| o.canonicalize().unwrap() == objects), "{:?}", got.objects);
        assert_eq!(Git::new(&mine).head().unwrap().commit, git(&theirs, &["rev-parse", "HEAD"]));
        // A missing alternate is git's to report, not another user's.
        std::fs::write(&alternates, "/no/such/objects\n").unwrap();
        assert_eq!(super::foreign_owner(&mine, me), None);
        // Five deep is as far as git follows, and a cycle ends.
        let chain: Vec<PathBuf> = (0..7).map(|n| dir.path().join(format!("alt{n}/objects"))).collect();
        for (n, objects) in chain.iter().enumerate() {
            std::fs::create_dir_all(objects.join("info")).unwrap();
            let next = chain.get(n + 1).unwrap_or(&chain[0]);
            std::fs::write(objects.join("info/alternates"), format!("{}\n", next.display())).unwrap();
        }
        std::fs::write(&alternates, format!("{}\n", chain[0].display())).unwrap();
        let got = within_20s({
            let mine = mine.clone();
            move || super::places(&mine, Some(me)).unwrap().objects
        });
        assert!(got.contains(&chain[5]) && !got.contains(&chain[6]), "{got:?}");
        if me != 0 {
            return;
        }
        // As root: another user's object directory, named absolute, relative, C-quoted, through
        // a symlink, or nested behind the user's own, refuses the repo.
        let chown = |uid: &str, path: &Path| {
            let out = Command::new("chown").args(["-R", uid, path.to_str().unwrap()]).output();
            assert!(out.unwrap().status.success());
        };
        chown("65534", &theirs);
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&objects, &link).unwrap();
        let own = dir.path().join("own/objects");
        std::fs::create_dir_all(own.join("info")).unwrap();
        std::fs::write(own.join("info/alternates"), format!("{}\n", objects.display())).unwrap();
        let spelled = [
            format!("{}\n", objects.display()),
            "../../../theirs/.git/objects\n".to_owned(),
            format!("# first\n\"{}\"\n", objects.display()),
            format!("{}\n", link.display()),
            format!("{}\n", own.display()),
        ];
        for line in spelled {
            std::fs::write(&alternates, &line).unwrap();
            let (place, uid) = super::foreign_owner(&mine, me).unwrap_or_else(|| panic!("{line}"));
            assert_eq!((place.canonicalize().unwrap(), uid), (objects.clone(), 65534), "{line}");
            let what = super::places(&mine, Some(me)).unwrap_err().what;
            assert_eq!(what, super::OwnedPlace::Objects, "{line}");
        }
        // A FIFO in that store, where git would open it, never blocks the check.
        let fifo = objects.join("info/alternates");
        assert!(Command::new("mkfifo").arg(&fifo).output().unwrap().status.success());
        std::fs::write(&alternates, format!("{}\n", objects.display())).unwrap();
        let m = mine.clone();
        assert_eq!(within_20s(move || super::foreign_owner(&m, me)).map(|f| f.1), Some(65534));
        // The repo's own objects directory, a symlink to another user's, counts the same.
        std::fs::remove_file(&alternates).unwrap();
        let own_objects = mine.join(".git/objects");
        std::fs::rename(&own_objects, dir.path().join("kept-objects")).unwrap();
        std::os::unix::fs::symlink(&objects, &own_objects).unwrap();
        let m = mine.clone();
        assert_eq!(within_20s(move || super::foreign_owner(&m, me)).map(|f| f.1), Some(65534));
    }

    #[test]
    fn an_alternate_through_a_symlink_and_dotdot_is_read_where_git_reads_it() {
        // git from 2.39 on resolves an alternate as `realpath` does, following a symlink before
        // the `..` after it; an older git resolves the `..` by name. The owner check counts both,
        // so a line that names one directory by name and another through the link can't slip
        // another user's object store past it, whichever git runs.
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().canonicalize().unwrap();
        // By name, `lnk/../theirs` is `P/theirs`; through the link it is `P/deep/theirs`. Both
        // hold a store, so the running git reads one of them, whatever its version.
        let repos = [p.join("theirs"), p.join("deep/theirs")];
        for repo in &repos {
            committed(repo, "t.txt");
        }
        let stores = repos.clone().map(|repo| repo.join(".git/objects"));
        std::fs::create_dir_all(p.join("deep/sub")).unwrap();
        std::os::unix::fs::symlink(p.join("deep/sub"), p.join("lnk")).unwrap();
        let mine = p.join("ws/mine");
        committed(&mine, "m.txt");
        let alternates = mine.join(".git/objects/info/alternates");
        let me = std::fs::metadata(&mine).unwrap().uid();
        for line in [
            format!("{}\n", p.join("lnk/../theirs/.git/objects").display()),
            "../../../../lnk/../theirs/.git/objects\n".to_owned(),
        ] {
            std::fs::write(&alternates, &line).unwrap();
            let got = super::places(&mine, Some(me)).unwrap().objects;
            for store in &stores {
                assert!(got.contains(store), "{line}: {} isn't in {got:?}", store.display());
            }
            // The control: the running git reads exactly one of the two.
            let count =
                Command::new("git").args(["count-objects", "-v"]).current_dir(&mine).output().unwrap();
            let count = String::from_utf8_lossy(&count.stdout).into_owned();
            let read: Vec<&str> = count.lines().filter_map(|l| l.strip_prefix("alternate: ")).collect();
            let known = |one: &str| stores.iter().any(|store| store.to_str() == Some(one));
            assert!(matches!(read[..], [one] if known(one)), "the control: {count}");
            if me == 0 {
                for (repo, store) in repos.iter().zip(&stores) {
                    let out = Command::new("chown").args(["-R", "65534", repo.to_str().unwrap()]).output();
                    assert!(out.unwrap().status.success());
                    let (place, uid) = super::foreign_owner(&mine, me).unwrap_or_else(|| panic!("{line}"));
                    assert_eq!((place, uid), (store.clone(), 65534), "{line}");
                    let out = Command::new("chown").args(["-R", "0", repo.to_str().unwrap()]).output();
                    assert!(out.unwrap().status.success());
                }
            }
        }
    }

    #[test]
    fn a_long_alternates_file_is_read_in_linear_time() {
        // 40,000 distinct missing entries, a quarter of the bytes the check reads: git reads such
        // a file in under a second, and so must the owner check, on every call.
        let dir = tempfile::tempdir().unwrap();
        let mine = dir.path().join("mine");
        let head = committed(&mine, "m.txt");
        let text: String = (0..40_000).map(|n| format!("../missing/{n:06}/objects\n")).collect();
        std::fs::write(mine.join(".git/objects/info/alternates"), text).unwrap();
        let m = mine.clone();
        let started = std::time::Instant::now();
        let got = within_20s(move || Git::new(&m).head().unwrap().commit);
        assert_eq!(got, head);
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
        // The bytes bound counts every file the check reads, so nesting doesn't multiply it.
        let half = "#".repeat(600 * 1024);
        let nested = dir.path().join("nested/objects");
        std::fs::create_dir_all(nested.join("info")).unwrap();
        std::fs::write(nested.join("info/alternates"), format!("{half}\n")).unwrap();
        let top = format!("{}\n{half}\n", nested.display());
        std::fs::write(mine.join(".git/objects/info/alternates"), top).unwrap();
        let err = Git::new(&mine).head().unwrap_err();
        assert!(!err.is_other_owner() && err.stderr.contains("bytes"), "{err:?}");
    }

    #[test]
    fn a_fixed_file_that_is_dev_null_is_read_as_empty() {
        // git reads a file that resolves to /dev/null as empty, so the check takes it; any other
        // device, and a FIFO at a path named like it, is still refused.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        committed(&repo, "a.txt");
        let exclude = repo.join(".git/info/exclude");
        std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        let _ = std::fs::remove_file(&exclude);
        std::os::unix::fs::symlink("/dev/null", &exclude).unwrap();
        assert!(Git::new(&repo).head().is_ok(), "/dev/null reads as empty");
        std::fs::remove_file(&exclude).unwrap();
        std::os::unix::fs::symlink("/dev/../dev/./null", &exclude).unwrap();
        assert!(Git::new(&repo).head().is_ok(), "the same file, however it's spelled");
        for other in ["/dev/zero", "/dev/random"] {
            std::fs::remove_file(&exclude).unwrap();
            std::os::unix::fs::symlink(other, &exclude).unwrap();
            let err = Git::new(&repo).head().unwrap_err();
            assert!(matches!(err.refusal, Some(super::Refusal::NotAFile(_))), "{other}: {err:?}");
        }
        let lookalike = dir.path().join("dev/null");
        std::fs::create_dir_all(lookalike.parent().unwrap()).unwrap();
        assert!(Command::new("mkfifo").arg(&lookalike).output().unwrap().status.success());
        std::fs::remove_file(&exclude).unwrap();
        std::os::unix::fs::symlink(&lookalike, &exclude).unwrap();
        let r = repo.clone();
        let err = within_20s(move || Git::new(&r).head()).unwrap_err();
        assert!(matches!(err.refusal, Some(super::Refusal::NotAFile(_))), "{err:?}");
    }

    #[test]
    fn git_never_walks_up_from_a_repo_whose_git_directory_it_cant_use() {
        // A repo whose `.git` is an empty directory, inside another repo: git would look for a
        // repo further up and run in the outer one, which the owner check never looked at.
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("outer");
        std::fs::create_dir_all(&outer).unwrap();
        std::fs::write(outer.join("outer.txt"), "outer\n").unwrap();
        git(&outer, &["init", "-q", "-b", "main"]);
        git(&outer, &["add", "."]);
        git(&outer, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Outer"]);
        let inner = outer.join("ws/r");
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        std::fs::write(inner.join("a.txt"), "a\n").unwrap();
        let git_inner = Git::new(&inner);
        assert!(git_inner.head().is_err(), "git read the outer repo's HEAD");
        assert!(git_inner.absolute_git_dir().is_err(), "git found the outer repo");
        // A directory whose name holds a `:`, between the repo and the outer one, changes nothing.
        let colon = outer.join("a:b/r");
        std::fs::create_dir_all(colon.join(".git")).unwrap();
        assert!(Git::new(&colon).head().is_err(), "git read the outer repo's HEAD past a:b");
        // Nor does a bare repo at such a path, which git would take as readily as a `.git`.
        let bare = dir.path().join("x:y");
        git(dir.path(), &["clone", "-q", "--bare", outer.to_str().unwrap(), bare.to_str().unwrap()]);
        let under_bare = bare.join("ws/r");
        std::fs::create_dir_all(under_bare.join(".git")).unwrap();
        let err = Git::new(&under_bare).head().unwrap_err();
        assert!(!err.is_other_owner(), "{err:?}");
        assert!(Git::new(&under_bare).absolute_git_dir().is_err(), "git found the bare repo");
        let mut read = Vec::new();
        let _ = Git::new(&under_bare).log("HEAD", 10, 10, |c| read.push(c.sha));
        assert!(read.is_empty(), "git read the bare repo's history: {read:?}");
        // A repo of its own there still reads.
        let own = outer.join("a:b/own");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("o.txt"), "o\n").unwrap();
        git(&own, &["init", "-q", "-b", "main"]);
        git(&own, &["add", "."]);
        git(&own, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Own"]);
        assert_eq!(Git::new(&own).head().unwrap().commit, git(&own, &["rev-parse", "HEAD"]));
    }

    #[test]
    fn a_graft_file_never_changes_the_history_read() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        for n in 0..3 {
            std::fs::write(repo.join(format!("f{n}.txt")), "x\n").unwrap();
            git(&repo, &["add", "."]);
            git(&repo, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Next"]);
        }
        let head = git(&repo, &["rev-parse", "HEAD"]);
        let read = |repo: &Path| {
            let mut shas = Vec::new();
            Git::new(repo).log(&head, 10, 10, |c| shas.push(c.sha)).unwrap();
            (shas, Git::new(repo).history_cut(&head, 2).unwrap())
        };
        let before = read(&repo);
        assert_eq!(before.0.len(), 3);
        assert!(before.1.is_some());
        // A graft that gives HEAD no parents cuts its ancestry for a plain `git log`.
        std::fs::create_dir_all(repo.join(".git/info")).unwrap();
        std::fs::write(repo.join(".git/info/grafts"), format!("{head}\n")).unwrap();
        assert_eq!(git(&repo, &["rev-list", "--count", "HEAD"]), "1", "the control: the graft applies");
        assert_eq!(read(&repo), before, "the graft changed the history read");
    }

    #[test]
    fn rename_output_keeps_only_nameable_paths() {
        let out = b"M\0kept.txt\0R100\0a.txt\0b.txt\0R087\0x\\y\0z\0D\0gone\0R100\0two\nlines\0t\0";
        assert_eq!(super::parse_renames(out), [("a.txt".to_owned(), "b.txt".to_owned())]);
        // Files that moved apart, or into a directory the old tree already had, move no directory.
        let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert!(super::moved_dirs(&["d/a", "d/b"], &[pair("d/a", "e/a"), pair("d/b", "f/b")]).is_empty());
        assert!(super::moved_dirs(&["d/a", "e/x"], &[pair("d/a", "e/a")]).is_empty());
        assert_eq!(super::moved_dirs(&["d/a"], &[pair("d/a", "e/a")]), [pair("d", "e")]);
    }

    #[test]
    fn a_directory_tracks_what_the_repo_above_it_tracks() {
        // git never looks above a directory itself, so the repo that holds one is found for it: a
        // directory in no repo tracks nothing, and one inside a repo tracks what that repo does.
        let dir = tempfile::tempdir().unwrap();
        let alone = dir.path().join("alone");
        std::fs::create_dir_all(alone.join(".planroom")).unwrap();
        assert!(!Git::new(&alone).tracks_any(Path::new(".planroom")).unwrap());

        let repo = dir.path().join("repo");
        let inner = repo.join("work/shop");
        std::fs::create_dir_all(inner.join(".planroom")).unwrap();
        std::fs::write(inner.join(".planroom/config.toml"), "").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        assert!(!Git::new(&inner).tracks_any(Path::new(".planroom")).unwrap());
        git(&repo, &["add", "."]);
        assert!(Git::new(&inner).tracks_any(Path::new(".planroom")).unwrap());
    }
}
