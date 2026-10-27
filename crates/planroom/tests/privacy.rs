//! Redaction on every output path (architecture.md, What leaves the machine; DR-64, DR-75, DR-89):
//! the engine lines that name repo text, a reply's `summary`, an empty state's `line`, and the
//! lines of warnings, errors and stream frames, are masked like the names they hold, while the
//! IDs, handles, counts and commit SHAs beside them stay whole. `PLANROOM_RAW=1` shows them raw
//! only on a read command's `--json` stdout.

use std::path::Path;
use std::process::Command;

use planroom_contract::{EventBody, StreamFrame};
use planroom_store::Store;
use planroom_testkit::{fixture, git_repo};
use serde_json::Value;

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

fn planroom(root: &Path, raw: bool) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", root.join(".data")).env_remove("PLANROOM_RAW");
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
    use std::io::Write as _;
    let row = serde_json::json!({"path": "inbox/u_test/events/d1", "version": 1, "row": {
        "format": "planroom.inbox/v1", "instance": "i1", "counter": 1, "command": "map",
        "request": request, "base": 5, "at": "2026-10-26T00:00:00Z"}});
    let mut child = planroom(root, false)
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
