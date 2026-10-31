//! Starting from nothing: `intent new` in a folder with no store makes a plan-only workspace
//! (DR-115).

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use serde_json::Value;

const AGENT: &str = "agent:aaaaaaaa:chat";

fn key() -> SigningKey {
    let mut bytes = [0u8; 32];
    bytes[31] = 7;
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

/// One temporary tree per test: `work/` is the folder under test, `data/` the data root and
/// `home/` the home directory, so the data root never sits inside the folder.
struct Tree {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Tree {
    fn new() -> Tree {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        for dir in ["work", "data", "home"] {
            std::fs::create_dir(root.join(dir)).unwrap();
        }
        Tree { _tmp: tmp, root }
    }

    fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// `planroom ARGS --json` in `cwd` with `env`, stdin piped: the exit code and the one envelope.
    fn run_in(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, Value) {
        self.run_with(cwd, args, env, "")
    }

    /// [`Tree::run_in`] with `stdin` written to the command.
    fn run_with(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (i32, Value) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
        cmd.current_dir(cwd)
            .env("PLANROOM_DATA", self.data())
            .env("HOME", self.home())
            .env_remove("PLANROOM_PERSON_TOKEN")
            .env_remove("PLANROOM_ACTOR")
            .env_remove("PLANROOM_CLIENT")
            .args(args)
            .arg("--json");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child =
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
        }
        let out = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1, "{args:?}: one JSON line: {stdout}");
        let env = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}"));
        (out.status.code().unwrap(), env)
    }

    /// `planroom ARGS` in `work/` as the agent.
    fn agent(&self, args: &[&str]) -> (i32, Value) {
        self.run_in(&self.work(), args, &[("PLANROOM_ACTOR", AGENT)])
    }

    /// `planroom COMMAND ARGS --json` in `work/` as the person, signed; `base` is the `--base` the
    /// args carry, `stdin` what the command reads.
    fn person(&self, command: &str, args: &[&str], base: &str, stdin: &str) -> (i32, Value) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let sig: Signature = key().sign(&person_token_message(command, &args, "", base, at));
        let signed = format!("{at}.{}", b64url(&sig.to_bytes()));
        let argv: Vec<&str> = command.split(' ').chain(args.iter().map(String::as_str)).collect();
        self.run_with(&self.work(), &argv, &[("PLANROOM_PERSON_TOKEN", &signed)], stdin)
    }

    /// `data/workspaces.json`, parsed.
    fn workspaces(&self) -> Value {
        let text = std::fs::read_to_string(self.data().join("workspaces.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn intent_new_in_an_empty_folder_starts_a_plan_only_workspace() {
    let tree = Tree::new();
    let work = tree.work();

    // Nobody to write as: refused, and nothing is made.
    let (code, env) = tree.run_in(&work, &["intent", "new", "Add a billing service"], &[]);
    assert_eq!((code, &env["error"]["code"]), (2, &Value::from("actor_required")), "{env}");
    assert!(!work.join(".planroom").exists(), "a refusal makes nothing");

    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let summary = env["summary"].as_str().unwrap();
    assert!(summary.contains("in work/"), "{summary}");
    assert!(summary.contains("No repos yet"), "{summary}");

    let state = work.join(".planroom");
    assert!(state.is_dir(), "the state directory is made");
    assert_eq!(std::fs::metadata(&state).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::read_to_string(state.join(".gitignore")).unwrap(), "*\n");
    assert!(state.join("store.db").exists());

    // Trusted by the engine; no `workspaces.json` entry, which only `serve` writes.
    let workspaces = tree.workspaces();
    let trusted = workspaces["trusted"].as_array().unwrap();
    assert_eq!(trusted.len(), 1, "{workspaces}");
    assert_eq!(trusted[0]["state_dir"], Value::from(state.to_str().unwrap()), "{workspaces}");
    assert_eq!(trusted[0]["by"], "engine");
    assert!(workspaces.get("workspaces").is_none_or(|w| w.as_array().unwrap().is_empty()), "{workspaces}");

    // The workspace exists now: a second plan is an ordinary write.
    let (code, env) = tree.agent(&["intent", "new", "Add a search page"]);
    assert_eq!(code, 0, "{env}");
    let summary = env["summary"].as_str().unwrap();
    assert!(summary.starts_with("Started the plan"), "{summary}");
    assert!(!summary.contains("No repos yet"), "{summary}");
}

#[test]
fn a_person_token_cannot_start_a_workspace_no_key_is_registered_in() {
    let tree = Tree::new();
    let work = tree.work();
    let (code, env) = tree.run_in(
        &work,
        &["intent", "new", "Add a billing service"],
        &[("PLANROOM_PERSON_TOKEN", "1.AAAA")],
    );
    assert_eq!((code, &env["error"]["code"]), (5, &Value::from("person_required")), "{env}");
    let hint = env["error"]["hint"].as_str().unwrap();
    assert!(hint.starts_with("claude, started in this workspace"), "{hint}");
    assert!(!work.join(".planroom").exists(), "a refusal makes nothing");
}

// The filesystem root's half is checked on the path alone, in confine.rs's
// `a_plan_folder_at_the_filesystem_root_is_refused`: a command run at `/` here would write the
// host's `/.planroom` the moment the refusal regressed.
#[test]
fn intent_new_refuses_the_home_directory_and_the_filesystem_root() {
    let tree = Tree::new();
    let home = tree.home();
    assert!(!home.join(".planroom").exists());
    let (code, env) =
        tree.run_in(&home, &["intent", "new", "Add a billing service"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(code, 5, "{env}");
    assert_eq!(env["error"]["code"], "path_refused", "{env}");
    assert_eq!(env["error"]["details"]["reason"], "home_directory", "{env}");
    assert_eq!(env["error"]["hint"], "mkdir DIR && cd DIR", "{env}");
    assert!(!home.join(".planroom").exists());
}

#[test]
fn an_agents_intent_new_in_an_empty_folder_waits_as_a_ghost_project() {
    let tree = Tree::new();
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let event = env["result"]["event"].as_str().unwrap().to_owned();

    let (code, env) = tree.agent(&["intent", "list"]);
    assert_eq!(code, 0, "{env}");
    let projects = env["result"]["projects"].as_array().unwrap();
    assert_eq!(projects.len(), 1, "{env}");
    assert_eq!(projects[0]["status"], "ghost", "an agent's project waits as a proposal: {env}");
    assert_eq!(projects[0]["ghosts"], 1, "{env}");

    let (code, env) = tree.agent(&["focus"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["empty"]["reason"], "plan_only", "{env}");

    // The store exists now, so the person can register their key in it.
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let (code, env) = tree.person("person init", &[], "", &public);
    assert_eq!(code, 0, "{env}");

    let (_, env) = tree.agent(&["focus"]);
    let seq = env["meta"]["seq"].as_u64().unwrap().to_string();
    let (code, env) = tree.person("review", &["--base", &seq, &event, "accept"], &seq, "");
    assert_eq!(code, 0, "{env}");

    let (_, env) = tree.agent(&["intent", "list"]);
    let project = &env["result"]["projects"][0];
    assert_ne!(project["status"], "ghost", "the accepted project is live: {env}");
    assert_eq!(project["ghosts"], 0, "{env}");
    let (_, env) = tree.agent(&["focus"]);
    assert_eq!(env["result"]["empty"]["reason"], "plan_only", "{env}");
}

/// `git -C DIR ARGS` with a fixed identity, panicking on a failure.
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "Test Author")
        .env("GIT_COMMITTER_EMAIL", "author@example.com")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// The ID of the project named `name`, read from `intent list`.
fn project_id(tree: &Tree, name: &str) -> String {
    let (code, env) = tree.agent(&["intent", "list"]);
    assert_eq!(code, 0, "{env}");
    let projects = env["result"]["projects"].as_array().unwrap();
    let project = projects.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("{name}: {env}"));
    project["id"].as_str().unwrap().to_owned()
}

#[test]
fn scan_in_a_plan_only_workspace_maps_nothing_and_keeps_the_plan() {
    let tree = Tree::new();
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let (_, env) = tree.agent(&["focus"]);
    let seq = env["meta"]["seq"].as_u64().unwrap();

    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["summary"], "No repos to map yet; your plan is kept.", "{env}");
    assert_eq!(env["result"]["map"]["empty"]["reason"], "plan_only", "{env}");
    assert_eq!(env["meta"]["seq"].as_u64().unwrap(), seq, "a scan stores nothing: {env}");

    let (_, env) = tree.agent(&["focus"]);
    assert_eq!(env["meta"]["seq"].as_u64().unwrap(), seq, "{env}");
    assert!(env["result"]["decision"].is_null(), "no card is dealt: {env}");
    let (code, env) = tree.agent(&["intent", "list"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["projects"].as_array().unwrap().len(), 1, "{env}");

    let other = tree.root.join("other");
    std::fs::create_dir(&other).unwrap();
    let (code, env) = tree.run_in(&other, &["scan"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!((code, &env["error"]["code"]), (3, &Value::from("workspace_not_found")), "{env}");
    assert!(!other.join(".planroom").exists(), "a refused scan makes nothing");
}

#[test]
fn focusing_a_project_with_no_items_shows_its_empty_view() {
    let tree = Tree::new();
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let p = project_id(&tree, "Add a billing service");

    let shows_empty = |id: &str, name: &str| {
        let (code, env) = tree.agent(&["map", "--level", "intent", "--focus", id]);
        assert_eq!(code, 0, "{env}");
        let result = &env["result"];
        assert_eq!(result["focus"], id, "{env}");
        assert_eq!(result["nodes"].as_array().unwrap().len(), 0, "{env}");
        assert_eq!(result["empty"]["reason"], "no_plan_items", "{env}");
        assert!(result["empty"]["line"].as_str().unwrap().contains(name), "{env}");
        let next = &result["empty"]["next"];
        assert_eq!(next["command"], "ask", "{env}");
        assert_eq!(next["args"][0], "stage", "{env}");
        assert_eq!(next["args"][2], id, "{env}");
    };
    shows_empty(&p, "Add a billing service");

    // The same in a workspace that has mapped a repo.
    let repo = tree.work().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    std::fs::write(repo.join("README.md"), "# repo\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "first"]);
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    let (code, env) = tree.agent(&["intent", "new", "Add a search page"]);
    assert_eq!(code, 0, "{env}");
    let q = project_id(&tree, "Add a search page");
    shows_empty(&q, "Add a search page");
}

/// A committed repo at `dir`, made there.
fn committed_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-b", "main"]);
    std::fs::write(dir.join("README.md"), "# repo\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "first"]);
}

#[test]
fn a_project_with_no_items_never_takes_a_blocks_focus() {
    let tree = Tree::new();
    committed_repo(&tree.work().join("billing"));
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    let (code, env) = tree.agent(&["map", "--focus", "billing"]);
    assert_eq!(code, 0, "{env}");
    let block = env["result"]["focus"].as_str().unwrap().to_owned();
    assert!(!block.starts_with("plan:"), "{env}");

    // An agent's proposal named as the block is: the block keeps the focus.
    let (code, env) = tree.agent(&["intent", "new", "billing"]);
    assert_eq!(code, 0, "{env}");
    let p = project_id(&tree, "billing");
    let (code, env) = tree.agent(&["map", "--focus", "billing"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["focus"], Value::from(block.as_str()), "{env}");

    // The project's ID still names it, and the level asked for is the level shown.
    let (code, env) = tree.agent(&["map", "--focus", &p]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["focus"], Value::from(p.as_str()), "{env}");
    assert_eq!(env["result"]["level"], "intent", "{env}");
    assert_eq!(env["result"]["empty"]["reason"], "no_plan_items", "{env}");
    let (code, env) = tree.agent(&["map", "--level", "container", "--focus", &p]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["level"], "container", "{env}");
    assert_eq!(env["result"]["empty"]["reason"], "no_plan_items", "{env}");

    // A key names the project its ID is built from; the word every project ID ends with names none.
    let (code, env) = tree.agent(&["intent", "new", "Add a search page"]);
    assert_eq!(code, 0, "{env}");
    let q = project_id(&tree, "Add a search page");
    let key = q.strip_prefix("plan:").and_then(|k| k.strip_suffix(":project")).unwrap().to_owned();
    let (code, env) = tree.agent(&["map", "--focus", &key]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["focus"], Value::from(q.as_str()), "{env}");
    let (code, env) = tree.agent(&["map", "--focus", "project"]);
    assert_eq!((code, &env["error"]["code"]), (3, &Value::from("not_found")), "{env}");
}

#[test]
fn intent_new_beside_repos_never_says_there_are_none() {
    let tree = Tree::new();
    let work = tree.work();
    committed_repo(&work.join("a"));
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let summary = env["summary"].as_str().unwrap();
    assert!(summary.starts_with("Started the plan"), "{summary}");
    assert!(!summary.contains("No repos yet"), "a folder of repos: {summary}");

    let repo = tree.root.join("repo");
    committed_repo(&repo);
    let (code, env) =
        tree.run_in(&repo, &["intent", "new", "Add a search page"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(code, 0, "{env}");
    let summary = env["summary"].as_str().unwrap();
    assert!(summary.starts_with("Started the plan"), "{summary}");
    assert!(!summary.contains("No repos yet"), "a repo's root: {summary}");
}

#[test]
fn intent_new_in_a_repos_working_tree_is_refused_as_scan_refuses_there() {
    let tree = Tree::new();
    let repo = tree.work().join("repo");
    committed_repo(&repo);
    let inside = repo.join("src");
    std::fs::create_dir(&inside).unwrap();

    let (code, scanned) = tree.run_in(&inside, &["scan"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!((code, &scanned["error"]["code"]), (3, &Value::from("workspace_not_found")), "{scanned}");
    let (code, env) =
        tree.run_in(&inside, &["intent", "new", "Add a billing service"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(code, 3, "{env}");
    for field in ["code", "message", "details", "hint"] {
        assert_eq!(env["error"][field], scanned["error"][field], "{field}: {env}");
    }
    assert!(!inside.join(".planroom").exists(), "a refusal makes nothing");
    assert!(!repo.join(".git/planroom").exists(), "a refusal makes nothing");
}

#[test]
fn a_folder_in_an_unscanned_repo_offers_that_repos_scan_not_a_plan() {
    let tree = Tree::new();
    let repo = tree.work().join("repo");
    committed_repo(&repo);
    let inside = repo.join("src");
    std::fs::create_dir(&inside).unwrap();
    let root = repo.to_str().unwrap();
    let as_agent = [("PLANROOM_ACTOR", AGENT)];

    // No repo lies below the folder, and intent new is refused there, so the empty state offers
    // the scan of the repo it lies in, never a plan.
    let (code, focus) = tree.run_in(&inside, &["focus"], &as_agent);
    assert_eq!(code, 0, "{focus}");
    let (code, map) = tree.run_in(&inside, &["map"], &as_agent);
    assert_eq!(code, 0, "{map}");
    let empty = &focus["result"]["empty"];
    assert_eq!(empty, &map["result"]["empty"], "focus and map say the same");
    assert_eq!(empty["reason"], "no_repos", "{focus}");
    assert_eq!(
        empty["line"],
        "This folder is in the repo repo, which isn't mapped yet. Map it to start a plan here.",
        "{focus}"
    );
    assert_eq!(
        empty["next"],
        serde_json::json!({"command": "scan", "args": [root], "label": "Map the repo this folder is in? About 2 s."}),
        "{focus}"
    );

    // The scan runs from where it is offered, and maps the repo the folder is in.
    let (code, env) = tree.run_in(&inside, &["scan", root], &as_agent);
    assert_eq!(code, 0, "{env}");
    assert!(repo.join(".git/planroom").is_dir(), "the store is the repo's: {env}");
    let (code, focus) = tree.run_in(&inside, &["focus"], &as_agent);
    assert_eq!(code, 0, "{focus}");
    assert!(focus["result"]["empty"].is_null(), "{focus}");
    let (code, env) = tree.run_in(&inside, &["intent", "new", "Add a billing service"], &as_agent);
    assert_eq!(code, 0, "a plan starts once the repo is mapped: {env}");
}

/// The empty state `focus` and `map` give in `inside`, a folder below `repo`'s root, after
/// checking that they agree with each other and with the one `focus` gives at `repo`'s root.
fn empty_below(tree: &Tree, repo: &Path, inside: &Path) -> Value {
    let as_agent = [("PLANROOM_ACTOR", AGENT)];
    let (code, focus) = tree.run_in(inside, &["focus"], &as_agent);
    assert_eq!(code, 0, "{focus}");
    let (code, map) = tree.run_in(inside, &["map"], &as_agent);
    assert_eq!(code, 0, "{map}");
    let (code, at_root) = tree.run_in(repo, &["focus"], &as_agent);
    assert_eq!(code, 0, "{at_root}");
    let empty = focus["result"]["empty"].clone();
    assert_eq!(empty, map["result"]["empty"], "focus and map say the same");
    assert_eq!(empty, at_root["result"]["empty"], "the folder says what the repo's root says");
    empty
}

#[test]
fn a_folder_in_a_repo_with_no_commit_says_so_instead_of_offering_its_scan() {
    let tree = Tree::new();
    let repo = tree.work().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    let inside = repo.join("src");
    std::fs::create_dir(&inside).unwrap();

    // A scan of a repo with no commit maps nothing and makes no store, so offering it would come
    // back forever: the folder says what the repo's root says, and checks again.
    let empty = empty_below(&tree, &repo, &inside);
    assert_eq!(empty["reason"], "repos_unborn", "{empty}");
    assert_eq!(
        empty["line"],
        "No commits yet in repo, so there's nothing to map. Make a first commit, then check again.",
        "{empty}"
    );
    assert_eq!(empty["next"], serde_json::json!({"command": "focus", "args": [], "label": "Check again"}));

    // Its first commit makes it one a scan maps, so checking again offers that scan.
    std::fs::write(repo.join("README.md"), "# repo\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "first"]);
    let (_, focus) = tree.run_in(&inside, &["focus"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(focus["result"]["empty"]["next"]["command"], "scan", "{focus}");
    assert_eq!(
        focus["result"]["empty"]["next"]["args"],
        serde_json::json!([repo.to_str().unwrap()]),
        "{focus}"
    );
}

#[test]
fn a_folder_in_a_repo_git_cannot_read_says_so_instead_of_offering_its_scan() {
    let tree = Tree::new();
    let repo = tree.work().join("repo");
    committed_repo(&repo);
    let inside = repo.join("src");
    std::fs::create_dir(&inside).unwrap();
    let config = std::fs::read(repo.join(".git/config")).unwrap();
    std::fs::write(repo.join(".git/config"), "[core\nthis is not a config\n").unwrap();

    // A scan skips a repo git can't read, maps nothing and makes no store, so offering it would
    // come back forever: the folder says what the repo's root says, and checks again.
    let empty = empty_below(&tree, &repo, &inside);
    assert_eq!(empty["reason"], "repos_unreadable", "{empty}");
    assert_eq!(
        empty["line"],
        "git can't read repo here, so there's nothing to map yet. Run git status in each to see why, then check again.",
        "{empty}"
    );
    assert_eq!(empty["next"], serde_json::json!({"command": "focus", "args": [], "label": "Check again"}));

    // Once git can read it, checking again offers its scan.
    std::fs::write(repo.join(".git/config"), config).unwrap();
    let (_, focus) = tree.run_in(&inside, &["focus"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(focus["result"]["empty"]["next"]["command"], "scan", "{focus}");
    assert_eq!(
        focus["result"]["empty"]["next"]["args"],
        serde_json::json!([repo.to_str().unwrap()]),
        "{focus}"
    );
}

#[test]
fn a_repo_whose_submodules_git_cannot_list_says_so_at_its_root_and_below() {
    let tree = Tree::new();
    let repo = tree.work().join("repo");
    committed_repo(&repo);
    std::fs::write(repo.join(".gitmodules"), "[submodule \"lib\"]\n\tpath = lib\n\turl = ./lib\n").unwrap();
    git(&repo, &["add", ".gitmodules"]);
    git(&repo, &["commit", "-m", "submodules"]);
    let inside = repo.join("src");
    std::fs::create_dir(&inside).unwrap();
    std::fs::write(repo.join(".git/config"), "[core\nthis is not a config\n").unwrap();

    // Listing its submodules fails, so which repos the workspace holds is unknown: the repo counts
    // as one git can't read, at its root and below it, never as no repos at all.
    let empty = empty_below(&tree, &repo, &inside);
    assert_eq!(empty["reason"], "repos_unreadable", "{empty}");
    assert_eq!(
        empty["line"],
        "git can't read repo here, so there's nothing to map yet. Run git status in each to see why, then check again.",
        "{empty}"
    );
    assert_eq!(empty["next"], serde_json::json!({"command": "focus", "args": [], "label": "Check again"}));
}

#[test]
fn a_repo_with_an_oversized_gitmodules_inside_another_repo_never_offers_the_outer_scan() {
    let tree = Tree::new();
    let outer = tree.work().join("outer");
    committed_repo(&outer);
    let inner = outer.join("inner");
    committed_repo(&inner);
    let mut gitmodules = String::from("[submodule \"lib\"]\n\tpath = lib\n\turl = ./lib\n");
    while gitmodules.len() <= 1024 * 1024 {
        gitmodules.push_str("# padding padding padding padding padding padding padding padding\n");
    }
    std::fs::write(inner.join(".gitmodules"), gitmodules).unwrap();
    git(&inner, &["add", ".gitmodules"]);
    git(&inner, &["commit", "-m", "submodules"]);
    let inside = inner.join("src");
    std::fs::create_dir(&inside).unwrap();

    // The inner repo's submodules can't be listed, so the folder names that repo as one git can't
    // read, as its root does, and never offers the scan of the repo around it.
    let empty = empty_below(&tree, &inner, &inside);
    assert_eq!(empty["reason"], "repos_unreadable", "{empty}");
    assert_eq!(
        empty["line"],
        "git can't read inner here, so there's nothing to map yet. Run git status in each to see why, then check again.",
        "{empty}"
    );
    assert_eq!(empty["next"], serde_json::json!({"command": "focus", "args": [], "label": "Check again"}));
}

#[test]
fn scan_with_no_repos_answers_ok_only_where_the_store_holds_a_plan() {
    let tree = Tree::new();
    let work = tree.work();
    committed_repo(&work.join("a"));
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");

    // The repo is gone and no plan was ever made: no scan there can succeed.
    std::fs::remove_dir_all(work.join("a")).unwrap();
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!((code, &env["error"]["code"]), (3, &Value::from("workspace_not_found")), "{env}");
    assert_eq!(env["error"]["details"]["path"], Value::from(work.to_str().unwrap()), "{env}");

    // A plan makes it a plan-only workspace, which a scan keeps.
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["summary"], "No repos to map yet; your plan is kept.", "{env}");
}

/// `git -C DIR ARGS` with a fixed identity, returning its stdout.
fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn a_plan_begun_before_git_init_stays_the_workspace() {
    let tree = Tree::new();
    let work = tree.work();
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    let (_, env) = tree.agent(&["focus"]);
    let seq = env["meta"]["seq"].as_u64().unwrap();

    git(&work, &["init", "-b", "main"]);
    let (code, env) = tree.agent(&["focus"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["empty"]["reason"], "plan_only", "{env}");
    assert_eq!(env["meta"]["seq"].as_u64().unwrap(), seq, "{env}");
    assert!(!work.join(".git/planroom").exists(), "the state directory stays .planroom/");
    project_id(&tree, "Add a billing service");

    std::fs::write(work.join("README.md"), "# Billing\n").unwrap();
    git(&work, &["add", "-A"]);
    assert_eq!(git_out(&work, &["ls-files"]).trim(), "README.md", "the ignore file keeps .planroom/ out");
    git(&work, &["commit", "-m", "start"]);

    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    assert!(!env["result"]["map"]["nodes"].as_array().unwrap().is_empty(), "{env}");
    assert!(!work.join(".git/planroom").exists(), "a scan keeps the state directory too");
    project_id(&tree, "Add a billing service");
}

#[test]
fn a_committed_planroom_dir_at_a_repo_root_still_loads_nothing() {
    let tree = Tree::new();
    let work = tree.work();
    let (code, env) = tree.agent(&["intent", "new", "Add a billing service"]);
    assert_eq!(code, 0, "{env}");
    git(&work, &["init", "-b", "main"]);
    std::fs::write(work.join("README.md"), "# Billing\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-m", "start"]);

    project_id(&tree, "Add a billing service");

    // Once git tracks something in it, the directory is no longer the state directory.
    git(&work, &["add", "-f", ".planroom"]);
    git(&work, &["commit", "-m", "planroom"]);
    let (code, env) = tree.agent(&["focus"]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["empty"]["reason"], "not_scanned", "{env}");
    let (code, env) = tree.agent(&["scan"]);
    assert_eq!(code, 0, "{env}");
    assert!(work.join(".git/planroom/store.db").exists(), "the scan makes its own state directory");

    // A clone that brings a committed .planroom/ loads nothing from it.
    std::fs::write(work.join(".planroom/config.toml"), "[scan]\nexclude = [\"**\"]\n").unwrap();
    git(&work, &["add", "-f", ".planroom/config.toml"]);
    git(&work, &["commit", "-m", "config"]);
    git(&tree.root, &["clone", work.to_str().unwrap(), "clone"]);
    let clone = tree.root.join("clone");
    let (code, env) = tree.run_in(&clone, &["scan"], &[("PLANROOM_ACTOR", AGENT)]);
    assert_eq!(code, 0, "{env}");
    assert!(!env["result"]["map"]["nodes"].as_array().unwrap().is_empty(), "the exclude did not load: {env}");
    assert!(clone.join(".git/planroom/store.db").exists());
}
