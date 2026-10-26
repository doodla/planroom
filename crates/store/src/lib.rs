//! The SQLite store: tables, commit, tail, leases, presence and migrations.
//!
//! Every engine process writes through [`Store::commit`] or [`Store::commit_scan`], each one
//! `BEGIN IMMEDIATE` transaction in WAL mode with `synchronous = FULL` and a 5 s busy timeout
//! (DR-1). The store holds no business rules: the keys a scan derives come from functions the
//! composition root passes in.

mod commit;
mod error;
mod read;
mod tail;

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use planroom_contract::Seq;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

pub use commit::{
    Committed, NewEvent, RepoOutcome, RepoScan, ScanCommitted, ScanKey, ScanRules, ScanWrite, Write,
};
pub use error::StoreError;
pub use read::{ActiveSet, FactRow, Shape};
pub use tail::Tail;

/// How long a connection waits for the write lock before `store_busy`. SQLite's busy handler
/// backs off to 100 ms sleeps, so under steady contention one commit can wait ~630 ms; WS4
/// measured that against a 1 ms fixed step and kept SQLite's (docs/contributing/research/ws4-concurrency.md).
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The numbered migrations, applied in order by `PRAGMA user_version`.
const MIGRATIONS: &[(u32, &str)] = &[(1, include_str!("../migrations/0001_init.sql"))];

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

impl Store {
    /// Open the store at `path`, creating it (mode 0600) and migrating it as needed.
    pub fn open(path: &Path, init: &Init) -> Result<Store, StoreError> {
        // Create the file first, so SQLite and its WAL files take mode 0600 from it (DR-29).
        match OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
            Ok(_) => {}
            // Another process created it first; that file is fine.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(StoreError::Io(format!("{}: {e}", path.display()))),
        }
        let mut store = Store::connect(path)?;
        store.migrate(Some(init))?;
        Ok(store)
    }

    /// Open an existing store; `None` when there is no store file at `path`.
    pub fn open_existing(path: &Path) -> Result<Option<Store>, StoreError> {
        if !path.is_file() {
            return Ok(None);
        }
        let mut store = Store::connect(path)?;
        store.migrate(None)?;
        Ok(Some(store))
    }

    fn connect(path: &Path) -> Result<Store, StoreError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::Io(format!("{}: WAL mode is unavailable ({mode})", path.display())));
        }
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Store { conn, path: path.to_path_buf(), hook: None })
    }

    /// Apply every migration above the store's `user_version`, each in one IMMEDIATE transaction.
    fn migrate(&mut self, init: Option<&Init>) -> Result<(), StoreError> {
        let found: u32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found > STORE_VERSION {
            let writer = self.meta("engine_version").ok().flatten();
            return Err(StoreError::SchemaMismatch {
                found,
                supported: STORE_VERSION,
                writer_version: writer,
            });
        }
        for &(version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > found) {
            let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            // Another process may have migrated while this one waited for the lock.
            let now: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
            if now >= version {
                continue;
            }
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('store_version', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [version.to_string()],
            )?;
            if version == 1 {
                let init =
                    init.ok_or_else(|| StoreError::Io("a new store needs its workspace record".into()))?;
                for (key, value) in [
                    ("workspace_id", &init.workspace_id),
                    ("engine_version", &init.engine_version),
                    ("created_at", &init.created_at),
                ] {
                    tx.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)", [key, value])?;
                }
            }
            tx.commit()?;
        }
        Ok(())
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
        tx.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)", [key, value])?;
        let held: String = tx.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))?;
        tx.commit()?;
        Ok(held)
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
