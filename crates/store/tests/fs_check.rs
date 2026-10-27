//! The filesystem check at store open and the store's file modes (architecture.md, Where the
//! workspace lives; DR-1, DR-29).
//!
//! The filesystem type comes through the store's seam: this test binary registers a data root
//! and a type function that calls a directory `nfs` when it sits under a place the running test
//! marked unsafe on its own thread, `ext4` otherwise, and knows no type for a path that doesn't
//! exist, as `statfs` doesn't. The tests share the registration and the store's notices, so they
//! run one at a time.

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use planroom_contract::{Actor, Client, EventBody, EventId, Status, StorePointer, Surface, Timestamp};
use planroom_store::{Environment, Init, NewEvent, Notice, Placement, Store, StoreError, Tail, Write};
use serde_json::json;

thread_local! {
    /// The places the running test made unsafe.
    static UNSAFE: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

fn fake_fs_type(dir: &Path) -> Option<String> {
    if !dir.exists() {
        return None;
    }
    let unsafe_here = UNSAFE.with(|u| u.borrow().iter().any(|p| dir.starts_with(p)));
    Some(if unsafe_here { "nfs" } else { "ext4" }.into())
}

/// Mark `place` as on a network filesystem, for this test's thread.
fn make_unsafe(place: &Path) {
    let place = fs::canonicalize(place).unwrap();
    UNSAFE.with(|u| u.borrow_mut().push(place));
}

/// The registered data root, created afresh when a test removed it, and a guard that keeps the
/// other tests out until this one ends.
fn setup() -> (&'static Path, MutexGuard<'static, ()>) {
    static SERIAL: Mutex<()> = Mutex::new(());
    static DATA: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_, root) = DATA.get_or_init(|| {
        let parent = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(parent.path()).unwrap().join("data");
        planroom_store::register(Environment {
            data_root: root.clone(),
            fs_type: fake_fs_type,
            engine_version: "0.2.0".into(),
        });
        (parent, root)
    });
    fs::DirBuilder::new().recursive(true).mode(0o700).create(root).unwrap();
    UNSAFE.with(|u| u.borrow_mut().clear());
    planroom_store::take_notices();
    (root, guard)
}

/// A workspace state directory, mode 0700, under `parent`.
fn state_dir(parent: &Path) -> PathBuf {
    let dir = parent.join("ws").join(".planroom");
    fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).unwrap();
    fs::canonicalize(dir).unwrap()
}

fn init(id: &str) -> Init {
    Init {
        workspace_id: id.into(),
        engine_version: "0.1.0".into(),
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}

fn commit(store: &mut Store, n: usize) {
    let write = Write {
        idem: format!("w{n}"),
        request_hash: "h".into(),
        events: vec![NewEvent {
            id: EventId(format!("e{n}")),
            at: Timestamp("2026-10-26T00:00:00Z".into()),
            actor: Actor::System { part: "test".into() },
            client: Client { surface: Surface::Test, id: "test".into() },
            project: None,
            cause: None,
            status: Status::Applied,
            target: None,
            body: EventBody::Version { project: None, label: format!("v{n}") },
        }],
    };
    store.commit(write, |_| json!({})).unwrap();
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn unsafe_filesystem_moves_the_store() {
    let (data, _serial) = setup();
    let mount = tempfile::tempdir().unwrap();
    let state = state_dir(mount.path());
    make_unsafe(mount.path());
    let asked = state.join("store.db");

    let mut store = Store::open(&asked, &init("ws-moved")).unwrap();
    let moved = fs::canonicalize(data).unwrap().join("stores/ws-moved/store.db");
    assert_eq!(store.path(), moved, "the new store lives under the data root");
    assert!(!asked.exists(), "nothing was created on the unsafe filesystem but the pointer");
    commit(&mut store, 1);
    drop(store);

    let pointer: StorePointer =
        planroom_contract::format::read_str(&fs::read_to_string(state.join("store.pointer")).unwrap())
            .unwrap();
    assert_eq!(pointer.format, "planroom.store-pointer/v1");
    assert_eq!(pointer.workspace_id, "ws-moved");
    assert_eq!(Path::new(&pointer.path), moved);
    assert_eq!(mode(&state.join("store.pointer")), 0o600);

    let notices = planroom_store::take_notices();
    assert_eq!(
        notices,
        vec![Notice::UnsafeFilesystem {
            placement: Placement::Moved,
            path: state.clone(),
            fs: "nfs".into(),
            store: moved.clone(),
        }],
        "it says so once, naming both places"
    );

    // Every later open follows the pointer: a read, the live loop's tail and a scan's open.
    // A process killed while it published a pointer left its temporary file behind; one that is
    // publishing now has a fresh one. The next open sweeps only the stale one.
    let stale = state.join(".store.pointer.4242-0.tmp");
    let fresh = state.join(".store.pointer.4243-0.tmp");
    for tmp in [&stale, &fresh] {
        fs::write(tmp, "{}").unwrap();
    }
    let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    fs::File::options().write(true).open(&stale).unwrap().set_modified(hour_ago).unwrap();

    let reopened = Store::open_existing(&asked).unwrap().expect("the pointer leads to the store");
    assert_eq!(reopened.path(), moved);
    assert_eq!(reopened.seq().unwrap(), 1);
    assert_eq!(Tail::open(&asked).unwrap().expect("the tail follows it too").seq().unwrap(), 1);
    let mut again = Store::open(&asked, &init("ws-other")).unwrap();
    assert_eq!(again.path(), moved, "a second open never creates a second store");
    assert_eq!(again.meta("workspace_id").unwrap().as_deref(), Some("ws-moved"));
    commit(&mut again, 2);
    assert!(!asked.exists());
    assert!(!stale.exists(), "a killed process's temporary pointer file is swept");
    assert!(fresh.exists(), "a temporary file another process may still link is left alone");
    assert_eq!(planroom_store::take_notices(), vec![], "the moved store sits on a safe filesystem");
}

#[test]
fn an_unsafe_data_root_is_reported() {
    let (data, _serial) = setup();
    let mount = tempfile::tempdir().unwrap();
    let state = state_dir(mount.path());
    make_unsafe(mount.path());
    make_unsafe(data);
    let asked = state.join("store.db");

    let store = Store::open(&asked, &init("ws-nowhere")).unwrap();
    assert_eq!(store.path(), asked, "with nowhere safer to go, the store stays where it is");
    assert!(!state.join("store.pointer").exists());
    assert!(!data.join("stores/ws-nowhere").exists());
    drop(store);
    assert_eq!(
        planroom_store::take_notices(),
        vec![Notice::UnsafeFilesystem {
            placement: Placement::DataRoot,
            path: data.to_path_buf(),
            fs: "nfs".into(),
            store: asked.clone(),
        }],
        "one warning names the data root and where the store stays"
    );

    // An existing store on an unsafe filesystem stays, and each open warns about it.
    let store = Store::open_existing(&asked).unwrap().unwrap();
    assert_eq!(store.path(), asked);
    assert_eq!(
        planroom_store::take_notices(),
        vec![Notice::UnsafeFilesystem {
            placement: Placement::Kept,
            path: state.clone(),
            fs: "nfs".into(),
            store: asked.clone(),
        }]
    );
    drop(store);

    // A data root not created yet is checked where it will be: on the unsafe place above it.
    fs::remove_dir_all(data).unwrap();
    make_unsafe(data.parent().unwrap());
    let other = tempfile::tempdir().unwrap();
    let state = state_dir(other.path());
    make_unsafe(other.path());
    let asked = state.join("store.db");
    let store = Store::open(&asked, &init("ws-unborn-root")).unwrap();
    assert_eq!(store.path(), asked, "a data root on an unsafe place takes no store, created or not");
    assert!(!state.join("store.pointer").exists());
    assert!(!data.join("stores/ws-unborn-root").exists());
    drop(store);
    assert_eq!(
        planroom_store::take_notices(),
        vec![Notice::UnsafeFilesystem {
            placement: Placement::DataRoot,
            path: data.to_path_buf(),
            fs: "nfs".into(),
            store: asked.clone(),
        }]
    );
}

#[test]
fn store_files_are_0600_in_a_0700_directory() {
    let (data, _serial) = setup();

    // A store moved under the data root: the directories the store creates are 0700.
    let mount = tempfile::tempdir().unwrap();
    let state = state_dir(mount.path());
    make_unsafe(mount.path());
    let mut moved = Store::open(&state.join("store.db"), &init("ws-modes")).unwrap();
    commit(&mut moved, 1);
    let dir = data.join("stores/ws-modes");
    assert_eq!(mode(&data.join("stores")), 0o700);
    assert_eq!(mode(&dir), 0o700);
    for file in ["store.db", "store.db-wal", "store.db-shm"] {
        assert_eq!(mode(&dir.join(file)), 0o600, "{file}");
    }

    // A store in its state directory, where an older engine left its files wider: opening it
    // narrows them. The state directory is the app's, created 0700; the store leaves its mode.
    UNSAFE.with(|u| u.borrow_mut().clear());
    let local = tempfile::tempdir().unwrap();
    let state = state_dir(local.path());
    let path = state.join("store.db");
    let mut store = Store::open(&path, &init("ws-local")).unwrap();
    commit(&mut store, 1);
    drop(store);
    for file in ["store.db", "store.db-wal", "store.db-shm"] {
        if state.join(file).exists() {
            fs::set_permissions(state.join(file), fs::Permissions::from_mode(0o644)).unwrap();
        }
    }
    let mut store = Store::open_existing(&path).unwrap().unwrap();
    commit(&mut store, 2);
    for file in ["store.db", "store.db-wal", "store.db-shm"] {
        assert_eq!(mode(&state.join(file)), 0o600, "{file}");
    }
    planroom_store::take_notices();
}

#[test]
fn engines_creating_a_moved_store_at_once_agree_on_one() {
    let (data, _serial) = setup();
    let mount = tempfile::tempdir().unwrap();
    let state = state_dir(mount.path());
    let mount = mount.path().to_path_buf();
    let asked = state.join("store.db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let engines: Vec<_> = (0..4)
        .map(|n| {
            let (mount, asked, barrier) = (mount.clone(), asked.clone(), barrier.clone());
            std::thread::spawn(move || {
                make_unsafe(&mount);
                barrier.wait();
                let mut store = Store::open(&asked, &init(&format!("ws-race-{n}"))).unwrap();
                commit(&mut store, n);
                (store.path().to_path_buf(), store.meta("workspace_id").unwrap().unwrap())
            })
        })
        .collect();
    let opened: Vec<_> = engines.into_iter().map(|t| t.join().unwrap()).collect();
    let (path, id) = opened[0].clone();
    assert!(opened.iter().all(|o| *o == (path.clone(), id.clone())), "one store, one workspace: {opened:?}");
    assert_eq!(path, fs::canonicalize(data).unwrap().join("stores").join(&id).join("store.db"));
    let pointer: StorePointer =
        planroom_contract::format::read_str(&fs::read_to_string(state.join("store.pointer")).unwrap())
            .unwrap();
    assert_eq!(pointer.workspace_id, id);
    assert_eq!(
        Store::open_existing(&asked).unwrap().unwrap().seq().unwrap(),
        4,
        "every engine's commit landed there"
    );
    let stores: Vec<_> = fs::read_dir(data.join("stores")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(
        stores.iter().filter(|s| s.to_string_lossy().starts_with("ws-race-")).count() == 1,
        "no second store was left under the data root: {stores:?}"
    );
    planroom_store::take_notices();
}

/// Write a pointer by hand, as a crafted state directory would hold one.
fn plant_pointer(state: &Path, path: &str, workspace_id: &str) {
    let pointer = StorePointer::new(path.into(), workspace_id.into());
    fs::write(state.join("store.pointer"), serde_json::to_string(&pointer).unwrap()).unwrap();
}

#[test]
fn a_pointer_is_followed_only_to_its_place_under_the_data_root() {
    let (data, _serial) = setup();
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = elsewhere.path().join("store.db");
    drop(Store::open(&outside, &init("ws-outside")).unwrap());

    let local = tempfile::tempdir().unwrap();
    let state = state_dir(local.path());
    let asked = state.join("store.db");
    let refused = |what: &str| {
        let open = Store::open_existing(&asked).err();
        let tail = Tail::open(&asked).err();
        let create = Store::open(&asked, &init("ws-x")).err();
        for e in [open, tail, create] {
            match e {
                Some(StoreError::Io(why)) => assert!(why.contains("won't open"), "{what}: {why}"),
                other => panic!("{what}: a pointer there must be refused, got {other:?}"),
            }
        }
        assert!(!asked.exists(), "{what}: no store is created beside a refused pointer");
    };

    // A store outside the data root.
    plant_pointer(&state, &outside.display().to_string(), "ws-outside");
    refused("outside the data root");
    // The right shape, but a workspace ID that climbs out.
    let climbing = data.join("stores/../../store.db");
    plant_pointer(&state, &climbing.display().to_string(), "../..");
    refused("a workspace ID with ..");
    // Its place under the data root, reached through a symbolic link that leads outside.
    fs::create_dir_all(data.join("stores")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), data.join("stores/ws-linked")).unwrap();
    let linked = data.join("stores/ws-linked/store.db");
    plant_pointer(&state, &linked.display().to_string(), "ws-linked");
    refused("a symbolic link under the data root");
    fs::remove_file(data.join("stores/ws-linked")).unwrap();

    // A newer pointer format is schema_mismatch naming the file.
    let newer = json!({"format": "planroom.store-pointer/v9", "path": "x", "workspace_id": "w"});
    fs::write(state.join("store.pointer"), newer.to_string()).unwrap();
    match Store::open_existing(&asked) {
        Err(StoreError::NewerFile { file, .. }) => assert_eq!(file, state.join("store.pointer")),
        other => panic!("a newer pointer is schema_mismatch, got {:?}", other.err()),
    }
    planroom_store::take_notices();
}

#[test]
fn sync_folders_and_wsl_drives_count_as_unsafe() {
    use planroom_store::fs_check::classify;
    for (fs, path) in [
        ("nfs4", "/home/ann/ws"),
        ("cifs", "/srv/share/ws"),
        ("fuse.sshfs", "/home/ann/remote"),
        ("9p", "/mnt/c/Users/ann/ws"),
        ("ext4", "/mnt/d/ws"),
        ("apfs", "/Users/ann/Dropbox/ws"),
        ("apfs", "/Users/ann/Library/Mobile Documents/com~apple~CloudDocs/ws"),
        ("apfs", "/Users/ann/Library/CloudStorage/OneDrive-Personal/ws"),
        ("ext4", "/home/ann/OneDrive - Contoso/ws"),
    ] {
        assert!(classify(fs, Path::new(path)).is_some(), "{fs} at {path}");
    }
    for (fs, path) in [
        ("ext4", "/home/ann/ws"),
        ("apfs", "/Users/ann/src/dropbox-client"),
        ("ext4", "/mnt/data/ws"),
        ("btrfs", "/home/ann/Library/ws"),
    ] {
        assert!(classify(fs, Path::new(path)).is_none(), "{fs} at {path}");
    }
}
