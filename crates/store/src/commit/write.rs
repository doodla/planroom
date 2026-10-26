//! One command's events, appended under one idempotency key.

use planroom_contract::{Actor, Cause, Client, EventBody, EventId, NodeId, Seq, Status, Timestamp};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;

use super::insert_event;
use crate::{Store, StoreError};

/// One event to append. `seq` is assigned at commit.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub id: EventId,
    pub at: Timestamp,
    pub actor: Actor,
    pub client: Client,
    pub project: Option<NodeId>,
    pub cause: Option<Cause>,
    pub status: Status,
    /// The node, edge or repo the event is about, for the `events_target` index.
    pub target: Option<String>,
    pub body: EventBody,
}

/// The events one command writes, under one idempotency key.
#[derive(Debug, Clone)]
pub struct Write {
    /// The caller's key, a derived one, or a fresh uuid v7 (DR-8). Several events are keyed
    /// `idem#1`, `idem#2` and so on.
    pub idem: String,
    /// sha256 of the canonical request.
    pub request_hash: String,
    pub events: Vec<NewEvent>,
}

/// What a commit wrote, or what a retry found.
#[derive(Debug, Clone, PartialEq)]
pub struct Committed {
    /// The events' `seq`s, in order.
    pub seqs: Vec<Seq>,
    /// The whole reply, stored on the first event.
    pub reply: Value,
    /// The same key and request were committed before; nothing was written.
    pub replayed: bool,
}

impl Store {
    /// Append one command's events in one IMMEDIATE transaction. A key seen before with the same
    /// request hash returns the stored reply and writes nothing; with another hash it is
    /// `idempotency_mismatch`. `reply` builds the reply from the assigned `seq`s, inside the
    /// transaction, and it is stored on the first event.
    pub fn commit(
        &mut self,
        write: Write,
        reply: impl FnOnce(&[Seq]) -> Value,
    ) -> Result<Committed, StoreError> {
        assert!(!write.events.is_empty(), "a commit writes at least one event");
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(found) = replay(&tx, &write)? {
            return Ok(found);
        }
        let many = write.events.len() > 1;
        let mut seqs = Vec::with_capacity(write.events.len());
        for (i, event) in write.events.iter().enumerate() {
            let idem = if many { format!("{}#{}", write.idem, i + 1) } else { write.idem.clone() };
            seqs.push(insert_event(&tx, event, &idem, &write.request_hash)?);
        }
        let reply = reply(&seqs);
        tx.execute(
            "UPDATE events SET reply = ?1 WHERE seq = ?2",
            params![reply.to_string(), seqs[0] as i64],
        )?;
        tx.commit()?;
        self.notify(*seqs.last().expect("one event at least"));
        Ok(Committed { seqs, reply, replayed: false })
    }
}

/// The stored reply when `write`'s key was committed before with the same request hash.
fn replay(tx: &Transaction<'_>, write: &Write) -> Result<Option<Committed>, StoreError> {
    let found = tx
        .query_row(
            "SELECT seq, id, request_hash, reply FROM events WHERE idem = ?1 OR idem = ?1 || '#1'",
            [&write.idem],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((seq, id, hash, reply)) = found else { return Ok(None) };
    if hash != write.request_hash {
        return Err(StoreError::IdempotencyMismatch {
            idem: write.idem.clone(),
            stored_event: id,
            stored_seq: seq as Seq,
        });
    }
    let mut stmt =
        tx.prepare("SELECT seq FROM events WHERE idem = ?1 OR idem LIKE ?2 ESCAPE '\\' ORDER BY seq")?;
    let like = format!("{}#%", escape_like(&write.idem));
    let seqs =
        stmt.query_map(params![write.idem, like], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
    let reply = match reply {
        Some(text) => serde_json::from_str(&text)?,
        None => Value::Null,
    };
    Ok(Some(Committed { seqs: seqs.into_iter().map(|s| s as Seq).collect(), reply, replayed: true }))
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}
