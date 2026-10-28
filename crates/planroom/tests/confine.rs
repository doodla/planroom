//! Reads confined to the workspace (architecture.md, Security and privacy: Reads by auto-allowed
//! tools; DR-52): a read names only the workspace's repos, a credential path needs a person, and
//! `--workspace` on a read needs a workspace with a store, creating nothing.
//!
//! Each test has its own data root and home; its workspace sits in that home, beside `~/.ssh`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_testkit::git_repo;
use serde_json::Value;

const AGENT: &str = "agent:1a2b3c4d:chat";

struct Machine {
    tmp: tempfile::TempDir,
}

impl Machine {
    /// A home with `~/.ssh/id_test`, and a scanned workspace `~/ws` of one repo, `app`.
    fn new() -> (Machine, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().canonicalize().unwrap().join("home");
        fs::create_dir_all(home.join(".ssh")).unwrap();
        fs::write(home.join(".ssh/id_test"), "not a real key\n").unwrap();
        let ws = home.join("ws");
        git_repo(
            &ws.join("app"),
            &[
                ("docker-compose.yml".into(), b"services:\n  web:\n    image: redis:7\n".to_vec()),
                (".env".into(), b"TOKEN=fake\n".to_vec()),
            ],
        );
        let m = Machine { tmp };
        let (code, env) = m.run(&ws, &["scan"], &[]);
        assert_eq!(code, 0, "{env}");
        (m, ws)
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().canonicalize().unwrap().join("home")
    }

    fn run(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, Value) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
        cmd.current_dir(cwd).env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            cmd.env("PATH", path);
        }
        let out = cmd
            .env("HOME", self.home())
            .env("PLANROOM_DATA", self.tmp.path().join("data"))
            .env("XDG_CONFIG_HOME", self.tmp.path().join("cfg"))
            .args(args)
            .arg("--json")
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let env: Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
        (out.status.code().unwrap(), env)
    }
}

fn refused(reply: &(i32, Value), reason: &str) {
    let (code, env) = reply;
    assert_eq!(*code, 5, "{env}");
    assert_eq!(env["error"]["code"], "path_refused", "{env}");
    assert_eq!(env["error"]["details"]["reason"], reason, "{env}");
    assert!(
        env["error"].get("hint").is_none_or(Value::is_null),
        "no command a person runs reads it yet: {env}"
    );
}

fn not_refused(reply: &(i32, Value)) {
    assert_ne!(reply.1["error"]["code"], "path_refused", "{}", reply.1);
}

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

fn token(command: &str, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let sig: Signature = key().sign(&person_token_message(command, &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

#[test]
fn a_read_outside_the_workspace_is_path_refused() {
    let (m, ws) = Machine::new();
    let app = ws.join("app");
    let outside = m.home().join("notes.txt");
    fs::write(&outside, "elsewhere\n").unwrap();
    std::os::unix::fs::symlink(m.home(), app.join("up")).unwrap();
    for arg in [
        outside.to_str().unwrap(),
        "../notes.txt",
        "src/../../../notes.txt",
        "file:///etc/hostname",
        "~/notes.txt",
        "up/notes.txt",
        "../.planroom/store.db",
        "./.git/config",
    ] {
        refused(&m.run(&app, &["ask", "breakdown", arg], &[("PLANROOM_ACTOR", AGENT)]), "outside_workspace");
    }
    // A missing path resolves through its longest existing parent.
    refused(&m.run(&app, &["ask", "breakdown", "nope/../../../x/y"], &[]), "outside_workspace");
    // A path inside a repo, and a node ID whose `:` comes before its first `/`, pass on to the question.
    not_refused(&m.run(&app, &["ask", "breakdown", "docker-compose.yml"], &[("PLANROOM_ACTOR", AGENT)]));
    not_refused(&m.run(&ws, &["ask", "breakdown", "app/docker-compose.yml"], &[("PLANROOM_ACTOR", AGENT)]));
    not_refused(&m.run(&ws, &["ask", "impact", "app:dir:src/../../x"], &[("PLANROOM_ACTOR", AGENT)]));
}

#[test]
fn a_credential_path_needs_a_person() {
    let (m, ws) = Machine::new();
    let app = ws.join("app");
    // An agent, or no one, never names one: as given, or as it resolves.
    for arg in ["~/.ssh/id_test", "../../.ssh/id_test", "src/../../../.ssh/id_test", "./.env", "keys/x.pem"] {
        for env in [&[("PLANROOM_ACTOR", AGENT)][..], &[]] {
            refused(&m.run(&app, &["ask", "breakdown", arg], env), "credential");
        }
    }
    refused(&m.run(&ws, &["scan", "../.ssh"], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    refused(&m.run(&ws, &["--workspace", "../.ssh", "scan"], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    // A relative `--workspace` is placed from the process's own directory, as `scan DIR` is.
    let sub = m.home().join(".ssh/sub");
    fs::create_dir_all(&sub).unwrap();
    refused(&m.run(&sub, &["scan", ".."], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    refused(&m.run(&sub, &["--workspace", "..", "scan"], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    refused(&m.run(&sub, &["scan", "--workspace=.."], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    // A password store kept as a repo is a credential path too.
    let pass = m.home().join(".password-store");
    git_repo(&pass, &[("site.gpg".into(), b"not a real secret\n".to_vec())]);
    refused(&m.run(&m.home(), &["scan", ".password-store"], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    let flag = ["--workspace", pass.to_str().unwrap(), "scan"];
    refused(&m.run(&ws, &flag, &[("PLANROOM_ACTOR", AGENT)]), "credential");
    // As it resolves, too: a link to it under another name.
    std::os::unix::fs::symlink(&pass, m.home().join("plain")).unwrap();
    refused(&m.run(&m.home(), &["scan", "plain"], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    refused(&m.run(&pass.join(".git"), &["scan", ".."], &[("PLANROOM_ACTOR", AGENT)]), "credential");
    // With no home to place `~` by, any `.ssh` is the home's: the check fails closed.
    refused(&m.run(&ws, &["scan", "../.ssh"], &[("PLANROOM_ACTOR", AGENT), ("HOME", "")]), "credential");

    // A person may: their key is registered while the workspace is trusted.
    let public = b64url(&key().verifying_key().to_sec1_point(false).to_bytes());
    let mut init = Command::new(env!("CARGO_BIN_EXE_planroom"));
    init.current_dir(&ws)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", m.home())
        .env("PLANROOM_DATA", m.tmp.path().join("data"))
        .env("PLANROOM_PERSON_TOKEN", token("person init", &[]))
        .args(["person", "init", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = init.spawn().unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(public.as_bytes()).unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    let ask = |arg: &str, signed: &str| {
        m.run(&app, &["ask", "breakdown", arg], &[("PLANROOM_PERSON_TOKEN", signed)])
    };
    // The person's in-repo dotfile passes on to the question.
    let signed = token("ask", &["breakdown", "./.env"]);
    not_refused(&ask("./.env", &signed));
    // A token works once: a second use is no person's.
    refused(&ask("./.env", &signed), "credential");
    // Outside the workspace it's still no read.
    refused(&ask("~/.ssh/id_test", &token("ask", &["breakdown", "~/.ssh/id_test"])), "outside_workspace");
    // A person's scan of one runs as any scan does: no repos there.
    let signed = token("scan", &["../.ssh"]);
    let (code, env) = m.run(&ws, &["scan", "../.ssh"], &[("PLANROOM_PERSON_TOKEN", &signed)]);
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("workspace_not_found")), "{env}");

    // From inside, by any spelling or none, a repo kept in one is still a credential path for an
    // agent: a plain `scan` reads the directory it runs in.
    let keys = m.home().join(".ssh/keysrepo");
    git_repo(&keys, &[("notes.txt".into(), b"not a real secret\n".to_vec())]);
    for dir in [&keys, &pass] {
        let abs = dir.to_str().unwrap();
        for args in [
            &["scan"][..],
            &["scan", "."],
            &["--workspace", ".", "scan"],
            &["--workspace", abs, "scan"],
            &["scan", abs],
        ] {
            refused(&m.run(dir, args, &[("PLANROOM_ACTOR", AGENT)]), "credential");
        }
    }

    // A hidden directory that is no store is no credential path: an agent maps a dotfiles repo,
    // and the `next` handle of its empty map runs.
    let dots = m.home().join(".dotfiles");
    git_repo(&dots, &[("vimrc".into(), b"set number\n".to_vec())]);
    let (_, empty) = m.run(&dots, &["map"], &[("PLANROOM_ACTOR", AGENT)]);
    let next = &empty["result"]["empty"]["next"];
    assert_eq!(next["command"], "scan", "{empty}");
    let next: Vec<String> = serde_json::from_value(next["args"].clone()).unwrap();
    let mut args = vec!["scan"];
    args.extend(next.iter().map(String::as_str));
    not_refused(&m.run(&dots, &args, &[("PLANROOM_ACTOR", AGENT)]));
    not_refused(&m.run(&dots, &["scan"], &[("PLANROOM_ACTOR", AGENT)]));
}

#[test]
fn workspace_flag_on_a_read_needs_an_existing_store() {
    let (m, ws) = Machine::new();
    let empty = m.home().join("empty");
    fs::create_dir_all(&empty).unwrap();
    let elsewhere = m.home();
    for read in [&["ask", "kinds"][..], &["map"], &["find", "web"], &["focus"]] {
        let mut argv = vec!["--workspace", empty.to_str().unwrap()];
        argv.extend(read);
        let (code, env) = m.run(&elsewhere, &argv, &[]);
        assert_eq!(code, 3, "{read:?}: {env}");
        assert_eq!(env["error"]["code"], "workspace_not_found", "{env}");
        assert_eq!(env["error"]["details"]["path"], empty.to_str().unwrap(), "{env}");
        assert_eq!(env["command"], read[0], "{env}");
        // Its hint is the scan that makes DIR a workspace, run from where it was offered.
        assert_eq!(env["error"]["hint"], format!("planroom scan {}", empty.display()), "{env}");
        // The flag's other spelling, and a directory that doesn't exist, which no command fixes.
        let spelled = format!("--workspace={}", empty.join("nope").display());
        let mut argv = vec![spelled.as_str()];
        argv.extend(read);
        let (code, env) = m.run(&elsewhere, &argv, &[]);
        assert_eq!(code, 3, "{env}");
        assert!(env["error"].get("hint").is_none_or(Value::is_null), "{env}");
    }
    assert_eq!(fs::read_dir(&empty).unwrap().count(), 0, "nothing was created");
    // A scanned workspace reads from anywhere, and a scan runs in the directory it names.
    let (code, env) = m.run(&elsewhere, &["--workspace", ws.to_str().unwrap(), "map"], &[]);
    assert_eq!(code, 0, "{env}");
    assert!(env["result"]["nodes"].as_array().is_some_and(|n| n.iter().any(|n| n["name"] == "app")), "{env}");
    let (code, env) = m.run(&elsewhere, &["scan", "--workspace", ws.join("app").to_str().unwrap()], &[]);
    assert_eq!(code, 0, "{env}");
    // version and schema ignore it; a usage error still names its leaf.
    assert_eq!(m.run(&elsewhere, &["--workspace", empty.to_str().unwrap(), "version"], &[]).0, 0);
    let (code, env) = m.run(&elsewhere, &["--workspace", empty.to_str().unwrap(), "map", "--bogus"], &[]);
    assert_eq!(code, 2, "{env}");
    assert_eq!(env["command"], "map", "{env}");
}
