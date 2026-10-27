//! The open-card read: the events that decide which cards are open, read from `events` by `kind`
//! with no table of its own (DR-89). The store keeps no rule: `kernel::fold::open_cards` decides
//! from these events, as the fold does.

use planroom_contract::Event;

use crate::StoreError;
use crate::read::{Snapshot, decode_rows};

impl Snapshot<'_> {
    /// Every event that bears on which cards are open, in `seq` order: each `card` event, each
    /// `review` that names one, each `undo`, and each event an undo names, whose key stem says
    /// which command the undo reverses.
    pub fn card_events(&self) -> Result<Vec<Event>, StoreError> {
        let mut stmt = self.tx().prepare(
            "SELECT seq, id, idem, request_hash, at, actor, client, project, cause, status, body
             FROM events
             WHERE kind IN ('card', 'undo')
                OR (kind = 'review'
                    AND json_extract(body, '$.of') IN (SELECT id FROM events WHERE kind = 'card'))
                OR id IN (SELECT json_extract(body, '$.of') FROM events WHERE kind = 'undo')
             ORDER BY seq",
        )?;
        decode_rows(&mut stmt)
    }
}
