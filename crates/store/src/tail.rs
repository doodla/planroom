//! `tail`: the live loop's own read-only connection, which never writes.

use std::path::Path;

use planroom_contract::{Event, PresenceRow, Seq, Timestamp};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::{BUSY_TIMEOUT, STORE_VERSION, StoreError, read};

/// A read-only connection for polling `PRAGMA data_version`. It never writes, so its
/// `data_version` changes on every commit by any other connection, in any process (DR-1).
pub struct Tail {
    conn: Connection,
}

impl Tail {
    /// Open the store at `path` read-only, following `store.pointer` as
    /// [`crate::Store::open_existing`] does; `None` when there is no store file there.
    pub fn open(path: &Path) -> Result<Option<Tail>, StoreError> {
        let Some(path) = crate::locate(path)? else { return Ok(None) };
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(&path, flags)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        Ok(Some(Tail { conn }))
    }

    /// `PRAGMA data_version`: it changes when another connection commits.
    pub fn data_version(&self) -> Result<i64, StoreError> {
        Ok(self.conn.pragma_query_value(None, "data_version", |r| r.get(0))?)
    }

    /// Refuse a store a newer engine migrated past this build's `STORE_VERSION`, with
    /// `schema_mismatch` naming the version that wrote it. The live loop calls this whenever
    /// `data_version` moves, so an older `watch` exits within one poll of a migration (DR-78).
    pub fn check_version(&self) -> Result<(), StoreError> {
        let found: u32 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if found <= STORE_VERSION {
            return Ok(());
        }
        let writer_version = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'engine_version'", [], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        Err(StoreError::SchemaMismatch { found, supported: STORE_VERSION, writer_version })
    }

    /// The latest committed `seq`.
    pub fn seq(&self) -> Result<Seq, StoreError> {
        read::max_seq(&self.conn)
    }

    /// Events with `seq > after`, at most `limit`, in order.
    pub fn events_after(&self, after: Seq, limit: u32) -> Result<Vec<Event>, StoreError> {
        read::events_after(&self.conn, after, limit)
    }

    /// One read transaction on this connection, as [`crate::Store::snapshot`] takes one: what
    /// the live loop reads to keep its fold (DR-61).
    pub fn snapshot(&self) -> Result<read::Snapshot<'_>, StoreError> {
        read::Snapshot::begin(&self.conn)
    }

    /// Every presence row seen at or after `since`, the latest first.
    pub fn presence_since(&self, since: &Timestamp) -> Result<Vec<PresenceRow>, StoreError> {
        crate::presence::presence_since(&self.conn, since)
    }

    /// How many events have `seq > after`.
    pub fn count_after(&self, after: Seq) -> Result<u64, StoreError> {
        let n: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM events WHERE seq > ?1", [after as i64], |r| r.get(0))?;
        Ok(n as u64)
    }
}
