//! The layout cache: auto-layout positions per view and structure, last writer wins.

use rusqlite::{OptionalExtension, params};

use crate::{Store, StoreError};

impl Store {
    /// Cache `positions` (JSON) for `view_key` at `structure_hash`, replacing what was there.
    pub fn put_layout(
        &self,
        view_key: &str,
        structure_hash: &str,
        positions: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO layouts (view_key, structure_hash, positions) VALUES (?1, ?2, ?3)
             ON CONFLICT (view_key, structure_hash) DO UPDATE SET positions = excluded.positions",
            params![view_key, structure_hash, positions],
        )?;
        Ok(())
    }

    /// The cached positions (JSON) for `view_key` at `structure_hash`, if any.
    pub fn layout(&self, view_key: &str, structure_hash: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT positions FROM layouts WHERE view_key = ?1 AND structure_hash = ?2",
                params![view_key, structure_hash],
                |r| r.get(0),
            )
            .optional()?)
    }
}
