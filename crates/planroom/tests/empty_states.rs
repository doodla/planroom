//! No blank states (design-language.md, Empty states): a map level with no blocks is an `Empty`
//! with a reason, its one line and a next action that works today, never `nodes: []` with
//! `empty: null`. A level with blocks says exactly what it shows.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use planroom_testkit::fixture;
use serde_json::Value;

fn run(cwd: &Path, args: &[&str]) -> (Value, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    (serde_json::from_str(&stdout).unwrap_or(Value::Null), stdout)
}

/// The line a level with blocks prints, built from its blocks: how many there are at that level,
/// then the first 12 by name (a name two blocks share is told apart as `repo/key`) with their
/// counts of what lies inside them, and how many more there are.
fn summary_of(view: &Value, level: &str, total: usize) -> String {
    let nodes = view["nodes"].as_array().unwrap();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for n in nodes {
        *seen.entry(n["name"].as_str().unwrap()).or_default() += 1;
    }
    let listed: Vec<String> = nodes
        .iter()
        .take(12)
        .map(|n| {
            let name = n["name"].as_str().unwrap();
            let name = if seen[name] > 1 {
                let parts: Vec<&str> = n["id"].as_str().unwrap().splitn(3, ':').collect();
                format!("{}/{}", parts[0], parts[2])
            } else {
                name.to_owned()
            };
            let counts: Vec<String> = n["counts"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(level, count)| format!("{count} {level}"))
                .collect();
            if counts.is_empty() { name } else { format!("{name} ({})", counts.join(", ")) }
        })
        .collect();
    let names = if total > listed.len() {
        format!("{}; and {} more", listed.join("; "), total - listed.len())
    } else {
        listed.join("; ")
    };
    format!("{total} at the {level} level: {names}.")
}

#[test]
fn every_map_level_has_blocks_or_an_empty_state_whose_next_action_works() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture(&tmp.path().join("shop")).root;
    run(&root, &["scan", "--json"]);
    for level in ["intent", "system", "container", "component", "code"] {
        let (env, _) = run(&root, &["map", "--level", level, "--json"]);
        let view = &env["result"];
        let blocks = view["nodes"].as_array().unwrap().len();
        let (_, text) = run(&root, &["map", "--level", level]);
        let line = text.lines().next().unwrap();
        assert_eq!(env["summary"], line, "{level}: the text mode prints the summary first");
        if blocks > 0 {
            assert!(view["empty"].is_null(), "{level}");
            // The whole line, exactly: a cut map counts every block it has, kept or not.
            let total = env["meta"]["warnings"]
                .as_array()
                .and_then(|w| w.iter().find(|w| w["code"] == "truncated"))
                .map_or(blocks, |w| w["details"]["total"].as_u64().unwrap() as usize);
            assert_eq!(line, summary_of(view, level, total), "{level}");
            continue;
        }
        let empty = &view["empty"];
        assert!(empty["reason"].is_string(), "{level}: a level with no blocks is an Empty: {view}");
        assert_eq!(line, empty["line"], "{level}: the text mode prints the empty state's line");
        assert_ne!(
            empty["line"], empty["next"]["label"],
            "{level}: the line and the action say different things"
        );
        let mut args = vec![empty["next"]["command"].as_str().unwrap().to_owned()];
        args.extend(empty["next"]["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()));
        args.push("--json".into());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let (next, _) = run(&root, &args);
        assert_eq!(next["ok"], true, "{level}: {next}");
        assert!(
            !next["result"]["nodes"].as_array().unwrap().is_empty(),
            "{level}: the next action shows blocks"
        );
    }
    let (intent, _) = run(&root, &["map", "--level", "intent", "--json"]);
    assert_eq!(intent["result"]["empty"]["reason"], "no_intent");
    assert_eq!(intent["summary"], "No plans at the intent level yet.");
    assert_eq!(intent["result"]["empty"]["next"]["label"], "Show the top of the map");
}
