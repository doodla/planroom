//! Migrations (contracts.md, Store schema and migrations; DR-53).
//!
//! A store found below this build's version is migrated in one IMMEDIATE transaction: every
//! pending migration, the new `user_version` and `meta`, together, so a failure anywhere rolls
//! the whole store back to the version it had. Before that transaction writes anything, a store
//! that already held a schema (any version above 0) is copied with `VACUUM INTO` to
//! `DATA_ROOT/backups/<workspace_id>/store-vN.db` and the copy is checked with
//! `PRAGMA integrity_check`. The copy is taken while the transaction holds the write lock, from a
//! second, read-only connection, so it is exactly the store the migration starts from. The last
//! three copies are kept, and a failed migration keeps its copy.

use std::fs::{self, DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use crate::fs_check::{self, Notice};
use crate::pointer::{sync_dir, unique, valid_workspace_id};
use crate::{BUSY_TIMEOUT, Init, Store, StoreError};

/// One numbered migration: its version and its SQL.
pub type Migration = (u32, &'static str);

/// The numbered migrations, applied in order by `PRAGMA user_version`.
pub const MIGRATIONS: &[Migration] = &[(1, include_str!("../migrations/0001_init.sql"))];

/// How many pre-migration copies a workspace keeps.
pub const KEPT_COPIES: usize = 3;

impl Store {
    /// Bring the store to the last of `migrations`, as the module says. `init` fills a new
    /// store's `meta`.
    pub(crate) fn migrate(
        &mut self,
        init: Option<&Init>,
        migrations: &[Migration],
    ) -> Result<(), StoreError> {
        let target = migrations.last().map_or(0, |m| m.0);
        let found: u32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found > target {
            return Err(self.newer(found, target));
        }
        if found == target {
            return Ok(());
        }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Another process may have migrated while this one waited for the lock.
        let from: u32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if from >= target {
            drop(tx);
            return if from > target { Err(self.newer(from, target)) } else { Ok(()) };
        }
        let backup = if from == 0 { None } else { Some(copy(&self.path, &tx, from)?) };
        let applied = (|| {
            for &(version, sql) in migrations.iter().filter(|(v, _)| *v > from) {
                tx.execute_batch(sql)?;
                tx.pragma_update(None, "user_version", version)?;
            }
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('store_version', ?1)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [target.to_string()],
            )?;
            if from == 0 {
                let init =
                    init.ok_or_else(|| StoreError::Io("a new store needs its workspace record".into()))?;
                for (key, value) in [
                    ("workspace_id", &init.workspace_id),
                    ("engine_version", &init.engine_version),
                    ("created_at", &init.created_at),
                ] {
                    tx.execute("INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)", [key, value])?;
                }
            } else if let Some(env) = fs_check::environment() {
                // The engine that wrote the store last, which an older engine names (DR-69).
                tx.execute(
                    "INSERT INTO meta (key, value) VALUES ('engine_version', ?1)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    [&env.engine_version],
                )?;
            }
            Ok::<(), StoreError>(())
        })();
        let committed = applied.and_then(|()| tx.commit().map_err(StoreError::from));
        if let Err(e) = committed {
            // Dropping the transaction rolled every migration back; the copy stays.
            return Err(match backup {
                Some(copy) => StoreError::Io(format!(
                    "migrating the store from version {from} to {target} failed and was rolled back, so it is as it was; its copy is at {}: {e}",
                    copy.display()
                )),
                None => e,
            });
        }
        if let Some(backup) = backup {
            fs_check::notify(Notice::Migrated { from, to: target, backup });
        }
        Ok(())
    }

    fn newer(&self, found: u32, supported: u32) -> StoreError {
        let writer_version = self.meta("engine_version").ok().flatten();
        StoreError::SchemaMismatch { found, supported, writer_version }
    }
}

/// Copy the store at `path`, at version `from`, to `DATA_ROOT/backups/<workspace_id>/store-v<from>.db`
/// while `held` holds the write lock, check the copy, and keep the last three.
fn copy(path: &Path, held: &Connection, from: u32) -> Result<PathBuf, StoreError> {
    let env = fs_check::environment().ok_or_else(|| {
        StoreError::Io(format!(
            "{}: migrating from version {from} needs the data root for its copy, and this process has none",
            path.display()
        ))
    })?;
    let id: Option<String> =
        held.query_row("SELECT value FROM meta WHERE key = 'workspace_id'", [], |r| r.get(0)).optional()?;
    let id = id.filter(|id| valid_workspace_id(id)).ok_or_else(|| {
        StoreError::Io(format!("{}: its workspace ID can't name a backup directory", path.display()))
    })?;
    let dir = env.data_root.join("backups").join(&id);
    private_dir(&dir)?;
    let dest = dir.join(format!("store-v{from}.db"));
    // A temporary copy nothing has written to for STALE_COPY is a killed process's: it would stay
    // there, a whole store, forever. One being written is written to all the while, and another
    // store can be writing here when two share a workspace ID, as a copied state directory does,
    // so a fresh one is left alone.
    sweep(&dir, ".store-v", ".tmp", Some(STALE_COPY));
    let tmp = dir.join(format!(".store-v{from}.db.{}.tmp", unique()));
    let fail = |e: &dyn std::fmt::Display| StoreError::Io(format!("{}: {e}", dest.display()));
    // VACUUM INTO writes into an empty file it finds, so the copy keeps this file's mode 0600.
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).map_err(|e| fail(&e))?;
    let written = (|| {
        let reader = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        reader.busy_timeout(BUSY_TIMEOUT)?;
        let into = tmp.to_str().ok_or_else(|| StoreError::Io(format!("{}: not UTF-8", tmp.display())))?;
        reader.execute("VACUUM INTO ?1", [into])?;
        drop(reader);
        let check = Connection::open_with_flags(
            &tmp,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let verdict: String = check.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if verdict != "ok" {
            return Err(StoreError::Io(format!("the copy failed its integrity check: {verdict}")));
        }
        drop(check);
        fs::File::open(&tmp).and_then(|f| f.sync_all()).map_err(|e| StoreError::Io(e.to_string()))?;
        fs::rename(&tmp, &dest).map_err(|e| StoreError::Io(e.to_string()))
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(fail(&e));
    }
    sync_dir(Some(&dir));
    prune(&dir);
    Ok(dest)
}

/// How long a temporary migration copy may go unwritten before a later migration takes it for a
/// killed process's and removes it.
const STALE_COPY: std::time::Duration = std::time::Duration::from_secs(600);

/// Remove the files in `dir` named `<prefix>...<suffix>`, those last modified longer than
/// `older_than` ago when it's given, best effort: the temporary files killed processes left.
pub(crate) fn sweep(dir: &Path, prefix: &str, suffix: &str, older_than: Option<std::time::Duration>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with(prefix) || !name.ends_with(suffix) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let stale = older_than.is_none_or(|age| {
            meta.modified().ok().and_then(|at| at.elapsed().ok()).is_some_and(|elapsed| elapsed > age)
        });
        if meta.is_file() && stale {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Keep the [`KEPT_COPIES`] newest `store-vN.db` copies in `dir`, by `N`.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut copies: Vec<(u32, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix("store-v")?.strip_suffix(".db")?;
            Some((n.parse().ok()?, e.path()))
        })
        .collect();
    copies.sort_by_key(|c| std::cmp::Reverse(c.0));
    for (_, old) in copies.into_iter().skip(KEPT_COPIES) {
        let _ = fs::remove_file(old);
    }
}

/// Create `dir`, `<data root>/<area>/<workspace_id>`, and its missing parents with mode 0700,
/// and narrow `dir` and its parent, the area the store owns under the data root (`stores/` or
/// `backups/`), to 0700 when either is wider.
pub(crate) fn private_dir(dir: &Path) -> Result<(), StoreError> {
    let fail = |e: std::io::Error| StoreError::Io(format!("{}: {e}", dir.display()));
    DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(fail)?;
    if let Some(area) = dir.parent() {
        narrow(area, 0o700);
    }
    narrow(dir, 0o700);
    Ok(())
}

/// Narrow the mode of `path` to `mode` when it grants more, best effort (only its owner can).
pub(crate) fn narrow(path: &Path, mode: u32) {
    if let Ok(meta) = fs::symlink_metadata(path)
        && !meta.file_type().is_symlink()
        && meta.permissions().mode() & 0o777 & !mode != 0
    {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
}
