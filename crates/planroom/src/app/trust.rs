//! Workspace trust (architecture.md, Where the workspace lives: Trust; DR-48): a workspace's own
//! `config.toml` loads only from a state directory the data root records as trusted.
//!
//! - A record matches only the directory it was written for: the workspace's canonical root, the
//!   state directory's path, its inode number and, where both the record and the filesystem have
//!   one, its birth time. A freed inode number can be reused at once, so `rm -rf .planroom && tar
//!   x` may get the old number but never the old birth time; where no birth time exists, a
//!   remount that renumbers inodes asks again.
//! - The engine records a state directory it creates (`by: engine`). Such a record also needs git
//!   to track nothing in a `.planroom/`; a directory in no repo tracks nothing, and any other git
//!   failure counts as untrusted.
//! - A person's accept of the trust card records it too (`by: person`), and waives the git check,
//!   since accepting is what trusts a `.planroom/` a repo commits (DR-89).
//! - With no data root to read, nothing is trusted.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use planroom_extract::git::Git;

use crate::app::data::{self, Trusted, TrustedBy};
use crate::app::error::AppError;
use crate::app::workspace::{self, Workspace};
use crate::clock;

/// Whether a workspace's state directory is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// No state directory yet: nothing to load and nothing to trust.
    Absent,
    Trusted,
    Untrusted,
}

/// How [`Trust`] stands for `ws`.
pub fn check(ws: &Workspace) -> Trust {
    at(&ws.root)
}

/// How [`Trust`] stands for the workspace rooted at `root`.
pub fn at(root: &Path) -> Trust {
    let id = match identity(root) {
        Found::Absent => return Trust::Absent,
        Found::Unreadable => return Trust::Untrusted,
        Found::Dir(id) => id,
    };
    let Ok(data) = data::root() else { return Trust::Untrusted };
    let Ok(records) = data::trusted(&data, &id.root) else { return Trust::Untrusted };
    let Some(record) = records.into_iter().find(|r| id.matches(r)) else { return Trust::Untrusted };
    if record.by == TrustedBy::Person || !id.dot_planroom {
        return Trust::Trusted;
    }
    match Git::new(&id.root_path).tracks_any(Path::new(workspace::DIR)) {
        Ok(false) => Trust::Trusted,
        Ok(true) | Err(_) => Trust::Untrusted,
    }
}

/// Record the state directory of the workspace at `root` as trusted `by` the engine or a person,
/// with its current identity. Nothing is recorded when it doesn't exist.
pub fn record(root: &Path, by: TrustedBy) -> Result<(), AppError> {
    let Found::Dir(id) = identity(root) else { return Ok(()) };
    let data = data::root()?;
    data::trust(
        &data,
        Trusted {
            root: id.root,
            state_dir: id.state_dir,
            ino: id.ino,
            created: id.created,
            by,
            at: clock::now().0,
            extra: Default::default(),
        },
    )
}

/// Remove every trust record of the workspace at `root`.
pub fn revoke(root: &Path) -> Result<(), AppError> {
    let data = data::root()?;
    data::untrust(&data, &canonical(root).display().to_string())
}

/// `root` as the records name it: canonical, or as given when it can't be resolved.
pub fn canonical(root: &Path) -> PathBuf {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

/// The state directory's name as a person reads it: `.planroom/`, or `.git/planroom/` for a
/// workspace whose root is a repo.
pub fn dir_name(root: &Path) -> &'static str {
    if workspace::state_dir(root) == root.join(workspace::DIR) { ".planroom/" } else { ".git/planroom/" }
}

/// A state directory as a trust record names it.
struct Identity {
    root: String,
    root_path: PathBuf,
    state_dir: String,
    ino: u64,
    created: Option<u64>,
    /// The `.planroom/` form, which a repo could commit.
    dot_planroom: bool,
}

impl Identity {
    fn matches(&self, record: &Trusted) -> bool {
        record.state_dir == self.state_dir
            && record.ino == self.ino
            && match (record.created, self.created) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

enum Found {
    Absent,
    /// Not a directory the engine could have made, such as a symbolic link, or not readable.
    Unreadable,
    Dir(Identity),
}

fn identity(root: &Path) -> Found {
    let Ok(root) = fs::canonicalize(root) else { return Found::Unreadable };
    let state = workspace::state_dir(&root);
    let (Some(parent), Some(name)) = (state.parent(), state.file_name()) else { return Found::Unreadable };
    let parent = match fs::canonicalize(parent) {
        Ok(parent) => parent,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Found::Absent,
        Err(_) => return Found::Unreadable,
    };
    let state = parent.join(name);
    let meta = match fs::symlink_metadata(&state) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Found::Absent,
        Err(_) => return Found::Unreadable,
    };
    // The engine makes a directory, never a link to one.
    if !meta.file_type().is_dir() {
        return Found::Unreadable;
    }
    let created = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_nanos()).ok());
    Found::Dir(Identity {
        root: root.display().to_string(),
        dot_planroom: state == root.join(workspace::DIR),
        state_dir: state.display().to_string(),
        root_path: root,
        ino: meta.ino(),
        created,
    })
}
