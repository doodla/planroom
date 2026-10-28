//! `store.pointer` (DR-1): the state directory's note of where its store lives when the state
//! directory's filesystem can't hold it. It's read through the contract's one format check, and
//! followed only to `DATA_ROOT/stores/<workspace_id>/store.db`, through no symbolic link, so a
//! pointer a repo or a person planted can't send the engine anywhere else.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use planroom_contract::format::{self, FormatError, ReadError};
use planroom_contract::{STORE_POINTER_FILE, StorePointer};

use crate::StoreError;
use crate::fs_check::Environment;

/// The store file's name, in the state directory or under the data root.
pub(crate) const STORE_FILE: &str = "store.db";

/// How old a temporary pointer file must be before an open removes it: publishing one takes
/// milliseconds, so one this old is a killed process's, never one another process may still link.
pub(crate) const STALE_TMP: std::time::Duration = std::time::Duration::from_secs(60);

/// The longest workspace ID a path is built from.
const MAX_WORKSPACE_ID: usize = 64;

/// Whether `id` can name a directory under the data root: 1 to 64 ASCII letters, digits, `-`
/// and `_`, so it never holds a separator or `..`.
pub(crate) fn valid_workspace_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_WORKSPACE_ID
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The one place a pointer may name for `workspace_id`.
pub(crate) fn store_under(env: &Environment, workspace_id: &str) -> PathBuf {
    env.data_root.join("stores").join(workspace_id).join(STORE_FILE)
}

/// The pointer beside the store path `path` (`<state dir>/store.db`).
pub(crate) fn pointer_path(path: &Path) -> PathBuf {
    path.with_file_name(STORE_POINTER_FILE)
}

/// The pointer beside `path`, read and checked; `None` when there is none, which includes a
/// state directory whose place is a regular file, such as the `.git` file of a repo whose git
/// directory the engine can't name: nothing can be stored under a file.
pub(crate) fn read(path: &Path) -> Result<Option<StorePointer>, StoreError> {
    use std::io::ErrorKind;
    let file = pointer_path(path);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => return Ok(None),
        Err(e) => return Err(StoreError::Io(format!("{}: {e}", file.display()))),
    };
    match format::read_str::<StorePointer>(&text) {
        Ok(pointer) => Ok(Some(pointer)),
        Err(ReadError::Format(FormatError::Newer { found, supported })) => {
            Err(StoreError::NewerFile { file, found, supported })
        }
        Err(e) => Err(StoreError::Io(format!("{}: {e}", file.display()))),
    }
}

/// The store file `pointer` names, when it's the one place it may name under `env`'s data root:
/// `stores/<workspace_id>/store.db`, with no symbolic link on the way. Anything else is refused.
pub(crate) fn target(
    env: Option<&Environment>,
    path: &Path,
    pointer: &StorePointer,
) -> Result<PathBuf, StoreError> {
    let file = pointer_path(path);
    let refuse = |why: &str| {
        StoreError::Io(format!(
            "{} names {:?}, which Planroom won't open: {why}",
            file.display(),
            pointer.path
        ))
    };
    let Some(env) = env else {
        return Err(refuse("this process has no data root to find it under"));
    };
    if !valid_workspace_id(&pointer.workspace_id) {
        return Err(refuse("its workspace ID can't name a directory"));
    }
    let expected = store_under(env, &pointer.workspace_id);
    if Path::new(&pointer.path) != expected {
        return Err(refuse(&format!("a store's only place under the data root is {}", expected.display())));
    }
    // Every directory from the data root down, and the file, must be what it says: a symbolic
    // link there could lead outside the data root.
    let stores = env.data_root.join("stores");
    for step in [stores.clone(), stores.join(&pointer.workspace_id), expected.clone()] {
        if fs::symlink_metadata(&step).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(refuse(&format!("{} is a symbolic link", step.display())));
        }
    }
    Ok(expected)
}

/// Write `pointer` beside `path` unless a pointer is already there, through a private temporary
/// file that is synced and then linked into place, so a reader never sees half a pointer and two
/// processes creating the store at once agree on one. Returns the pointer that holds afterwards.
pub(crate) fn publish(path: &Path, pointer: &StorePointer) -> Result<StorePointer, StoreError> {
    let file = pointer_path(path);
    sweep_stale(path);
    let tmp = file.with_file_name(format!(".{STORE_POINTER_FILE}.{}.tmp", unique()));
    let fail = |e: std::io::Error| StoreError::Io(format!("{}: {e}", file.display()));
    let text = serde_json::to_string_pretty(pointer)?;
    let mut out = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(fail)?;
    out.write_all(text.as_bytes()).and_then(|()| out.sync_all()).map_err(fail)?;
    drop(out);
    let linked = fs::hard_link(&tmp, &file);
    let _ = fs::remove_file(&tmp);
    let published = match linked {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(e),
        // Network and WSL filesystems may not take a hard link: write the pointer in place,
        // still only when none is there.
        Err(_) => OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&file)
            .and_then(|mut out| out.write_all(text.as_bytes()).and_then(|()| out.sync_all())),
    };
    match published {
        Ok(()) => {
            sync_dir(file.parent());
            Ok(pointer.clone())
        }
        // Another process published first: its pointer is the one.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            read(path)?.ok_or_else(|| StoreError::Io(format!("{}: vanished", file.display())))
        }
        Err(e) => Err(fail(e)),
    }
}

/// Remove the temporary pointer files beside `path` that a killed process left, those older
/// than [`STALE_TMP`].
pub(crate) fn sweep_stale(path: &Path) {
    if let Some(dir) = path.parent() {
        crate::migrate::sweep(dir, &format!(".{STORE_POINTER_FILE}."), ".tmp", Some(STALE_TMP));
    }
}

/// A temporary file's name part no other writer uses at the same time: the process, and a count
/// within it, since a server's threads share one process ID.
pub(crate) fn unique() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Sync a directory, so a rename or link in it survives a power loss; best effort.
pub(crate) fn sync_dir(dir: Option<&Path>) {
    if let Some(dir) = dir
        && let Ok(handle) = fs::File::open(dir)
    {
        let _ = handle.sync_all();
    }
}
