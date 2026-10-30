//! The pack record in `meta` (DR-62): the resolved pack list, its hash, the engine that recorded
//! it and when the workspace config's list was read, and the guard every authored write runs on
//! it under the write lock.
//!
//! The store holds no rules: whether a process may write over the record, and what it records
//! first, is the [`PackVerdict`] the composition root's guard returns. A store with no guard
//! checks nothing.

use planroom_contract::PackRecord;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::commit::check_version;
use crate::{Snapshot, Store, StoreError};

/// The `meta` keys the pack record spans.
pub const META_PACKS: &str = "packs";
pub const META_PACKS_HASH: &str = "packs_hash";
pub const META_PACKS_ENGINE: &str = "packs_engine";
pub const META_PACKS_CONFIG_READ: &str = "packs_config_read";

/// The pack record, read in one call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedPacks {
    /// The resolved pack list, in load order.
    pub packs: Vec<PackRecord>,
    /// Its hash, under the identity of the engine that recorded it.
    pub hash: String,
    /// The identity of the engine that recorded it; none for a record an older engine wrote.
    pub engine: Option<String>,
    /// When the workspace config's `packs` list was read; none until a trusted scan read it, or
    /// `pack add` or `pack remove` wrote it.
    pub config_read: Option<String>,
}

/// What a guard decides about the record a write finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackVerdict {
    /// The record names this process's set: write on.
    Keep,
    /// Record this set first, in the write's own transaction, then write on.
    Record(RecordedPacks),
    /// An engine older than the one that recorded the set: refuse the write with
    /// `packs_mismatch`, naming both engines' identities.
    Refuse { recorded: Option<String>, resolved: String },
}

/// The guard a store runs on the pack record before every authored write.
pub type PackGuard = dyn Fn(Option<&RecordedPacks>) -> PackVerdict + Send + Sync;

/// The record as `conn` reads it; none when no pack list or hash was ever recorded.
pub(crate) fn read(conn: &Connection) -> Result<Option<RecordedPacks>, StoreError> {
    let get = |key: &str| -> Result<Option<String>, StoreError> {
        Ok(conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    };
    let (packs, hash) = (get(META_PACKS)?, get(META_PACKS_HASH)?);
    if packs.is_none() && hash.is_none() {
        return Ok(None);
    }
    let packs: Vec<PackRecord> = match packs {
        Some(text) => serde_json::from_str(&text)?,
        None => Vec::new(),
    };
    Ok(Some(RecordedPacks {
        packs,
        hash: hash.unwrap_or_default(),
        engine: get(META_PACKS_ENGINE)?,
        config_read: get(META_PACKS_CONFIG_READ)?,
    }))
}

/// Write `record` over the pack record: every key, a key `record` leaves empty removed.
pub(crate) fn write(conn: &Connection, record: &RecordedPacks) -> Result<(), StoreError> {
    let set = |key: &str, value: Option<&str>| -> Result<(), StoreError> {
        match value {
            Some(value) => conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?,
            None => conn.execute("DELETE FROM meta WHERE key = ?1", [key])?,
        };
        Ok(())
    };
    set(META_PACKS, Some(&serde_json::to_string(&record.packs)?))?;
    set(META_PACKS_HASH, Some(&record.hash))?;
    set(META_PACKS_ENGINE, record.engine.as_deref())?;
    set(META_PACKS_CONFIG_READ, record.config_read.as_deref())
}

/// What `guard` decides on the record `conn` reads; `Keep` with no guard.
fn verdict(guard: Option<&PackGuard>, conn: &Connection) -> Result<PackVerdict, StoreError> {
    let Some(guard) = guard else { return Ok(PackVerdict::Keep) };
    Ok(guard(read(conn)?.as_ref()))
}

/// The check every authored write makes right after `BEGIN IMMEDIATE`: the store version, then
/// the store's pack guard on the record this transaction reads. A `Record` verdict writes the
/// record in that transaction, so it commits with the write or not at all; a `Refuse` verdict is
/// `packs_mismatch`.
pub(crate) fn check_writer(guard: Option<&PackGuard>, conn: &Connection) -> Result<(), StoreError> {
    check_version(conn)?;
    match verdict(guard, conn)? {
        PackVerdict::Keep => Ok(()),
        PackVerdict::Record(record) => write(conn, &record),
        PackVerdict::Refuse { recorded, resolved } => Err(StoreError::PacksMismatch { recorded, resolved }),
    }
}

impl Snapshot<'_> {
    /// The pack record, read in this read transaction, so a reply folds with the set its own
    /// snapshot records.
    pub fn recorded_packs(&self) -> Result<Option<RecordedPacks>, StoreError> {
        read(self.tx())
    }
}

impl Store {
    /// Install the guard every authored write runs on the pack record ([`check_writer`]). A store
    /// has one: the last install wins.
    pub fn guard_packs(&mut self, guard: Box<PackGuard>) {
        self.guard = Some(guard);
    }

    /// The pack record, read outside a write.
    pub fn recorded_packs(&self) -> Result<Option<RecordedPacks>, StoreError> {
        read(&self.conn)
    }

    /// The guard's verdict on the record as it stands, read outside a write: how a command that
    /// reads and copies before it writes is refused before it starts. The write checks again.
    pub fn pack_verdict(&self) -> Result<PackVerdict, StoreError> {
        verdict(self.guard.as_deref(), &self.conn)
    }

    /// Change the pack record in one IMMEDIATE transaction: the store version and the guard's
    /// `Refuse` check first, then `f` with the record as the lock finds it, the guard's `Record`
    /// applied. What `f` returns is written (`None`: nothing beyond the guard's record) and
    /// committed. So a caller always builds on the record the lock finds, and never loses another
    /// writer's change between its read and its write.
    pub fn record_packs<E: From<StoreError>>(
        &mut self,
        f: impl FnOnce(Option<&RecordedPacks>) -> Result<Option<RecordedPacks>, E>,
    ) -> Result<Option<RecordedPacks>, E> {
        let tx =
            self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(StoreError::from)?;
        check_version(&tx)?;
        let current = match verdict(self.guard.as_deref(), &tx)? {
            PackVerdict::Keep => read(&tx)?,
            PackVerdict::Record(record) => {
                write(&tx, &record)?;
                Some(record)
            }
            PackVerdict::Refuse { recorded, resolved } => {
                return Err(StoreError::PacksMismatch { recorded, resolved }.into());
            }
        };
        let next = f(current.as_ref())?;
        if let Some(record) = &next {
            write(&tx, record)?;
        }
        tx.commit().map_err(StoreError::from)?;
        Ok(next.or(current))
    }
}
