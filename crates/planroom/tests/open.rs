//! `planroom open` where no browser can be seen (DR-29): no display on Linux, an SSH session, or an
//! opener that fails. It opens nothing, keeps no login page whose code could still sign a tab in,
//! and offers sharing the canvas as an Artifact instead.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_testkit::fixture;
use serde_json::{Value, json};

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

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// A press's token over `command` with no arguments, signed at `at`.
fn press(key: &SigningKey, command: &str, at: u64) -> String {
    let sig: Signature = key.sign(&person_token_message(command, &[], "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

fn planroom(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root)
        .env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_ACTOR")
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_TTY")
        .stdin(Stdio::null());
    cmd
}

/// A scanned fixture workspace with a registered person key.
fn workspace() -> (tempfile::TempDir, PathBuf, SigningKey) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    assert!(planroom(&root).args(["scan", "--json"]).output().unwrap().status.success());
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    bytes[31] = 41;
    let key = SigningKey::from_slice(&bytes).unwrap();
    let mut child = planroom(&root)
        .args(["person", "init", "--json"])
        .env("PLANROOM_PERSON_TOKEN", press(&key, "person init", now()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let public = b64url(&key.verifying_key().to_sec1_point(false).to_bytes());
    child.stdin.take().unwrap().write_all(public.as_bytes()).unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    (tmp, root, key)
}

/// A stand-in opener that leaves a mark when it runs, and exits with `status`.
fn opener(dir: &Path, status: u8) -> (PathBuf, PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    let (path, mark) = (dir.join(format!("opener-{status}")), dir.join(format!("ran-{status}")));
    std::fs::write(&path, format!("#!/bin/sh\ntouch '{}'\nexit {status}\n", mark.display())).unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    (path, mark)
}

/// Every server the workspace's data root records, stopped when dropped.
struct Serves(PathBuf);

impl Drop for Serves {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(self.0.join(".data/serve")).into_iter().flatten().flatten() {
            let record: Value =
                serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap_or_default())
                    .unwrap_or_default();
            if let Some(pid) = record["pid"].as_u64() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
        }
    }
}

fn login_pages(root: &Path) -> usize {
    std::fs::read_dir(root.join(".data/open")).map_or(0, |d| d.flatten().count())
}

#[test]
fn open_without_a_display_offers_share() {
    let (tmp, root, key) = workspace();
    let _serves = Serves(root.clone());
    let (works, ran) = opener(&tmp.path().join("bin"), 0);
    let (fails, failed) = opener(&tmp.path().join("bin"), 1);
    // Each press its own token, a second apart: two signed at one second over one line would be the
    // same token.
    let first = now();
    let mut ago = 0;
    let mut open = |env: &[(&str, &str)], remove: &[&str]| {
        let mut cmd = planroom(&root);
        cmd.args(["open", "--json"]).env("PLANROOM_PERSON_TOKEN", press(&key, "open", first - ago));
        ago += 1;
        for name in remove {
            cmd.env_remove(name);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().unwrap();
        let env: Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
        assert_eq!(env["ok"], true, "{env}");
        env
    };
    let offers_share = |env: &Value| {
        assert_eq!(env["result"]["opened"], false, "{env}");
        let empty = &env["result"]["empty"];
        assert_eq!(empty["reason"], "no_browser", "{env}");
        assert_eq!(empty["next"]["command"], "share");
        // An absolute `--out`, so the handle runs from any directory the pane runs it in.
        let state = root.canonicalize().unwrap().join(".planroom/share");
        assert_eq!(empty["next"]["args"], json!(["--out", state.to_str().unwrap()]));
        assert!(empty["line"].as_str().is_some_and(|l| l.contains("nothing was opened")), "{env}");
        assert_eq!(env["summary"], empty["line"]);
    };
    let works = works.to_str().unwrap();

    // No display on Linux: the opener isn't even looked for. (A Mac always has its display.)
    if cfg!(target_os = "linux") {
        let env = open(&[("PLANROOM_TEST_OPENER", works)], &["DISPLAY", "WAYLAND_DISPLAY"]);
        offers_share(&env);
        assert!(!ran.exists(), "nothing was opened");
        // Run from a repo inside the workspace, as the pane runs it from the session's directory,
        // the handle writes the bundle into the state directory, never into that repo's tree.
        let next = &env["result"]["empty"]["next"];
        let mut args: Vec<String> =
            next["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap().to_owned()).collect();
        args.insert(0, next["command"].as_str().unwrap().to_owned());
        args.push("--json".into());
        let repo = root.join("orders");
        let out = planroom(&root).current_dir(&repo).args(&args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        assert!(!repo.join(".planroom").exists(), "nothing written in the repo's tree");
        assert!(root.join(".planroom/share").is_dir());
    }
    // An SSH session: the browser it would open is on another machine's screen, or none.
    let env = open(
        &[("PLANROOM_TEST_OPENER", works), ("DISPLAY", ":0"), ("SSH_CONNECTION", "10.0.0.1 22 10.0.0.2 22")],
        &[],
    );
    offers_share(&env);
    assert!(!ran.exists(), "nothing was opened");
    // An opener that fails.
    let env = open(&[("PLANROOM_TEST_OPENER", fails.to_str().unwrap()), ("DISPLAY", ":0")], &[]);
    offers_share(&env);
    assert!(failed.exists(), "the opener ran and failed");
    // No login page is left behind whose code could still sign a tab in.
    assert_eq!(login_pages(&root), 0);

    // Where one can be seen, it opens.
    let env = open(&[("PLANROOM_TEST_OPENER", works), ("DISPLAY", ":0")], &[]);
    assert_eq!(env["result"], json!({"opened": true}), "{env}");
    assert!(ran.exists());
}
