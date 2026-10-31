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

/// `planroom ARGS --json` in `cwd` with its own data root and home, no actor and no token, stdin
/// closed: the exit code and the one envelope, whatever the outcome.
fn run_env(cwd: &Path, home: &Path, args: &[&str], actor: Option<&str>) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args)
        .arg("--json")
        .current_dir(cwd)
        .env("PLANROOM_DATA", home.join("data"))
        .env("HOME", home.join("home"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_CLIENT")
        .stdin(std::process::Stdio::null());
    if let Some(actor) = actor {
        cmd.env("PLANROOM_ACTOR", actor);
    }
    let out = cmd.output().expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    (out.status.code().unwrap(), env)
}

/// A temporary tree with `data/` and `home/` beside the folders a test makes.
fn tree() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    for dir in ["data", "home"] {
        std::fs::create_dir(base.join(dir)).unwrap();
    }
    (tmp, base)
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

const AGENT: &str = "agent:aaaaaaaa:chat";

/// The line a level with blocks prints, built from its blocks: how many there are at that level,
/// then the first 12 by name (a name two blocks share is told apart as `repo/key`) with their
/// counts of what lies inside them, in the words and the order of `ladder`, a graph reply's
/// `levels`, and how many more there are.
fn summary_of(view: &Value, level: &str, total: usize, ladder: &[Value]) -> String {
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
            let mut counts: Vec<(usize, String)> = n["counts"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(level, count)| {
                    let at = ladder.iter().position(|l| l["id"] == level.as_str()).expect("a held level");
                    let words = if count == 1 { "one" } else { "many" };
                    let count = count.to_string();
                    (at, ladder[at][words].as_str().unwrap().replace("{count}", &count))
                })
                .collect();
            counts.sort();
            let counts: Vec<String> = counts.into_iter().map(|(_, words)| words).collect();
            if counts.is_empty() { name } else { format!("{name} ({})", counts.join(", ")) }
        })
        .collect();
    let names = if total > listed.len() {
        format!("{}; and {} more", listed.join("; "), total - listed.len())
    } else {
        listed.join("; ")
    };
    let label = ladder.iter().find(|l| l["id"] == level).map_or(level, |l| l["label"].as_str().unwrap());
    format!("{total} at the {label} level: {names}.")
}

#[test]
fn every_map_level_has_blocks_or_an_empty_state_whose_next_action_works() {
    let tmp = tempfile::tempdir().unwrap();
    let root = fixture(&tmp.path().join("shop")).root;
    run(&root, &["scan", "--json"]);
    let (graph, _) = run(&root, &["map", "--detail", "graph", "--json"]);
    let ladder = graph["result"]["levels"].as_array().unwrap().clone();
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
            assert_eq!(line, summary_of(view, level, total, &ladder), "{level}");
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

#[test]
fn no_repos_offers_to_start_a_plan() {
    let (_tmp, base) = tree();
    let work = base.join("work");
    std::fs::create_dir(&work).unwrap();
    let (code, focus) = run_env(&work, &base, &["focus"], None);
    assert_eq!(code, 0, "{focus}");
    let (code, map) = run_env(&work, &base, &["map"], None);
    assert_eq!(code, 0, "{map}");
    let empty = &focus["result"]["empty"];
    assert_eq!(empty["reason"], "no_repos", "{focus}");
    assert_eq!(empty, &map["result"]["empty"], "focus and map say the same");
    assert_eq!(empty["line"], "No git repos here. Start a plan, or add a repo.", "{focus}");
    assert_eq!(
        empty["next"],
        serde_json::json!({"command": "schema", "args": ["--command", "intent new"], "label": "Start a plan here"}),
        "{focus}"
    );

    let (code, schema) = run_env(&work, &base, &["schema", "--command", "intent new"], None);
    assert_eq!(code, 0, "{schema}");
    assert!(!work.join(".planroom").exists(), "looking at how to start makes nothing");
}

#[test]
fn a_plan_only_workspace_says_the_map_fills_from_the_plan() {
    let (_tmp, base) = tree();
    let work = base.join("work");
    std::fs::create_dir(&work).unwrap();
    let (code, env) = run_env(&work, &base, &["intent", "new", "Add a billing service"], Some(AGENT));
    assert_eq!(code, 0, "{env}");
    let (_, list) = run_env(&work, &base, &["intent", "list"], None);
    let project = list["result"]["projects"][0]["id"].as_str().unwrap().to_owned();

    let line = "Your plan is kept here. The map fills from each repo's first commit.";
    let next = serde_json::json!({"command": "intent list", "args": [], "label": "Show the plan"});
    let check = |what: &str, env: &Value| {
        let empty = &env["result"]["empty"];
        assert_eq!(empty["reason"], "plan_only", "{what}: {env}");
        assert_eq!(empty["line"], line, "{what}: {env}");
        assert_eq!(empty["next"], next, "{what}: {env}");
    };

    let (_, focus) = run_env(&work, &base, &["focus"], None);
    check("focus", &focus);
    assert_eq!(focus["result"]["focus"]["seq"], focus["meta"]["seq"], "{focus}");
    assert!(focus["meta"]["seq"].as_u64().unwrap() > 0, "{focus}");
    let (_, map) = run_env(&work, &base, &["map"], None);
    check("map", &map);
    let (_, graph) = run_env(&work, &base, &["map", "--detail", "graph"], None);
    for level in graph["result"]["levels"].as_array().unwrap() {
        let level = level["id"].as_str().unwrap();
        let (_, map) = run_env(&work, &base, &["map", "--level", level], None);
        check(&format!("map --level {level}"), &map);
    }
    let (_, found) = run_env(&work, &base, &["find", "billing"], None);
    check("find", &found);
    let (_, asked) = run_env(&work, &base, &["ask", "impact", &project], None);
    check("ask", &asked);
}

#[test]
fn a_plan_wins_over_repos_unborn_but_not_over_repos_unreadable() {
    let (_tmp, base) = tree();
    let a = base.join("a");
    std::fs::create_dir(&a).unwrap();
    git(&a, &["init", "-q", "-b", "main", "app"]);
    let (_, focus) = run_env(&a, &base, &["focus"], None);
    assert_eq!(focus["result"]["empty"]["reason"], "repos_unborn", "{focus}");
    let (code, env) = run_env(&a, &base, &["intent", "new", "Add a billing service"], Some(AGENT));
    assert_eq!(code, 0, "{env}");
    for command in ["focus", "map"] {
        let (_, env) = run_env(&a, &base, &[command], None);
        assert_eq!(env["result"]["empty"]["reason"], "plan_only", "{command}: {env}");
    }

    // A repo git refuses stays reported, plan or not: the plan can't make it mappable.
    let b = base.join("b");
    std::fs::create_dir(&b).unwrap();
    git(&b, &["init", "-q", "-b", "main", "broken"]);
    std::fs::write(b.join("broken/.git/config"), "[core\nthis is not a config\n").unwrap();
    let (code, env) = run_env(&b, &base, &["intent", "new", "Add a billing service"], Some(AGENT));
    assert_eq!(code, 0, "{env}");
    let (_, focus) = run_env(&b, &base, &["focus"], None);
    assert_eq!(focus["result"]["empty"]["reason"], "repos_unreadable", "{focus}");
}
