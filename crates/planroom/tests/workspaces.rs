//! `workspaces.json` and `serve.json` under the versioning rules (contracts.md, Versioning rules;
//! DR-78): unknown fields survive a rewrite, a newer `format` or a `min_reader_version` above this
//! engine is refused with `schema_mismatch` and the file is left as it was, and two engines
//! recording at once lose nothing.
//!
//! The concurrent recorders are this test binary run again with `RECORD_CHILD_DATA` set.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use planroom::app::data::{self, WorkspaceEntry};
use planroom::server::ServeFile;
use planroom_contract::format::{self, FormatError, ReadError};
use planroom_testkit::fixture;
use serde_json::{Value, json};

const RECORDS: u16 = 40;

fn planroom(root: &Path, data: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", data);
    cmd
}

/// A scanned fixture workspace, and a data root beside it.
fn workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    let data = tmp.path().join("data");
    fixture(&root);
    assert!(planroom(&root, &data).args(["scan", "--json"]).output().unwrap().status.success());
    (tmp, root, data)
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port()
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// One recorder process: wait for the start file, then record `RECORDS` workspaces of its own.
#[test]
fn record_child() {
    let Ok(data) = std::env::var("RECORD_CHILD_DATA") else { return };
    let name = std::env::var("RECORD_CHILD_NAME").unwrap();
    let start = PathBuf::from(std::env::var("RECORD_CHILD_START").unwrap());
    while !start.exists() {
        std::thread::sleep(Duration::from_millis(1));
    }
    for i in 0..RECORDS {
        let entry = WorkspaceEntry::new(format!("/{name}/{i}"), format!("w-{name}-{i}"), 20_000 + i);
        data::record(Path::new(&data), entry).unwrap();
    }
}

#[test]
fn two_engines_recording_at_once_lose_no_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let start = tmp.path().join("start");
    let children: Vec<_> = ["a", "b", "c"]
        .iter()
        .map(|name| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "record_child", "--nocapture", "--test-threads=1"])
                .env("RECORD_CHILD_DATA", &data)
                .env("RECORD_CHILD_NAME", name)
                .env("RECORD_CHILD_START", &start)
                .spawn()
                .unwrap()
        })
        .collect();
    std::fs::write(&start, "").unwrap();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let all = data::workspaces(&data).unwrap();
    assert_eq!(all.len(), 3 * usize::from(RECORDS), "every record survives");
}

#[test]
fn unknown_fields_survive_a_serve_detach() {
    let (_tmp, root, data) = workspace();
    let root_key = std::fs::canonicalize(&root).unwrap().display().to_string();
    let old_port = free_port();
    std::fs::create_dir_all(&data).unwrap();
    let file = data.join("workspaces.json");
    std::fs::write(
        &file,
        json!({
            "format": "planroom.workspaces/v1",
            "min_reader_version": "0.1.0-dev",
            "future_key": {"kept": [1, 2]},
            "workspaces": [
                {"root": "/elsewhere", "workspace_id": "w_other", "port": 41001, "label": "keep me"},
                {"root": root_key, "workspace_id": "w_shop", "port": old_port, "trusted_at": "2026-10-26"},
            ],
        })
        .to_string(),
    )
    .unwrap();

    // --port moves the workspace, so serve records it again.
    let port = free_port();
    let out = planroom(&root, &data)
        .args(["serve", "--detach", "--json", "--port", &port.to_string()])
        .output()
        .unwrap();
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["ok"], true, "{env}");
    let pid = env["result"]["pid"].as_u64().unwrap().to_string();

    let after = read(&file);
    assert_eq!(after["format"], "planroom.workspaces/v1");
    assert_eq!(after["min_reader_version"], "0.1.0-dev");
    assert_eq!(after["future_key"], json!({"kept": [1, 2]}));
    let entries = after["workspaces"].as_array().unwrap();
    let other = entries.iter().find(|w| w["root"] == "/elsewhere").unwrap();
    assert_eq!(other["label"], "keep me");
    let shop = entries.iter().find(|w| w["root"] == root_key.as_str()).unwrap();
    assert_eq!(shop["port"], port);
    assert_eq!(shop["trusted_at"], "2026-10-26", "the entry's own unknown field survives its rewrite");

    // serve.json keeps the versioning rules too: an unknown field survives a read and write.
    let serve_json = root.join(".planroom/serve.json");
    let mut doc = read(&serve_json);
    assert_eq!(doc["format"], "planroom.serve/v1");
    doc["added_later"] = json!(true);
    let parsed: ServeFile = format::read(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), doc);
    doc["format"] = json!("planroom.serve/v2");
    assert!(matches!(format::read::<ServeFile>(doc), Err(ReadError::Format(FormatError::Newer { .. }))));

    assert!(Command::new("kill").arg(&pid).status().unwrap().success());
}

#[test]
fn an_engine_older_than_the_file_refuses_it_and_leaves_it_as_it_was() {
    let (_tmp, root, data) = workspace();
    std::fs::create_dir_all(&data).unwrap();
    let file = data.join("workspaces.json");
    for doc in [
        json!({"format": "planroom.workspaces/v1", "min_reader_version": "99.0.0", "workspaces": []}),
        json!({"format": "planroom.workspaces/v2", "workspaces": [], "new_shape": true}),
    ] {
        let bytes = doc.to_string();
        std::fs::write(&file, &bytes).unwrap();
        let port = free_port().to_string();
        let out =
            planroom(&root, &data).args(["serve", "--detach", "--json", "--port", &port]).output().unwrap();
        assert_eq!(out.status.code(), Some(7), "{}", String::from_utf8_lossy(&out.stdout));
        let env: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(env["error"]["code"], "schema_mismatch");
        assert_eq!(env["error"]["details"]["file"], file.display().to_string());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), bytes, "an older engine never rewrites it");
    }
}

#[test]
fn a_file_from_before_min_reader_version_loads_and_gains_it() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let file = data.join("workspaces.json");
    // As the walking skeleton wrote it: no min_reader_version.
    std::fs::write(
        &file,
        json!({"format": "planroom.workspaces/v1",
               "workspaces": [{"root": "/a", "workspace_id": "w_a", "port": 41002}]})
        .to_string(),
    )
    .unwrap();
    assert_eq!(data::workspaces(&data).unwrap().len(), 1);
    data::record(&data, WorkspaceEntry::new("/b".into(), "w_b".into(), 41003)).unwrap();
    let after = read(&file);
    assert_eq!(after["min_reader_version"], data::MIN_READER_VERSION);
    assert_eq!(after["workspaces"].as_array().unwrap().len(), 2);
}
