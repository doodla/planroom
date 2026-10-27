//! The filesystem check at store open (architecture.md, Where the workspace lives; DR-1).
//!
//! Several processes share one store through SQLite's write-ahead log, which needs a local
//! filesystem: its shared-memory index and its locks don't hold across a network mount, WSL's
//! drvfs or 9p `/mnt/<drive>`, FUSE, or a folder a sync client rewrites behind SQLite's back.
//!
//! This crate never reads a filesystem's type itself: the composition root registers the data
//! root and a function that names the type of the filesystem a directory is on, once at start-up
//! ([`register`]), and the check classifies on that name and the path. With nothing registered
//! the store opens where it's asked, as a store test expects.

use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Names the type of the filesystem the existing directory `dir` is on, such as `ext4`, `nfs`
/// or `apfs`; `None` when it can't tell.
pub type FsType = fn(&Path) -> Option<String>;

/// What the composition root registers once at start-up.
#[derive(Debug, Clone)]
pub struct Environment {
    /// The data root, absolute: a store moves to `stores/<workspace_id>/` under it, and a
    /// migration's copy lands in `backups/<workspace_id>/` (DR-53).
    pub data_root: PathBuf,
    /// The filesystem-type function, the one place that reads a type.
    pub fs_type: FsType,
    /// This engine's version, which a migration records in `meta.engine_version`, so an older
    /// engine names the version that wrote the store (DR-69).
    pub engine_version: String,
}

static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();

/// Register the process's environment. Only the first call counts; it returns whether this one
/// did.
pub fn register(environment: Environment) -> bool {
    ENVIRONMENT.set(environment).is_ok()
}

/// The registered environment, if any.
pub(crate) fn environment() -> Option<&'static Environment> {
    ENVIRONMENT.get()
}

/// Filesystem type names that can't share a write-ahead log between processes: network
/// filesystems, WSL's Windows drives and FUSE. A name starting with `fuse` is FUSE too.
const UNSAFE_TYPES: &[&str] = &[
    "nfs",
    "nfs4",
    "smb",
    "smb2",
    "smb3",
    "smbfs",
    "cifs",
    "afpfs",
    "webdav",
    "davfs",
    "9p",
    "v9fs",
    "drvfs",
    "afs",
    "ceph",
    "coda",
    "ncpfs",
    "lustre",
    "gpfs",
    "glusterfs",
    "sshfs",
    "macfuse",
    "osxfuse",
];

/// A path component a sync client owns, on any system: everything under it is synced.
fn is_sync_folder(name: &str) -> bool {
    matches!(name, "Dropbox" | "OneDrive" | "Google Drive" | "iCloud Drive" | "Box Sync" | "pCloud Drive")
        || name.starts_with("Dropbox (")
        || name.starts_with("OneDrive - ")
}

/// How the place at `path`, on a filesystem of type `fs`, can't hold a shared store: the label
/// the `unsafe_filesystem` warning names, or `None` when it's fine. An unknown type is fine,
/// since most local filesystems aren't listed.
pub fn classify(fs: &str, path: &Path) -> Option<String> {
    let lower = fs.to_ascii_lowercase();
    if UNSAFE_TYPES.contains(&lower.as_str()) || lower.starts_with("fuse") {
        return Some(fs.to_owned());
    }
    let names: Vec<&str> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    // WSL mounts each Windows drive at `/mnt/<letter>`, through drvfs or 9p.
    let wsl_drive = path.is_absolute()
        && names.len() >= 2
        && names[0] == "mnt"
        && names[1].len() == 1
        && names[1].bytes().all(|b| b.is_ascii_alphabetic());
    if wsl_drive {
        return Some(format!("{fs} (a WSL Windows drive)"));
    }
    // macOS keeps iCloud Drive under ~/Library/Mobile Documents and File Provider sync clients
    // (Dropbox, OneDrive, Google Drive) under ~/Library/CloudStorage.
    let synced = names.iter().any(|n| is_sync_folder(n))
        || names.windows(2).any(|w| w[0] == "Library" && matches!(w[1], "Mobile Documents" | "CloudStorage"));
    if synced {
        return Some(format!("{fs} (a sync folder)"));
    }
    None
}

/// The type name a place gets when the type function can't tell: no type rule matches it, so
/// only the path rules (a WSL drive, a sync folder) can.
const UNKNOWN_TYPE: &str = "unknown";

/// The label for the directory `dir` under `env`, or `None` when it's fine.
///
/// A directory that doesn't exist yet, such as a data root no command has created, is checked
/// where it will be: the type is read on its nearest existing ancestor, the filesystem it will
/// be created on, and the path rules apply to the whole path. A type the function can't tell
/// still leaves the path rules.
pub(crate) fn check_dir(env: &Environment, dir: &Path) -> Option<String> {
    let existing = dir.ancestors().find(|a| !a.as_os_str().is_empty() && a.exists()).unwrap_or(dir);
    let rest = dir.strip_prefix(existing).unwrap_or(Path::new(""));
    let existing = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    let fs = (env.fs_type)(&existing).unwrap_or_else(|| UNKNOWN_TYPE.to_owned());
    classify(&fs, &existing.join(rest))
}

/// Where the store went, for the `unsafe_filesystem` warning's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A new store on an unsafe place moved under the data root, with a pointer left behind.
    Moved,
    /// An existing store on an unsafe place stayed where it was.
    Kept,
    /// The data root is on an unsafe place too, so a new store stayed in the state directory.
    DataRoot,
}

/// What a store open or a migration has to tell the person, which the composition root turns
/// into a warning on the command's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// `unsafe_filesystem`: `path` is on `fs`, and the store is at `store`.
    UnsafeFilesystem { placement: Placement, path: PathBuf, fs: String, store: PathBuf },
    /// `store_migrated`: the store moved from `from` to `to`, and its copy is at `backup`.
    Migrated { from: u32, to: u32, backup: PathBuf },
}

/// The most notices kept until the composition root takes them: a long-running server opens the
/// store again and again, and only distinct notices are kept, so this bound is never reached in
/// practice.
pub const MAX_NOTICES: usize = 16;

static NOTICES: Mutex<Vec<Notice>> = Mutex::new(Vec::new());

/// Keep `notice` for [`take_notices`], once. A store gets one `unsafe_filesystem` notice, the
/// first: the open that keeps a new store in its state directory because the data root is unsafe
/// too says so, and the store's own check of its place adds nothing to that line.
pub(crate) fn notify(notice: Notice) {
    let mut kept = NOTICES.lock().unwrap_or_else(|e| e.into_inner());
    let said = kept.iter().any(|k| match (k, &notice) {
        (Notice::UnsafeFilesystem { store: a, .. }, Notice::UnsafeFilesystem { store: b, .. }) => a == b,
        (k, n) => k == n,
    });
    if !said && kept.len() < MAX_NOTICES {
        kept.push(notice);
    }
}

/// The notices since the last call, in order, emptying the list.
pub fn take_notices() -> Vec<Notice> {
    std::mem::take(&mut *NOTICES.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_not_created_yet_is_checked_where_it_will_be() {
        fn untold(_: &Path) -> Option<String> {
            None
        }
        let env = Environment { data_root: PathBuf::from("/"), fs_type: untold, engine_version: "0".into() };
        // statfs fails on a missing path; the path rules still hold.
        assert_eq!(
            check_dir(&env, Path::new("/mnt/z/no-such-dir/planroom")).as_deref(),
            Some("unknown (a WSL Windows drive)")
        );
        assert_eq!(
            check_dir(&env, Path::new("/home/no-such-user/Dropbox/planroom")).as_deref(),
            Some("unknown (a sync folder)")
        );
        assert_eq!(check_dir(&env, Path::new("/no-such-dir/planroom")), None);
    }
}
