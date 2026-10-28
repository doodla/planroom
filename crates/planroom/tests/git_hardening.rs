//! The git adapter runs nothing a scanned repo names: no lazy fetch from a partial clone's
//! promisor remote, so no `uploadpack`, `sshCommand` or `ext::` command from its local config,
//! even where the repo allows that transport itself (correctness-15, unasked-r3-2). The adapter's
//! own tests (`crates/extract/src/git.rs`) run the same plants as a git that ignores
//! `GIT_NO_LAZY_FETCH` would. It runs no git in a repo another user owns, and on no git older
//! than its floor (DR-78).

use std::ffi::OsStr;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use planroom_testkit::git_repo;
use serde_json::Value;

fn planroom(cwd: &Path, args: &[&str]) -> Output {
    planroom_with(cwd, args, &[])
}

/// `planroom ARGS` in `cwd` with `env` set besides its data root.
fn planroom_with(cwd: &Path, args: &[&str], env: &[(&str, &OsStr)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .envs(env.iter().copied())
        .output()
        .expect("the binary runs")
}

/// The one JSON envelope `out` printed.
fn envelope(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
}

/// The warnings of `code` in `env`; none when it carries no warnings at all.
fn warnings<'a>(env: &'a Value, code: &str) -> Vec<&'a Value> {
    let all = env["meta"]["warnings"].as_array().map(Vec::as_slice).unwrap_or_default();
    all.iter().filter(|w| w["code"] == code).collect()
}

/// A workspace `ws` under `dir` with two one-commit repos, `mine` and `alien`.
fn two_repos(dir: &Path) -> PathBuf {
    let ws = dir.join("ws");
    for name in ["mine", "alien"] {
        git_repo(&ws.join(name), &[(format!("{name}.txt"), b"hello\n".to_vec())]);
    }
    ws.canonicalize().unwrap()
}

/// The uid that owns `path`.
fn owner(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().uid()
}

/// `chown -R UID PATH`, which only root can run.
fn chown(uid: u32, path: &Path) {
    let out = Command::new("chown").arg("-R").arg(uid.to_string()).arg(path).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn scan(root: &Path) -> Value {
    let out = planroom(root, &["scan", ".", "--json"]);
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success() && env["ok"] == true, "{env}");
    env
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn code_ids(root: &Path) -> Vec<String> {
    let out = planroom(root, &["map", "--level", "code", "--json"]);
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut ids: Vec<String> = env["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    ids
}

/// A blob-less sparse clone of a three-directory repo, as `ws/orders`, holding only the
/// top-level file's blob.
fn partial_clone(dir: &Path) -> std::path::PathBuf {
    let src = dir.join("src");
    let files: Vec<(String, Vec<u8>)> = ["a/f1.txt", "a/f2.txt", "b/g1.txt", "top.txt"]
        .iter()
        .map(|p| (p.to_string(), format!("{p}\n").into_bytes()))
        .collect();
    git_repo(&src, &files);
    git(&src, &["config", "uploadpack.allowFilter", "true"]);
    let ws = dir.join("ws");
    std::fs::create_dir_all(ws.join(".planroom")).unwrap();
    let url = format!("file://{}", src.display());
    git(&ws, &["clone", "-q", "--filter=blob:none", "--sparse", &url, "orders"]);
    ws
}

#[test]
fn a_scan_of_a_partial_clone_never_fetches_and_reports_the_missing_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    let mark = dir.path().join("ran");
    let planted = format!("sh -c 'echo ran >> {}'; git-upload-pack", mark.display());
    git(&ws.join("orders"), &["config", "remote.origin.uploadpack", &planted]);

    let env = scan(&ws);
    assert!(!mark.exists(), "the repo's upload-pack command ran");
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    let partial = warnings.iter().find(|w| w["code"] == "partial_scan").expect("a partial_scan warning");
    assert_eq!(partial["details"]["repo"], "orders");
    assert_eq!(partial["details"]["uncovered"], serde_json::json!(["a/f1.txt", "a/f2.txt", "b/g1.txt"]));
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
    // Nothing was fetched into the clone either.
    let out = Command::new("git")
        .args(["rev-list", "--objects", "--missing=print", "--no-walk", "HEAD"])
        .current_dir(ws.join("orders"))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.starts_with('?')).count(), 3);
}

#[test]
fn a_partial_clone_that_gains_blobs_maps_them_at_the_same_commit() {
    // Which blobs a clone holds is an input of the walk, so it's part of the snapshot's key.
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    scan(&ws);
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
    // The person's own git widens the cone, which fetches the blobs.
    git(&ws.join("orders"), &["sparse-checkout", "add", "a"]);
    let env = scan(&ws);
    let partial =
        env["meta"]["warnings"].as_array().unwrap().iter().find(|w| w["code"] == "partial_scan").cloned();
    assert_eq!(partial.unwrap()["details"]["uncovered"], serde_json::json!(["b/g1.txt"]));
    assert_eq!(code_ids(&ws), ["orders:file:a/f1.txt", "orders:file:a/f2.txt", "orders:file:top.txt"]);
}

#[test]
fn a_partial_clone_that_allows_an_ext_remote_never_runs_it() {
    // unasked-r3-2: the repo's own protocol.ext.allow=always beats protocol.allow=never.
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    let mark = dir.path().join("PWNED");
    let url = format!("ext::sh -c touch% {}% >&2", mark.display());
    let orders = ws.join("orders");
    git(&orders, &["config", "remote.origin.url", &url]);
    git(&orders, &["config", "protocol.ext.allow", "always"]);
    git(&orders, &["config", "protocol.file.allow", "always"]);

    let env = scan(&ws);
    assert!(!mark.exists(), "the repo's ext:: command ran");
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|w| w["code"] == "partial_scan"), "{env}");
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
}

#[test]
fn a_promisor_key_with_no_value_still_marks_a_partial_clone() {
    // correctness-r3-3: git reads `promisor` with no value as true. Read as false, the walk asked
    // for every blob's size, which a lazy fetch would answer, and the repo was skipped.
    let dir = tempfile::tempdir().unwrap();
    let ws = partial_clone(dir.path());
    let orders = ws.join("orders");
    for key in ["extensions.partialclone", "remote.origin.promisor"] {
        // Whichever of the two this git's clone wrote.
        let _ = Command::new("git").args(["config", "--unset", key]).current_dir(&orders).output().unwrap();
    }
    let config = orders.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}[remote \"origin\"]\n\tpromisor\n")).unwrap();

    let env = scan(&ws);
    let warnings = env["meta"]["warnings"].as_array().unwrap();
    assert!(!warnings.iter().any(|w| w["code"] == "repo_skipped"), "{env}");
    let partial = warnings.iter().find(|w| w["code"] == "partial_scan").expect("a partial_scan warning");
    assert_eq!(partial["details"]["uncovered"], serde_json::json!(["a/f1.txt", "a/f2.txt", "b/g1.txt"]));
    assert_eq!(code_ids(&ws), ["orders:file:top.txt"]);
}

/// The person IDs the map holds under `repo`.
fn people(ws: &Path, repo: &str) -> Vec<String> {
    let focus = format!("{repo}:repo:{repo}");
    let out = planroom(ws, &["map", "--level", "component", "--focus", &focus, "--json"]);
    let env = envelope(&out);
    let mut ids: Vec<String> = env["result"]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("{env}"))
        .iter()
        .filter_map(|n| n["id"].as_str().filter(|id| id.contains(":person:")).map(str::to_owned))
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_shallow_clone_reports_its_cut_history_and_a_deepened_one_maps_it_all() {
    // A shallow file cuts the ancestry git walks, as a graft would, but it can't be turned off:
    // a real shallow clone lacks the parents. So the scan says where the history stops, and the
    // cut keys the snapshot, so the same commit deepened maps its whole history.
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let app = ws.join("app");
    git_repo(&app, &[("a.txt".to_owned(), b"a\n".to_vec())]);
    std::fs::write(app.join("b.txt"), "b\n").unwrap();
    git(&app, &["add", "-A"]);
    git(&app, &["-c", "user.name=Ben", "-c", "user.email=ben@example.test", "commit", "-q", "-m", "More"]);
    let head = String::from_utf8(
        Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&app).output().unwrap().stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let whole = {
        let env = scan(&ws);
        assert!(warnings(&env, "partial_scan").is_empty(), "{env}");
        people(&ws, "app")
    };
    assert_eq!(whole.len(), 2, "{whole:?}");

    std::fs::write(app.join(".git/shallow"), format!("{head}\n")).unwrap();
    let env = scan(&ws);
    let partial = warnings(&env, "partial_scan");
    assert_eq!(partial.len(), 1, "{env}");
    assert_eq!(partial[0]["details"]["extractor"], "code/git");
    assert_eq!(partial[0]["details"]["uncovered"], serde_json::json!([format!("history before {head}")]));
    assert_eq!(
        partial[0]["message"],
        format!(
            "code/git read app's history back to where its shallow clone stops; its history before {} isn't mapped.",
            &head[..7]
        )
    );
    assert_eq!(people(&ws, "app").len(), 1, "only Ben's commit is read");

    std::fs::remove_file(app.join(".git/shallow")).unwrap();
    let env = scan(&ws);
    assert!(warnings(&env, "partial_scan").is_empty(), "{env}");
    assert_eq!(people(&ws, "app"), whole, "the same commit, deepened, maps its whole history");
}

/// How a `repo_skipped` line for a repo another user owns ends: who may scan it, and no command,
/// since Planroom never runs git in another user's repo and its `fix` is null.
const NO_COMMAND: &str = "Planroom never runs git in another user's repo, so this scan left it out. Its owner can scan it, or you can scan a copy you own.";

/// The `repo_skipped` line for `repo` when user `uid` owns its working tree.
fn owns_repo(uid: u32, repo: &str) -> String {
    format!("User {uid} owns {repo}, and {NO_COMMAND}")
}

/// The `repo_skipped` line for `repo` when user `uid` owns `place`: its `.git`, or the git
/// directory or common directory it uses.
fn owns_git_dir(uid: u32, repo: &str, place: &str) -> String {
    format!("User {uid} owns the git directory {repo} uses, {place}, and {NO_COMMAND}")
}

/// Check that `warning` skips its repo as `other_owner` with a null `fix` and the one-line
/// message `want`, but for words redaction masked, and that it names no command.
fn refused_with_no_command(warning: &Value, want: &str) {
    assert_eq!(warning["details"]["reason"], "other_owner", "{warning}");
    assert_eq!(warning["details"]["fix"], Value::Null, "{warning}");
    assert!(same_but_masked(&warning["message"], want), "{warning} isn't {want}");
    let message = warning["message"].as_str().unwrap();
    assert!(!message.contains('\n'), "one line: {message}");
    for command in ["cp ", "sudo", "chown", "git -C", "planroom scan", "ls -l"] {
        assert!(!message.contains(command), "{message} names {command}");
    }
}

/// The `repo_skipped` line for `repo`, the person's own, when user `uid` owns `objects`, an
/// object directory it reads: only a copy can be scanned, never the repo by its owner.
fn borrows_from(uid: u32, repo: &str, objects: &Path) -> String {
    format!(
        "User {uid} owns {}, an object directory {repo} reads, and Planroom never runs git in another user's repo, so this scan left it out. You can scan a copy of {repo} that borrows none of its objects.",
        objects.display()
    )
}

/// The empty state's line when another user owns every repo in the workspace, `names`.
fn others_line(names: &str) -> String {
    format!(
        "A user other than you owns {names}, and Planroom never runs git in another user's repo, so there's nothing to map yet. Its owner can scan each, or you can scan a copy you own, then check again."
    )
}

/// The IDs `map --json` lists at the top, run with `env`.
fn top_ids(ws: &Path, env: &[(&str, &OsStr)]) -> Vec<String> {
    let map = envelope(&planroom_with(ws, &["map", "--json"], env));
    map["result"]["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn a_repo_another_user_owns_is_refused() {
    // DR-78: the engine compares each repo's owner with its own effective user ID before any git
    // call runs there, so the refusal doesn't rest on git's safe.directory, which a git before
    // 2.35.2 without the backport lacks.
    let dir = tempfile::tempdir().unwrap();
    let ws = two_repos(dir.path());
    let alien = ws.join("alien");
    // A config git can't parse: had git run in alien before the check, the reason would be
    // git_failed.
    let config = alien.join(".git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}[core\nthis is not a config\n")).unwrap();
    let me = owner(&alien);

    // The debug build's seam stands in for the effective user ID: every repo is another user's.
    let other = me.wrapping_add(1).to_string();
    let seam: &[(&str, &OsStr)] = &[("PLANROOM_TEST_EUID", OsStr::new(&other))];
    let focus = envelope(&planroom_with(&ws, &["focus", "--json"], seam));
    assert_eq!(focus["result"]["empty"]["reason"], "repos_unreadable", "{focus}");
    // The empty line names the repos and no path, so a long one keeps it short. Git never ran in
    // them, so it names no command to run there either.
    assert_eq!(focus["result"]["empty"]["line"], others_line("alien, mine"));
    let env = envelope(&planroom_with(&ws, &["scan", ".", "--json"], seam));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(env["result"]["map"]["empty"]["line"], others_line("alien, mine"), "{env}");
    let skipped = warnings(&env, "repo_skipped");
    let reasons: Vec<(&str, &str)> = skipped
        .iter()
        .map(|w| (w["details"]["repo"].as_str().unwrap(), w["details"]["reason"].as_str().unwrap()))
        .collect();
    assert_eq!(reasons, [("alien", "other_owner"), ("mine", "other_owner")]);
    refused_with_no_command(skipped[0], &owns_repo(me, "alien"));
    assert_eq!(skipped[0]["message"], owns_repo(me, "alien"));
    assert!(!ws.join(".planroom/store.db").exists(), "nothing mapped, so no store");

    // Without the seam, this user's repos map, and only a repo another user really owns is
    // refused, which only root can plant.
    std::fs::write(&config, text).unwrap();
    if me == 0 {
        chown(65534, &alien);
        // Beside a repo git refuses, the line runs git status only in that one.
        let mine_config = ws.join("mine/.git/config");
        let mine_text = std::fs::read_to_string(&mine_config).unwrap();
        std::fs::write(&mine_config, format!("{mine_text}[core\nthis is not a config\n")).unwrap();
        let focus = envelope(&planroom_with(&ws, &["focus", "--json"], &[]));
        assert_eq!(focus["result"]["empty"]["reason"], "repos_unreadable", "{focus}");
        assert_eq!(
            focus["result"]["empty"]["line"],
            "git can't read mine here, and a user other than you owns alien, where Planroom never runs git, so there's nothing to map yet. Run git status in each repo git can't read to see why, then check again."
        );
        std::fs::write(&mine_config, mine_text).unwrap();
        let env = scan(&ws);
        let skipped = warnings(&env, "repo_skipped");
        assert_eq!(skipped.len(), 1, "{env}");
        assert_eq!(
            (&skipped[0]["details"]["repo"], &skipped[0]["details"]["reason"]),
            (&"alien".into(), &"other_owner".into())
        );
        assert_eq!(top_ids(&ws, &[]), ["mine:repo:mine"]);
    } else {
        scan(&ws);
        assert_eq!(top_ids(&ws, &[]), ["alien:repo:alien", "mine:repo:mine"]);
    }
}

/// `word` as the engine quotes it for the shell: as it is when it holds only characters the shell
/// takes literally, else in single quotes, each `'` as `'\''`.
fn shell_word(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c);
    if word.chars().all(plain) { word.to_owned() } else { format!("'{}'", word.replace('\'', "'\\''")) }
}

/// Whether `got` is `want`, but for words redaction masked: a temporary directory's random name
/// can read as a secret.
fn same_but_masked(got: &Value, want: &str) -> bool {
    let got = got.as_str().unwrap_or_default();
    let (g, w): (Vec<&str>, Vec<&str>) = (got.split(' ').collect(), want.split(' ').collect());
    g.len() == w.len() && g.iter().zip(&w).all(|(g, w)| g == w || g.contains("[redacted]"))
}

/// Each `repo_skipped` warning in `env` as its repo and reason.
fn skips(env: &Value) -> Vec<(String, String)> {
    let pair = |w: &&Value| {
        let field = |key: &str| w["details"][key].as_str().unwrap().to_owned();
        (field("repo"), field("reason"))
    };
    warnings(env, "repo_skipped").iter().map(pair).collect()
}

#[test]
fn a_repo_git_cant_use_maps_nothing_of_a_repo_above_it() {
    // Every git call names the repo's own git directory, so a `.git` git can't use fails there
    // as git_failed, and git never looks further up: not into a parent repo, and not into a bare
    // repo at a path holding a `:`, which git would take as readily as a `.git`.
    let dir = tempfile::tempdir().unwrap();
    let me = owner(dir.path());
    let outer = dir.path().join("outer");
    git_repo(&outer, &[("outerfile.txt".to_owned(), b"outer\n".to_vec())]);
    let bare = dir.path().join("x:y");
    git(dir.path(), &["clone", "-q", "--bare", outer.to_str().unwrap(), bare.to_str().unwrap()]);
    for ws in [outer.join("ws"), bare.join("ws")] {
        std::fs::create_dir_all(ws.join("r/.git")).unwrap();
        std::fs::write(ws.join("r/r.txt"), "r\n").unwrap();
        let check = |seam: &[(&str, &OsStr)]| {
            let env = envelope(&planroom_with(&ws, &["scan", ".", "--json"], seam));
            assert_eq!(env["ok"], true, "{env}");
            assert_eq!(skips(&env), [("r".to_owned(), "git_failed".to_owned())], "{env}");
            // The fix names the repo's own git directory too, so it says why this repo fails
            // instead of reporting the repo above it. (The path itself may come back masked.)
            let r = ws.join("r").display().to_string();
            let fix = &warnings(&env, "repo_skipped")[0]["details"]["fix"];
            let command = format!("git -C {} --git-dir=.git status", shell_word(&r));
            let want = format!("Run {command} to see why, fix it, then scan again.");
            assert!(same_but_masked(fix, &want), "{fix} isn't {want}");
            let out = Command::new("sh").args(["-c", &command]).output().unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success() && stderr.contains("not a git repository"), "{stderr}");
            assert!(!String::from_utf8_lossy(&out.stdout).contains("On branch"));
            let found = envelope(&planroom_with(&ws, &["find", "outerfile", "--json"], seam));
            assert!(!found.to_string().contains("outerfile.txt"), "{found}");
            assert!(!ws.join(".planroom/store.db").exists(), "nothing mapped, so no store");
        };
        check(&[]);
        if me == 0 {
            // As root, the workspace another user's, which the debug build's seam makes the
            // engine's user, and the repos above it root's: still the same failure, never
            // other_owner, which no copy of the repo could clear.
            chown(1234, &ws);
            check(&[("PLANROOM_TEST_EUID", OsStr::new("1234"))]);
        }
    }
}

#[test]
fn a_directory_another_user_owns_above_a_repo_never_refuses_it() {
    // The owner check reads only the places git uses for a repo, since git never looks above
    // it: a directory above, whatever its name holds and whoever owns it, never refuses the
    // user's own repo. Only root can plant one: `a:b` stays root's, and the engine runs as uid
    // 1234, who owns the workspace.
    let dir = tempfile::tempdir().unwrap();
    if owner(dir.path()) != 0 {
        return;
    }
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let ws = dir.path().join("over/a:b/ws");
    git_repo(&ws.join("kit"), &[("kit.txt".to_owned(), b"kit\n".to_vec())]);
    chown(1234, &ws);
    let engine = env!("CARGO_BIN_EXE_planroom");
    let run = |args: &[&str]| -> Value {
        let mut cmd = Command::new(engine);
        cmd.args(args)
            .current_dir(&ws)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", &ws)
            .env("PLANROOM_DATA", ws.join(".data"))
            .uid(1234)
            .gid(1234);
        envelope(&cmd.output().unwrap())
    };
    let env = run(&["scan", ".", "--json"]);
    assert_eq!(env["ok"], true, "{env}");
    assert!(skips(&env).is_empty(), "{env}");
    let map = run(&["map", "--json"]);
    let ids: Vec<&str> =
        map["result"]["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["kit:repo:kit"], "{map}");

    // The same as root, with the debug build's seam standing in for uid 1234, in a workspace of
    // its own beside the first.
    let ws = dir.path().join("over/a:b/ws2");
    git_repo(&ws.join("kit"), &[("kit.txt".to_owned(), b"kit\n".to_vec())]);
    chown(1234, &ws);
    let seam: &[(&str, &OsStr)] = &[("PLANROOM_TEST_EUID", OsStr::new("1234"))];
    let env = envelope(&planroom_with(&ws, &["scan", ".", "--json"], seam));
    assert_eq!(env["ok"], true, "{env}");
    assert!(skips(&env).is_empty(), "{env}");
    assert_eq!(top_ids(&ws, seam), ["kit:repo:kit"]);
}

/// `planroom ARGS` in `cwd` with `env`, killed when it hasn't finished in 30 seconds.
fn planroom_within_30s(cwd: &Path, args: &[&str], env: &[(&str, &OsStr)]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(args)
        .current_dir(cwd)
        .env("PLANROOM_DATA", cwd.join(".data"))
        .envs(env.iter().copied())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("planroom {args:?} still running after 30 s");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn a_fifo_as_a_commondir_or_a_named_git_directory_never_blocks_a_scan() {
    // A `commondir` that's a FIFO with no writer blocks whoever opens it. The owner check reads
    // nothing in a repo before it compares that repo's owners, so another user's FIFO never
    // blocks a scan; and the engine opens no `commondir` that isn't a regular file, nor lets git
    // open one, so the user's own repo with one fails as git_failed. Only these places are
    // checked: a FIFO elsewhere that git opens still blocks it (architecture.md's git row).
    let dir = tempfile::tempdir().unwrap();
    let ws = two_repos(dir.path());
    let fifo = ws.join("alien/.git/commondir");
    let out = Command::new("mkfifo").arg(&fifo).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // And a repo whose `.git` file names a FIFO as its git directory.
    let named = dir.path().join("named-fifo");
    assert!(Command::new("mkfifo").arg(&named).output().unwrap().status.success());
    std::fs::create_dir_all(ws.join("pointed")).unwrap();
    std::fs::write(ws.join("pointed/.git"), format!("gitdir: {}\n", named.display())).unwrap();
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(env["ok"], true, "{env}");
    let failed = |repo: &str| (repo.to_owned(), "git_failed".to_owned());
    assert_eq!(skips(&env), [failed("alien"), failed("pointed")], "{env}");
    // Each fix shows what's there with `ls -l`, which never opens it, where git, which would
    // block on it, can't say why.
    for (warning, place) in warnings(&env, "repo_skipped").iter().zip([&fifo, &named]) {
        let shown = shell_word(&place.display().to_string());
        let fix = format!(
            "Run ls -l {shown} to see what it is: git needs a directory or a regular file there. Replace it, then scan again."
        );
        assert!(same_but_masked(&warning["details"]["fix"], &fix), "{warning} isn't {fix}");
        let mut ls = Command::new("ls").arg("-l").arg(place).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while ls.try_wait().unwrap().is_none() {
            assert!(std::time::Instant::now() < deadline, "ls -l {shown} still running after 10 s");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    std::fs::remove_dir_all(ws.join("pointed")).unwrap();
    if owner(dir.path()) == 0 {
        // As root: the workspace and `mine` another user's, which the debug build's seam makes
        // the engine's user, and `alien` root's.
        chown(1234, &ws);
        chown(0, &ws.join("alien"));
        let seam: &[(&str, &OsStr)] = &[("PLANROOM_TEST_EUID", OsStr::new("1234"))];
        let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], seam));
        assert_eq!(env["ok"], true, "{env}");
        assert_eq!(skips(&env), [("alien".to_owned(), "other_owner".to_owned())], "{env}");
        assert_eq!(top_ids(&ws, seam), ["mine:repo:mine"]);
    }
}

#[test]
fn an_old_git_gets_git_too_old() {
    // A stub `git` first on PATH reports its version and runs the real git for everything else.
    let dir = tempfile::tempdir().unwrap();
    let ws = two_repos(dir.path());
    let real = Command::new("sh").args(["-c", "command -v git"]).output().unwrap();
    let real = String::from_utf8(real.stdout).unwrap().trim().to_owned();
    let path = std::env::var_os("PATH").unwrap();
    let stub = |version: &str| -> PathBuf {
        let bin = dir.path().join(format!("git-{version}"));
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'git version {version}'; exit 0; fi\nexec '{real}' \"$@\"\n"
        );
        std::fs::write(bin.join("git"), script).unwrap();
        std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o755)).unwrap();
        bin
    };
    let on_path = |bin: &Path| -> std::ffi::OsString {
        let mut joined = bin.as_os_str().to_owned();
        joined.push(":");
        joined.push(&path);
        joined
    };
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/code");
    let pack = pack.to_str().unwrap();

    let old = on_path(&stub("2.30.0"));
    for args in [&["scan", ".", "--json"][..], &["doctor", "--pack", pack, "--json"]] {
        let out = planroom_with(&ws, args, &[("PATH", &old)]);
        let env = envelope(&out);
        assert_eq!(out.status.code(), Some(7), "{args:?}: {env}");
        let error = &env["error"];
        assert_eq!(error["code"], "git_too_old", "{args:?}: {env}");
        assert_eq!(error["message"], "git 2.30.0 is too old; Planroom needs 2.34.1 or newer.");
        assert_eq!(error["details"]["found"], "2.30.0");
        assert_eq!(error["details"]["minimum"], "2.34.1");
        assert_eq!(error["details"]["fix"], "install git 2.34.1 or newer, then retry");
        assert_eq!(error["retry"], "never");
    }
    assert!(!ws.join(".planroom").exists(), "a scan on too old a git reads and writes nothing");

    // No git at all answers: the same code, saying so.
    let empty = dir.path().join("no-git");
    std::fs::create_dir_all(&empty).unwrap();
    let env = envelope(&planroom_with(&ws, &["scan", ".", "--json"], &[("PATH", empty.as_os_str())]));
    assert_eq!(env["error"]["code"], "git_too_old", "{env}");
    assert_eq!(env["error"]["details"]["found"], Value::Null);
    assert_eq!(env["error"]["message"], "No git on PATH answered; Planroom needs git 2.34.1 or newer.");

    // At the floor, the scan runs.
    let floor = on_path(&stub("2.34.1"));
    let env = envelope(&planroom_with(&ws, &["scan", ".", "--json"], &[("PATH", &floor)]));
    assert_eq!(env["ok"], true, "{env}");
    assert_eq!(top_ids(&ws, &[]), ["alien:repo:alien", "mine:repo:mine"]);
}

#[test]
fn the_other_owner_skip_names_no_command() {
    // Planroom never runs git in another user's repo, and offers no command to run there either:
    // a copy of another user's repo can carry that user's FIFOs and config includes, so the
    // skip's `fix` is null and its one line says who owns the repo, that Planroom won't run git
    // there, and who can scan it. The workspace's path holds a space and a quote.
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("my ws's");
    let ws = two_repos(&base);
    let alien = ws.join("alien");
    let me = owner(&alien);
    // Who runs the engine: a user other than root, and the user it treats as itself. As root,
    // that's uid 1234 against a repo uid 65534 owns; otherwise this user, with the seam standing
    // in for another one.
    let user = if me == 0 { 1234 } else { me };
    let run_in = |ws: &Path, program: &str, args: &[&str], seam: Option<u32>| -> Output {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(ws)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", ws)
            .env("PLANROOM_DATA", ws.join(".data"));
        if let Some(uid) = seam {
            cmd.env("PLANROOM_TEST_EUID", uid.to_string());
        }
        if me == 0 {
            cmd.uid(user).gid(user);
        }
        cmd.output().unwrap()
    };
    let engine = env!("CARGO_BIN_EXE_planroom");
    let (seam, alien_owner) = if me == 0 {
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        chown(user, &base);
        chown(65534, &alien);
        (None, 65534)
    } else {
        (Some(me.wrapping_add(1)), me)
    };

    let env = envelope(&run_in(&ws, engine, &["scan", ".", "--json"], seam));
    assert_eq!(env["ok"], true, "{env}");
    let alien_skip = skip_of(&env, "alien");
    refused_with_no_command(&alien_skip, &owns_repo(alien_owner, "alien"));
    // The registry, and the reference built from it, say other_owner's fix is null.
    let meaning = planroom_contract::WarningCode::RepoSkipped.meaning();
    assert!(meaning.contains("null for other_owner"), "{meaning}");

    // A linked worktree whose git directory sits in a repo another user owns, a worktree of a
    // bare repo another user owns, and a repo whose git directory another user keeps apart from
    // its working tree: each is skipped, naming the git directory, with no command. Only root
    // can plant them: the engine runs as uid 1234.
    if me != 0 {
        return;
    }
    let ws = dir.path().join("held ws's");
    let main = ws.join("main");
    git_repo(&main, &[("main.txt".to_owned(), b"main\n".to_vec())]);
    git(&main, &["worktree", "add", "-q", "-b", "topic", "../wt"]);
    let bare = dir.path().join("bare.git");
    git(dir.path(), &["clone", "-q", "--bare", main.to_str().unwrap(), bare.to_str().unwrap()]);
    git(&bare, &["worktree", "add", "-q", "-b", "side", ws.join("bwt").to_str().unwrap()]);
    let apart = dir.path().join("apart.git");
    let (apart_arg, main_arg, sep_arg) =
        (apart.to_str().unwrap(), main.to_str().unwrap(), ws.join("sep").display().to_string());
    git(dir.path(), &["clone", "-q", "--separate-git-dir", apart_arg, main_arg, &sep_arg]);
    chown(user, &ws);
    chown(65534, &main);
    chown(65534, &bare);
    chown(65534, &apart);
    let ws = ws.canonicalize().unwrap();
    let env = envelope(&run_in(&ws, engine, &["scan", ".", "--json"], None));
    assert_eq!(env["ok"], true, "{env}");
    let skipped = ["bwt", "main", "sep", "wt"].map(|repo| skipped_for(repo, "other_owner"));
    assert_eq!(skips(&env), skipped, "{env}");
    refused_with_no_command(&skip_of(&env, "main"), &owns_repo(65534, "main"));
    let held = |place: &Path| place.canonicalize().unwrap().display().to_string();
    let wt_git = held(&main.join(".git/worktrees/wt"));
    refused_with_no_command(&skip_of(&env, "wt"), &owns_git_dir(65534, "wt", &wt_git));
    let bwt_git = held(&bare.join("worktrees/bwt"));
    refused_with_no_command(&skip_of(&env, "bwt"), &owns_git_dir(65534, "bwt", &bwt_git));
    refused_with_no_command(&skip_of(&env, "sep"), &owns_git_dir(65534, "sep", &held(&apart)));
}

/// A temporary directory root owns at mode 755, which uid 1234 can reach into; `None` unless the
/// tests run as root, since only root can plant a repo another user owns.
fn root_tempdir() -> Option<tempfile::TempDir> {
    let dir = tempfile::tempdir().unwrap();
    if owner(dir.path()) != 0 {
        return None;
    }
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    Some(dir)
}

/// `program ARGS` in `cwd` as uid 1234, which isn't root, with a cleared environment, `HOME` at
/// `home` and the data root under it.
fn as_user(cwd: &Path, home: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .env("PLANROOM_DATA", home.join(".data"))
        .uid(1234)
        .gid(1234)
        .output()
        .unwrap()
}

/// A scan of `ws` as uid 1234, which succeeds.
fn scan_as_user(ws: &Path, home: &Path) -> Value {
    let env = envelope(&as_user(ws, home, env!("CARGO_BIN_EXE_planroom"), &["scan", ".", "--json"]));
    assert_eq!(env["ok"], true, "{env}");
    env
}

/// A scan of `ws` as uid 1234, killed when it hasn't finished in 30 seconds, which succeeds.
fn scan_as_user_within_30s(ws: &Path, home: &Path) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", ".", "--json"])
        .current_dir(ws)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .env("PLANROOM_DATA", home.join(".data"))
        .uid(1234)
        .gid(1234)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("the scan of {} still running after 30 s", ws.display());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let env = envelope(&child.wait_with_output().unwrap());
    assert_eq!(env["ok"], true, "{env}");
    env
}

/// The top-level IDs `map --json` lists in `ws`, run as uid 1234.
fn top_ids_as_user(ws: &Path, home: &Path) -> Vec<String> {
    let map = envelope(&as_user(ws, home, env!("CARGO_BIN_EXE_planroom"), &["map", "--json"]));
    let nodes = map["result"]["nodes"].as_array().unwrap_or_else(|| panic!("{map}"));
    nodes.iter().map(|n| n["id"].as_str().unwrap().to_owned()).collect()
}

/// The `repo_skipped` warning for `repo` in `env`.
fn skip_of(env: &Value, repo: &str) -> Value {
    let found = warnings(env, "repo_skipped").into_iter().find(|w| w["details"]["repo"] == repo);
    found.unwrap_or_else(|| panic!("{repo} isn't skipped: {env}")).clone()
}

/// The pair [`skips`] lists for `repo` skipped with `reason`.
fn skipped_for(repo: &str, reason: &str) -> (String, String) {
    (repo.to_owned(), reason.to_owned())
}

#[test]
fn another_users_linked_worktree_and_its_main_repo_are_skipped_with_no_command() {
    // Another user owns a main repo and its linked worktree, both in the user's workspace. Each
    // is skipped, naming its owner, with no command to run.
    let Some(dir) = root_tempdir() else { return };
    let ws = dir.path().join("ws");
    let main = ws.join("main");
    git_repo(&main, &[("main.txt".to_owned(), b"main\n".to_vec())]);
    git(&main, &["worktree", "add", "-q", "-b", "topic", "../wt"]);
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &ws);
    chown(1234, &home);
    chown(65534, &main);
    chown(65534, &ws.join("wt"));
    let ws = ws.canonicalize().unwrap();

    let env = scan_as_user(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("main", "other_owner"), skipped_for("wt", "other_owner")]);
    refused_with_no_command(&skip_of(&env, "main"), &owns_repo(65534, "main"));
    refused_with_no_command(&skip_of(&env, "wt"), &owns_repo(65534, "wt"));
}

#[test]
fn a_worktree_whose_main_repo_another_user_owns_is_skipped_with_no_command() {
    // The main repo of the user's linked worktree is root's, outside the workspace. The worktree
    // is the user's, and the git directory it uses is root's, so it's skipped, naming that.
    let Some(dir) = root_tempdir() else { return };
    let outside = dir.path().join("outside");
    let main = outside.join("main");
    git_repo(&main, &[("main.txt".to_owned(), b"main\n".to_vec())]);
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    git(&main, &["worktree", "add", "-q", "-b", "topic", ws.join("wt").to_str().unwrap()]);
    for place in [&outside, &main] {
        std::fs::set_permissions(place, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &ws);
    chown(1234, &home);
    let ws = ws.canonicalize().unwrap();

    let env = scan_as_user(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("wt", "other_owner")], "{env}");
    let wt_git = main.join(".git/worktrees/wt").canonicalize().unwrap().display().to_string();
    refused_with_no_command(&skip_of(&env, "wt"), &owns_git_dir(0, "wt", &wt_git));
}

#[test]
fn a_repo_whose_git_directory_is_a_symlink_to_another_users_is_skipped() {
    // The user's repo's `.git` is a symlink to a git directory root owns: the git directory it
    // uses is root's, so it's skipped, naming that, with no command.
    let Some(dir) = root_tempdir() else { return };
    let kit = dir.path().join("other/kit");
    git_repo(&kit, &[("kit.txt".to_owned(), b"kit\n".to_vec())]);
    let ws = dir.path().join("ws");
    let r = ws.join("r");
    std::fs::create_dir_all(&r).unwrap();
    std::fs::write(r.join("kit.txt"), "kit\n").unwrap();
    std::os::unix::fs::symlink(kit.join(".git"), r.join(".git")).unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &ws);
    chown(1234, &home);
    let ws = ws.canonicalize().unwrap();

    let env = scan_as_user(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("r", "other_owner")], "{env}");
    let shown = ws.join("r/.git").display().to_string();
    refused_with_no_command(&skip_of(&env, "r"), &owns_git_dir(0, "r", &shown));
}

#[test]
fn a_submodule_or_a_single_repo_workspace_another_user_owns_is_skipped() {
    // A submodule another user owns in the user's own superproject, and a workspace that is one
    // repo another user owns: each is skipped, naming its owner, with no command.
    let Some(dir) = root_tempdir() else { return };
    let src = dir.path().join("src");
    git_repo(&src, &[("s.txt".to_owned(), b"s\n".to_vec())]);
    let sup = dir.path().join("super");
    git_repo(&sup, &[("x.txt".to_owned(), b"x\n".to_vec())]);
    let url = src.display().to_string();
    git(&sup, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "sub"]);
    git(&sup, &["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "Sub"]);
    let home = dir.path().join("home ws's");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &sup);
    chown(1234, &home);
    chown(65534, &sup.join("sub"));
    chown(65534, &sup.join(".git/modules/sub"));

    let env = scan_as_user(&sup, &home);
    assert_eq!(skips(&env), [skipped_for("sub", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "sub"), &owns_repo(65534, "sub"));
    let ids = top_ids_as_user(&sup, &home);
    assert!(ids.iter().all(|id| !id.starts_with("sub:")), "{ids:?}");

    let solo = dir.path().join("solo");
    git_repo(&solo, &[("solo.txt".to_owned(), b"solo\n".to_vec())]);
    chown(65534, &solo);
    let env = scan_as_user(&solo, &home);
    assert_eq!(skips(&env), [skipped_for("solo", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "solo"), &owns_repo(65534, "solo"));
    // Nothing maps, and the empty state's line names no command to run in the repo either.
    assert_eq!(env["result"]["map"]["empty"]["reason"], "repos_unreadable", "{env}");
    assert_eq!(env["result"]["map"]["empty"]["line"], others_line("solo"), "{env}");
}

#[test]
fn a_shared_clone_of_another_users_repo_is_refused_before_git_runs() {
    // `git clone --shared` leaves a repo the user owns whole that reads its objects from the
    // repo it was cloned from, through `objects/info/alternates`. That object store is another
    // user's, so a FIFO they plant there could block git. The engine reads the alternates before
    // git runs and skips the repo as other_owner, naming the object directory, with no command;
    // a FIFO planted in that store never blocks the scan.
    let Some(dir) = root_tempdir() else { return };
    let theirs = dir.path().join("theirs");
    git_repo(&theirs, &[("t.txt".to_owned(), b"t\n".to_vec())]);
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    git(&ws, &["clone", "-q", "--shared", theirs.to_str().unwrap(), "mine"]);
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &ws);
    chown(1234, &home);
    let ws = ws.canonicalize().unwrap();
    let objects = theirs.join(".git/objects").canonicalize().unwrap();
    let want = borrows_from(0, "mine", &objects);
    let env = scan_as_user_within_30s(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("mine", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "mine"), &want);

    // Root plants FIFOs where git would open them in its own object store.
    for place in ["objects/info/alternates", "objects/info/commit-graph", "objects/info/packs"] {
        let fifo = theirs.join(".git").join(place);
        let _ = std::fs::remove_file(&fifo);
        assert!(Command::new("mkfifo").arg(&fifo).output().unwrap().status.success());
    }
    let env = scan_as_user_within_30s(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("mine", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "mine"), &want);
}

#[test]
fn an_alternate_through_a_symlink_and_dotdot_is_refused_where_git_reads_it() {
    // The user's own repo names an alternate as `P/lnk/../theirs/.git/objects`, where `lnk` links
    // to `P/deep/sub`. By name that's `P/theirs`, which is missing; git from 2.39 on follows the
    // link first and reads root's `P/deep/theirs`. The owner check counts both readings, so the
    // repo is skipped as other_owner before git runs, and a FIFO root plants there never blocks.
    let Some(dir) = root_tempdir() else { return };
    let p = dir.path().canonicalize().unwrap();
    let theirs = p.join("deep/theirs");
    git_repo(&theirs, &[("t.txt".to_owned(), b"t\n".to_vec())]);
    std::fs::create_dir_all(p.join("deep/sub")).unwrap();
    std::os::unix::fs::symlink(p.join("deep/sub"), p.join("lnk")).unwrap();
    let ws = p.join("ws");
    let mine = ws.join("mine");
    git_repo(&mine, &[("m.txt".to_owned(), b"m\n".to_vec())]);
    let line = format!("{}\n", p.join("lnk/../theirs/.git/objects").display());
    std::fs::write(mine.join(".git/objects/info/alternates"), line).unwrap();
    let home = p.join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &ws);
    chown(1234, &home);
    let objects = theirs.join(".git/objects");
    let want = borrows_from(0, "mine", &objects);
    let env = scan_as_user_within_30s(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("mine", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "mine"), &want);
    for place in ["objects/info/alternates", "objects/info/commit-graph"] {
        let fifo = theirs.join(".git").join(place);
        let _ = std::fs::remove_file(&fifo);
        assert!(Command::new("mkfifo").arg(&fifo).output().unwrap().status.success());
    }
    let env = scan_as_user_within_30s(&ws, &home);
    assert_eq!(skips(&env), [skipped_for("mine", "other_owner")], "{env}");
}

/// `mkfifo PLACE`, in place of whatever was there.
fn plant_fifo(place: &Path) {
    let _ = std::fs::remove_file(place);
    assert!(Command::new("mkfifo").arg(place).output().unwrap().status.success());
}

#[test]
fn a_superproject_runs_no_git_in_its_submodules_before_their_own_check() {
    // The user's own superproject has two submodules: `sub` reads its objects from root's repo
    // (`--reference`), and uid 65534 owns `held`, its working tree and its git directory. The
    // submodules are listed from the superproject's own index and `.gitmodules`, so no git runs
    // in either before its own owner check, which skips each. FIFOs their owners plant where git
    // would open them, in root's object store and at held's HEAD, never block the scan, not even
    // the uncommitted-files count, whose git status would read held's HEAD.
    let Some(dir) = root_tempdir() else { return };
    let p = dir.path().canonicalize().unwrap();
    let theirs = p.join("theirs");
    git_repo(&theirs, &[("t.txt".to_owned(), b"t\n".to_vec())]);
    let other = p.join("other");
    git_repo(&other, &[("o.txt".to_owned(), b"o\n".to_vec())]);
    let sup = p.join("sup");
    git_repo(&sup, &[("x.txt".to_owned(), b"x\n".to_vec())]);
    let (reference, held_url) = (theirs.to_str().unwrap(), other.to_str().unwrap());
    let add = ["-c", "protocol.file.allow=always", "submodule", "add", "-q"];
    git(&sup, &[&add[..], &["--reference", reference, reference, "sub"]].concat());
    git(&sup, &[&add[..], &[held_url, "held"]].concat());
    git(&sup, &["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "Subs"]);
    let home = p.join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &sup);
    chown(1234, &home);
    chown(65534, &sup.join("held"));
    chown(65534, &sup.join(".git/modules/held"));
    let objects = theirs.join(".git/objects");

    let check = |env: &Value| {
        assert_eq!(skips(env), [skipped_for("held", "other_owner"), skipped_for("sub", "other_owner")]);
        refused_with_no_command(&skip_of(env, "held"), &owns_repo(65534, "held"));
        refused_with_no_command(&skip_of(env, "sub"), &borrows_from(0, "sub", &objects));
    };
    check(&scan_as_user_within_30s(&sup, &home));
    assert_eq!(top_ids_as_user(&sup, &home), ["sup:repo:sup"]);
    plant_fifo(&objects.join("info/alternates"));
    plant_fifo(&sup.join(".git/modules/held/HEAD"));
    check(&scan_as_user_within_30s(&sup, &home));
}

#[test]
fn a_root_repo_another_user_owns_or_borrows_from_is_skipped_whatever_its_submodules() {
    // A workspace that is one repo with a submodule, scanned as uid 1234: when uid 65534 owns the
    // repo, or the repo is the user's own but reads root's objects, the owner check runs on it
    // before any git call there, the submodule listing included, so it's skipped as other_owner
    // with no command, as it would be with no `.gitmodules`; the scan never fails as git_failed.
    // The empty state's line says who can scan each: never the user's own repo by its owner.
    let Some(dir) = root_tempdir() else { return };
    let p = dir.path().canonicalize().unwrap();
    let src = p.join("src");
    git_repo(&src, &[("s.txt".to_owned(), b"s\n".to_vec())]);
    let with_submodule = |repo: &Path| {
        let url = src.to_str().unwrap();
        git(repo, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", url, "sub"]);
        git(
            repo,
            &["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "Sub"],
        );
    };
    let home = p.join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &home);

    let solo = p.join("solo");
    git_repo(&solo, &[("solo.txt".to_owned(), b"solo\n".to_vec())]);
    with_submodule(&solo);
    chown(65534, &solo);
    let env = scan_as_user_within_30s(&solo, &home);
    assert_eq!(skips(&env), [skipped_for("solo", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "solo"), &owns_repo(65534, "solo"));
    assert_eq!(env["result"]["map"]["empty"]["line"], others_line("solo"), "{env}");

    let theirs = p.join("theirs");
    git_repo(&theirs, &[("t.txt".to_owned(), b"t\n".to_vec())]);
    let ws = p.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    git(&ws, &["clone", "-q", "--shared", theirs.to_str().unwrap(), "mine"]);
    let mine = ws.join("mine");
    with_submodule(&mine);
    chown(1234, &ws);
    let objects = theirs.join(".git/objects");
    let env = scan_as_user_within_30s(&mine, &home);
    assert_eq!(skips(&env), [skipped_for("mine", "other_owner")], "{env}");
    refused_with_no_command(&skip_of(&env, "mine"), &borrows_from(0, "mine", &objects));
    assert_eq!(env["result"]["map"]["empty"]["line"], borrowed_line("mine"), "{env}");

    // Beside a repo another user owns and one git can't read, each reason reads apart. The scan
    // of `mine` alone may have made it a workspace root of its own, which a scan of `ws` skips.
    let _ = std::fs::remove_dir_all(mine.join(".git/planroom"));
    git_repo(&ws.join("alien"), &[("a.txt".to_owned(), b"a\n".to_vec())]);
    git_repo(&ws.join("broken"), &[("b.txt".to_owned(), b"b\n".to_vec())]);
    let config = ws.join("broken/.git/config");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}[core\nthis is not a config\n")).unwrap();
    chown(1234, &ws.join("broken"));
    chown(65534, &ws.join("alien"));
    let env = scan_as_user_within_30s(&ws, &home);
    let line = "Nothing here can be mapped yet. git can't read broken; run git status in each to see why. A user other than you owns alien, where Planroom never runs git; its owner can scan each, or you can scan a copy you own. Another user owns objects read by mine, and Planroom never runs git there; you can scan a copy of each that borrows none of those objects. Then check again.";
    assert_eq!(env["result"]["map"]["empty"]["line"], line, "{env}");
}

/// The empty state's line when every repo in the workspace, `names`, is the person's own but
/// reads objects another user owns; worded to fit one repo or several.
fn borrowed_line(names: &str) -> String {
    format!(
        "Another user owns objects read by {names}, and Planroom never runs git in another user's repo, so there's nothing to map yet. You can scan a copy of each that borrows none of those objects, then check again."
    )
}

/// `planroom ARGS --json` in `ws` as uid 1234, which succeeds.
fn json_as_user(ws: &Path, home: &Path, args: &[&str]) -> Value {
    let args = [args, &["--json"]].concat();
    let env = envelope(&as_user(ws, home, env!("CARGO_BIN_EXE_planroom"), &args));
    assert_eq!(env["ok"], true, "{env}");
    env
}

#[test]
fn a_root_whose_dot_git_file_the_owner_check_refuses_is_skipped_as_other_owner() {
    // Workspaces whose root is a working tree with a `.git` file, scanned as uid 1234: a
    // submodule checkout of the user's own that reads root's objects (`--reference`), a linked
    // worktree of the user's own `--shared` clone of root's repo, a working tree whose `.git` file
    // names root's git directory, and a linked worktree of root's own repo. The owner check
    // refuses each before any git call there, so the scan skips the root as other_owner with no
    // command, and focus and map show the empty state that says so; none stops with internal or
    // says there are no repos.
    let Some(dir) = root_tempdir() else { return };
    let p = dir.path().canonicalize().unwrap();
    let theirs = p.join("theirs");
    git_repo(&theirs, &[("t.txt".to_owned(), b"t\n".to_vec())]);
    let objects = theirs.join(".git/objects");
    let home = p.join("home");
    std::fs::create_dir_all(&home).unwrap();
    chown(1234, &home);

    let sup = p.join("sup");
    git_repo(&sup, &[("x.txt".to_owned(), b"x\n".to_vec())]);
    let reference = theirs.to_str().unwrap();
    let add = ["-c", "protocol.file.allow=always", "submodule", "add", "-q", "--reference", reference];
    git(&sup, &[&add[..], &[reference, "sub"]].concat());
    git(&sup, &["-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "Sub"]);
    chown(1234, &sup);

    git(&p, &["clone", "-q", "--shared", reference, "clone"]);
    let clone = p.join("clone");
    git(&clone, &["worktree", "add", "-q", "-b", "topic", p.join("wt2").to_str().unwrap()]);
    chown(1234, &clone);
    chown(1234, &p.join("wt2"));

    let named = p.join("named");
    std::fs::create_dir_all(&named).unwrap();
    std::fs::write(named.join("t.txt"), "t\n").unwrap();
    std::fs::write(named.join(".git"), format!("gitdir: {}\n", theirs.join(".git").display())).unwrap();
    chown(1234, &named);

    let main = p.join("main");
    git_repo(&main, &[("m.txt".to_owned(), b"m\n".to_vec())]);
    git(&main, &["worktree", "add", "-q", "-b", "topic", p.join("wt").to_str().unwrap()]);
    chown(1234, &p.join("wt"));

    let wt_git = main.join(".git/worktrees/wt").display().to_string();
    let theirs_git = theirs.join(".git").display().to_string();
    let cases = [
        (sup.join("sub"), "sub", borrows_from(0, "sub", &objects), borrowed_line("sub")),
        (p.join("wt2"), "wt2", borrows_from(0, "wt2", &objects), borrowed_line("wt2")),
        (named.clone(), "named", owns_git_dir(0, "named", &theirs_git), others_line("named")),
        (p.join("wt"), "wt", owns_git_dir(0, "wt", &wt_git), others_line("wt")),
    ];
    for (ws, repo, skip, line) in cases {
        let env = scan_as_user_within_30s(&ws, &home);
        assert_eq!(skips(&env), [skipped_for(repo, "other_owner")], "{env}");
        refused_with_no_command(&skip_of(&env, repo), &skip);
        assert_eq!(env["result"]["map"]["empty"]["reason"], "repos_unreadable", "{env}");
        assert_eq!(env["result"]["map"]["empty"]["line"], line, "{env}");
        for command in ["focus", "map"] {
            let got = json_as_user(&ws, &home, &[command]);
            assert_eq!(got["result"]["empty"]["reason"], "repos_unreadable", "{command} in {repo}: {got}");
            assert_eq!(got["result"]["empty"]["line"], line, "{command} in {repo}: {got}");
        }
    }
}

#[test]
fn an_exclude_file_that_is_dev_null_maps() {
    // git reads a file that resolves to /dev/null as empty, so the user's own repo whose
    // `info/exclude` is a symlink to it maps; any other device there is still refused.
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let r = ws.join("r");
    git_repo(&r, &[("a.txt".to_owned(), b"a\n".to_vec())]);
    let exclude = r.join(".git/info/exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&exclude);
    std::os::unix::fs::symlink("/dev/null", &exclude).unwrap();
    let ws = ws.canonicalize().unwrap();
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(env["ok"], true, "{env}");
    assert!(skips(&env).is_empty(), "{env}");
    assert_eq!(top_ids(&ws, &[]), ["r:repo:r"]);

    std::fs::remove_file(&exclude).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &exclude).unwrap();
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(skips(&env), [skipped_for("r", "git_failed")], "{env}");
    // Its fix shows the link in the repo, the file to replace, never the device it names.
    let shown = shell_word(&ws.join("r/.git/info/exclude").display().to_string());
    let fix = format!(
        "Run ls -l {shown} to see what it is: git needs a directory or a regular file there. Replace it, then scan again."
    );
    let warning = skip_of(&env, "r");
    assert!(same_but_masked(&warning["details"]["fix"], &fix), "{warning} isn't {fix}");
}

/// The files git opens at fixed paths in a git directory or a common directory.
const FIXED_FILES: [&str; 9] = [
    "HEAD",
    "config",
    "config.worktree",
    "packed-refs",
    "index",
    "shallow",
    "objects/info/alternates",
    "info/exclude",
    "info/attributes",
];

/// The `git_failed` fix for a place git needs as a regular file that isn't one.
fn not_a_file_fix(place: &Path) -> String {
    let shown = shell_word(&place.canonicalize().unwrap().display().to_string());
    format!(
        "Run ls -l {shown} to see what it is: git needs a directory or a regular file there. Replace it, then scan again."
    )
}

#[test]
fn a_fifo_at_a_file_git_opens_never_blocks_a_scan() {
    // A FIFO with no writer blocks whoever opens it, git included. Before git runs, the engine
    // checks each file git opens at a fixed path in the repo's git directory and common
    // directory, so one that isn't a regular file fails as git_failed, whose fix shows it with
    // `ls -l`, which never opens it.
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let mut planted = Vec::new();
    for (i, file) in FIXED_FILES.iter().enumerate() {
        let repo = ws.join(format!("r{i}"));
        git_repo(&repo, &[("a.txt".to_owned(), b"a\n".to_vec())]);
        planted.push((format!("r{i}"), repo.join(".git").join(file)));
    }
    // A linked worktree's own git directory, under the main repo's.
    let main = dir.path().join("main");
    git_repo(&main, &[("m.txt".to_owned(), b"m\n".to_vec())]);
    git(&main, &["worktree", "add", "-q", "-b", "topic", ws.join("wt").to_str().unwrap()]);
    let wt_git = main.join(".git/worktrees/wt");
    planted.push(("wt".to_owned(), wt_git.join("index")));
    let fifo = |place: &Path| {
        let _ = std::fs::remove_file(place);
        std::fs::create_dir_all(place.parent().unwrap()).unwrap();
        let out = Command::new("mkfifo").arg(place).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    for (_, place) in &planted {
        fifo(place);
    }
    let ws = ws.canonicalize().unwrap();
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(env["ok"], true, "{env}");
    let mut want: Vec<(String, String)> = planted.iter().map(|(r, _)| skipped_for(r, "git_failed")).collect();
    want.sort();
    assert_eq!(skips(&env), want, "{env}");
    for (repo, place) in &planted {
        let warning = skip_of(&env, repo);
        let fix = not_a_file_fix(place);
        assert!(same_but_masked(&warning["details"]["fix"], &fix), "{warning} isn't {fix}");
    }

    // The common directory's files count for the worktree too.
    std::fs::remove_file(wt_git.join("index")).unwrap();
    let common = main.join(".git/packed-refs");
    fifo(&common);
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(env["ok"], true, "{env}");
    let warning = skip_of(&env, "wt");
    let fix = not_a_file_fix(&common);
    assert!(same_but_masked(&warning["details"]["fix"], &fix), "{warning} isn't {fix}");
}

#[test]
fn a_dot_git_that_is_a_fifo_gets_the_git_status_fix_which_ends_at_once() {
    // A `.git` that is neither a directory nor a regular file names no git directory. Its fix is
    // git status with the repo's own `.git` named, which git refuses at once without opening it.
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    let r = ws.join("r");
    std::fs::create_dir_all(&r).unwrap();
    assert!(Command::new("mkfifo").arg(r.join(".git")).output().unwrap().status.success());
    let ws = ws.canonicalize().unwrap();
    let env = envelope(&planroom_within_30s(&ws, &["scan", ".", "--json"], &[]));
    assert_eq!(skips(&env), [skipped_for("r", "git_failed")], "{env}");
    let r = ws.join("r").display().to_string();
    let command = format!("git -C {} --git-dir=.git status", shell_word(&r));
    let want = format!("Run {command} to see why, fix it, then scan again.");
    let warning = skip_of(&env, "r");
    assert!(same_but_masked(&warning["details"]["fix"], &want), "{warning} isn't {want}");
    let mut child = Command::new("sh")
        .args(["-c", &command])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        assert!(std::time::Instant::now() < deadline, "{command} still running after 10 s");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && stderr.contains("not a git repository"), "{stderr}");
}
