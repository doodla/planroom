//! The one next action (architecture.md, "The pane" and "Lenses and questions"; DR-54): the pane's
//! `next` is the loaded packs' highest-priority `[[suggestions]]` entry whose `when` holds, a leaf
//! that can be undone, hands nothing off and never spends, and it opens a view that isn't empty.

use std::path::Path;
use std::process::Command;

use planroom_contract::{Class, Command as Leaf, Undo};
use planroom_testkit::{fixture, git_repo, repo_root};
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs")
}

/// A `--json` reply's envelope, which must be `ok`.
fn ok(cwd: &Path, args: &[&str]) -> Value {
    let out = planroom(cwd, &[args, &["--json"]].concat());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"));
    assert_eq!(env["ok"], true, "{args:?}: {env}");
    env
}

/// The code pack's suggestions, as its manifest declares them.
fn suggestions() -> Vec<toml::Value> {
    let text = std::fs::read_to_string(repo_root().join("packs/code/pack.toml")).unwrap();
    let pack: toml::Value = toml::from_str(&text).unwrap();
    pack.get("suggestions").and_then(|s| s.as_array()).cloned().unwrap_or_default()
}

/// The pane's one next action in `root`: one handle, one `Next:` line in text mode, a leaf the
/// band may name, and a view that isn't empty when it runs.
fn one_next(root: &Path) -> Value {
    let pane = ok(root, &["focus"])["result"].clone();
    assert!(pane["empty"].is_null(), "a scanned workspace's pane: {pane}");
    let next = pane["next"].clone();
    assert!(next.is_object(), "one next action: {pane}");

    let text = String::from_utf8(planroom(root, &["focus"]).stdout).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("Next: ")).collect();
    assert_eq!(lines.len(), 1, "one Next line: {text}");
    assert_eq!(lines[0], format!("Next: {}", next["label"].as_str().unwrap()));

    let command = next["command"].as_str().unwrap();
    let spec =
        Leaf::SPECS.iter().find(|s| s.name() == command).unwrap_or_else(|| panic!("a leaf: {command}"));
    assert!(!matches!(spec.undo, Undo::NotUndoable(_)), "{command} can be undone");
    assert_ne!(spec.class, Class::Handoff, "{command} hands nothing off");
    assert!(!spec.spends, "{command} never spends");

    let args: Vec<&str> = next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    let words: Vec<&str> = command.split(' ').chain(args.iter().copied()).collect();
    let opened = ok(root, &words);
    assert!(opened["result"]["empty"].is_null(), "the next action opens a view with blocks: {opened}");
    next
}

/// The pack suggestion `next` is, by its command, arguments and text.
fn suggestion_of(next: &Value) -> toml::Value {
    let args: Vec<&str> = next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    suggestions()
        .into_iter()
        .find(|s| {
            let action = &s["action"];
            let theirs: Vec<&str> = action
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().map(|v| v.as_str().unwrap()).collect())
                .unwrap_or_default();
            action["command"].as_str() == next["command"].as_str()
                && theirs == args
                && s["text"].as_str() == next["label"].as_str()
        })
        .unwrap_or_else(|| panic!("the next action is one of the code pack's suggestions: {next}"))
}

fn priority(s: &toml::Value) -> i64 {
    s.get("priority").and_then(toml::Value::as_integer).unwrap_or(0)
}

#[test]
fn top_level_has_one_next_action() {
    // The fixture holds containers, so the next action is the code pack's top suggestion.
    let tmp = tempfile::tempdir().unwrap();
    let shop = tmp.path().join("shop");
    fixture(&shop);
    ok(&shop, &["scan"]);
    let next = one_next(&shop);
    let chosen = suggestion_of(&next);
    let top = suggestions().iter().map(priority).max().unwrap();
    assert_eq!(priority(&chosen), top, "the highest-priority suggestion: {next}");

    // A workspace with files in directories and no container: the suggestion whose `when` holds
    // is the next action, never a level the workspace has nothing at.
    let plain = tmp.path().join("plain");
    git_repo(
        &plain.join("notes"),
        &[
            ("src/tally.py".to_owned(), b"def tally():\n    return 1\n".to_vec()),
            ("src/report.py".to_owned(), b"def report():\n    return 2\n".to_vec()),
        ],
    );
    ok(&plain, &["scan"]);
    let next = one_next(&plain);
    let chosen = suggestion_of(&next);
    assert!(priority(&chosen) < top, "a lower-ranked suggestion holds here: {next}");
}
