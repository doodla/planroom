//! Person keys and action tokens (DR-64, DR-72, DR-75): `person init` registers a public key only
//! when none is registered, and a person-only command needs a fresh, unspent ECDSA P-256 token
//! over its own command and arguments. The test harness signs its own tokens with test keys.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use planroom_kernel::keys::person_token_message;
use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::Value;

/// A test key pair from a fixed scalar.
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

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// A token over `person init` with `args`, at `at`.
fn token(key: &SigningKey, args: &[&str], at: u64) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let sig: Signature = key.sign(&person_token_message("person init", &args, "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// `planroom person init ARGS --json` with `stdin` and the environment `env`.
fn init(root: &Path, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.args(["person", "init"]).args(args).arg("--json").current_dir(root);
    cmd.env("PLANROOM_DATA", root.join(".data"))
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_ACTOR");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    // The command may refuse before it reads its stdin; its reply is what the test checks.
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{e}");
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let env: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}: {stdout}\n{}", String::from_utf8_lossy(&out.stderr)));
    (out.status.code().unwrap(), env)
}

fn reason(env: &Value) -> &str {
    env["error"]["details"]["reason"].as_str().unwrap_or("")
}

fn scanned() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    let out = Command::new(env!("CARGO_BIN_EXE_planroom"))
        .args(["scan", "--json"])
        .current_dir(&root)
        .env("PLANROOM_DATA", root.join(".data"))
        .output()
        .unwrap();
    assert!(out.status.success());
    (tmp, root)
}

#[test]
fn person_init_registers_a_key_only_when_none_is_registered() {
    let (_tmp, root) = scanned();
    let (a, b) = (key(7), key(9));
    let (pa, pb) = (public(&a), public(&b));

    // No token, or an agent: never a person.
    let (code, env) = init(&root, &[], &pa, &[]);
    assert_eq!((code, env["error"]["code"].as_str(), reason(&env)), (5, Some("person_required"), "missing"));
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_ACTOR", "agent:1a2b3c4d:chat")]);
    assert_eq!(reason(&env), "not_a_person");

    // A token from another key, a stale one, or one for other arguments.
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&b, &[], now()))]);
    assert_eq!(reason(&env), "bad_signature");
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now() - 60))]);
    assert_eq!(reason(&env), "stale");
    let (_, env) = init(&root, &["--rekey"], &pa, &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!(reason(&env), "bad_signature");
    let (code, env) = init(&root, &[], "not a key", &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (2, Some("invalid_argument")));

    // The first registration, signed by the key it registers, works once.
    let first = token(&a, &[], now());
    let (code, env) =
        init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &first), ("PLANROOM_CLIENT", "mod:s1")]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["surface"], "mod");
    assert_eq!(env["result"]["rekeyed"], false);
    let (_, env) = init(&root, &[], &pa, &[("PLANROOM_PERSON_TOKEN", &first)]);
    assert_eq!(reason(&env), "spent", "a token works once");

    // Another key is a mismatch, even signed by itself, until a rekey.
    let (code, env) = init(&root, &[], &pb, &[("PLANROOM_PERSON_TOKEN", &token(&b, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (5, Some("person_key_mismatch")));
    assert_eq!(env["error"]["details"]["surface"], "mod");
    assert_eq!(env["error"]["retry"], "never");

    // The store holds only the public key.
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap().as_deref(), Some(pa.as_str()));
    drop(store);

    // A rekey under the old key replaces it.
    let (code, env) =
        init(&root, &["--rekey"], &pb, &[("PLANROOM_PERSON_TOKEN", &token(&a, &["--rekey"], now()))]);
    assert_eq!(code, 0, "{env}");
    assert_eq!(env["result"]["rekeyed"], true);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert_eq!(store.meta("person_pubkey").unwrap().as_deref(), Some(pb.as_str()));
}

#[test]
fn a_rekey_takes_the_mismatch_panes_press_under_the_new_key() {
    let (_tmp, root) = scanned();
    let (a, b) = (key(3), key(4));
    let (code, _) = init(&root, &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!(code, 0);
    let press = token(&b, &["--rekey"], now());
    let (code, env) = init(&root, &["--rekey"], &public(&b), &[("PLANROOM_PERSON_TOKEN", &press)]);
    assert_eq!(code, 0, "{env}");
    assert!(env["result"]["fingerprint"].as_str().unwrap().len() == 16);
}

#[test]
fn person_init_needs_a_scanned_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let a = key(5);
    let (code, env) =
        init(dir.path(), &[], &public(&a), &[("PLANROOM_PERSON_TOKEN", &token(&a, &[], now()))]);
    assert_eq!((code, env["error"]["code"].as_str()), (3, Some("workspace_not_found")));
}
