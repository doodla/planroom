//! `store::commit`: every write is one `BEGIN IMMEDIATE` transaction with the idempotency check:
//! one command's events ([`write`]), or a scan of every repo of a workspace ([`scan`]).

mod scan;
mod write;

use planroom_contract::Seq;
use rusqlite::{OptionalExtension, Transaction, params};

use crate::StoreError;

pub use scan::{RepoOutcome, RepoScan, ScanCommitted, ScanKey, ScanRules, ScanWrite};
pub use write::{Committed, NewEvent, Write};

fn idem_exists(tx: &Transaction<'_>, idem: &str) -> Result<bool, StoreError> {
    Ok(tx.query_row("SELECT 1 FROM events WHERE idem = ?1", [idem], |_| Ok(())).optional()?.is_some())
}

fn insert_event(
    tx: &Transaction<'_>,
    event: &NewEvent,
    idem: &str,
    request_hash: &str,
) -> Result<Seq, StoreError> {
    let status = serde_json::to_value(event.status)?;
    tx.execute(
        "INSERT INTO events (id, idem, request_hash, at, actor, client, project, cause, kind, target, status, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event.id.0,
            idem,
            request_hash,
            event.at.0,
            serde_json::to_string(&event.actor)?,
            serde_json::to_string(&event.client)?,
            event.project.as_ref().map(|p| p.0.as_str()),
            event.cause.as_ref().map(serde_json::to_string).transpose()?,
            event.body.kind().as_str(),
            event.target,
            status.as_str().unwrap_or_default(),
            serde_json::to_string(&event.body)?,
        ],
    )?;
    Ok(tx.last_insert_rowid() as Seq)
}
