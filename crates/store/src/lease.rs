//! Leases (architecture.md, Duties; DR-24): a duty runs only while its process holds the row in
//! `leases` named for it, such as `gc`.
//!
//! - [`Store::take_lease`] takes a lease whose row is missing or expired, with its epoch bumped
//!   (1 for a new row) and a 30 s expiry. A live row refuses everyone, its own holder included,
//!   so two runs in one process, which share a pid, never both hold it.
//! - Each write a duty makes renews the lease inside its own transaction ([`hold`]). A lease that
//!   is missing, expired, or taken over with another holder or epoch is lost, and the write
//!   refuses with [`StoreError::Busy`] (`store_busy`), so a holder whose lease expired, a laptop
//!   asleep, commits nothing.
//! - [`Store::release_lease`] deletes the row only while the holder and the epoch still match.
//!
//! Times are SQLite's clock, RFC 3339 UTC with milliseconds, whose strings sort in time order.
//! The holder is compact JSON, `{"pid":N}`; readers keep the fields they don't know, so a later
//! engine can add the process start time and host a dead holder's takeover needs.
//!
//! Taking, renewing and releasing a lease write bookkeeping only, never anything a person or an
//! agent authored, so they check the store version (DR-96) and nothing else.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Store, StoreError, commit};

/// A lease's expiry from SQLite's clock, 30 s from now.
const EXPIRES: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+30 seconds')";

/// Now, by SQLite's clock, in the same format.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

/// A lease this process holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub name: String,
    pub holder: String,
    pub epoch: i64,
}

/// This process as a lease's holder: `{"pid":N}`.
pub fn process_holder() -> String {
    serde_json::json!({"pid": std::process::id()}).to_string()
}

impl Store {
    /// Take the lease `name` for `holder` when no one holds it, or its holder let it expire;
    /// `None` while a live lease is held, by anyone.
    pub fn take_lease(&mut self, name: &str, holder: &str) -> Result<Option<Lease>, StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        let live: Option<bool> = tx
            .query_row(&format!("SELECT expires_at > {NOW} FROM leases WHERE name = ?1"), [name], |r| {
                r.get(0)
            })
            .optional()?;
        if live == Some(true) {
            return Ok(None);
        }
        let epoch: i64 = tx.query_row(
            &format!(
                "INSERT INTO leases (name, holder, epoch, expires_at) VALUES (?1, ?2, 1, {EXPIRES})
                 ON CONFLICT (name) DO UPDATE
                 SET holder = excluded.holder, epoch = leases.epoch + 1, expires_at = excluded.expires_at
                 RETURNING epoch"
            ),
            params![name, holder],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(Some(Lease { name: name.to_owned(), holder: holder.to_owned(), epoch }))
    }

    /// Release `lease`, unless another holder took it over meanwhile.
    pub fn release_lease(&mut self, lease: &Lease) -> Result<(), StoreError> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        commit::check_version(&tx)?;
        tx.execute(
            "DELETE FROM leases WHERE name = ?1 AND holder = ?2 AND epoch = ?3",
            params![lease.name, lease.holder, lease.epoch],
        )?;
        tx.commit()?;
        Ok(())
    }
}

/// Inside a duty's write transaction `tx`: renew `lease` for 30 s, or refuse with `store_busy`
/// when it was lost (missing, expired, or taken over).
pub(crate) fn hold(tx: &Connection, lease: &Lease) -> Result<(), StoreError> {
    let renewed = tx.execute(
        &format!(
            "UPDATE leases SET expires_at = {EXPIRES}
             WHERE name = ?1 AND holder = ?2 AND epoch = ?3 AND expires_at > {NOW}"
        ),
        params![lease.name, lease.holder, lease.epoch],
    )?;
    if renewed == 1 { Ok(()) } else { Err(StoreError::Busy) }
}
