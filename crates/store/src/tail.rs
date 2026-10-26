//! `tail`: the live loop's own read-only connection, which never writes.

use std::path::Path;

use planroom_contract::{Event, Seq};
use rusqlite::{Connection, OpenFlags};

use crate::{BUSY_TIMEOUT, StoreError, read};

/// A read-only connection for polling `PRAGMA data_version`. It never writes, so its
/// `data_version` changes on every commit by any other connection, in any process (DR-1).
pub struct Tail {
    conn: Connection,
}

impl Tail {
    /// Open the store at `path` read-only; `None` when there is no store file there.
    pub fn open(path: &Path) -> Result<Option<Tail>, StoreError> {
        if !path.is_file() {
            return Ok(None);
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        Ok(Some(Tail { conn }))
    }

    /// `PRAGMA data_version`: it changes when another connection commits.
    pub fn data_version(&self) -> Result<i64, StoreError> {
        Ok(self.conn.pragma_query_value(None, "data_version", |r| r.get(0))?)
    }

    /// The latest committed `seq`.
    pub fn seq(&self) -> Result<Seq, StoreError> {
        read::max_seq(&self.conn)
    }

    /// Events with `seq > after`, at most `limit`, in order.
    pub fn events_after(&self, after: Seq, limit: u32) -> Result<Vec<Event>, StoreError> {
        read::events_after(&self.conn, after, limit)
    }

    /// How many events have `seq > after`.
    pub fn count_after(&self, after: Seq) -> Result<u64, StoreError> {
        let n: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM events WHERE seq > ?1", [after as i64], |r| r.get(0))?;
        Ok(n as u64)
    }
}
