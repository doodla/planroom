//! Reads: the active snapshot set, its facts and the event log.

use std::collections::BTreeMap;

use planroom_contract::{Event, EventBody, EventId, NodeId, Seq, Timestamp};
use rusqlite::{Connection, Row, params};

use crate::{Store, StoreError};

/// Which of the four fact shapes a row holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Shape {
    Node,
    Edge,
    Hint,
}

impl Shape {
    pub const fn as_str(self) -> &'static str {
        match self {
            Shape::Node => "node",
            Shape::Edge => "edge",
            Shape::Hint => "hint",
        }
    }

    fn parse(s: &str) -> Result<Shape, StoreError> {
        match s {
            "node" => Ok(Shape::Node),
            "edge" => Ok(Shape::Edge),
            "hint" => Ok(Shape::Hint),
            other => Err(StoreError::Sqlite(format!("unknown fact shape `{other}`"))),
        }
    }
}

/// One stored fact: its ID, shape, kind, the sha256 of its body, and the body as JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactRow {
    pub id: String,
    pub shape: Shape,
    pub kind: String,
    pub hash: String,
    pub body: String,
}

/// A repo's active snapshot and the `scan` event that made it active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Active {
    pub snapshot: u64,
    pub seq: Seq,
}

/// The snapshots a fold reads at the latest `seq`: each repo's active one, plus the link.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActiveSet {
    /// Repo name to its active snapshot, sorted by repo.
    pub repos: BTreeMap<String, u64>,
    /// The link snapshot the last `scan` event names.
    pub link: Option<u64>,
}

impl ActiveSet {
    /// Every snapshot in the set, sorted.
    pub fn snapshots(&self) -> Vec<u64> {
        let mut all: Vec<u64> = self.repos.values().copied().chain(self.link).collect();
        all.sort_unstable();
        all
    }
}

impl Store {
    /// The active snapshot set at the latest `seq`.
    pub fn active_set(&self) -> Result<ActiveSet, StoreError> {
        let repos = active_snapshots(&self.conn)?.into_iter().map(|(repo, a)| (repo, a.snapshot)).collect();
        let link = self
            .conn
            .query_row("SELECT body FROM events WHERE kind = 'scan' ORDER BY seq DESC LIMIT 1", [], |r| {
                r.get::<_, String>(0)
            })
            .map(Some)
            .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })?
            .map(|body| scan_body(&body).map(|(_, link)| link))
            .transpose()?;
        Ok(ActiveSet { repos, link })
    }

    /// Every fact of the given snapshots, sorted by snapshot then ID.
    pub fn facts(&self, snapshots: &[u64]) -> Result<Vec<FactRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, shape, kind, hash, body FROM facts WHERE snapshot = ?1 ORDER BY id")?;
        let mut out = Vec::new();
        for &snapshot in snapshots {
            let rows = stmt.query_map(params![snapshot as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?;
            for row in rows {
                let (id, shape, kind, hash, body) = row?;
                out.push(FactRow { id, shape: Shape::parse(&shape)?, kind, hash, body });
            }
        }
        Ok(out)
    }

    /// Events with `seq > after`, at most `limit`, in order.
    pub fn events_after(&self, after: Seq, limit: u32) -> Result<Vec<Event>, StoreError> {
        events_after(&self.conn, after, limit)
    }
}

/// Each repo's last `scan` event: the snapshot it made active.
pub(crate) fn active_snapshots(conn: &Connection) -> Result<BTreeMap<String, Active>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT seq, body FROM events WHERE seq IN (SELECT MAX(seq) FROM events WHERE kind = 'scan' GROUP BY target)",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (seq, body) = row?;
        let EventBody::Scan { repo, snapshot, .. } = serde_json::from_str(&body)? else {
            return Err(StoreError::Sqlite(format!("event {seq} is not a scan")));
        };
        out.insert(repo, Active { snapshot, seq: seq as Seq });
    }
    Ok(out)
}

fn scan_body(body: &str) -> Result<(u64, u64), StoreError> {
    match serde_json::from_str(body)? {
        EventBody::Scan { snapshot, link, .. } => Ok((snapshot, link)),
        _ => Err(StoreError::Sqlite("a scan row holds another event body".into())),
    }
}

pub(crate) fn max_seq(conn: &Connection) -> Result<Seq, StoreError> {
    let seq: Option<i64> = conn.query_row("SELECT MAX(seq) FROM events", [], |r| r.get(0))?;
    Ok(seq.unwrap_or(0) as Seq)
}

pub(crate) fn events_after(conn: &Connection, after: Seq, limit: u32) -> Result<Vec<Event>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT seq, id, idem, request_hash, at, actor, client, project, cause, status, body
         FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![after as i64, limit], raw_event)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?.decode()?);
    }
    Ok(out)
}

struct RawEvent {
    seq: i64,
    id: String,
    idem: String,
    request_hash: String,
    at: String,
    actor: String,
    client: String,
    project: Option<String>,
    cause: Option<String>,
    status: String,
    body: String,
}

fn raw_event(r: &Row<'_>) -> rusqlite::Result<RawEvent> {
    Ok(RawEvent {
        seq: r.get(0)?,
        id: r.get(1)?,
        idem: r.get(2)?,
        request_hash: r.get(3)?,
        at: r.get(4)?,
        actor: r.get(5)?,
        client: r.get(6)?,
        project: r.get(7)?,
        cause: r.get(8)?,
        status: r.get(9)?,
        body: r.get(10)?,
    })
}

impl RawEvent {
    fn decode(self) -> Result<Event, StoreError> {
        Ok(Event {
            seq: self.seq as Seq,
            id: EventId(self.id),
            idem: self.idem,
            request_hash: self.request_hash,
            at: Timestamp(self.at),
            actor: serde_json::from_str(&self.actor)?,
            client: serde_json::from_str(&self.client)?,
            project: self.project.map(NodeId),
            cause: self.cause.as_deref().map(serde_json::from_str).transpose()?,
            status: serde_json::from_value(serde_json::Value::String(self.status))?,
            body: serde_json::from_str(&self.body)?,
        })
    }
}
