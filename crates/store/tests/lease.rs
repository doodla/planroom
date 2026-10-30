//! Leases (architecture.md, Duties; DR-24): an expired lease is taken over with its epoch bumped,
//! a live one refuses everyone, its own holder included, and a holder that lost its lease writes
//! nothing more: its release leaves the new holder's row, and its `gc` run stops with `store_busy`.

use std::path::Path;

use planroom_store::{Init, Lease, Store, StoreError};

fn init() -> Init {
    Init {
        workspace_id: "ws-lease".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-28T00:00:00Z".into(),
    }
}

const A: &str = r#"{"pid":101}"#;
const B: &str = r#"{"pid":202}"#;

fn row(path: &Path) -> Option<(String, i64)> {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("SELECT holder, epoch FROM leases WHERE name = 'gc'", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .ok()
}

/// Plant `n` snapshots of `repo`, each with one fact, as a scan would leave them.
fn plant(conn: &rusqlite::Connection, repo: &str, n: usize) {
    for i in 0..n {
        conn.execute(
            "INSERT INTO snapshots (repo, commit_id, branch, purpose, packs_hash) VALUES (?1, ?2, '', 'head', 'p')",
            [repo, &format!("c{i}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO facts (snapshot, id, shape, kind, hash, body) VALUES (?1, 'n', 'node', 'k', 'h', '{}')",
            [conn.last_insert_rowid()],
        )
        .unwrap();
    }
}

#[test]
fn an_expired_lease_is_taken_over_and_fences_its_old_holder() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("store.db");
    let mut store = Store::open(&path, &init()).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();

    // A's lease expired long ago, at epoch 3.
    conn.execute(
        "INSERT INTO leases (name, holder, epoch, expires_at) VALUES ('gc', ?1, 3, '2000-01-01T00:00:00.000Z')",
        [A],
    )
    .unwrap();
    let b = store.take_lease("gc", B).unwrap().expect("an expired lease is taken over");
    assert_eq!(b, Lease { name: "gc".into(), holder: B.into(), epoch: 4 });
    assert_eq!(row(&path), Some((B.to_owned(), 4)));

    // While B's lease is live, neither A nor B itself takes it.
    assert_eq!(store.take_lease("gc", A).unwrap(), None);
    assert_eq!(store.take_lease("gc", B).unwrap(), None);
    // A's late release, at its old epoch, leaves B's row.
    store.release_lease(&Lease { name: "gc".into(), holder: A.into(), epoch: 3 }).unwrap();
    assert_eq!(row(&path), Some((B.to_owned(), 4)));
    store.release_lease(&b).unwrap();
    assert_eq!(row(&path), None);

    // A `gc` run whose lease is taken over between two deletes stops with `store_busy`: after its
    // first delete, B takes the lease over at a later epoch.
    plant(&conn, "svc", 23);
    conn.execute_batch(
        r#"CREATE TRIGGER take_over AFTER DELETE ON snapshots
           BEGIN
             UPDATE leases SET holder = '{"pid":202}', epoch = epoch + 1 WHERE name = 'gc';
           END;"#,
    )
    .unwrap();
    match store.collect(A, &[]) {
        Err(StoreError::Busy) => {}
        other => panic!("expected store_busy, got {other:?}"),
    }
    let left: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 22, "the first delete went, and nothing after the takeover");
    // The new holder's row stays: A's release no longer matches it.
    let (holder, _) = row(&path).expect("the new holder's lease stays");
    assert_eq!(holder, B);
}
