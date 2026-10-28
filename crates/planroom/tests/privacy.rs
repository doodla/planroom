//! Redaction on every output path (architecture.md, What leaves the machine; DR-64, DR-75, DR-89):
//! the engine lines that name repo text, a reply's `summary`, an empty state's `line`, and the
//! lines of warnings, errors and stream frames, are masked like the names they hold, while the
//! IDs, handles, counts and commit SHAs beside them stay whole. `PLANROOM_RAW=1` shows them raw
//! only on a read command's `--json` stdout. Every process these tests start holds a fake value
//! for each credential variable, and no output may hold one.

use std::io::{BufRead, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use planroom_contract::config::Claude as ClaudeConfig;
use planroom_contract::{
    CacheTtl, Envelope, ErrorCode, EventBody, FactBatch, Hint, MapView, ModelTier, Node, StreamFrame, Tier,
};
use planroom_models::claude::{self, Call};
use planroom_store::{Shape, Store};
use planroom_testkit::{fixture, git_repo};
use serde_json::{Value, json};

const MASK: &str = "[redacted]";
/// A repo named like an assignment of a password.
const REPO: &str = "db_PASSWORD=hunter2";
/// A directory named like an AWS access key ID.
const DIR: &str = "AKIAIOSFODNN7EXAMPLE";
/// A file named like an assignment of a token.
const FILE: &str = "api_token=tok123.txt";
/// A compose service named like a DSN with its user and password.
const DSN: &str = "postgres://app:hunter2@db";
/// A repo with no commit yet, named like a GitHub token.
const UNBORN: &str = "ghp_0123456789abcdefghijABCDEFGHIJ012345";
/// Each secret's text, none of which may reach a masked output.
const SECRETS: [&str; 5] = ["hunter2", "AKIAIOSFODNN7EXAMPLE", "tok123", "app:hunter2", "ghp_0123456789"];

/// Repo content that holds secrets, as opposed to the names above, in no repo, directory, file or
/// service name: a file's first line, a compose service's image and one of its environment
/// values. Each is a fake that only looks like what it plants.
const CONTENT: [&str; 3] = ["sk_live_51PlantedFakeKey0000", "AKIAZ7PLANTEDEXMPL01", "s3cr3tPl4nt"];
/// The second repo's file, whose first line a file's evidence keeps.
const CHARGE: &str = "// STRIPE_SECRET_KEY=sk_live_51PlantedFakeKey0000\nexport const charge = 1;\n";
/// The second repo's compose file: the image is a prop of the service, and the variable a hint.
const BILLING_COMPOSE: &str = "services:\n  payments:\n    image: acme/payments:AKIAZ7PLANTEDEXMPL01\n    \
                               environment:\n      - DATABASE_URL=postgres://app:s3cr3tPl4nt@db/orders\n";
/// A fake value for each credential variable a model call could read, set on every process
/// these tests start.
const FAKE_KEYS: [(&str, &str); 4] = [
    ("AI_GATEWAY_API_KEY", "fake-gateway-key-PLANTED"),
    ("TYPESAFE_API_KEY", "fake-typesafe-key-PLANTED"),
    ("OPENAI_API_KEY", "fake-openai-key-PLANTED"),
    ("GEMINI_API_KEY", "fake-gemini-key-PLANTED"),
];

fn planroom(root: &Path, raw: bool) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", root.join(".data")).env_remove("PLANROOM_RAW");
    cmd.envs(FAKE_KEYS);
    if raw {
        cmd.env("PLANROOM_RAW", "1");
    }
    cmd
}

/// A `--json` command's envelope, whatever its exit code, with `PLANROOM_RAW=1` when `raw`.
fn json(root: &Path, args: &[&str], raw: bool) -> Value {
    let out = planroom(root, raw).args(args).arg("--json").output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}"))
}

/// A command's text mode, stdout then stderr.
fn text(root: &Path, args: &[&str], raw: bool) -> String {
    let out = planroom(root, raw).args(args).output().unwrap();
    format!("{}{}", String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn assert_masked(what: &str, line: &str) {
    for secret in SECRETS {
        assert!(!line.contains(secret), "{what} shows {secret}: {line}");
    }
    assert!(line.contains(MASK), "{what} masks nothing: {line}");
}

/// The fixture with a repo whose name, directory, file and compose service hold secret shapes, and
/// an unborn repo named like a token, scanned. Returns the root and the scan's envelope.
fn planted(dir: &Path) -> (std::path::PathBuf, Value) {
    let root = fixture(dir).root;
    let compose = format!("services:\n  \"{DSN}\":\n    image: postgres:16\n");
    git_repo(
        &root.join(REPO),
        &[(format!("{DIR}/{FILE}"), b"x\n".to_vec()), ("docker-compose.yml".into(), compose.into_bytes())],
    );
    std::fs::create_dir(root.join(UNBORN)).unwrap();
    let init = Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(root.join(UNBORN)).output();
    assert!(init.unwrap().status.success());
    let scan = json(&root, &["scan", root.to_str().unwrap()], false);
    assert_eq!(scan["ok"], true, "{scan}");
    (root, scan)
}

fn ids(view: &Value) -> Vec<&str> {
    view["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect()
}

#[test]
fn engine_lines_mask_repo_text_and_keep_engine_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let (root, scan) = planted(dir.path());
    let (repo_id, dir_id, file_id) =
        (format!("{REPO}:repo:{REPO}"), format!("{REPO}:dir:{DIR}"), format!("{REPO}:file:{DIR}/{FILE}"));
    let dsn_id = format!("ws:container:{DSN}");

    // scan: the summary names the unborn repo, and so does its warning; the counts stay whole.
    let summary = scan["summary"].as_str().unwrap();
    assert_masked("scan's summary", summary);
    assert!(summary.starts_with("Mapped 6 repos, 6 of them moved: "), "{summary}");
    assert!(
        summary.ends_with(&format!("No commits yet in {MASK}; each maps after its first commit.")),
        "{summary}"
    );
    let unborn: Vec<&Value> =
        scan["meta"]["warnings"].as_array().unwrap().iter().filter(|w| w["code"] == "repo_unborn").collect();
    assert_eq!(unborn.len(), 1, "{scan}");
    assert_eq!(unborn[0]["message"], format!("{MASK} has no commits yet; it maps after its first commit."));
    // The map it returns masks the names and keeps the IDs.
    let top = &scan["result"]["map"];
    assert!(ids(top).contains(&repo_id.as_str()) && ids(top).contains(&dsn_id.as_str()), "{top}");
    let name_of = |view: &Value, id: &str| {
        view["nodes"].as_array().unwrap().iter().find(|n| n["id"] == id).unwrap()["name"].clone()
    };
    assert_eq!(name_of(top, &repo_id), format!("db_PASSWORD={MASK}"));
    assert_eq!(name_of(top, &dsn_id), format!("postgres://{MASK}@db"));

    // map: the summary lists the masked names.
    let map = json(&root, &["map", "--lens", "all"], false);
    let summary = map["summary"].as_str().unwrap();
    assert_masked("map's summary", summary);
    assert!(summary.contains(&format!("db_PASSWORD={MASK} (")), "{summary}");
    assert!(summary.contains(&format!("postgres://{MASK}@db")), "{summary}");
    assert_eq!(map["meta"]["seq"], scan["meta"]["seq"], "the seq stays whole");

    // map --focus: the line names the node it is under, and the names under it.
    let focus = json(&root, &["map", "--lens", "all", "--focus", &repo_id], false);
    let summary = focus["summary"].as_str().unwrap();
    assert_masked("map --focus's summary", summary);
    assert!(
        summary.starts_with(&format!("3 under db_PASSWORD={MASK} at the container level: ")),
        "{summary}"
    );
    assert!(summary.contains(&format!("{MASK} (1 code)")), "{summary}");
    assert_eq!(focus["result"]["focus"], repo_id.as_str(), "the focus is an ID");
    assert!(ids(&focus["result"]).contains(&dir_id.as_str()), "{focus}");
    let first_line = text(&root, &["map", "--lens", "all", "--focus", &repo_id], false);
    assert_eq!(first_line.lines().next(), Some(summary), "text mode prints the same masked line");

    // nothing_under: the empty line names the node; the handle beside it keeps its IDs and its
    // label names no repo text.
    for (id, line) in [
        (file_id.as_str(), format!("Nothing under api_token={MASK} at the code level.")),
        (dsn_id.as_str(), format!("Nothing under postgres://{MASK}@db at the component level.")),
    ] {
        let env = json(&root, &["map", "--lens", "all", "--focus", id], false);
        let empty = &env["result"]["empty"];
        assert_eq!(empty["reason"], "nothing_under", "{env}");
        assert_eq!(empty["line"], line.as_str());
        assert_eq!(env["summary"], line.as_str());
        assert_eq!(env["result"]["focus"], id);
        assert_eq!(empty["next"]["label"], "Show it among the blocks beside it");
        assert_masked("nothing_under's line", &line);
    }
    let env = json(&root, &["map", "--lens", "all", "--focus", &file_id], false);
    let args = &env["result"]["empty"]["next"]["args"];
    assert_eq!(
        *args,
        serde_json::json!(["--level", "code", "--focus", dir_id]),
        "the handle's IDs stay whole"
    );

    // An error's line masks the target it quotes; its details keep the IDs.
    let missing = json(&root, &["map", "--focus", &format!("{REPO}:file:nope")], false);
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    assert_masked("an error's message", missing["error"]["message"].as_str().unwrap());

    // A stream frame names the repo it scanned, masked, and keeps the commit's short SHA.
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let events = store.events_after(0, 100).unwrap();
    let scanned = events
        .iter()
        .find(|e| matches!(&e.body, EventBody::Scan { repo, .. } if repo == REPO))
        .expect("a scan event for the planted repo");
    let EventBody::Scan { commit, .. } = &scanned.body else { unreachable!() };
    assert_eq!(commit.len(), 40, "a full SHA");
    let StreamFrame::Event { summary, target, .. } = planroom::live::frame(scanned) else {
        panic!("an event frame")
    };
    assert_eq!(summary, format!("Scanned db_PASSWORD={MASK} at {}.", &commit[..7]));
    assert_eq!(target.as_deref(), Some(repo_id.as_str()), "the frame's target is an ID");
}

#[test]
fn an_unborn_repos_empty_line_masks_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join(UNBORN);
    std::fs::create_dir(&repo).unwrap();
    let init = Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(&repo).output();
    assert!(init.unwrap().status.success());
    for leaf in ["map", "focus"] {
        let env = json(dir.path(), &[leaf], false);
        let empty = &env["result"]["empty"];
        assert_eq!(empty["reason"], "repos_unborn", "{env}");
        let line = empty["line"].as_str().unwrap();
        assert_eq!(
            line,
            format!(
                "No commits yet in {MASK}, so there's nothing to map. Make a first commit, then check again."
            )
        );
        assert_masked(leaf, env["summary"].as_str().unwrap());
    }
}

#[test]
fn raw_shows_engine_lines_only_on_a_read_commands_json() {
    let dir = tempfile::tempdir().unwrap();
    let (root, _) = planted(dir.path());
    let file_id = format!("{REPO}:file:{DIR}/{FILE}");
    let raw_line = format!("Nothing under {FILE} at the code level.");

    // A read's --json: raw for the truth comparator.
    let env = json(&root, &["map", "--lens", "all", "--focus", &file_id], true);
    assert_eq!(env["summary"], raw_line.as_str());
    assert_eq!(env["result"]["empty"]["line"], raw_line.as_str());
    let top = json(&root, &["map", "--lens", "all"], true);
    assert!(top["summary"].as_str().unwrap().contains(REPO), "{top}");

    // Its text mode, and a scan's --json, stay masked whatever PLANROOM_RAW says.
    let shown = text(&root, &["map", "--lens", "all", "--focus", &file_id], true);
    assert_eq!(
        shown.lines().next(),
        Some(format!("Nothing under api_token={MASK} at the code level.").as_str())
    );
    let scan = json(&root, &["scan", root.to_str().unwrap()], true);
    assert_masked("scan's summary under PLANROOM_RAW", scan["summary"].as_str().unwrap());
}

/// `import -` with one map read row of `request`; returns the import's envelope and the answer
/// document its sync writes under `answers/d1`.
fn import_map(root: &Path, request: Value) -> (Value, Value) {
    import_map_with(root, request, false)
}

/// [`import_map`], with `PLANROOM_RAW=1` when `raw`.
fn import_map_with(root: &Path, request: Value, raw: bool) -> (Value, Value) {
    let row = serde_json::json!({"path": "inbox/u_test/events/d1", "version": 1, "row": {
        "format": "planroom.inbox/v1", "instance": "i1", "counter": 1, "command": "map",
        "request": request, "base": 5, "at": "2026-10-26T00:00:00Z"}});
    let mut child = planroom(root, raw)
        .args(["import", "-", "--json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(format!("{row}\n").as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["ok"], true, "{env}");
    let base = Path::new(env["result"]["root"].as_str().unwrap()).to_path_buf();
    for file in env["result"]["batch_files"].as_array().unwrap() {
        let batch: Value =
            serde_json::from_str(&std::fs::read_to_string(base.join(file.as_str().unwrap())).unwrap())
                .unwrap();
        for write in batch["writes"].as_array().unwrap() {
            if write["collection"] == "answers" && write["doc_id"] == "d1" {
                let path = write["file_path"].as_str().unwrap();
                let answer = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                return (env, answer);
            }
        }
    }
    panic!("no answer in {env}");
}

#[test]
fn an_import_answer_masks_repo_text_in_its_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    git_repo(&root.join(REPO), &[("src/app.txt".into(), b"x\n".to_vec())]);
    let scan = json(&root, &["scan", root.to_str().unwrap()], false);
    assert_eq!(scan["ok"], true, "{scan}");

    // `src` names a block in several repos, the planted one among them, so the read fails and
    // its line lists their IDs.
    let (env, answer) = import_map(&root, serde_json::json!({"focus": "src", "lens": "all"}));
    let error = &answer["error"];
    assert_eq!(error["code"], "invalid_argument", "{answer}");
    let message = error["message"].as_str().unwrap();
    assert_masked("the answer's error", message);
    // The planted ID's own segment holds the secret, so the assignment rule masks it to the
    // end of the ID; the IDs beside it stay whole.
    assert!(message.contains(&format!("db_PASSWORD={MASK}, ")), "{message}");
    assert!(message.contains("kit:dir:src"), "the other IDs stay whole: {message}");
    // The same line, as the CLI gives it.
    let cli = json(&root, &["map", "--lens", "all", "--focus", "src"], false);
    assert_eq!(cli["error"]["message"], error["message"], "{cli}");
    // The import's own reply names the row's reason, masked as well.
    let reason = env["result"]["rows"][0]["reason"].as_str().unwrap();
    assert_masked("the imported row's reason", reason);
}

#[test]
fn a_path_id_in_a_masked_line_stays_whole() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for repo in ["alpha", "beta"] {
        git_repo(&root.join(repo), &[("lib/v3/OAuth2PKCEFlow.ts".into(), b"export const a = 1;\n".to_vec())]);
    }
    for repo in ["x", "y"] {
        git_repo(
            &root.join(repo),
            &[("internal/pkg/xdg/b64url/Encode.go".into(), b"package b64url\n".to_vec())],
        );
    }
    let scan = json(&root, &["scan", root.to_str().unwrap()], false);
    assert_eq!(scan["ok"], true, "{scan}");
    for (target, ids) in [
        ("OAuth2PKCEFlow.ts", ["alpha:file:lib/v3/OAuth2PKCEFlow.ts", "beta:file:lib/v3/OAuth2PKCEFlow.ts"]),
        (
            "Encode.go",
            ["x:file:internal/pkg/xdg/b64url/Encode.go", "y:file:internal/pkg/xdg/b64url/Encode.go"],
        ),
    ] {
        let env = json(&root, &["map", "--lens", "all", "--focus", target], false);
        assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
        let message = env["error"]["message"].as_str().unwrap();
        let shown = text(&root, &["map", "--lens", "all", "--focus", target], false);
        for id in ids {
            assert!(message.contains(id), "{id} isn't whole in {message}");
            assert!(shown.contains(id), "{id} isn't whole in text mode: {shown}");
        }
        assert!(!message.contains(MASK), "{message}");
    }
}

#[test]
fn an_id_whose_namespace_reads_like_a_secret_name_stays_whole_in_a_masked_line() {
    let dir = tempfile::tempdir().unwrap();
    let root = fixture(dir.path()).root;
    git_repo(&root.join("access_token"), &[("src/app.txt".into(), b"x\n".to_vec())]);
    git_repo(&root.join(REPO), &[("src/app.txt".into(), b"x\n".to_vec())]);
    let scan = json(&root, &["scan", root.to_str().unwrap()], false);
    assert_eq!(scan["ok"], true, "{scan}");

    // focus_ambiguous lists the IDs to pass: a repo named like a token's name keeps its IDs whole,
    // while an ID that holds an assignment is masked as before.
    let args = ["map", "--lens", "all", "--focus", "src"];
    let env = json(&root, &args, false);
    assert_eq!(env["error"]["code"], "invalid_argument", "{env}");
    let message = env["error"]["message"].as_str().unwrap();
    let shown = text(&root, &args, false);
    for line in [message, shown.as_str()] {
        assert!(line.contains("access_token:dir:src, "), "the ID isn't whole: {line}");
        assert!(line.contains("kit:dir:src"), "{line}");
        assert!(line.contains(&format!("db_PASSWORD={MASK}, ")), "{line}");
        assert!(!line.contains("hunter2"), "{line}");
    }

    // not_found quotes the ID it was given, whole.
    let env = json(&root, &["map", "--focus", "access_token:dir:nope"], false);
    assert_eq!(env["error"]["code"], "not_found", "{env}");
    let message = env["error"]["message"].as_str().unwrap();
    assert!(message.contains("access_token:dir:nope"), "the ID isn't whole: {message}");
    assert!(!message.contains(MASK), "{message}");
}

#[test]
fn details_mask_the_repo_text_their_code_declares_and_keep_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (root, scan) = planted(dir.path());

    // repo_unborn's details name the repo, masked, and its branch.
    let unborn = scan["meta"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["code"] == "repo_unborn")
        .unwrap_or_else(|| panic!("{scan}"));
    assert_eq!(unborn["details"]["repo"], MASK, "{unborn}");
    assert_eq!(unborn["details"]["branch"], "main", "{unborn}");

    // not_found's details list the closest IDs, which are never masked.
    let missing = json(&root, &["map", "--focus", "hunter2"], false);
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    let closest: Vec<&str> = missing["error"]["details"]["closest"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert!(closest.contains(&format!("{REPO}:repo:{REPO}").as_str()), "{missing}");

    // PLANROOM_RAW=1 shows a read's details raw; a scan's stay masked.
    let raw = json(&root, &["scan", root.to_str().unwrap()], true);
    let unborn =
        raw["meta"]["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "repo_unborn").unwrap();
    assert_eq!(unborn["details"]["repo"], MASK, "{unborn}");
}

/// Every file under `dir`, at any depth.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// The IDs a masked output may carry whole (contracts.md, the redaction paragraph): every node's
/// `id` and `aliases` among the store's active facts, and each workspace repo's namespace followed
/// by `:`, the unborn repo's included.
struct Ids(Vec<String>);

impl Ids {
    fn of(root: &Path) -> Ids {
        let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
        let active = store.active_set().unwrap();
        let mut ids: Vec<String> = active.repos.keys().map(|repo| format!("{repo}:")).collect();
        ids.push(format!("{UNBORN}:"));
        for row in store.facts(&active.snapshots()).unwrap() {
            if row.shape == Shape::Node {
                let node: Node = serde_json::from_str(&row.body).unwrap();
                ids.push(node.id.0.clone());
                ids.extend(node.aliases.iter().cloned());
            }
        }
        // Longest first, so an ID is cut whole before a shorter one inside it.
        ids.sort_by_key(|id| std::cmp::Reverse(id.len()));
        ids.dedup();
        Ids(ids)
    }

    /// Check that `out`, with every ID cut out, holds no planted name, content or fake key.
    fn check(&self, what: &str, out: &str) {
        let mut rest = out.to_owned();
        for id in &self.0 {
            rest = rest.replace(id.as_str(), "");
        }
        let planted = SECRETS.iter().chain(CONTENT.iter()).chain(FAKE_KEYS.iter().map(|(_, value)| value));
        for secret in planted {
            if let Some(at) = rest.find(secret) {
                let from = rest.floor_char_boundary(at.saturating_sub(120));
                let to = rest.ceil_char_boundary((at + secret.len() + 120).min(rest.len()));
                panic!("{what} shows {secret} outside an ID: …{}…", &rest[from..to]);
            }
        }
    }

    /// [`Ids::check`], and `out` masks a planted name where it was.
    fn check_masked(&self, what: &str, out: &str) {
        self.check(what, out);
        assert!(out.contains(MASK), "{what} masks nothing: {out}");
    }

    /// [`Ids::check`] over the whole envelope, and over each field a tool returns and `/room`
    /// draws from: its `summary`, its `result` and its error's `message`.
    fn check_envelope(&self, what: &str, env: &Value) {
        self.check(what, &env.to_string());
        for field in ["summary", "result"] {
            self.check(&format!("{what}'s {field}"), &env[field].to_string());
        }
        self.check(&format!("{what}'s error message"), &env["error"]["message"].to_string());
    }
}

/// The planted workspace, plus a repo `billing` whose content, but no name, holds secrets,
/// rescanned. Returns the root and the rescan's envelope.
fn planted_with_content(dir: &Path) -> (PathBuf, Value) {
    let (root, _) = planted(dir);
    git_repo(
        &root.join("billing"),
        &[
            ("src/charge.ts".into(), CHARGE.as_bytes().to_vec()),
            ("docker-compose.yml".into(), BILLING_COMPOSE.as_bytes().to_vec()),
        ],
    );
    let scan = json(&root, &["scan", root.to_str().unwrap()], false);
    assert_eq!(scan["ok"], true, "{scan}");
    (root, scan)
}

/// The facts of the store's active snapshots, parsed: its nodes and its hints.
fn active_facts(root: &Path) -> (Vec<Node>, Vec<Hint>, Vec<planroom_store::FactRow>) {
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let rows = store.facts(&store.active_set().unwrap().snapshots()).unwrap();
    let nodes = rows
        .iter()
        .filter(|r| r.shape == Shape::Node)
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    let hints = rows
        .iter()
        .filter(|r| r.shape == Shape::Hint)
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    (nodes, hints, rows)
}

/// A model request's slice: `billing`'s file nodes and hint rows, and the `payments` service,
/// looked up by its kind, whichever extractor or linker minted it.
fn billing_batch(root: &Path) -> FactBatch {
    let (all, _, _) = active_facts(root);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let billing = store.active_set().unwrap().repos["billing"];
    let hints: Vec<Hint> = store
        .facts(&[billing])
        .unwrap()
        .iter()
        .filter(|r| r.shape == Shape::Hint)
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect();
    assert!(!hints.is_empty(), "billing's hint rows");
    let mut nodes: Vec<Node> =
        all.iter().filter(|n| n.id.0.starts_with("billing:") && n.kind.0 == "code.file").cloned().collect();
    assert!(!nodes.is_empty(), "billing's file nodes");
    let service =
        all.iter().find(|n| n.kind.0 == "code.service" && n.name == "payments").unwrap_or_else(|| {
            panic!("no payments service among {:?}", all.iter().map(|n| &n.id).collect::<Vec<_>>())
        });
    nodes.push(service.clone());
    FactBatch {
        format: "planroom.facts/v1".into(),
        extractor: "code/compose".into(),
        tier: Tier::Config,
        nodes,
        edges: vec![],
        hints,
        partial: vec![],
    }
}

/// `live_gate.rs`'s Claude settings, a gateway with test models, holding the gateway variable's fake
/// value as their key: the one credential the request builder can reach, since this process holds
/// no credential variable of its own.
fn claude_config() -> ClaudeConfig {
    let (_, key) = FAKE_KEYS[0];
    serde_json::from_value(json!({
        "backend": "auto", "base_url": "https://ai-gateway.vercel.sh", "api_key_env": "AI_GATEWAY_API_KEY",
        "api_key": key, "model_cheap": "test/cheap", "model_capable": "test/capable", "providers": ["anthropic"]
    }))
    .unwrap()
}

/// A Claude call whose tail is `tail`.
fn call_with(tail: Vec<String>) -> Call {
    Call {
        purpose: "test.privacy".into(),
        tier: ModelTier::Cheap,
        prefix: vec!["Rules for the test.".into()],
        ttl: CacheTtl::M5,
        tail,
        output_schema: None,
        max_tokens: 50,
        effort: None,
        ceiling_usd: 0.01,
        scenario: "privacy".into(),
    }
}

/// One HTTP/1.1 request to `port` from the page's origin, as `stream.rs` sends it; the status and
/// the body.
fn http(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default();
    (status, body)
}

/// A detached `serve`'s process, stopped when dropped.
struct Detached(u32);

impl Drop for Detached {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg(self.0.to_string()).status();
    }
}

/// Whether `pid` still runs; a zombie counts as gone.
fn alive(pid: u32) -> bool {
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        return stat.rsplit_once(')').is_some_and(|(_, rest)| !rest.trim_start().starts_with('Z'));
    }
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success()
}

/// A signed-in tab on a detached `serve`.
struct Tab {
    port: u16,
    token: String,
}

impl Tab {
    /// Trade the login page's code for a bearer token, as the page does.
    fn sign_in(port: u16, login_page: &str) -> Tab {
        let page = std::fs::read_to_string(login_page).unwrap();
        let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap();
        let origin = format!("http://127.0.0.1:{port}");
        let body = json!({"code": code}).to_string();
        let (status, reply) = http(port, "POST", "/api/v1/session", &[("Origin", &origin)], &body);
        assert_eq!(status, 200, "{reply}");
        let env: Value = serde_json::from_str(&reply).unwrap();
        Tab { port, token: env["result"]["token"].as_str().unwrap().to_owned() }
    }

    /// `POST /api/v1/{command}` with `body`; the envelope as sent.
    fn post(&self, command: &str, body: &Value) -> String {
        let origin = format!("http://127.0.0.1:{}", self.port);
        let bearer = format!("Bearer {}", self.token);
        let headers = [("Origin", origin.as_str()), ("Authorization", bearer.as_str())];
        let (status, reply) =
            http(self.port, "POST", &format!("/api/v1/{command}"), &headers, &body.to_string());
        assert_eq!(status, 200, "{command}: {reply}");
        reply
    }

    /// The live stream, read until its text holds `needle` or 10 s pass; what it read.
    fn stream_until(&self, start: impl FnOnce(), needle: &str) -> String {
        let mut socket = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let request = format!(
            "GET /api/v1/stream HTTP/1.1\r\nHost: 127.0.0.1:{0}\r\nOrigin: http://127.0.0.1:{0}\r\nAuthorization: Bearer {1}\r\n\r\n",
            self.port, self.token
        );
        socket.write_all(request.as_bytes()).unwrap();
        socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let mut seen = String::new();
        let mut buf = [0u8; 4096];
        let mut start = Some(start);
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(10) && !seen.contains(needle) {
            // Once the stream is open, so the writes' frames come after its first ping.
            if seen.contains("event: ping")
                && let Some(start) = start.take()
            {
                start();
            }
            match socket.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => seen.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(_) => {}
            }
        }
        assert!(seen.contains(needle), "the stream carries {needle}: {seen}");
        seen
    }
}

/// Move a repo's head, so the next scan of it records a new scan event and sends its frame.
fn commit_in(repo: &Path) {
    let out = Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", "move the head"])
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@example.test")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@example.test")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// `watch --json --since 0`'s lines, read until one carries a `seq` of at least `seq` or 10 s pass.
fn watch_lines(root: &Path, seq: u64) -> String {
    let mut child = planroom(root, false)
        .args(["watch", "--json", "--since", "0"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut seen = Vec::new();
    let began = Instant::now();
    while let Some(left) = Duration::from_secs(10).checked_sub(began.elapsed()) {
        let Ok(line) = rx.recv_timeout(left) else { break };
        let done = serde_json::from_str::<Value>(&line)
            .ok()
            .and_then(|frame| frame["seq"].as_u64())
            .is_some_and(|at| at >= seq);
        seen.push(line);
        if done {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let seen = seen.join("\n");
    assert!(seen.contains("Scanned"), "watch replays the scans: {seen}");
    seen
}

/// `share`'s envelope and every file under its `--out` directory, `live/head` among them.
fn share_outputs(root: &Path, out: &str, extra: &[&str], raw: bool) -> Vec<(String, String)> {
    let mut args = vec!["share", "--out", out];
    args.extend(extra);
    let env = json(root, &args, raw);
    assert_eq!(env["ok"], true, "{env}");
    let dir = root.join(out);
    let head = dir.join("db/live-head.json");
    assert!(head.is_file(), "share writes live/head's first document");
    let snapshot = dir.join("data/snapshot.json");
    assert!(snapshot.is_file(), "share writes the snapshot");
    let mut outputs = vec![(format!("share {extra:?}'s envelope"), env.to_string())];
    for file in files_under(&dir) {
        let body = String::from_utf8_lossy(&std::fs::read(&file).unwrap()).into_owned();
        outputs.push((format!("share's {}", file.display()), body));
    }
    outputs
}

/// `import -` of one map read row: its envelope, its answer and every file under its ack root.
fn import_outputs(root: &Path, raw: bool) -> Vec<(String, String)> {
    let (env, answer) = import_map_with(root, json!({"lens": "all", "detail": "graph"}), raw);
    let mut outputs = vec![("import's envelope".to_owned(), env.to_string())];
    outputs.push(("import's answer".to_owned(), answer.to_string()));
    let base = Path::new(env["result"]["root"].as_str().unwrap());
    for file in files_under(base) {
        let body = String::from_utf8_lossy(&std::fs::read(&file).unwrap()).into_owned();
        outputs.push((format!("import's {}", file.display()), body));
    }
    outputs
}

/// Repo text, names and content alike, and the credential variables' values reach no output:
/// the CLI's `--json` and text, which the mod's tools return and `/room` draws from; HTTP; the
/// live stream over SSE and `watch`; `share`'s snapshot and `live/head`; `import`'s answers and
/// acks; the server's logs; and a model request built through `app::models::request_tail`
/// (DR-49, DR-64, DR-75, DR-89). The store holds the content raw, which proves it entered the
/// engine. IDs stay whole, so each output is checked with every ID cut out.
#[test]
fn a_planted_repo_secret_reaches_no_request_or_tool_output() {
    let dir = tempfile::tempdir().unwrap();
    let (root, scan) = planted_with_content(dir.path());

    // The content entered the engine: the store holds each secret raw.
    let (nodes, _, rows) = active_facts(&root);
    let stored: String = rows.iter().map(|r| r.body.as_str()).collect();
    for secret in CONTENT {
        assert!(stored.contains(secret), "the store holds {secret}");
    }
    let ids = Ids::of(&root);
    let repo_id = format!("{REPO}:repo:{REPO}");
    let billing_id = "billing:repo:billing";
    let file_id = "billing:file:src/charge.ts";
    let service_id =
        nodes.iter().find(|n| n.kind.0 == "code.service" && n.name == "payments").unwrap().id.0.clone();

    // The CLI, --json and text. Each envelope is checked whole and by the fields a tool returns.
    ids.check_envelope("scan", &scan);
    ids.check_masked("scan", &scan.to_string());
    let mut runs: Vec<(Vec<String>, bool)> = vec![(vec!["scan".into(), root.display().to_string()], true)];
    for level in ["intent", "system", "container", "component", "code"] {
        let named = level != "intent";
        runs.push((
            ["map", "--level", level, "--detail", "graph", "--lens", "all"].map(String::from).to_vec(),
            named,
        ));
    }
    runs.push((["map", "--lens", "all", "--focus", &repo_id].map(String::from).to_vec(), true));
    runs.push((["map", "--lens", "all", "--focus", billing_id].map(String::from).to_vec(), false));
    runs.push((vec!["focus".into()], true));
    runs.push((["find", "db_PASSWORD"].map(String::from).to_vec(), true));
    runs.push((["ask", "impact", &repo_id].map(String::from).to_vec(), true));
    runs.push((["ask", "path", file_id, &service_id].map(String::from).to_vec(), false));
    runs.push((["ask", "why", file_id, &service_id].map(String::from).to_vec(), false));
    runs.push((["map", "--focus", &format!("{REPO}:file:nope")].map(String::from).to_vec(), true));
    for (args, named) in &runs {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let env = json(&root, &args, false);
        ids.check_envelope(&format!("{args:?} --json"), &env);
        let shown = text(&root, &args, false);
        ids.check(&format!("{args:?} as text"), &shown);
        if *named {
            ids.check_masked(&format!("{args:?} --json"), &env.to_string());
            ids.check_masked(&format!("{args:?} as text"), &shown);
        }
    }
    let missing = json(&root, &["map", "--focus", &format!("{REPO}:file:nope")], false);
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    // A usage error quotes argv: text mode prints clap's text on stderr, --json an envelope.
    let usage = text(&root, &["map", "--level", REPO], false);
    assert!(usage.contains("--level"), "a usage error: {usage}");
    ids.check_masked("a usage error as text", &usage);
    let usage = json(&root, &["map", "--level", REPO], false);
    assert_eq!(usage["error"]["code"], "invalid_argument", "{usage}");
    ids.check_envelope("a usage error --json", &usage);
    ids.check_masked("a usage error --json", &usage.to_string());

    // HTTP, the SSE stream and the log, on one detached `serve` with the same data root.
    let env = json(&root, &["serve", "--detach"], false);
    assert_eq!(env["ok"], true, "{env}");
    ids.check_envelope("serve --detach", &env);
    let (pid, port) =
        (env["result"]["pid"].as_u64().unwrap() as u32, env["result"]["port"].as_u64().unwrap() as u16);
    let running = Detached(pid);
    // The server holds the fake keys, as every process here does: where /proc shows a process's
    // environment, it proves the values were there to leak.
    if let Ok(environ) = std::fs::read(format!("/proc/{pid}/environ")) {
        let environ = String::from_utf8_lossy(&environ);
        for (name, value) in FAKE_KEYS {
            assert!(environ.contains(&format!("{name}={value}")), "the server holds {name}");
        }
    }
    let tab = Tab::sign_in(port, env["result"]["login_page"].as_str().unwrap());
    let mut scanned = String::new();
    let frames = tab.stream_until(
        || {
            commit_in(&root.join(REPO));
            commit_in(&root.join("billing"));
            scanned = tab.post("scan", &json!({}));
        },
        "Scanned billing at",
    );
    ids.check_masked("POST /api/v1/scan", &scanned);
    ids.check_masked("the SSE frames", &frames);
    let map = tab.post("map", &json!({"detail": "graph", "lens": "all"}));
    ids.check_masked("POST /api/v1/map", &map);
    let focus = tab.post("focus", &json!({}));
    ids.check_masked("POST /api/v1/focus", &focus);
    let last_scan: Value = serde_json::from_str(&scanned).unwrap();
    let last_seq = last_scan["meta"]["seq"].as_u64().unwrap();

    // `watch`, a second process, replays every frame up to the last scan's.
    ids.check_masked("watch's frames", &watch_lines(&root, last_seq));

    // `share`, with and without snippets: its result, its snapshot and `live/head`.
    let shared = [
        share_outputs(&root, ".planroom/share", &[], false),
        share_outputs(&root, ".planroom/share-snippets", &["--include-snippets"], false),
    ];
    for (what, body) in shared.iter().flatten() {
        ids.check(what, body);
    }
    for outputs in &shared {
        let snapshot = outputs.iter().find(|(what, _)| what.ends_with("snapshot.json")).unwrap();
        ids.check_masked(&snapshot.0, &snapshot.1);
    }

    // `import -` of a map read row: its answer and its acks.
    let imported = import_outputs(&root, false);
    for (what, body) in &imported {
        ids.check(what, body);
    }
    ids.check_masked("import's answer", &imported[1].1);

    // The log: drain the server through its record's control token, then read every log file.
    // A request that succeeds writes no log line yet, so this guards the writers to come.
    let record = root.join(".data/serve").join(format!("{port}.json"));
    let record: Value = serde_json::from_str(&std::fs::read_to_string(record).unwrap()).unwrap();
    let body = json!({"token": record["control_token"].as_str().unwrap()}).to_string();
    let (status, _) = http(port, "POST", "/api/v1/drain", &[], &body);
    assert_eq!(status, 202, "the server drains");
    let began = Instant::now();
    while alive(pid) && began.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!alive(pid), "the drained server exits");
    drop(running);
    for file in files_under(&root.join(".data/logs")) {
        let body = String::from_utf8_lossy(&std::fs::read(&file).unwrap()).into_owned();
        ids.check(&format!("the log {}", file.display()), &body);
    }
    // No credential is ever persisted: no file under the data root or the state directory holds
    // a fake key's value, the store and the server's records included.
    for file in files_under(&root.join(".data")).into_iter().chain(files_under(&root.join(".planroom"))) {
        let body = std::fs::read(&file).unwrap();
        for (name, value) in FAKE_KEYS {
            let held = body.windows(value.len()).any(|w| w == value.as_bytes());
            assert!(!held, "{} holds {name}'s value", file.display());
        }
    }

    // A model request: billing's facts, which hold the content raw, through the one hook that
    // builds a request's tail, then the adapter's request builder.
    let batch = billing_batch(&root);
    let raw = serde_json::to_string(&batch).unwrap();
    for secret in CONTENT {
        assert!(raw.contains(secret), "the slice holds {secret} before the hook");
    }
    let tail = planroom::app::models::request_tail(&batch, "What does the payments service set up?").unwrap();
    assert_eq!(tail.len(), 2, "the slice, then the question");
    assert!(tail[0].contains(MASK), "the slice is masked: {}", tail[0]);
    // The builder's settings hold a fake key, so the checks below see one copied into the request.
    // The other variables' values never reach this process: a test can't set its own environment
    // safely, and the builder reads none.
    let config = claude_config();
    assert_eq!(config.api_key.expose(), Some(FAKE_KEYS[0].1), "the builder can reach the gateway key");
    let built = claude::build(&config, &call_with(tail));
    let bytes = String::from_utf8(built.bytes.clone()).unwrap();
    ids.check("the request's bytes", &bytes);
    ids.check("the request's body", &built.body.to_string());
    ids.check("the request's headers", &format!("{:?}", built.headers));
    assert!(bytes.contains(MASK), "{bytes}");
    // A slice of a type the redaction list doesn't name would pass through unmasked: refused.
    let refused = planroom::app::models::request_tail(&batch.nodes, "What is this?").unwrap_err();
    assert_eq!(refused.code, ErrorCode::Internal);
    // So is a listed type whose open payload no mark covers, such as an envelope around the same
    // facts, typed or not: its own marks reach only its summary, error and warnings.
    let mut wrapped = scan.clone();
    wrapped["result"] = serde_json::to_value(&batch).unwrap();
    assert!(wrapped.to_string().contains(CONTENT[0]), "the envelope holds the content");
    let untyped: Envelope = serde_json::from_value(wrapped.clone()).unwrap();
    let refused = planroom::app::models::request_tail(&untyped, "What is this?").unwrap_err();
    assert_eq!(refused.code, ErrorCode::Internal);
    let typed: Envelope<FactBatch> = serde_json::from_value(wrapped).unwrap();
    let refused = planroom::app::models::request_tail(&typed, "What is this?").unwrap_err();
    assert_eq!(refused.code, ErrorCode::Internal);
}

/// `PLANROOM_RAW=1` shows repo text raw on a read command's `--json` stdout and nowhere else: not
/// in `share`'s result, snapshot or `live/head`, not in `import`'s answers or acks, and not in a
/// model request. Session request files are checked here once their backend lands.
#[test]
fn raw_never_reaches_share_or_model_requests() {
    let dir = tempfile::tempdir().unwrap();
    let (root, _) = planted(dir.path());
    let ids = Ids::of(&root);

    // The variable is on: a read's --json shows the planted name raw, in a node and the summary.
    let map = json(&root, &["map", "--lens", "all"], true);
    let raw_name = map["result"]["nodes"].as_array().unwrap().iter().any(|n| n["name"] == REPO);
    assert!(raw_name, "a node's name is raw: {map}");
    assert!(map["summary"].as_str().unwrap().contains(REPO), "{map}");

    // With the same variable, share and import stay masked.
    let shared = share_outputs(&root, ".planroom/share", &[], true);
    for (what, body) in &shared {
        ids.check(&format!("{what} under PLANROOM_RAW"), body);
    }
    let snapshot = shared.iter().find(|(what, _)| what.ends_with("snapshot.json")).unwrap();
    ids.check_masked("share's snapshot under PLANROOM_RAW", &snapshot.1);
    let imported = import_outputs(&root, true);
    for (what, body) in &imported {
        ids.check(&format!("{what} under PLANROOM_RAW"), body);
    }
    ids.check_masked("import's answer under PLANROOM_RAW", &imported[1].1);

    // A model request is masked by construction. A test can't set PLANROOM_RAW in its own
    // process safely, so this holds by how the hook is built: `request_tail` takes no raw flag
    // and reads no environment, and `cli::raw_for` is the one reader of PLANROOM_RAW (DR-75).
    // The slice is the very map PLANROOM_RAW showed raw above.
    let view: MapView = serde_json::from_value(map["result"].clone()).unwrap();
    assert!(serde_json::to_string(&view).unwrap().contains("hunter2"), "the slice holds the name raw");
    let tail = planroom::app::models::request_tail(&view, &format!("What does {REPO} hold?")).unwrap();
    for block in &tail {
        ids.check_masked("a model request's tail", block);
    }
}
