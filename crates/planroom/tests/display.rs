//! The display-sanitizing pass on the engine's human-facing output (security-5, DR-64), and the
//! envelope's `summary`, the line text mode prints first (ux-2).

use std::path::Path;
use std::process::{Command, Output};

use planroom_testkit::{fixture, git_repo};
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out
}

fn json(cwd: &Path, args: &[&str]) -> Value {
    serde_json::from_slice(&planroom(cwd, args).stdout).unwrap()
}

/// An OSC 52 clipboard write and an RLO, in a compose service's name.
const HOSTILE: &str = "evil\x1b]52;c;aGVsbG8=\x07name\u{202e}abc";

#[test]
fn a_hostile_name_reaches_the_terminal_and_the_summary_as_markers() {
    let tmp = tempfile::tempdir().unwrap();
    // YAML's own escapes for ESC, BEL and U+202E, as the review's repro wrote them.
    let compose = "services:\n  \"evil\\e]52;c;aGVsbG8=\\aname\\u202eabc\":\n    image: redis:7\n".to_owned();
    git_repo(&tmp.path().join("app"), &[("docker-compose.yml".into(), compose.into_bytes())]);
    planroom(tmp.path(), &["scan"]);

    let text = String::from_utf8(planroom(tmp.path(), &["map", "--level", "container"]).stdout).unwrap();
    assert!(text.contains("evil\\x1b]52;c;aGVsbG8=\\x07name⟨U+202E⟩abc"), "{text}");
    assert!(!text.chars().any(|c| c.is_control() && c != '\n') && !text.contains('\u{202e}'), "{text:?}");

    let env = json(tmp.path(), &["map", "--level", "container", "--json"]);
    // The result stays data: the name as the repo spells it, which surfaces sanitize as they draw.
    let names: Vec<&str> =
        env["result"]["nodes"].as_array().unwrap().iter().filter_map(|n| n["name"].as_str()).collect();
    assert!(names.contains(&HOSTILE), "{names:?}");
    // The engine-authored line is display text, so it is sanitized.
    let summary = env["summary"].as_str().unwrap();
    assert_eq!(summary, text.lines().next().unwrap());
}

#[test]
fn every_reply_carries_the_line_text_mode_prints_first() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture(&tmp.path().join("shop")).root;
    planroom(&root, &["scan"]);
    for args in [
        &["map"][..],
        &["map", "--level", "container"],
        &["focus"],
        &["schema"],
        &["schema", "--commands"],
        &["schema", "--command", "map"],
        &["version"],
    ] {
        let text = String::from_utf8(planroom(&root, args).stdout).unwrap();
        let mut with_json = args.to_vec();
        with_json.push("--json");
        let env = json(&root, &with_json);
        assert_eq!(env["summary"], text.lines().next().unwrap(), "{args:?}");
    }
}

#[test]
fn an_error_line_on_stderr_is_sanitized() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "no\x1b]52;c;aGVsbG8=\x07where\u{202e}"])
        .current_dir(tmp.path())
        .env("PLANROOM_DATA", tmp.path().join(".data"))
        .output()
        .expect("the binary runs");
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("no\\x1b]52;c;aGVsbG8=\\x07where⟨U+202E⟩"), "{stderr}");
    assert!(!stderr.contains('\x1b') && !stderr.contains('\u{202e}'), "{stderr:?}");
}
