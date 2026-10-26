//! Configuration (contracts.md, Configuration): every key has a default, the generated reference
//! and the contract's input schema name the same keys, a workspace's file never sets a credential
//! setting, the user-level `api_key` counts only at mode 0600, the default
//! excludes leave fixture and test-data trees out of a scan, and no path is ever relative.
//!
//! A workspace's own file loads only once the workspace is trusted, and nothing records trust yet,
//! so what a trusted workspace file does is checked through `planroom::config::load`; everything
//! a scan does is checked through the CLI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use planroom::config::schema::{self, Scope};
use planroom::config::{self, Config, Sources};
use planroom_testkit::{git_repo, write_config};
use serde_json::Value;

const FAKE_KEY: &str = "sk-fake-0000-not-a-real-key";

fn no_env(_: &str) -> Option<String> {
    None
}

fn load(user: Option<&Path>, workspace: Option<&Path>) -> config::Loaded {
    config::load(&Sources { user, workspace, env: &no_env }).expect("the configuration loads")
}

fn write(path: &Path, text: &str, mode: u32) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn codes(loaded: &config::Loaded) -> Vec<(String, String)> {
    loaded
        .warnings
        .iter()
        .map(|w| {
            assert_eq!(w.code.as_str(), "config_key_ignored");
            (w.details["file"].as_str().unwrap().to_owned(), w.details["key"].as_str().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn every_key_has_a_default() {
    // The defaults fill the schema exactly: no key left over, none missing.
    let defaults: Config =
        toml::Value::Table(schema::defaults().clone()).try_into().expect("defaults fit the schema");
    let declared: Vec<&str> = schema::keys().iter().map(|k| k.path.as_str()).collect();
    let mut leaves: Vec<String> = schema::leaves(schema::defaults()).into_keys().collect();
    leaves.sort();
    let mut sorted = declared.clone();
    sorted.sort();
    assert_eq!(sorted, leaves, "every default carries its `# [scope] meaning` comment");
    for key in schema::keys() {
        assert!(!key.meaning.is_empty(), "{} says what it means", key.path);
    }
    // The places contracts.md gives.
    let scope = |path: &str| schema::key(path).unwrap().scope;
    assert_eq!(scope("packs"), Scope::Workspace);
    for user in [
        "claude.backend",
        "claude.base_url",
        "claude.api_key_env",
        "claude.api_key",
        "jev.backend",
        "budget.claude_per_day_usd",
        "budget.live_tests_total_usd",
    ] {
        assert_eq!(scope(user), Scope::User, "{user}");
    }
    assert!(schema::key("claude.api_key").unwrap().needs_private_file);
    // The pinned model IDs, and the rest, as the defaults give them.
    assert_eq!(defaults.claude.model_cheap, "anthropic/claude-haiku-5.5");
    assert_eq!(defaults.claude.model_capable, "anthropic/claude-sonnet-5.5");
    assert_eq!(defaults.jev.model, "typesafe-ai/jev-1.14.0");
    assert!(!defaults.claude.api_key.is_set(), "no gateway key by default");
    // With no file and no variable, the configuration is the defaults.
    let loaded = load(None, None);
    assert_eq!(loaded.config, defaults);
    assert!(loaded.warnings.is_empty() && loaded.ignored.is_empty());
}

#[test]
fn a_workspace_file_never_holds_a_credential_key() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join(".planroom/config.toml");
    write(
        &file,
        &format!(
            "models = \"off\"\n\
             [claude]\nbackend = \"gateway\"\nbase_url = \"https://collector.example\"\n\
             api_key_env = \"TYPESAFE_API_KEY\"\napi_key = \"{FAKE_KEY}\"\nmodel_cheap = \"anthropic/other\"\n\
             [jev]\nbackend = \"typesafe\"\n\
             [budget]\nclaude_per_day_usd = 500\nlive_tests_total_usd = 500\njev_per_scan_usd = 0.01\n"
        ),
        0o600,
    );
    let loaded = load(None, Some(&file));
    let defaults = load(None, None).config;
    let c = &loaded.config;
    // Every credential setting and cross-workspace limit keeps its default.
    assert_eq!(c.claude.backend, defaults.claude.backend);
    assert_eq!(c.claude.base_url, defaults.claude.base_url);
    assert_eq!(c.claude.api_key_env, defaults.claude.api_key_env);
    assert!(!c.claude.api_key.is_set());
    assert_eq!(c.jev.backend, defaults.jev.backend);
    assert_eq!(c.budget.claude_per_day_usd, defaults.budget.claude_per_day_usd);
    assert_eq!(c.budget.live_tests_total_usd, defaults.budget.live_tests_total_usd);
    // The workspace's own keys apply.
    assert_eq!(c.models, "off");
    assert_eq!(c.claude.model_cheap, "anthropic/other");
    assert_eq!(c.budget.jev_per_scan_usd, 0.01);
    // One warning per ignored key, naming the file and the key, never the value.
    let shown = file.display().to_string();
    let mut warned = codes(&loaded);
    warned.sort();
    let expected: Vec<(String, String)> = [
        "budget.claude_per_day_usd",
        "budget.live_tests_total_usd",
        "claude.api_key",
        "claude.api_key_env",
        "claude.backend",
        "claude.base_url",
        "jev.backend",
    ]
    .iter()
    .map(|k| (shown.clone(), k.to_string()))
    .collect();
    assert_eq!(warned, expected);
    let all = serde_json::to_string(&loaded.warnings).unwrap();
    assert!(!all.contains(FAKE_KEY) && !all.contains("collector.example"), "{all}");
    assert!(!format!("{loaded:?}").contains(FAKE_KEY), "a credential never reaches Debug");

    // The same keys from the user-level file apply.
    let user = tmp.path().join("cfg/planroom/config.toml");
    fs::create_dir_all(user.parent().unwrap()).unwrap();
    fs::copy(&file, &user).unwrap();
    let loaded = load(Some(&user), None);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.config.claude.base_url, "https://collector.example");
    assert_eq!(loaded.config.claude.api_key.expose(), Some(FAKE_KEY));
}

#[test]
fn api_key_needs_mode_0600() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tmp.path().join("planroom/config.toml");
    write(
        &user,
        &format!("[claude]\napi_key = \"{FAKE_KEY}\"\nbase_url = \"https://api.typesafe.ai\"\n"),
        0o644,
    );
    // Only mode 0600 counts: a file others can read, and an owner-only file of any other mode,
    // leave the key out.
    for mode in [0o644, 0o640, 0o400, 0o700] {
        fs::set_permissions(&user, fs::Permissions::from_mode(mode)).unwrap();
        let loaded = load(Some(&user), None);
        assert!(!loaded.config.claude.api_key.is_set(), "mode {mode:o}: the key is ignored");
        assert_eq!(
            codes(&loaded),
            vec![(user.display().to_string(), "claude.api_key".to_owned())],
            "mode {mode:o}"
        );
        let message = &loaded.warnings[0].message;
        assert!(message.contains("mode 0600") && message.contains("chmod 600"), "mode {mode:o}: {message}");
        assert!(!serde_json::to_string(&loaded.warnings).unwrap().contains(FAKE_KEY));
        // The file's other keys still apply.
        assert_eq!(loaded.config.claude.base_url, "https://api.typesafe.ai");
    }

    fs::set_permissions(&user, fs::Permissions::from_mode(0o600)).unwrap();
    let loaded = load(Some(&user), None);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert_eq!(loaded.config.claude.api_key.expose(), Some(FAKE_KEY));

    // No variable sets the key itself, whose variable is the one api_key_env names, nor the host
    // it goes to; a variable does set the other credential settings.
    let env = |name: &str| match name {
        "PLANROOM_CLAUDE_API_KEY" => Some("sk-from-env".to_owned()),
        "PLANROOM_CLAUDE_BASE_URL" => Some("https://collector.example".to_owned()),
        "PLANROOM_CLAUDE_BACKEND" => Some("off".to_owned()),
        _ => None,
    };
    let loaded = config::load(&Sources { user: None, workspace: None, env: &env }).unwrap();
    assert!(!loaded.config.claude.api_key.is_set());
    assert_eq!(loaded.config.claude.base_url, "https://ai-gateway.vercel.sh");
    assert_eq!(loaded.config.claude.backend, "off");
}

#[test]
fn a_user_only_key_in_a_workspace_file_is_ignored_whatever_its_type() {
    // A trusted workspace file whose credential settings and cross-workspace limits hold values of
    // the wrong type: they are ignored, as any user-only key there is, and the load succeeds.
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join(".planroom/config.toml");
    write(
        &file,
        "models = \"off\"\n\
         [claude]\nbackend = 3\napi_key = [\"x\"]\nbase_url = true\n\
         [budget]\nclaude_per_day_usd = \"lots\"\n",
        0o600,
    );
    let loaded = load(None, Some(&file));
    let defaults = load(None, None).config;
    assert_eq!(loaded.config.models, "off", "the workspace's own keys apply");
    assert_eq!(loaded.config.claude, defaults.claude);
    assert_eq!(loaded.config.budget, defaults.budget);
    let shown = file.display().to_string();
    let mut warned = codes(&loaded);
    warned.sort();
    let expected: Vec<(String, String)> =
        ["budget.claude_per_day_usd", "claude.api_key", "claude.backend", "claude.base_url"]
            .iter()
            .map(|k| (shown.clone(), k.to_string()))
            .collect();
    assert_eq!(warned, expected);

    // A key the workspace file may set still has to fit its type.
    write(&file, "models = 3\n", 0o600);
    let refused = config::load(&Sources { user: None, workspace: Some(&file), env: &no_env });
    assert!(refused.is_err(), "a mistyped workspace key fails the load");
}

/// The cells of a Markdown table row, with `\|` read as a literal bar.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => cell.push(chars.next().unwrap()),
            '|' => cells.push(std::mem::take(&mut cell).trim().to_owned()),
            c => cell.push(c),
        }
    }
    cells.push(cell.trim().to_owned());
    cells
}

#[test]
fn configuration_reference_equals_defaults() {
    // The generated reference has one row per key the engine reads from defaults.toml, with the
    // place, default, variable and meaning the engine gives it.
    let path = planroom_testkit::repo_root().join("docs/reference/configuration.md");
    let reference = fs::read_to_string(&path).expect("docs/reference/configuration.md");
    let rows: Vec<Vec<String>> = reference.lines().filter(|l| l.starts_with("| `")).map(cells).collect();
    let keys = schema::keys();
    assert_eq!(rows.len(), keys.len(), "one row per key");
    for (row, key) in rows.iter().zip(keys) {
        let shown = match key.path.split_once('.') {
            Some((section, name)) => format!("`[{section}] {name}`"),
            None => format!("`{}`", key.path),
        };
        assert_eq!(row[0], shown, "the rows follow defaults.toml's order");
        let scope = match key.scope {
            Scope::Workspace => "workspace",
            Scope::Either => "either",
            Scope::User => "user",
        };
        assert_eq!(row[1].split(',').next().unwrap(), scope, "{}", key.path);
        assert_eq!(row[1].contains("mode 0600"), key.needs_private_file, "{}", key.path);
        let default = match schema::lookup(schema::defaults(), &key.path).unwrap() {
            toml::Value::String(s) if s.is_empty() => "none".to_owned(),
            other => format!("`{other}`"),
        };
        assert_eq!(row[2], default, "{}", key.path);
        let variable = key.variable().map_or("none".to_owned(), |v| format!("`{v}`"));
        assert_eq!(row[3], variable, "{}", key.path);
        assert_eq!(row[4], key.meaning, "{}", key.path);
    }

    // The contract's input schema describes the same keys, as one file holds them: any key may be
    // left out.
    let input = planroom_contract::schema::generate(planroom_contract::schema::View::Input);
    let defs = &input["$defs"];
    let properties = |name: &str| -> Vec<String> {
        let def = &defs[name];
        assert!(def.get("required").is_none(), "{name}: a file may leave any key out");
        def["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name} has properties"))
            .keys()
            .cloned()
            .collect()
    };
    let mut described = Vec::new();
    for top in properties("Config") {
        let section =
            defs["Config"]["properties"][&top]["$ref"].as_str().and_then(|r| r.strip_prefix("#/$defs/"));
        match section {
            Some(section) => described.extend(properties(section).into_iter().map(|k| format!("{top}.{k}"))),
            None => described.push(top),
        }
    }
    described.sort();
    let mut declared: Vec<String> = keys.iter().map(|k| k.path.clone()).collect();
    declared.sort();
    assert_eq!(described, declared, "the input schema's Config holds every key of defaults.toml");
}

#[test]
fn the_packs_key_is_known() {
    // The workspace config the fixture writes, as `cargo xtask fixture` writes it.
    let tmp = tempfile::tempdir().unwrap();
    let pack = tmp.path().join("packs/ruby");
    write_config(tmp.path(), &pack);
    let loaded = load(None, Some(&tmp.path().join(".planroom/config.toml")));
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    assert!(loaded.ignored.is_empty(), "{:?}", loaded.ignored);
    assert_eq!(loaded.config.packs, vec![pack.display().to_string()]);
    // A key the schema doesn't know is left out and listed, and the file still loads.
    let file = tmp.path().join("other.toml");
    write(&file, "packs = []\nfuture_key = 1\n[scan]\nlater = true\n", 0o600);
    let loaded = load(None, Some(&file));
    let shown = file.display();
    assert_eq!(loaded.ignored, vec![format!("{shown}: future_key"), format!("{shown}: scan.later")]);
    // `packs` is workspace state: no variable sets it, and the user-level file doesn't either.
    let env = |name: &str| (name == "PLANROOM_PACKS").then(|| "/elsewhere".to_owned());
    let loaded = config::load(&Sources { user: Some(&file), workspace: None, env: &env }).unwrap();
    assert!(loaded.config.packs.is_empty());
    assert_eq!(loaded.ignored[0], format!("{shown}: future_key"));
    assert!(loaded.ignored.contains(&format!("{shown}: packs")), "{:?}", loaded.ignored);
}

/// A workspace `ws/` of one repo, `app`, with a compose file at its root and two more inside a
/// fixture tree and a test-data tree.
fn workspace(dir: &Path) -> PathBuf {
    let ws = dir.join("ws");
    let compose = |service: &str| format!("services:\n  {service}:\n    image: redis:7\n").into_bytes();
    git_repo(
        &ws.join("app"),
        &[
            ("docker-compose.yml".into(), compose("web")),
            ("examples/fixture/repos/shop/docker-compose.yml".into(), compose("ghost")),
            ("testdata/compose.yml".into(), compose("phantom")),
            ("src/fixtures.rs".into(), b"// a lookalike name, not a fixture tree\n".to_vec()),
        ],
    );
    ws
}

/// The binary in `cwd` with exactly `env` and `PATH`.
fn planroom(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(args).current_dir(cwd).env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.envs(env.iter().copied());
    cmd.output().expect("the binary runs")
}

fn json(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
    let out = planroom(cwd, args, env);
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("{args:?}: {}", String::from_utf8_lossy(&out.stderr)))
}

fn containers(ws: &Path, env: &[(&str, &str)]) -> Vec<String> {
    let scanned = json(ws, &["scan", "--json"], env);
    assert_eq!(scanned["ok"], true, "{scanned}");
    let map = json(ws, &["map", "--level", "container", "--json"], env);
    let mut names: Vec<String> = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn fixture_and_test_data_trees_are_excluded_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(tmp.path());
    let data = tmp.path().join("data");
    let cfg = tmp.path().join("cfg");
    let base = [("PLANROOM_DATA", data.to_str().unwrap()), ("XDG_CONFIG_HOME", cfg.to_str().unwrap())];
    assert_eq!(containers(&ws, &base), ["app", "web"]);

    // A person's value replaces the list, and a rescan at the same commit maps what it lets in:
    // the excludes key the snapshot.
    write(&cfg.join("planroom/config.toml"), "[scan]\nexclude = [\"**/testdata/**\"]\n", 0o600);
    assert_eq!(containers(&ws, &base), ["app", "ghost", "web"]);

    // A variable wins over the file.
    let env = [base[0], base[1], ("PLANROOM_SCAN_EXCLUDE", "[]")];
    assert_eq!(containers(&ws, &env), ["app", "ghost", "phantom", "web"]);

    // Back to the default list, back to the first map.
    fs::remove_file(cfg.join("planroom/config.toml")).unwrap();
    assert_eq!(containers(&ws, &base), ["app", "web"]);

    // A glob that doesn't parse is invalid_argument naming the key.
    let env = [base[0], base[1], ("PLANROOM_SCAN_EXCLUDE", "a/{b")];
    let out = planroom(&ws, &["scan", "--json"], &env);
    let refused: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(2), "{refused}");
    assert_eq!(refused["error"]["code"], "invalid_argument");
    assert_eq!(refused["error"]["details"]["field"], "scan.exclude");
}

#[test]
fn with_home_unset_no_path_is_relative() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(tmp.path());
    // A relative XDG_CONFIG_HOME would read this file, which a repo or a stranger could plant in
    // the directory a process runs in.
    write(&ws.join("cfg/planroom/config.toml"), "[scan]\nexclude = []\n", 0o600);
    let env = [("XDG_CONFIG_HOME", "cfg"), ("XDG_DATA_HOME", "data")];
    let before: Vec<_> = fs::read_dir(&ws).unwrap().map(|e| e.unwrap().file_name()).collect();

    assert_eq!(containers(&ws, &env), ["app", "web"], "the relative config was never read");

    // A command that needs the data root refuses rather than placing it here.
    let out = planroom(&ws, &["serve", "--json"], &env);
    let refused: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(refused["error"]["code"], "invalid_argument", "{refused}");
    assert_eq!(refused["error"]["details"]["field"], "PLANROOM_DATA");

    // Nothing landed in the working directory but the state directory the scan made.
    let mut after: Vec<_> = fs::read_dir(&ws).unwrap().map(|e| e.unwrap().file_name()).collect();
    after.retain(|name| !before.contains(name));
    assert_eq!(after, [".planroom"], "only the state directory is new");
    for relative in ["data", ".local", ".config"] {
        assert!(!ws.join(relative).exists() && !ws.join("app").join(relative).exists(), "{relative}");
    }
}
