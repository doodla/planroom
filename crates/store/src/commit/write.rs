//! One command's events, appended under one idempotency key.

use planroom_contract::{Actor, Cause, Client, EventBody, EventId, NodeId, Seq, Status, Timestamp};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;

use super::{Dealt, check_version, insert_event};
use crate::read::Snapshot;
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

/// One event a planned write appends: one of the command's own, keyed `idem`, or `idem#1`,
/// `idem#2` and so on, or one under a key of its own, in the same transaction, such as a card the
/// write deals or a card's own review. An undo reverses a command by its key, so an undo of the
/// command leaves an event under its own key standing, and an undo naming that event reverses it
/// alone (DR-61).
#[derive(Debug, Clone)]
pub enum Planned {
    Command(NewEvent),
    Own(Dealt),
}

/// What a commit wrote, or what a retry found.
#[derive(Debug, Clone, PartialEq)]
pub struct Committed {
    /// The command's own events' `seq`s, in order.
    pub seqs: Vec<Seq>,
    /// The `seq`s of the events it wrote under keys of their own, in order; none on a replay.
    pub own: Vec<Seq>,
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
        let Write { idem, request_hash, events } = write;
        self.commit_planned(&idem, &request_hash, |_| Ok::<_, StoreError>(events), reply)
    }

    /// [`Store::commit`] for a write whose events depend on the store as the commit finds it: a
    /// write checked against what a person and other writers wrote since its base. In one
    /// IMMEDIATE transaction it checks the store version, then the key, and only then calls
    /// `plan` with a [`Snapshot`] read inside that same transaction, so no other writer commits
    /// between what `plan` read and the events it returns (DR-5, DR-61, DR-73). An error from
    /// `plan` writes nothing. The store holds no rules: `plan` is the caller's, which folds and
    /// checks with the kernel.
    pub fn commit_planned<E: From<StoreError>>(
        &mut self,
        idem: &str,
        request_hash: &str,
        plan: impl FnOnce(&Snapshot<'_>) -> Result<Vec<NewEvent>, E>,
        reply: impl FnOnce(&[Seq]) -> Value,
    ) -> Result<Committed, E> {
        let plan = |snapshot: &Snapshot<'_>| Ok(plan(snapshot)?.into_iter().map(Planned::Command).collect());
        self.commit_keyed(idem, request_hash, plan, reply)
    }

    /// [`Store::commit_planned`] for a write whose plan also appends events under keys of their
    /// own ([`Planned::Own`]), in the same transaction and in the order the plan gives them. At
    /// least one event is the command's own, which holds the reply. A retry under the command's
    /// key replays before `plan` runs, so one command never writes an own key twice. The store
    /// holds no rules: which events take a key of their own is the caller's to say.
    pub fn commit_keyed<E: From<StoreError>>(
        &mut self,
        idem: &str,
        request_hash: &str,
        plan: impl FnOnce(&Snapshot<'_>) -> Result<Vec<Planned>, E>,
        reply: impl FnOnce(&[Seq]) -> Value,
    ) -> Result<Committed, E> {
        let tx =
            self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(StoreError::from)?;
        check_version(&tx)?;
        if let Some(found) = replay(&tx, idem, request_hash)? {
            return Ok(found);
        }
        let snapshot = Snapshot::over(tx)?;
        let events = plan(&snapshot)?;
        let count = events.iter().filter(|e| matches!(e, Planned::Command(_))).count();
        assert!(count > 0, "a commit writes at least one event of its own command");
        let many = count > 1;
        let (mut seqs, mut own) = (Vec::with_capacity(count), Vec::new());
        for planned in &events {
            match planned {
                Planned::Command(event) => {
                    let key = if many { format!("{idem}#{}", seqs.len() + 1) } else { idem.to_owned() };
                    seqs.push(insert_event(snapshot.tx(), event, &key, request_hash)?);
                }
                Planned::Own(keyed) => {
                    own.push(insert_event(snapshot.tx(), &keyed.event, &keyed.idem, &keyed.request_hash)?);
                }
            }
        }
        let reply = reply(&seqs);
        snapshot
            .tx()
            .execute(
                "UPDATE events SET reply = ?1 WHERE seq = ?2",
                params![reply.to_string(), seqs[0] as i64],
            )
            .map_err(StoreError::from)?;
        snapshot.commit()?;
        let last = seqs.iter().chain(&own).copied().max().expect("one event at least");
        self.notify(last);
        Ok(Committed { seqs, own, reply, replayed: false })
    }
}

/// The stored reply when `idem` was committed before with the same request hash.
fn replay(tx: &Transaction<'_>, idem: &str, request_hash: &str) -> Result<Option<Committed>, StoreError> {
    let found = tx
        .query_row(
            "SELECT seq, id, request_hash, reply FROM events WHERE idem = ?1 OR idem = ?1 || '#1'",
            [idem],
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
    if hash != request_hash {
        return Err(StoreError::IdempotencyMismatch {
            idem: idem.to_owned(),
            stored_event: id,
            stored_seq: seq as Seq,
        });
    }
    // The command's own events: the key itself, or the key and a numbered suffix, `#1`, `#2` and
    // so on. Any other key that starts with this one, such as a scan's `KEY#after:SEQ`, belongs to
    // another write.
    let mut stmt = tx.prepare(
        "SELECT seq FROM events WHERE idem = ?1 OR (substr(idem, 1, length(?1) + 1) = ?1 || '#'
         AND length(idem) > length(?1) + 1 AND substr(idem, length(?1) + 2) NOT GLOB '*[^0-9]*')
         ORDER BY seq",
    )?;
    let seqs = stmt.query_map(params![idem], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
    let reply = match reply {
        Some(text) => serde_json::from_str(&text)?,
        None => Value::Null,
    };
    Ok(Some(Committed {
        seqs: seqs.into_iter().map(|s| s as Seq).collect(),
        own: Vec::new(),
        reply,
        replayed: true,
    }))
}
