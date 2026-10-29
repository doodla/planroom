//! The pack record and its guard (DR-62): every authored write runs the store's guard on the
//! record it finds under the write lock. A `Refuse` writes nothing; a `Record` commits the record
//! with the write or not at all; and `record_packs` builds on the record as the lock finds it. A
//! store with no guard checks nothing. The store holds no rules: each verdict here is the test's.

use planroom_contract::{Actor, Client, EventBody, EventId, PackRecord, Status, Surface, Timestamp};
use planroom_store::{Init, NewEvent, PackVerdict, RecordedPacks, Store, StoreError, Write};
use serde_json::json;

fn store(dir: &tempfile::TempDir) -> Store {
    let init = Init {
        workspace_id: "packs".into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-28T00:00:00Z".into(),
    };
    Store::open(&dir.path().join("store.db"), &init).unwrap()
}

fn write(idem: &str, hash: &str) -> Write {
    Write {
        idem: idem.into(),
        request_hash: hash.into(),
        events: vec![NewEvent {
            id: EventId(format!("ev-{idem}")),
            at: Timestamp("2026-10-28T00:00:00Z".into()),
            actor: Actor::Person { id: "person:ann".into(), name: None },
            client: Client { surface: Surface::Test, id: "tab_1".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: idem.into() },
        }],
    }
}

fn record(engine: &str, packs: &[&str]) -> RecordedPacks {
    let packs: Vec<PackRecord> = packs
        .iter()
        .map(|id| PackRecord {
            id: (*id).into(),
            version: 1,
            source: format!("/src/{id}"),
            sha256: "a".repeat(64),
        })
        .collect();
    RecordedPacks {
        hash: format!("hash-{engine}-{}", packs.len()),
        packs,
        engine: Some(engine.into()),
        config_read: None,
    }
}

#[test]
fn a_store_with_no_guard_checks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(&dir);
    store.commit(write("one", "h1"), |_| json!({})).unwrap();
    assert_eq!(store.recorded_packs().unwrap(), None, "nothing recorded the set");
    assert_eq!(store.pack_verdict().unwrap(), PackVerdict::Keep);
}

#[test]
fn a_refused_write_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(&dir);
    store.guard_packs(Box::new(|_| PackVerdict::Refuse {
        recorded: Some("9.0.0".into()),
        resolved: "1.0.0".into(),
    }));
    let refused = store.commit(write("one", "h1"), |_| json!({})).unwrap_err();
    assert!(matches!(refused, StoreError::PacksMismatch { .. }), "{refused}");
    assert_eq!(refused.code(), planroom_contract::ErrorCode::PacksMismatch);
    assert_eq!(refused.details(), json!({"recorded": "9.0.0", "resolved": "1.0.0"}));
    assert_eq!(store.seq().unwrap(), 0, "nothing was written");
    assert!(matches!(store.spend_token("d", 1, 2), Err(StoreError::PacksMismatch { .. })));
    assert!(matches!(store.meta_swap("k", None, "v", &[]), Err(StoreError::PacksMismatch { .. })));
    // A cache is no authored write: a layout is kept whatever the guard says.
    store.put_layout("map:system:all", "s", "{}").unwrap();
}

#[test]
fn a_recorded_set_commits_with_its_write_or_not_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(&dir);
    let ours = record("1.0.0", &["ruby"]);
    let mine = ours.clone();
    store.guard_packs(Box::new(move |found| match found {
        Some(found) if *found == mine => PackVerdict::Keep,
        _ => PackVerdict::Record(mine.clone()),
    }));
    // A write that fails after the check, here on a reused key, leaves the record as it was.
    store.commit(write("one", "h1"), |_| json!({})).unwrap();
    assert_eq!(store.recorded_packs().unwrap().as_ref(), Some(&ours));
    let mut other = Store::open_existing(&dir.path().join("store.db")).unwrap().unwrap();
    other.record_packs(|_| Ok::<_, StoreError>(Some(record("0.9.0", &[])))).unwrap();
    assert!(store.commit(write("one", "h-other"), |_| json!({})).is_err());
    assert_eq!(
        store.recorded_packs().unwrap(),
        Some(record("0.9.0", &[])),
        "the failed write recorded nothing"
    );
    store.commit(write("two", "h2"), |_| json!({})).unwrap();
    assert_eq!(store.recorded_packs().unwrap(), Some(ours));
}

#[test]
fn record_packs_builds_on_the_record_the_lock_finds() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = store(&dir);
    store
        .record_packs(|found| {
            assert_eq!(found, None);
            Ok::<_, StoreError>(Some(record("1.0.0", &["ruby"])))
        })
        .unwrap();
    // Another process adds a pack between this one's read and its write.
    let read = store.recorded_packs().unwrap();
    let mut other = Store::open_existing(&dir.path().join("store.db")).unwrap().unwrap();
    other.record_packs(|_| Ok::<_, StoreError>(Some(record("1.0.0", &["ruby", "flags"])))).unwrap();
    let after = store
        .record_packs(|found| {
            assert_ne!(found, read.as_ref(), "the lock finds the newer record");
            assert_eq!(found.unwrap().packs.len(), 2);
            Ok::<_, StoreError>(None)
        })
        .unwrap();
    assert_eq!(
        after,
        Some(record("1.0.0", &["ruby", "flags"])),
        "None writes nothing beyond what the lock found"
    );
    // An engine absent from the record is written as absent.
    store
        .record_packs(|_| Ok::<_, StoreError>(Some(RecordedPacks { engine: None, ..record("1.0.0", &[]) })))
        .unwrap();
    assert_eq!(store.meta("packs_engine").unwrap(), None);
    // A refusal holds for this call too, and writes nothing.
    store.guard_packs(Box::new(|_| PackVerdict::Refuse { recorded: None, resolved: "1.0.0".into() }));
    let refused = store.record_packs(|_| Ok::<_, StoreError>(Some(record("2.0.0", &[])))).unwrap_err();
    assert!(matches!(refused, StoreError::PacksMismatch { .. }));
    assert_eq!(store.recorded_packs().unwrap().unwrap().engine, None);
}
