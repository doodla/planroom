//! The SQLite store: tables, commit, tail, leases, presence and migrations.
//!
//! Every engine process writes through [`Store::commit`] or [`Store::commit_scan`], each one
//! `BEGIN IMMEDIATE` transaction in WAL mode with `synchronous = FULL` and a 5 s busy timeout
//! (DR-1). The store holds no business rules: the keys a scan derives come from functions the
//! composition root passes in.

mod cards;
mod commit;
mod error;
pub mod fs_check;
mod layouts;
pub mod migrate;
mod pointer;
mod presence;
mod read;
mod tail;

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use planroom_contract::{Seq, StorePointer};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

pub use commit::{
    Committed, Dealt, META_REPOS, NewEvent, Planned, RepoOutcome, RepoScan, ScanCommitted, ScanKey,
    ScanRules, ScanWrite, Write,
};
pub use error::StoreError;
pub use fs_check::{Environment, Notice, Placement, register, take_notices};
pub use read::{ActiveSet, FactRow, Shape, Snapshot};
pub use tail::Tail;

use crate::fs_check::{check_dir, notify};
use crate::migrate::{MIGRATIONS, Migration, narrow, private_dir};

/// How long a connection waits for the write lock before `store_busy`. SQLite's busy handler
/// backs off to 100 ms sleeps, so under steady contention one commit can wait ~630 ms; WS4
/// measured that against a 1 ms fixed step and kept SQLite's (docs/contributing/research/ws4-concurrency.md).
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The store version this build writes: the last migration's number.
pub const STORE_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].0;

/// What a new store records in `meta`.
#[derive(Debug, Clone)]
pub struct Init {
    pub workspace_id: String,
    pub engine_version: String,
    pub created_at: String,
}

/// One read-write connection to a workspace's store.
pub struct Store {
    conn: Connection,
    path: PathBuf,
    hook: Option<Sender<Seq>>,
}

/// Where the store asked for at `path` (`<state dir>/store.db`) lives: the file itself, or the
/// one `store.pointer` beside it names; `None` when there is no store file there yet.
pub fn locate(path: &Path) -> Result<Option<PathBuf>, StoreError> {
    if path.is_file() {
        return Ok(Some(path.to_path_buf()));
    }
    let Some(pointer) = pointer::read(path)? else { return Ok(None) };
    let target = pointer::target(fs_check::environment(), path, &pointer)?;
    Ok(target.is_file().then_some(target))
}

impl Store {
    /// Open the store at `path`, `<state dir>/store.db`, creating it (mode 0600, in a 0700
    /// directory) and migrating it as needed.
    ///
    /// With an environment registered ([`register`]), a new store whose state directory is on a
    /// filesystem that can't share a write-ahead log is created at
    /// `DATA_ROOT/stores/<workspace_id>/store.db` instead, with a `store.pointer` left in the
    /// state directory, unless the data root is on one too; an existing store stays where it
    /// is. Each case leaves an `unsafe_filesystem` notice ([`take_notices`]).
    pub fn open(path: &Path, init: &Init) -> Result<Store, StoreError> {
        Store::open_with(path, init, MIGRATIONS)
    }

    /// [`Store::open`] with `migrations` in place of this build's: how a test plants a failing
    /// migration. Debug builds only, so no release build can reach it (DR-92).
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn open_with_migrations(
        path: &Path,
        init: &Init,
        migrations: &'static [Migration],
    ) -> Result<Store, StoreError> {
        Store::open_with(path, init, migrations)
    }

    fn open_with(path: &Path, init: &Init, migrations: &[Migration]) -> Result<Store, StoreError> {
        if !path.is_file() {
            if let Some(pointer) = pointer::read(path)? {
                pointer::sweep_stale(path);
                return Store::create_at_pointer(path, &pointer, init, migrations);
            }
            if let Some(env) = fs_check::environment()
                && let Some(dir) = path.parent()
                && let Some(fs) = check_dir(env, dir)
            {
                if let Some(root_fs) = check_dir(env, &env.data_root) {
                    // Nowhere safer to go: the store stays, and the warning names both places.
                    notify(Notice::UnsafeFilesystem {
                        placement: Placement::DataRoot,
                        path: env.data_root.clone(),
                        fs: root_fs,
                        store: path.to_path_buf(),
                    });
                } else {
                    if !pointer::valid_workspace_id(&init.workspace_id) {
                        return Err(StoreError::Io(format!(
                            "the workspace ID {:?} can't name a directory under the data root",
                            init.workspace_id
                        )));
                    }
                    // The pointer goes first: a store missing after it is created at the next
                    // open, while a store with no pointer would be lost to the workspace.
                    let target = pointer::store_under(env, &init.workspace_id);
                    let wanted = StorePointer::new(target.display().to_string(), init.workspace_id.clone());
                    let pointer = pointer::publish(path, &wanted)?;
                    let store = Store::create_at_pointer(path, &pointer, init, migrations)?;
                    notify(Notice::UnsafeFilesystem {
                        placement: Placement::Moved,
                        path: dir.to_path_buf(),
                        fs,
                        store: store.path.clone(),
                    });
                    return Ok(store);
                }
            }
        }
        Store::create_at(path, init, migrations)
    }

    /// Open, or create, the store a pointer names: a store created there records the pointer's
    /// workspace ID, whichever process created it.
    fn create_at_pointer(
        path: &Path,
        pointer: &StorePointer,
        init: &Init,
        migrations: &[Migration],
    ) -> Result<Store, StoreError> {
        let target = pointer::target(fs_check::environment(), path, pointer)?;
        if let Some(dir) = target.parent() {
            private_dir(dir)?;
        }
        let init = Init { workspace_id: pointer.workspace_id.clone(), ..init.clone() };
        let store = Store::create_at(&target, &init, migrations)?;
        store.check_workspace(pointer)?;
        Ok(store)
    }

    /// Open the store file at `path`, creating it with mode 0600 first when it's missing.
    fn create_at(path: &Path, init: &Init, migrations: &[Migration]) -> Result<Store, StoreError> {
        // Create the file first, so SQLite and its WAL files take mode 0600 from it (DR-29).
        match OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
            Ok(_) => {}
            // Another process created it first; that file is fine.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(StoreError::Io(format!("{}: {e}", path.display()))),
        }
        let mut store = Store::connect(path)?;
        store.migrate(Some(init), migrations)?;
        Ok(store)
    }

    /// Open an existing store, following `store.pointer` when the state directory holds one in
    /// place of `store.db`; `None` when there is no store file at `path` or where it points.
    pub fn open_existing(path: &Path) -> Result<Option<Store>, StoreError> {
        let Some(found) = locate(path)? else { return Ok(None) };
        let mut store = Store::connect(&found)?;
        if found != path
            && let Some(pointer) = pointer::read(path)?
        {
            store.check_workspace(&pointer)?;
        }
        store.migrate(None, MIGRATIONS)?;
        Ok(Some(store))
    }

    /// Refuse a store whose `meta.workspace_id` isn't the one its pointer names.
    fn check_workspace(&self, pointer: &StorePointer) -> Result<(), StoreError> {
        match self.meta("workspace_id")? {
            Some(id) if id != pointer.workspace_id => Err(StoreError::Io(format!(
                "{} belongs to workspace {id}, not {}, the one its store.pointer names",
                self.path.display(),
                pointer.workspace_id
            ))),
            _ => Ok(()),
        }
    }

    fn connect(path: &Path) -> Result<Store, StoreError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let mode = Store::wal_mode(&conn)?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Io(format!("{}: WAL mode is unavailable ({mode})", path.display())));
        }
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Store { conn, path: path.to_path_buf(), hook: None };
        store.check_place();
        Ok(store)
    }

    /// Switch the connection to WAL mode. Switching a store still in its first, rollback-journal
    /// form needs an exclusive lock, and SQLite answers busy at once, without its busy handler,
    /// when engines creating the store at once would otherwise wait on each other. So a busy
    /// switch is retried every 50 ms within the busy timeout.
    fn wal_mode(conn: &Connection) -> Result<String, StoreError> {
        const STEP: Duration = Duration::from_millis(50);
        let tries = BUSY_TIMEOUT.as_millis() / STEP.as_millis();
        let mut tried = 0;
        loop {
            match conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0)) {
                Ok(mode) => return Ok(mode),
                Err(e) => match StoreError::from(e) {
                    StoreError::Busy if tried < tries => {
                        tried += 1;
                        std::thread::sleep(STEP);
                    }
                    other => return Err(other),
                },
            }
        }
    }

    /// Keep the store's files at 0600 (DR-29), and leave a `Kept` notice when its directory is
    /// on a filesystem that can't share a write-ahead log. The directory keeps its mode: the
    /// store creates it 0700 only under the data root, and the app creates the state directory.
    fn check_place(&self) {
        let Some(dir) = self.path.parent() else { return };
        for suffix in ["", "-wal", "-shm"] {
            let mut file = self.path.clone().into_os_string();
            file.push(suffix);
            narrow(Path::new(&file), 0o600);
        }
        if let Some(env) = fs_check::environment()
            && let Some(fs) = check_dir(env, dir)
        {
            notify(Notice::UnsafeFilesystem {
                placement: Placement::Kept,
                path: dir.to_path_buf(),
                fs,
                store: self.path.clone(),
            });
        }
    }

    /// The file this store lives in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One `meta` value.
    pub fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    /// Set a `meta` value only when it has none yet; returns the value it holds afterwards.
    pub fn meta_insert_once(&mut self, key: &str, value: &str) -> Result<String, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        tx.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)", [key, value])?;
        let held: String = tx.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))?;
        tx.commit()?;
        Ok(held)
    }

    /// Set `key` to `value`, and each of `also`, only when `key` holds `expected` (`None`: no
    /// value); returns whether it did. One IMMEDIATE transaction, so two callers can't both win.
    pub fn meta_swap(
        &mut self,
        key: &str,
        expected: Option<&str>,
        value: &str,
        also: &[(&str, &str)],
    ) -> Result<bool, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        let held: Option<String> =
            tx.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        if held.as_deref() != expected {
            return Ok(false);
        }
        for (k, v) in std::iter::once((key, value)).chain(also.iter().copied()) {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [k, v],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Record a spent one-time token's `digest` in `meta.spent_tokens` until `expires` (Unix
    /// seconds), dropping digests expired by `now`. Returns false when the digest was already
    /// spent, so each token works once (DR-64).
    pub fn spend_token(&mut self, digest: &str, now: u64, expires: u64) -> Result<bool, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        let held: Option<String> =
            tx.query_row("SELECT value FROM meta WHERE key = 'spent_tokens'", [], |r| r.get(0)).optional()?;
        let mut spent: serde_json::Map<String, serde_json::Value> = match held {
            Some(text) => serde_json::from_str(&text)?,
            None => serde_json::Map::new(),
        };
        spent.retain(|_, until| until.as_u64().is_some_and(|u| u > now));
        if spent.contains_key(digest) {
            return Ok(false);
        }
        spent.insert(digest.to_owned(), expires.into());
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('spent_tokens', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [serde_json::Value::Object(spent).to_string()],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// The latest committed `seq`, 0 for a store with no events.
    pub fn seq(&self) -> Result<Seq, StoreError> {
        read::max_seq(&self.conn)
    }

    /// After each commit, send its last `seq` here: the process's own live loop, which
    /// `data_version` can't wake for this connection's commits (DR-1).
    pub fn set_commit_hook(&mut self, hook: Sender<Seq>) {
        self.hook = Some(hook);
    }

    fn notify(&self, seq: Seq) {
        if let Some(hook) = &self.hook {
            let _ = hook.send(seq);
        }
    }
}
