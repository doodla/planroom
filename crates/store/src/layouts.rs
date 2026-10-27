//! The layout cache: auto-layout positions per view and structure, last writer wins.

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Store, StoreError, commit};

impl Store {
    /// Cache `positions` (JSON) for `view_key` at `structure_hash`, replacing what was there.
    /// Like every write, one IMMEDIATE transaction that checks the store version first (DR-73).
    pub fn put_layout(
        &self,
        view_key: &str,
        structure_hash: &str,
        positions: &str,
    ) -> Result<(), StoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        tx.execute(
            "INSERT INTO layouts (view_key, structure_hash, positions) VALUES (?1, ?2, ?3)
             ON CONFLICT (view_key, structure_hash) DO UPDATE SET positions = excluded.positions",
            params![view_key, structure_hash, positions],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The cached positions (JSON) for `view_key` at `structure_hash`, if any.
    pub fn layout(&self, view_key: &str, structure_hash: &str) -> Result<Option<String>, StoreError> {
        cached(&self.conn, view_key, structure_hash)
    }
}

/// The cached positions for `view_key` at `structure_hash`, read on `conn`, which may be inside
/// a read snapshot.
pub(crate) fn cached(
    conn: &Connection,
    view_key: &str,
    structure_hash: &str,
) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT positions FROM layouts WHERE view_key = ?1 AND structure_hash = ?2",
            params![view_key, structure_hash],
            |r| r.get(0),
        )
        .optional()?)
}
