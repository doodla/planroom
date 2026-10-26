//! Presence: one row per writer instance, last-writer-wins, never user-authored.

use planroom_contract::{NodeId, PresenceRow, Surface, Timestamp};
use rusqlite::params;

use crate::{Store, StoreError};

fn surface_str(surface: Surface) -> Result<String, StoreError> {
    match serde_json::to_value(surface)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(StoreError::Sqlite(format!("a surface serialized as {other}"))),
    }
}

impl Store {
    /// Record that `row.actor`, from `client_id` on `row.surface`, was seen at `row.seen_at`.
    pub fn touch_presence(&self, row: &PresenceRow, client_id: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO presence (actor, surface, client_id, doing, focus, seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (actor, surface, client_id) DO UPDATE SET
               doing = excluded.doing, focus = excluded.focus, seen_at = excluded.seen_at",
            params![
                serde_json::to_string(&row.actor)?,
                surface_str(row.surface)?,
                client_id,
                row.doing,
                row.focus.as_ref().map(|f| f.0.as_str()),
                row.seen_at.0,
            ],
        )?;
        Ok(())
    }

    /// Every presence row seen at or after `since`, the latest first.
    pub fn presence_since(&self, since: &Timestamp) -> Result<Vec<PresenceRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT actor, surface, doing, focus, seen_at FROM presence
             WHERE seen_at >= ?1 ORDER BY seen_at DESC, actor, surface, client_id",
        )?;
        let rows = stmt.query_map([&since.0], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (actor, surface, doing, focus, seen_at) = row?;
            out.push(PresenceRow {
                actor: serde_json::from_str(&actor)?,
                surface: serde_json::from_value(serde_json::Value::String(surface))?,
                doing,
                focus: focus.map(NodeId),
                seen_at: Timestamp(seen_at),
            });
        }
        Ok(out)
    }
}
