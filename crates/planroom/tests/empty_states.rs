//! No blank states (design-language.md, Empty states): a map level with no blocks is an `Empty`
//! with a reason and a next action that works today, never `nodes: []` with `empty: null` (ux-3).

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
        assert!(!text.contains("0 at the top"), "{level}: {text}");
        if blocks > 0 {
            assert!(view["empty"].is_null(), "{level}");
            continue;
        }
        let empty = &view["empty"];
        assert!(empty["reason"].is_string(), "{level}: a level with no blocks is an Empty: {view}");
        assert_eq!(text.trim(), empty["next"]["label"], "{level}: the text mode says what to do");
        assert_eq!(env["summary"], empty["next"]["label"]);
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
}
