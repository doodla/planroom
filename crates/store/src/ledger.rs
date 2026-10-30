//! The user-level spend ledger, `DATA_ROOT/ledger.db` (architecture.md, Store schema; contracts.md,
//! Store schema and migrations; DR-51, DR-78).
//!
//! One file serves every engine version on the machine at once, so:
//!
//! - its migrations, `crates/store/ledger/NNNN_name.sql`, only add: a table, or a column that is
//!   nullable or has a default, never a rename or a removal. [`Ledger::record`] names its columns,
//!   so a column a newer engine added stays NULL, or takes its default, in an older engine's row;
//! - `meta.min_reader_version` is the oldest engine version that reads the file. An engine older
//!   than it refuses with `schema_mismatch` naming the file ([`StoreError::NewerFile`]), and
//!   neither writes nor migrates it. A migration raises it only when old readers would misread
//!   the file;
//! - a `user_version` above this build's last migration still opens: this build reads and writes
//!   the columns it knows, and never migrates the file down;
//! - below it, every pending migration and the new `user_version` go in one IMMEDIATE
//!   transaction. Before that transaction writes anything, a ledger that already held a schema is
//!   copied with `VACUUM INTO` to `DATA_ROOT/backups/ledger-vN.db`, checked with
//!   `PRAGMA integrity_check`, and the last three copies are kept. A failed migration rolls back
//!   and keeps its copy.
//!
//! The ledger stays in SQLite's rollback journal: no engine sets `journal_mode`, since a WAL mode
//! is stored in the file and would bind every engine that opens it. In that mode the migrating
//! transaction holds only a RESERVED lock until it writes, so the copy's reader still gets its
//! SHARED lock. The rollback journal needs no shared-memory index, so the ledger has no
//! filesystem check of its own.

use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use planroom_contract::format::reader_too_old;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::migrate::{KEPT_COPIES, Migration, narrow, prune_copies, vacuum_copy};
use crate::{BUSY_TIMEOUT, StoreError};

/// The ledger's numbered migrations, applied in order by `PRAGMA user_version`.
pub const LEDGER_MIGRATIONS: &[Migration] = &[(1, include_str!("../ledger/0001_init.sql"))];

/// The ledger's file name under the data root.
pub const LEDGER_FILE: &str = "ledger.db";

/// One row of the ledger, every column but `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerRow {
    /// When the reply arrived: RFC 3339 UTC to the second, `YYYY-MM-DDTHH:MM:SSZ`.
    pub at: String,
    /// The workspace the call was made for, its store's `meta.workspace_id`.
    pub workspace_id: Option<String>,
    /// `gateway` for every Claude call; `jev`, `typesafe`, `session` and `sample` later.
    pub provider: String,
    /// The pinned model ID sent.
    pub model: String,
    /// What the call was for, such as `plan.stage.shape`.
    pub purpose: String,
    /// The budget the call counts against: `day` for a Claude call; `scan:<seq>` and
    /// `decide:<idem>` for Jev's.
    pub scope: String,
    pub uncached_in: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub output: Option<i64>,
    /// Dollars spent; none for an answer read from a recording.
    pub cost_usd: Option<f64>,
    pub prefix_hash: Option<String>,
    pub generation_id: Option<String>,
    /// Whether the answer was read from an existing recording rather than sent.
    pub replayed: bool,
}

/// One read-write connection to the user-level ledger.
pub struct Ledger {
    conn: Connection,
    path: PathBuf,
    engine_version: String,
}

impl Ledger {
    /// Open `<data_root>/ledger.db` for an engine at `engine_version`, creating the data root
    /// (mode 0700) and the file (mode 0600) when they're missing, and migrating it as the module
    /// says.
    pub fn open_at(data_root: &Path, engine_version: &str) -> Result<Ledger, StoreError> {
        Ledger::open(data_root, engine_version, LEDGER_MIGRATIONS)
    }

    /// [`Ledger::open_at`] with `migrations` in place of this build's: how a test plants a newer
    /// or a failing migration. Debug builds only, so no release build can reach it (DR-92).
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn open_with_migrations(
        data_root: &Path,
        engine_version: &str,
        migrations: &'static [Migration],
    ) -> Result<Ledger, StoreError> {
        Ledger::open(data_root, engine_version, migrations)
    }

    fn open(data_root: &Path, engine_version: &str, migrations: &[Migration]) -> Result<Ledger, StoreError> {
        let io = |p: &Path, e: std::io::Error| StoreError::Io(format!("{}: {e}", p.display()));
        // A data root that exists keeps its mode; a new one is private.
        DirBuilder::new().recursive(true).mode(0o700).create(data_root).map_err(|e| io(data_root, e))?;
        let path = data_root.join(LEDGER_FILE);
        // Created first, so SQLite and its journal take mode 0600 from it; an existing ledger is
        // opened as it is, never truncated.
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&path)
            .map_err(|e| io(&path, e))?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(&path, flags)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        for suffix in ["", "-journal"] {
            let mut file = path.clone().into_os_string();
            file.push(suffix);
            narrow(Path::new(&file), 0o600);
        }
        check_reader(&conn, &path, engine_version)?;
        let mut ledger = Ledger { conn, path, engine_version: engine_version.to_owned() };
        ledger.migrate(data_root, migrations)?;
        Ok(ledger)
    }

    /// The ledger's file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bring the ledger up to the last of `migrations`, never down.
    fn migrate(&mut self, data_root: &Path, migrations: &[Migration]) -> Result<(), StoreError> {
        let target = migrations.last().map_or(0, |m| m.0);
        let found: u32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found >= target {
            return Ok(());
        }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Another engine may have migrated, or raised `min_reader_version`, while this one waited.
        let from: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        check_reader(&tx, &self.path, &self.engine_version)?;
        if from >= target {
            return Ok(());
        }
        let backup = if from == 0 { None } else { Some(copy(&self.path, data_root, from)?) };
        let applied = (|| {
            for &(version, sql) in migrations.iter().filter(|(v, _)| *v > from) {
                tx.execute_batch(sql)?;
                tx.pragma_update(None, "user_version", version)?;
            }
            Ok::<(), StoreError>(())
        })();
        let committed = applied.and_then(|()| tx.commit().map_err(StoreError::from));
        if let Err(e) = committed {
            // Dropping the transaction rolled every migration back; the copy stays.
            return Err(match backup {
                Some(copy) => StoreError::Io(format!(
                    "migrating the ledger from version {from} to {target} failed and was rolled back, so it is as it was; its copy is at {}: {e}",
                    copy.display()
                )),
                None => e,
            });
        }
        Ok(())
    }

    /// Append `row` in its own IMMEDIATE transaction and return its `id`. The transaction checks
    /// `min_reader_version` again, so a migration that raised it meanwhile refuses this engine.
    pub fn record(&mut self, row: &LedgerRow) -> Result<i64, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_reader(&tx, &self.path, &self.engine_version)?;
        tx.execute(
            "INSERT INTO ledger (at, workspace_id, provider, model, purpose, scope, uncached_in, cache_read,
                                 cache_write, output, cost_usd, prefix_hash, generation_id, replayed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                row.at,
                row.workspace_id,
                row.provider,
                row.model,
                row.purpose,
                row.scope,
                row.uncached_in,
                row.cache_read,
                row.cache_write,
                row.output,
                row.cost_usd,
                row.prefix_hash,
                row.generation_id,
                row.replayed,
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// The rows with `id > after_id`, at most `limit`, in `id` order: only the columns this build
    /// knows, whatever a newer engine added.
    pub fn rows(&self, after_id: i64, limit: u32) -> Result<Vec<(i64, LedgerRow)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, at, workspace_id, provider, model, purpose, scope, uncached_in, cache_read,
                    cache_write, output, cost_usd, prefix_hash, generation_id, replayed
             FROM ledger WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_id, limit], |r| {
            Ok((
                r.get(0)?,
                LedgerRow {
                    at: r.get(1)?,
                    workspace_id: r.get(2)?,
                    provider: r.get(3)?,
                    model: r.get(4)?,
                    purpose: r.get(5)?,
                    scope: r.get(6)?,
                    uncached_in: r.get(7)?,
                    cache_read: r.get(8)?,
                    cache_write: r.get(9)?,
                    output: r.get(10)?,
                    cost_usd: r.get(11)?,
                    prefix_hash: r.get(12)?,
                    generation_id: r.get(13)?,
                    replayed: r.get(14)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// Refuse the ledger at `path`, read through `conn`, when its `min_reader_version` is above
/// `engine_version`: `schema_mismatch` naming the file.
fn check_reader(conn: &Connection, path: &Path, engine_version: &str) -> Result<(), StoreError> {
    let has_meta = conn
        .query_row("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta'", [], |_| Ok(()))
        .optional()?
        .is_some();
    if !has_meta {
        return Ok(());
    }
    let min: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'min_reader_version'", [], |r| r.get(0))
        .optional()?;
    match min {
        Some(min) if reader_too_old(&min, engine_version) => Err(StoreError::NewerFile {
            file: path.to_path_buf(),
            found: min,
            supported: engine_version.to_owned(),
        }),
        _ => Ok(()),
    }
}

/// Copy the ledger at `path`, at version `from`, to `<data_root>/backups/ledger-v<from>.db` while
/// the caller holds the write lock, and keep the last three.
fn copy(path: &Path, data_root: &Path, from: u32) -> Result<PathBuf, StoreError> {
    let dir = data_root.join("backups");
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|e| StoreError::Io(format!("{}: {e}", dir.display())))?;
    narrow(&dir, 0o700);
    let dest = vacuum_copy(path, &dir, &format!("ledger-v{from}.db"))?;
    prune_copies(&dir, "ledger-v", KEPT_COPIES);
    Ok(dest)
}
