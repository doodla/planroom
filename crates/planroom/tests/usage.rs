//! A usage error with `--json` is still exactly one JSON document: the `invalid_argument`
//! envelope with the leaf's synopsis and its `schema --command LEAF` handle (drift-1, DR-21,
//! DR-66). Without `--json`, clap's text goes to stderr, display-sanitized. Both exit 2.

use std::io::Read;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn planroom(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(dir.path())
        .env("PLANROOM_DATA", dir.path().join(".data"))
        .output()
        .expect("the binary runs")
}

fn envelope(args: &[&str]) -> (Value, Output) {
    let out = planroom(args);
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout:?}");
    (serde_json::from_str(&stdout).unwrap(), out)
}

#[test]
fn a_bad_flag_or_value_with_json_is_one_invalid_argument_envelope() {
    let cases: [(&[&str], &str, &str); 4] = [
        (&["map", "--level", "Foo", "--json"], "map", "level"),
        (&["map", "--bogus", "--json"], "map", "bogus"),
        (&["watch", "--since", "abc", "--json"], "watch", "since"),
        (&["person", "init", "--json", "--nope"], "person init", "nope"),
    ];
    for (args, leaf, field) in cases {
        let (env, out) = envelope(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(env["ok"], false);
        assert_eq!(env["command"], leaf);
        let error = &env["error"];
        assert_eq!(error["code"], "invalid_argument");
        assert_eq!(error["retry"], "never");
        assert_eq!(error["details"]["field"], field, "{args:?}");
        let synopsis = error["details"]["synopsis"].as_str().unwrap();
        assert!(synopsis.starts_with(&format!("planroom {leaf} ")), "{synopsis}");
        assert!(!synopsis.contains(" -h") && !synopsis.contains("-V"), "long flags only: {synopsis}");
        assert_eq!(error["details"]["schema"]["command"], "schema");
        assert_eq!(error["details"]["schema"]["args"], serde_json::json!(["--command", leaf]));
        let hint = error["hint"].as_str().unwrap();
        assert!(hint.starts_with("planroom schema --command "), "{hint}");
        assert!(!error["message"].as_str().unwrap().contains('\n'));
    }
    let (env, _) = envelope(&["map", "--level", "Foo", "--json"]);
    assert!(env["error"]["details"]["synopsis"].as_str().unwrap().contains("--level LEVEL"));
}

/// Every leaf's usage error carries the synopsis `schema --commands` lists for it, one line from
/// the leaf's clap arguments, so the CLI's usage, the listing and the tools' descriptions agree.
#[test]
fn a_leafs_synopsis_is_the_one_schema_commands_lists() {
    // Every page of the listing, following `meta.next`.
    let mut items: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["schema", "--commands", "--json"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c.as_str()]);
        }
        let (listing, out) = envelope(&args);
        assert!(out.status.success(), "{listing}");
        items.extend(listing["result"]["items"].as_array().unwrap().iter().cloned());
        cursor = listing["meta"]["next"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(items.len(), planroom_contract::Command::SPECS.len());
    for item in &items {
        let leaf = item["name"].as_str().unwrap();
        let listed = item["synopsis"].as_str().unwrap_or_else(|| panic!("{leaf} lists no synopsis"));
        assert!(listed.starts_with(&format!("planroom {leaf}")) && listed.ends_with(" [--json]"), "{listed}");
        let mut args: Vec<&str> = leaf.split(' ').collect();
        args.extend(["--no-such-flag", "--json"]);
        let (env, out) = envelope(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {env}");
        assert_eq!(env["error"]["details"]["synopsis"], listed, "{leaf}");
    }
    // A leaf whose request has subcommands names them after its path.
    let edit = items.iter().find(|i| i["name"] == "edit").unwrap();
    assert!(
        edit["synopsis"]
            .as_str()
            .unwrap()
            .starts_with("planroom edit move|group|rename|hide|pin|relate|unrelate|forbid|undo "),
        "{edit}"
    );
}

/// A leaf's own flags run after its subcommand, where its synopsis writes them, as well as
/// before it: neither order is a usage error.
#[test]
fn a_leafs_own_flags_run_after_its_subcommand() {
    for args in [
        &["edit", "rename", "a", "b", "--base", "3", "--project", "plan:x:y", "--json"][..],
        &["edit", "--base", "3", "--project", "plan:x:y", "rename", "a", "b", "--json"][..],
        &["edit", "undo", "e1", "--idem", "k", "--json"][..],
    ] {
        let (env, _) = envelope(args);
        assert_eq!(env["ok"], false, "{args:?}: {env}");
        assert_ne!(env["error"]["code"], "invalid_argument", "{args:?}: {env}");
    }
}

#[test]
fn the_schema_handle_of_a_usage_error_resolves() {
    let (env, _) = envelope(&["map", "--level", "Foo", "--json"]);
    let handle = &env["error"]["details"]["schema"];
    let mut args: Vec<&str> = vec!["schema"];
    args.extend(handle["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()));
    args.push("--json");
    let (schema, out) = envelope(&args);
    assert!(out.status.success(), "{schema}");
    assert_eq!(schema["result"]["request"], "MapReq");
}

#[test]
fn an_unknown_command_with_json_names_the_catalogue() {
    let (env, out) = envelope(&["nope", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(env["error"]["code"], "invalid_argument");
    assert_eq!(env["error"]["details"]["field"], "command");
    assert_eq!(env["error"]["hint"], "planroom schema --commands");
}

#[test]
fn a_usage_error_without_json_is_sanitized_text_on_stderr() {
    let out = planroom(&["map", "--level", "\x1b]52;c;aGVsbG8=\x07\u{202e}"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    // clap's plain rendering drops the escape sequence itself; what is left shows as markers.
    assert!(stderr.contains("⟨U+202E⟩"), "{stderr}");
    assert!(!stderr.contains('\x1b') && !stderr.contains('\u{202e}'), "{stderr:?}");
}

#[test]
fn help_and_version_print_as_before_with_json() {
    let out = planroom(&["map", "--help", "--json"]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("Usage: planroom map"));
    let out = planroom(&["--version", "--json"]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().starts_with("planroom "));
}

#[test]
fn bare_planroom_into_a_closed_pipe_exits_cleanly() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    // Close the read end at once, as `planroom | head -0` does.
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(status.success(), "{status:?}: {stderr}");
}
