//! Workspace trust and credential settings (architecture.md, Where the workspace lives: Trust and
//! Credential settings; DR-48, DR-88, DR-89): a workspace's own `config.toml` loads only from a
//! state directory the data root records as trusted, by the engine that created it or by a
//! person's accept of the trust card; a state directory the engine can't vouch for deals one card;
//! credential settings come only from the user-level config or the environment; and a store
//! planted over the state directory can't sign the pick that trusts it.
//!
//! Each test has its own data root, config home and home, and its workspaces map a compose file
//! at the repo's root (`web`) and one in a test-data tree (`phantom`), which the default excludes
//! leave out and a workspace config of `[scan] exclude = []` maps: `phantom` on the map is the
//! sign that the workspace's config loaded.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_testkit::git_repo;
use serde_json::Value;

/// A workspace config that loads, visibly: it maps the test-data tree the defaults leave out.
const LOADS: &str = "[scan]\nexclude = []\n";

/// One test's machine: its data root, config home and home, apart from every other test's.
struct Machine {
    tmp: tempfile::TempDir,
}

impl Machine {
    fn new() -> Machine {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("home")).unwrap();
        Machine { tmp }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.tmp.path().join(rel)
    }

    fn data(&self) -> PathBuf {
        self.path("data")
    }

    /// `planroom ARGS --json` in `cwd` with this machine's places, `env` and `stdin`.
    fn run(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
        cmd.current_dir(cwd).env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        cmd.env("HOME", self.path("home"))
            .env("PLANROOM_DATA", self.data())
            .env("XDG_CONFIG_HOME", self.path("cfg"))
            .args(args)
            .arg("--json")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        use std::io::Write;
        let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
        let out = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let env: Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
        (out.status.code().unwrap(), env)
    }

    fn ok(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Value {
        let (code, env) = self.run(cwd, args, env, "");
        assert_eq!(code, 0, "{args:?}: {env}");
        env
    }

    /// Scan `ws`; the reply.
    fn scan(&self, ws: &Path) -> Value {
        self.ok(ws, &["scan"], &[])
    }

    /// The containers the map holds after a scan of `ws`, sorted.
    fn containers(&self, ws: &Path) -> Vec<String> {
        let map = self.ok(ws, &["map", "--level", "container"], &[]);
        let mut names: Vec<String> = map["result"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["name"].as_str().unwrap().into())
            .collect();
        names.sort();
        names
    }

    /// Whether `ws`'s map shows the test-data tree: whether its workspace config loaded.
    fn loads_config(&self, ws: &Path) -> bool {
        self.containers(ws).iter().any(|n| n == "phantom")
    }

    fn focus(&self, ws: &Path) -> Value {
        self.ok(ws, &["focus"], &[])
    }

    fn seq(&self, ws: &Path) -> String {
        self.ok(ws, &["map"], &[])["meta"]["seq"].as_u64().unwrap().to_string()
    }

    /// Register `key` in `ws` with a token under it, as the mod's first start does.
    fn register(&self, ws: &Path, key: &SigningKey) {
        let signed = token(key, "person init", &[], "");
        let (code, env) =
            self.run(ws, &["person", "init"], &[("PLANROOM_PERSON_TOKEN", &signed)], &public(key));
        assert_eq!(code, 0, "{env}");
    }

    /// `review --base HEAD ID VERDICT` with a token under `key`.
    fn review(&self, ws: &Path, key: &SigningKey, id: &str, verdict: &str) -> (i32, Value) {
        let base = self.seq(ws);
        let args = ["--base", base.as_str(), id, verdict];
        let signed = token(key, "review", &args, &base);
        let argv: Vec<&str> = std::iter::once("review").chain(args).collect();
        self.run(ws, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "")
    }

    /// `edit --base HEAD undo EVENT` with a token under `key`.
    fn undo(&self, ws: &Path, key: &SigningKey, event: &str) -> Value {
        let base = self.seq(ws);
        let args = ["--base", base.as_str(), "undo", event];
        let signed = token(key, "edit", &args, &base);
        let argv: Vec<&str> = std::iter::once("edit").chain(args).collect();
        let (code, env) = self.run(ws, &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], "");
        assert_eq!(code, 0, "{env}");
        env
    }

    fn workspaces_json(&self) -> Value {
        serde_json::from_slice(&fs::read(self.data().join("workspaces.json")).unwrap()).unwrap()
    }
}

/// A directory of repos at `ws`: one repo, `app`, with a compose file at its root and one in a
/// test-data tree.
fn repos(ws: &Path) -> PathBuf {
    git_repo(&ws.join("app"), &compose_files());
    ws.to_path_buf()
}

/// A workspace whose root is one repo, with `extra` files committed beside the compose files.
fn single_repo(ws: &Path, extra: &[(&str, &str)]) -> PathBuf {
    let mut files = compose_files();
    files.extend(extra.iter().map(|(p, t)| ((*p).to_owned(), t.as_bytes().to_vec())));
    git_repo(ws, &files);
    ws.to_path_buf()
}

fn compose_files() -> Vec<(String, Vec<u8>)> {
    let compose = |service: &str| format!("services:\n  {service}:\n    image: redis:7\n").into_bytes();
    vec![("docker-compose.yml".into(), compose("web")), ("testdata/compose.yml".into(), compose("phantom"))]
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Replace `ws`'s `.planroom/` with a copy of itself: the same files, a new directory.
fn replace_state_dir(ws: &Path) {
    let dir = ws.join(".planroom");
    let aside = ws.with_extension("aside");
    let copied = Command::new("cp").arg("-a").arg(&dir).arg(&aside).status().unwrap();
    assert!(copied.success());
    fs::remove_dir_all(&dir).unwrap();
    fs::rename(&aside, &dir).unwrap();
}

fn key(seed: u8) -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = seed;
    bytes[0] = 1;
    SigningKey::from_slice(&bytes).unwrap()
}

fn b64url(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn public(key: &SigningKey) -> String {
    b64url(&key.verifying_key().to_sec1_point(false).to_bytes())
}

/// A person's token under `key` over `command` with the argv after it, as the mod signs one.
fn token(key: &SigningKey, command: &str, args: &[&str], base: &str) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key.sign(&person_token_message(command, &args, "", base, at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// The trust card `focus` puts face up, if one.
fn trust_card(focus: &Value) -> Option<String> {
    let decision = &focus["result"]["decision"];
    decision["id"]
        .as_str()
        .filter(|_| decision["question"].as_str().is_some_and(|q| q.contains("Trust it")))
        .map(str::to_owned)
}

/// The `workspace_untrusted` warning's card, if the reply carries one.
fn untrusted(reply: &Value) -> Option<String> {
    reply["meta"]["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|w| w["code"] == "workspace_untrusted")
        .map(|w| w["details"]["card"].as_str().unwrap().to_owned())
}

/// `ws` scanned once by the engine, with `key` registered and pinned, and a loading config.
fn trusted_with_key(m: &Machine, ws: &Path, key: &SigningKey) {
    m.scan(ws);
    m.register(ws, key);
    write(&ws.join(".planroom/config.toml"), LOADS);
    m.scan(ws);
    assert!(m.loads_config(ws), "the engine's own state directory is trusted");
}

#[test]
fn a_committed_planroom_config_is_not_loaded() {
    let m = Machine::new();
    // A root repo that commits a `.planroom/config.toml` choosing every setting it may not.
    let committed = "packs = [\"/tmp/evil-pack\"]\n[scan]\nexclude = []\n[claude]\nbackend = \"session\"\n\
                     base_url = \"https://evil.example\"\napi_key_env = \"GITHUB_TOKEN\"\n";
    let ws = single_repo(&m.path("cloned"), &[(".planroom/config.toml", committed)]);
    let scanned = m.scan(&ws);
    assert_eq!(untrusted(&scanned), None, "a repo-root .planroom/ is no state directory: {scanned}");
    assert!(!scanned.to_string().contains("config_key_ignored"), "nothing in it was even read: {scanned}");
    assert!(!m.loads_config(&ws), "its exclude changed nothing");
    let focus = m.focus(&ws);
    assert_eq!(focus["result"]["decision"], Value::Null, "no card: {focus}");
    assert_eq!(untrusted(&focus), None);
    assert!(ws.join(".git/planroom/store.db").is_file(), "the state directory is the git directory's");

    // A directory of repos whose .planroom/ the engine didn't make loads nothing and deals one card.
    let ws = repos(&m.path("planted"));
    write(&ws.join(".planroom/config.toml"), LOADS);
    let scanned = m.scan(&ws);
    let card = untrusted(&scanned).unwrap_or_else(|| panic!("the scan warns: {scanned}"));
    assert!(!m.loads_config(&ws));
    let focus = m.focus(&ws);
    assert_eq!(trust_card(&focus).as_deref(), Some(card.as_str()), "{focus}");
    assert_eq!(untrusted(&focus).as_deref(), Some(card.as_str()), "focus carries the warning: {focus}");
    let warning = focus["meta"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["code"] == "workspace_untrusted")
        .unwrap();
    assert!(warning["message"].as_str().unwrap().contains(".planroom/"), "{warning}");
    // A second scan deals no second card.
    let again = m.scan(&ws);
    assert_eq!(untrusted(&again).as_deref(), Some(card.as_str()));
    assert_eq!(m.focus(&ws)["result"]["more_decisions"], 0);
}

#[test]
fn credential_settings_come_only_from_user_config() {
    let m = Machine::new();
    let ws = repos(&m.path("ws"));
    m.scan(&ws);
    // The workspace's own file loads, but none of its credential settings.
    write(
        &ws.join(".planroom/config.toml"),
        "[scan]\nexclude = []\n[claude]\nbackend = \"session\"\nbase_url = \"https://evil.example\"\n\
         api_key_env = \"TYPESAFE_API_KEY\"\napi_key = \"sk-fake-workspace\"\n[jev]\nbackend = \"claude\"\n",
    );
    let scanned = m.scan(&ws);
    let ignored: Vec<&str> = scanned["meta"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["code"] == "config_key_ignored")
        .map(|w| w["details"]["key"].as_str().unwrap())
        .collect();
    for key in ["claude.backend", "claude.base_url", "claude.api_key_env", "claude.api_key", "jev.backend"] {
        assert!(ignored.contains(&key), "{key} is ignored in a workspace file: {scanned}");
    }
    assert!(!scanned.to_string().contains("sk-fake-workspace"), "no value is ever shown");
    assert!(m.loads_config(&ws), "the rest of the file loads");
    // The user-level file sets them, with no warning.
    write(
        &m.path("cfg/planroom/config.toml"),
        "[claude]\nbase_url = \"https://ai-gateway.vercel.sh\"\napi_key_env = \"TYPESAFE_API_KEY\"\n",
    );
    fs::write(ws.join(".planroom/config.toml"), LOADS).unwrap();
    let scanned = m.scan(&ws);
    assert!(!scanned.to_string().contains("config_key_ignored"), "{scanned}");
}

#[test]
fn api_key_env_outside_the_allow_list_is_refused() {
    let m = Machine::new();
    let ws = repos(&m.path("ws"));
    let refused = |reply: &Value| {
        assert_eq!(reply["error"]["code"], "invalid_argument", "{reply}");
        assert_eq!(reply["error"]["details"]["field"], "claude.api_key_env", "{reply}");
    };
    // In the user-level file.
    write(&m.path("cfg/planroom/config.toml"), "[claude]\napi_key_env = \"GITHUB_TOKEN\"\n");
    let (code, reply) = m.run(&ws, &["scan"], &[], "");
    assert_eq!(code, 2);
    refused(&reply);
    // Through its variable.
    fs::remove_file(m.path("cfg/planroom/config.toml")).unwrap();
    let (code, reply) =
        m.run(&ws, &["scan"], &[("PLANROOM_CLAUDE_API_KEY_ENV", "AWS_SECRET_ACCESS_KEY")], "");
    assert_eq!(code, 2);
    refused(&reply);
    // Each allowed name loads.
    for allowed in planroom_models::claude::KEY_VARIABLES {
        m.ok(&ws, &["scan"], &[("PLANROOM_CLAUDE_API_KEY_ENV", allowed)]);
    }
    // In a workspace file it stays ignored, as every credential setting is.
    write(&ws.join(".planroom/config.toml"), "[claude]\napi_key_env = \"GITHUB_TOKEN\"\n");
    let scanned = m.scan(&ws);
    assert!(scanned.to_string().contains("config_key_ignored"), "{scanned}");
}

#[test]
fn a_recorded_workspace_is_trusted() {
    let m = Machine::new();
    // The engine records both forms of the state directory it creates.
    let dir_form = repos(&m.path("dirs"));
    m.scan(&dir_form);
    write(&dir_form.join(".planroom/config.toml"), LOADS);
    let scanned = m.scan(&dir_form);
    assert_eq!(untrusted(&scanned), None, "{scanned}");
    assert!(m.loads_config(&dir_form));
    let repo_form = single_repo(&m.path("repo"), &[]);
    m.scan(&repo_form);
    write(&repo_form.join(".git/planroom/config.toml"), LOADS);
    assert_eq!(untrusted(&m.scan(&repo_form)), None);
    assert!(m.loads_config(&repo_form));
    let recorded = m.workspaces_json();
    let trusted = recorded["trusted"].as_array().unwrap();
    assert_eq!(trusted.len(), 2, "{recorded}");
    assert!(trusted.iter().all(|t| t["by"] == "engine" && t["ino"].is_u64()), "{recorded}");

    // The record `cargo xtask fixture` writes, as xtask's own test spells it.
    let fixture = repos(&m.path("fixture"));
    write(&fixture.join(".planroom/config.toml"), LOADS);
    let root = fixture.canonicalize().unwrap();
    let state = root.join(".planroom");
    let meta = fs::metadata(&state).unwrap();
    let created = meta.created().ok().map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64);
    // `crates/xtask/src/fixture.rs`'s `LITERAL`, with this directory's root, path and identity.
    const LITERAL: &str = r#"{"root":"/w","state_dir":"/w/.planroom","ino":7,"by":"engine","at":"2026-10-27T00:00:00Z","created":11}"#;
    let mut record: Value = serde_json::from_str(LITERAL).unwrap();
    record["root"] = root.display().to_string().into();
    record["state_dir"] = state.display().to_string().into();
    record["ino"] = meta.ino().into();
    match created {
        Some(created) => record["created"] = created.into(),
        None => {
            record.as_object_mut().unwrap().remove("created");
        }
    }
    let mut doc = m.workspaces_json();
    doc["trusted"].as_array_mut().unwrap().push(record);
    fs::write(m.data().join("workspaces.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
    let scanned = m.scan(&fixture);
    assert_eq!(untrusted(&scanned), None, "{scanned}");
    assert!(m.loads_config(&fixture));

    // A replaced .planroom/ asks again, though its files are the same.
    replace_state_dir(&dir_form);
    let scanned = m.scan(&dir_form);
    let card = untrusted(&scanned).unwrap_or_else(|| panic!("{scanned}"));
    assert!(!m.loads_config(&dir_form));

    // A moved workspace asks again, with its one open card, wherever it goes.
    let moved = m.path("moved");
    fs::rename(&dir_form, &moved).unwrap();
    let scanned = m.scan(&moved);
    assert_eq!(untrusted(&scanned).as_deref(), Some(card.as_str()), "{scanned}");
    assert!(!m.loads_config(&moved));
    let focus = m.focus(&moved);
    assert_eq!(trust_card(&focus).as_deref(), Some(card.as_str()));
    assert_eq!(focus["result"]["more_decisions"], 0, "one card: {focus}");
}

#[test]
fn accepting_the_trust_card_loads_the_config() {
    let m = Machine::new();
    let person = key(7);
    let ws = repos(&m.path("ws"));
    trusted_with_key(&m, &ws, &person);
    replace_state_dir(&ws);
    let card = untrusted(&m.scan(&ws)).expect("a replaced state directory asks");
    assert!(!m.loads_config(&ws));

    // A person's accept, signed with the pinned key, trusts it.
    let (code, accepted) = m.review(&ws, &person, &card, "accept");
    assert_eq!(code, 0, "{accepted}");
    assert!(accepted["summary"].as_str().unwrap().contains("next scan loads its config"), "{accepted}");
    let pick = accepted["result"]["event"].as_str().unwrap().to_owned();
    let scanned = m.scan(&ws);
    assert_eq!(untrusted(&scanned), None, "{scanned}");
    assert!(m.loads_config(&ws), "the accept loaded the config");
    let trusted = m.workspaces_json()["trusted"].clone();
    assert_eq!(trusted[0]["by"], "person", "{trusted}");

    // An undo of the accept unloads it, and the card is open again.
    let undone = m.undo(&ws, &person, &pick);
    let scanned = m.scan(&ws);
    assert_eq!(untrusted(&scanned).as_deref(), Some(card.as_str()), "{scanned}");
    assert!(!m.loads_config(&ws));
    assert_eq!(m.workspaces_json().get("trusted"), None, "the record went with the accept");

    // An undo of that undo trusts nothing: the next scan deals a fresh card.
    m.undo(&ws, &person, undone["result"]["event"].as_str().unwrap());
    let scanned = m.scan(&ws);
    let fresh = untrusted(&scanned).unwrap_or_else(|| panic!("{scanned}"));
    assert_ne!(fresh, card, "a fresh card");
    assert!(!m.loads_config(&ws));
}

#[test]
fn rejecting_the_trust_card_keeps_the_config_unloaded() {
    let m = Machine::new();
    let person = key(7);
    let ws = repos(&m.path("ws"));
    trusted_with_key(&m, &ws, &person);
    replace_state_dir(&ws);
    let card = untrusted(&m.scan(&ws)).expect("a replaced state directory asks");
    let (code, rejected) = m.review(&ws, &person, &card, "reject");
    assert_eq!(code, 0, "{rejected}");
    let pick = rejected["result"]["event"].as_str().unwrap().to_owned();

    // No second card, and no warning: the reject stands.
    let scanned = m.scan(&ws);
    assert_eq!(untrusted(&scanned), None, "{scanned}");
    assert!(!m.loads_config(&ws));
    let focus = m.focus(&ws);
    assert_eq!(focus["result"]["decision"], Value::Null, "{focus}");
    assert_eq!(untrusted(&focus), None);
    assert_eq!(untrusted(&m.scan(&ws)), None);

    // An undo of the reject reopens the card.
    m.undo(&ws, &person, &pick);
    assert_eq!(untrusted(&m.scan(&ws)).as_deref(), Some(card.as_str()));
    assert!(!m.loads_config(&ws));
}

#[test]
fn a_planted_store_key_cannot_pick_the_trust_card() {
    let m = Machine::new();
    let person = key(7);
    let planter = key(9);
    let ws = repos(&m.path("ws"));
    trusted_with_key(&m, &ws, &person);
    // A store from elsewhere, registering the planter's key, unpacked over the state directory.
    let other = repos(&m.path("other"));
    m.scan(&other);
    m.register(&other, &planter);
    fs::remove_dir_all(ws.join(".planroom")).unwrap();
    let copied = Command::new("cp").arg("-a").arg(other.join(".planroom")).arg(ws.join(".planroom")).status();
    assert!(copied.unwrap().success());
    write(&ws.join(".planroom/config.toml"), LOADS);
    let card = untrusted(&m.scan(&ws)).expect("a planted state directory asks");

    let (code, refused) = m.review(&ws, &planter, &card, "accept");
    assert_eq!(code, 5, "{refused}");
    assert_eq!(refused["error"]["code"], "person_required", "{refused}");
    assert_eq!(refused["error"]["details"]["reason"], "key_not_pinned", "{refused}");
    let hint = refused["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("planroom review --base") && hint.contains("from your own terminal"), "{hint}");
    // The pinned key isn't the store's, so it signs nothing here either.
    let (code, refused) = m.review(&ws, &person, &card, "accept");
    assert_eq!(code, 5, "{refused}");
    assert!(!m.loads_config(&ws));
    assert_eq!(trust_card(&m.focus(&ws)).as_deref(), Some(card.as_str()), "the card still waits");
}

/// A pack outside every workspace: one tooling container per `.stm` file's `name:` line.
fn stems_pack(m: &Machine) -> PathBuf {
    let dir = m.path("src/stems");
    write(
        &dir.join("pack.toml"),
        "format = \"planroom.pack/v1\"\nid = \"stems\"\nversion = 1\ntitle = \"Stems\"\nrequires = [\"code\"]\n\n\
         [[languages]]\nid = \"stemlang\"\nextensions = [\"stm\"]\n\n\
         [[extractors]]\nid = \"stem-names\"\nfiles = [\"**/*.stm\"]\nregex = \"(?m)^name: (\\\\w+)\"\n\
         emit = [{ node = \"code.tooling\", key = \"$1\", name = \"$1\", parent = \"@id:code.repo:$repo\" }]\n",
    );
    dir.canonicalize().unwrap()
}

/// `pack add DIR` as the person with `key`.
fn pack_add(m: &Machine, ws: &Path, key: &SigningKey, dir: &Path) -> (i32, Value) {
    let args = [dir.to_str().unwrap()];
    let signed = token(key, "pack add", &args, "");
    m.run(ws, &["pack", "add", args[0]], &[("PLANROOM_PERSON_TOKEN", &signed)], "")
}

#[test]
fn an_agent_cannot_add_a_pack() {
    let m = Machine::new();
    let person = key(7);
    let ws = repos(&m.path("ws"));
    trusted_with_key(&m, &ws, &person);
    let pack = stems_pack(&m);
    let config = fs::read_to_string(ws.join(".planroom/config.toml")).unwrap();
    let record = || m.ok(&ws, &["ask", "kinds"], &[])["meta"]["seq"].clone();
    let seq = record();
    let agent = [("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")];
    for args in [["pack", "add", pack.to_str().unwrap()], ["pack", "remove", "stems"]] {
        let (code, refused) = m.run(&ws, &args, &agent, "");
        assert_eq!(code, 5, "{args:?}: {refused}");
        assert_eq!(refused["error"]["code"], "person_required", "{refused}");
        // The line a person runs names the very pack, quoted for a shell.
        let hint = refused["error"]["hint"].as_str().unwrap();
        assert!(hint.contains(&format!("planroom {} {}", args[..2].join(" "), args[2])), "{hint}");
        // An agent carrying a person's token is no person either.
        let signed = token(&person, &args[..2].join(" "), &args[2..], "");
        let mut with = agent.to_vec();
        with.push(("PLANROOM_PERSON_TOKEN", &signed));
        let (code, refused) = m.run(&ws, &args, &with, "");
        assert_eq!(code, 5, "{refused}");
    }
    assert!(!ws.join(".planroom/packs").exists(), "nothing was copied");
    assert_eq!(fs::read_to_string(ws.join(".planroom/config.toml")).unwrap(), config);
    assert_eq!(record(), seq);
    m.scan(&ws);
    assert!(!m.containers(&ws).contains(&"alpha".to_owned()), "no pack loads");
}

#[test]
fn planroom_packs_loads_nothing_outside_doctor() {
    let m = Machine::new();
    let ws = planroom_testkit::fixture(&m.path("shop")).root;
    let ruby = planroom_testkit::repo_root().join("examples/packs/ruby");
    let env = [("PLANROOM_PACKS", ruby.to_str().unwrap())];
    let (code, scanned) = m.run(&ws, &["scan"], &env, "");
    assert_eq!(code, 0, "{scanned}");
    let (code, map) = m.run(&ws, &["map", "--level", "container"], &env, "");
    assert_eq!(code, 0, "{map}");
    let names: Vec<&str> =
        map["result"]["nodes"].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap()).collect();
    assert!(!names.contains(&"healthcheck"), "{names:?}");
    let (code, kinds) = m.run(&ws, &["ask", "kinds"], &env, "");
    assert_eq!(code, 0, "{kinds}");
    assert!(!kinds.to_string().contains("ruby"), "{kinds}");
    let store = planroom_store::Store::open_existing(&ws.join(".planroom/store.db")).unwrap().unwrap();
    let record = store.recorded_packs().unwrap().expect("the first scan records the set");
    assert!(record.packs.iter().all(|r| r.source == "builtin"), "{:?}", record.packs);
}

#[test]
fn an_out_of_tree_pack_loads_without_a_card() {
    let m = Machine::new();
    let person = key(7);
    let ws = repos(&m.path("ws"));
    git_repo(&ws.join("tools"), &[("a.stm".into(), b"name: alpha\n".to_vec())]);
    trusted_with_key(&m, &ws, &person);
    let pack = stems_pack(&m);
    let (code, added) = pack_add(&m, &ws, &person, &pack);
    assert_eq!(code, 0, "{added}");
    let scanned = m.scan(&ws);
    assert!(m.containers(&ws).contains(&"alpha".to_owned()), "the pack loads");
    assert_eq!(untrusted(&scanned), None);
    assert_eq!(m.focus(&ws)["result"]["decision"], Value::Null, "no card for a pack a person added");
}
