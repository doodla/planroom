//! `planroom serve` (DR-29, DR-58, DR-78): the login code, the bearer token, the exact `Origin` and
//! `Host`, the live stream within 1 s of any commit, the layout cache, the recorded port and the
//! detached start. HTTP is spoken by hand over `TcpStream`, so the tests need no client crate.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use planroom_store::Store;
use planroom_testkit::fixture;
use serde_json::{Value, json};

fn planroom(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", root.join(".data"));
    cmd
}

/// A scanned fixture workspace named `shop`.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("shop");
    fixture(&root);
    assert!(planroom(&root).args(["scan", "--json"]).output().unwrap().status.success());
    (tmp, root)
}

/// A foreground `serve`, killed when dropped.
struct Serve {
    child: Child,
    port: u16,
    code: String,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn serve(root: &Path) -> (Serve, Value) {
    let mut child = planroom(root).args(["serve", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(env["ok"], true, "{env}");
    let port = env["result"]["port"].as_u64().unwrap() as u16;
    let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
    let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
    (Serve { child, port, code }, env)
}

/// One HTTP/1.1 request; returns the status and the body.
fn http(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        request.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
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

fn origin(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// Exchange the login code for a token.
fn login(s: &Serve) -> String {
    let (status, body) = http(
        s.port,
        "POST",
        "/api/v1/session",
        &[("Origin", &origin(s.port))],
        &json!({"code": s.code}).to_string(),
    );
    assert_eq!(status, 200, "{body}");
    let env: Value = serde_json::from_str(&body).unwrap();
    env["result"]["token"].as_str().unwrap().to_owned()
}

fn api(s: &Serve, token: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let bearer = format!("Bearer {token}");
    http(s.port, method, path, &[("Origin", &origin(s.port)), ("Authorization", &bearer)], body)
}

#[test]
fn requests_without_a_token_or_from_another_origin_or_host_are_refused() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let bearer = format!("Bearer {token}");
    let o = origin(s.port);
    let refused = |headers: &[(&str, &str)]| {
        let (status, body) = http(s.port, "POST", "/api/v1/focus", headers, "{}");
        assert_eq!(status, 401, "{headers:?}: {body}");
        let env: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(env["error"]["code"], "unauthorized");
    };
    refused(&[("Origin", &o)]);
    refused(&[("Origin", &o), ("Authorization", "Bearer not-a-token")]);
    refused(&[("Authorization", &bearer)]);
    refused(&[("Origin", "http://evil.test"), ("Authorization", &bearer)]);
    refused(&[("Origin", &format!("http://127.0.0.1:{}", s.port + 1)), ("Authorization", &bearer)]);
    refused(&[("Host", "evil.test"), ("Origin", &o), ("Authorization", &bearer)]);

    let (status, body) = api(&s, &token, "POST", "/api/v1/focus", "{}");
    assert_eq!(status, 200, "{body}");
    let env: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(env["result"]["focus"]["breadcrumb"][0]["name"], "shop");
    let (status, body) = api(&s, &token, "POST", "/api/v1/map", r#"{"level":"container"}"#);
    assert_eq!(status, 200, "{body}");
    let (status, body) = api(&s, &token, "POST", "/api/v1/handoff/merge", "{}");
    assert_eq!(status, 404, "{body}");
}

#[test]
fn the_ask_and_intent_list_routes_answer_as_the_cli_does() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let cli = |args: &[&str]| -> Value {
        let out = planroom(&root).args(args).arg("--json").output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let map = cli(&["map", "--detail", "graph", "--level", "container"]);
    let edge = &map["result"]["edges"][0];
    let (from, to) = (edge["from"].as_str().unwrap(), edge["to"].as_str().unwrap());

    let cases: [(&str, Value, Vec<&str>); 3] = [
        ("/api/v1/ask", json!({"question": "why", "args": [from, to]}), vec!["ask", "why", from, to]),
        ("/api/v1/ask", json!({}), vec!["ask"]),
        ("/api/v1/intent/list", json!({}), vec!["intent", "list"]),
    ];
    for (path, body, args) in cases {
        let (status, reply) = api(&s, &token, "POST", path, &body.to_string());
        assert_eq!(status, 200, "{path} {body}: {reply}");
        let served: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(served["result"], cli(&args)["result"], "{path} {body}");
    }
}

#[test]
fn a_login_code_works_once_and_a_second_try_revokes_its_token() {
    let (_tmp, root) = workspace();
    let (s, env) = serve(&root);
    let page = PathBuf::from(env["result"]["login_page"].as_str().unwrap());
    let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&page).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o600, "the login page is private");
    // The code is in the page, never in the reply.
    assert!(!env.to_string().contains(&s.code));

    let token = login(&s);
    assert!(!page.exists(), "the login page goes once its code is used");
    assert_eq!(api(&s, &token, "POST", "/api/v1/focus", "{}").0, 200);
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/session",
        &[("Origin", &origin(s.port))],
        &json!({"code": s.code}).to_string(),
    );
    assert_eq!(status, 401, "a code works once");
    assert_eq!(api(&s, &token, "POST", "/api/v1/focus", "{}").0, 401, "a second try revokes the first token");
}

/// Read SSE from the stream until `found` holds for the text so far, or `limit` passes.
fn read_stream_until(
    stream: &mut TcpStream,
    limit: Duration,
    found: impl Fn(&str) -> bool,
) -> Option<Duration> {
    let started = Instant::now();
    let mut seen = String::new();
    let mut buf = [0u8; 4096];
    while started.elapsed() < limit {
        stream.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        match stream.read(&mut buf) {
            Ok(0) => return None,
            Ok(n) => seen.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(_) => {}
        }
        if found(&seen) {
            return Some(started.elapsed());
        }
    }
    None
}

fn open_stream(s: &Serve, token: &str) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let request = format!(
        "GET /api/v1/stream HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nAuthorization: Bearer {token}\r\n\r\n",
        s.port,
        origin(s.port)
    );
    stream.write_all(request.as_bytes()).unwrap();
    let ping = read_stream_until(&mut stream, Duration::from_secs(5), |t| t.contains("event: ping"));
    assert!(ping.is_some(), "the stream opens with a ping");
    stream
}

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

#[test]
fn a_frame_arrives_within_1s_of_a_cli_commit_and_of_serves_own_write() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);

    // Another process commits: the poll on data_version finds it.
    let mut stream = open_stream(&s, &token);
    commit_in(&root.join("orders"));
    let out = planroom(&root).args(["scan", "--json"]).output().unwrap();
    assert!(out.status.success());
    let waited = read_stream_until(&mut stream, Duration::from_secs(3), |t| t.contains("Scanned orders at"));
    let waited = waited.expect("a frame for the CLI's scan");
    assert!(waited < Duration::from_secs(1), "the frame took {waited:?}");

    // serve's own commit: its live loop hears it from the commit hook.
    let mut stream = open_stream(&s, &token);
    commit_in(&root.join("worker"));
    let (status, body) = api(&s, &token, "POST", "/api/v1/scan", "{}");
    assert_eq!(status, 200, "{body}");
    let waited = read_stream_until(&mut stream, Duration::from_secs(3), |t| t.contains("Scanned worker at"));
    let waited = waited.expect("a frame for serve's own scan");
    assert!(waited < Duration::from_secs(1), "the frame took {waited:?}");

    // A stream resumed from a seq replays what came after it.
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let request = format!(
        "GET /api/v1/stream?since=0 HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nAuthorization: Bearer {token}\r\n\r\n",
        s.port,
        origin(s.port)
    );
    stream.write_all(request.as_bytes()).unwrap();
    let all = read_stream_until(&mut stream, Duration::from_secs(3), |t| {
        t.contains("Scanned kit at") && t.contains("Scanned worker at") && t.contains("id: 7")
    });
    assert!(all.is_some(), "since=0 replays every event");
}

#[test]
fn a_layout_is_cached_in_the_layouts_table() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let put = json!({"view_key": "map:system:all", "structure_hash": "abc", "positions": {"orders:repo:orders": {"x": 1.5, "y": -2.0}}});
    let (status, body) = api(&s, &token, "PUT", "/api/v1/layout", &put.to_string());
    assert_eq!(status, 204, "{body}");
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    let cached: Value =
        serde_json::from_str(&store.layout("map:system:all", "abc").unwrap().unwrap()).unwrap();
    assert_eq!(cached, put["positions"]);
    let (status, _) = api(&s, &token, "PUT", "/api/v1/layout", "{\"view_key\": 1}");
    assert_eq!(status, 400);
}

#[test]
fn the_canvas_or_its_fix_page_is_served_at_the_root() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let (status, body) = http(s.port, "GET", "/", &[], "");
    assert_eq!(status, 200);
    assert!(body.contains("<html") || body.contains("<!doctype"), "{body}");
    if !body.contains("id=\"root\"") {
        assert!(
            body.contains("cargo xtask gen --canvas") || body.contains("scripts/planroom build"),
            "{body}"
        );
    }
    assert_eq!(http(s.port, "GET", "/", &[("Host", "evil.test")], "").0, 401);
}

#[test]
fn serve_keeps_its_port_and_another_program_on_it_is_port_in_use() {
    let (_tmp, root) = workspace();
    let port = {
        let (s, _) = serve(&root);
        s.port
    };
    // The next serve takes the recorded port again.
    let (s, _) = serve(&root);
    assert_eq!(s.port, port);
    drop(s);

    let holder = TcpListener::bind(("127.0.0.1", port)).unwrap();
    let out = planroom(&root).args(["serve", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["error"]["code"], "port_in_use");
    assert_eq!(env["error"]["details"]["port"], port);
    // It never moves unasked: the workspace still records its port, and takes it back once free.
    let root_key = root.canonicalize().unwrap().display().to_string();
    assert_eq!(recorded_ports(&root.join(".data")), vec![(root_key.clone(), port)]);
    drop(holder);
    let (s, _) = serve(&root);
    assert_eq!(s.port, port);
    drop(s);

    // --port moves it, and records the move.
    let free = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
    let mut child = planroom(&root)
        .args(["serve", "--json", "--port", &free.to_string()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let _ = child.kill();
    let _ = child.wait();
    let env: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(env["result"]["port"], free);
    let (s, _) = serve(&root);
    assert_eq!(s.port, free);
    assert_eq!(recorded_ports(&root.join(".data")), vec![(root_key, free)]);
}

/// Each workspace `workspaces.json` under the data root `data` records, with its port.
fn recorded_ports(data: &Path) -> Vec<(String, u16)> {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("workspaces.json")).unwrap()).unwrap();
    doc["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| (w["root"].as_str().unwrap().to_owned(), w["port"].as_u64().unwrap() as u16))
        .collect()
}

/// `planroom` in `root` with the data root `data`, which several workspaces share.
fn planroom_in(root: &Path, data: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", data);
    cmd
}

/// A scanned fixture workspace at `root`, with the data root `data`.
fn workspace_at(root: &Path, data: &Path) {
    fixture(root);
    assert!(planroom_in(root, data).args(["scan", "--json"]).output().unwrap().status.success());
}

/// A detached `serve`'s process, stopped when dropped.
struct Detached(u32);

impl Drop for Detached {
    fn drop(&mut self) {
        let _ = Command::new("kill").arg(self.0.to_string()).status();
    }
}

/// `serve --detach --json` in `root`; its envelope.
fn detach(root: &Path, data: &Path) -> Value {
    let out = planroom_in(root, data).args(["serve", "--detach", "--json"]).output().unwrap();
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
}

/// The pid and port of a `serve --detach` that started or reused a server.
fn served(env: &Value) -> (u32, u16) {
    assert_eq!(env["ok"], true, "{env}");
    (env["result"]["pid"].as_u64().unwrap() as u32, env["result"]["port"].as_u64().unwrap() as u16)
}

/// Whether `pid` still runs. A detached server is no child of this test, and an exited one may
/// linger as a zombie until something reaps it, which counts as gone.
fn alive(pid: u32) -> bool {
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        return stat.rsplit_once(')').is_some_and(|(_, rest)| !rest.trim_start().starts_with('Z'));
    }
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success()
}

/// Wait up to 20 s for `pid` to exit.
fn exits(pid: u32) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Rewrite fields of the running server's records, its `serve.json` and its record under the data
/// root `data`, as another engine's server would have written them.
fn edit_serve_json(root: &Path, data: &Path, fields: Value) {
    let mut paths = vec![root.join(".planroom/serve.json")];
    paths.extend(std::fs::read_dir(data.join("serve")).unwrap().map(|e| e.unwrap().path()));
    for path in paths {
        let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for (k, v) in fields.as_object().unwrap() {
            doc[k] = v.clone();
        }
        std::fs::write(&path, doc.to_string()).unwrap();
    }
}

/// The control token of the server on `port`, from its record under the data root `data`.
fn control_token(data: &Path, port: u16) -> String {
    let path = data.join("serve").join(format!("{port}.json"));
    let record: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    record["control_token"].as_str().unwrap().to_owned()
}

/// Whether the login page a `serve --detach` reply names signs a tab in on `port`.
fn its_page_signs_in(env: &Value, port: u16) -> bool {
    let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
    let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap();
    let body = json!({"code": code}).to_string();
    http(port, "POST", "/api/v1/session", &[("Origin", &origin(port))], &body).0 == 200
}

#[test]
fn a_second_serve_uses_the_running_one_with_a_fresh_login_page() {
    let (tmp, root) = workspace();
    let data = tmp.path().join("data");
    let first = detach(&root, &data);
    let (pid, port) = served(&first);
    let _running = Detached(pid);
    let second = detach(&root, &data);
    assert_eq!(served(&second), (pid, port), "the same version and catalogue are reused");
    assert_ne!(first["result"]["login_page"], second["result"]["login_page"]);
    assert!(its_page_signs_in(&second, port), "{second}");
    assert!(its_page_signs_in(&first, port), "{first}");
}

#[test]
fn serve_drains_for_newer_engine() {
    let (tmp, root) = workspace();
    let data = tmp.path().join("data");
    let (old, port) = served(&detach(&root, &data));
    let _old = Detached(old);
    // The running server is an older engine's.
    edit_serve_json(&root, &data, json!({"version": "0.0.1"}));
    let (new, again) = served(&detach(&root, &data));
    let _new = Detached(new);
    assert_ne!(new, old, "an older server is replaced");
    assert_eq!(again, port, "on the workspace's own port");
    assert!(exits(old), "the older server drained and exited");
    let serve_json: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".planroom/serve.json")).unwrap()).unwrap();
    assert_eq!(serve_json["pid"], new);

    // A newer server keeps running, and this older engine says how to catch up.
    edit_serve_json(&root, &data, json!({"version": "999.0.0"}));
    let refused = detach(&root, &data);
    assert_eq!(refused["error"]["code"], "schema_mismatch", "{refused}");
    assert_eq!(refused["error"]["details"]["writer_version"], "999.0.0");
    assert!(refused["error"]["message"].as_str().unwrap().contains("/plugin update planroom@planroom"));
    assert!(alive(new), "the newer server is left running");
    assert_eq!(http(port, "GET", "/", &[], "").0, 200);
}

#[test]
fn serve_drains_for_another_catalogue() {
    let (tmp, root) = workspace();
    let data = tmp.path().join("data");
    let (old, port) = served(&detach(&root, &data));
    let _old = Detached(old);
    // The same version, built with another command catalogue from an older binary.
    edit_serve_json(&root, &data, json!({"catalogue_hash": "0".repeat(64), "binary_mtime_ms": 0}));
    let (new, again) = served(&detach(&root, &data));
    let _new = Detached(new);
    assert_ne!(new, old);
    assert_eq!(again, port);
    assert!(exits(old), "the other build drained and exited");
}

#[test]
fn a_same_version_server_drains_only_for_a_newer_binary() {
    let (tmp, root) = workspace();
    let data = tmp.path().join("data");
    let (old, port) = served(&detach(&root, &data));
    let _old = Detached(old);
    // Another build of this version whose binary is newer than this one: it is used, and the reply
    // says it runs another build, so two sessions on two builds don't drain each other in turn.
    edit_serve_json(
        &root,
        &data,
        json!({"catalogue_hash": "0".repeat(64), "binary_mtime_ms": 4_000_000_000_000_u64}),
    );
    let used = detach(&root, &data);
    assert_eq!(served(&used), (old, port));
    assert!(used["result"]["summary"].as_str().unwrap().contains("another build"), "{used}");
    assert!(its_page_signs_in(&used, port));
    assert!(alive(old));
    // Once this binary is the newer one, it drains that build.
    edit_serve_json(&root, &data, json!({"binary_mtime_ms": 1}));
    let (new, again) = served(&detach(&root, &data));
    let _new = Detached(new);
    assert_ne!(new, old);
    assert_eq!(again, port);
    assert!(exits(old));
}

#[test]
fn drain_needs_the_control_token_and_ends_the_streams_and_the_server() {
    let (_tmp, root) = workspace();
    let (mut s, _) = serve(&root);
    let token = login(&s);
    let mut stream = open_stream(&s, &token);
    let control = &control_token(&root.join(".data"), s.port);
    let drain = |headers: &[(&str, &str)], token: &str| {
        http(s.port, "POST", "/api/v1/drain", headers, &json!({"token": token}).to_string()).0
    };
    assert_eq!(drain(&[], "nope"), 401);
    assert_eq!(drain(&[("Origin", "http://evil.test")], control), 401);
    assert_eq!(drain(&[], &token), 401, "a tab's token is no control token");
    assert_eq!(drain(&[], control), 202);
    // The open stream ends, and the server exits and removes its serve.json.
    let mut rest = Vec::new();
    stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let _ = stream.read_to_end(&mut rest);
    let started = Instant::now();
    while s.child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(s.child.try_wait().unwrap().is_some_and(|status| status.success()), "a drained server exits");
    assert!(!root.join(".planroom/serve.json").exists());
}

#[test]
fn a_65th_stream_client_is_refused_with_serve_limit() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let open: Vec<TcpStream> = (0..64).map(|_| open_stream(&s, &token)).collect();
    let bearer = format!("Bearer {token}");
    let (status, body) =
        http(s.port, "GET", "/api/v1/stream", &[("Origin", &origin(s.port)), ("Authorization", &bearer)], "");
    assert_eq!(status, 503, "{body}");
    let env: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(env["error"]["code"], "serve_limit");
    assert_eq!(env["error"]["details"], json!({"limit": "stream_clients", "max": 64}));
    drop(open);
}

#[test]
fn two_workspaces_never_record_one_port() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    workspace_at(&a, &data);
    workspace_at(&b, &data);
    let (pid_a, port_a) = served(&detach(&a, &data));
    let _a = Detached(pid_a);
    // Asking for a port another workspace records is refused, running or not.
    let out =
        planroom_in(&b, &data).args(["serve", "--json", "--port", &port_a.to_string()]).output().unwrap();
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(env["error"]["code"], "port_in_use", "{env}");
    let (pid_b, port_b) = served(&detach(&b, &data));
    let _b = Detached(pid_b);
    assert_ne!(port_b, port_a);
    // A copy of a workspace brings the original's serve.json and workspace_id along, and still
    // gets a port, and a server, of its own.
    let copy = tmp.path().join("a-copy");
    assert!(Command::new("cp").arg("-a").arg(&a).arg(&copy).status().unwrap().success());
    let (pid_c, port_c) = served(&detach(&copy, &data));
    let _c = Detached(pid_c);
    assert_ne!(pid_c, pid_a, "a copy never reuses the original's server");
    assert!(alive(pid_a), "nor drains it");
    let ports: Vec<u16> = recorded_ports(&data).iter().map(|(_, p)| *p).collect();
    assert_eq!(ports.len(), 3);
    assert!(ports.contains(&port_a) && ports.contains(&port_b) && ports.contains(&port_c));
    let mut distinct = ports.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 3, "{ports:?}");
}

#[test]
fn a_moved_workspace_keeps_its_port() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    let (before, after) = (tmp.path().join("before"), tmp.path().join("after"));
    workspace_at(&before, &data);
    let (pid, port) = served(&detach(&before, &data));
    drop(Detached(pid));
    assert!(exits(pid));
    std::fs::rename(&before, &after).unwrap();
    let (pid, again) = served(&detach(&after, &data));
    let _moved = Detached(pid);
    assert_eq!(again, port, "a tab's pending edits keep their origin");
    let after_key = after.canonicalize().unwrap().display().to_string();
    assert_eq!(recorded_ports(&data), vec![(after_key, port)], "the entry moved with it");
}

#[test]
fn serve_detach_returns_at_once_and_keeps_serving() {
    let (_tmp, root) = workspace();
    let started = Instant::now();
    // output() waits for stdout and stderr to close, so this also proves the child let go of them.
    let out = planroom(&root).args(["serve", "--detach", "--json"]).output().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5), "detach took {:?}", started.elapsed());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let env: Value = serde_json::from_slice(&out.stdout).unwrap();
    let pid = env["result"]["pid"].as_u64().unwrap().to_string();
    let port = env["result"]["port"].as_u64().unwrap() as u16;
    let serve_json: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".planroom/serve.json")).unwrap()).unwrap();
    assert_eq!(serve_json["pid"].to_string(), pid);
    assert_eq!(serve_json["port"], port);
    assert!(serve_json["catalogue_hash"].as_str().is_some_and(|h| h.len() == 64));
    let (status, _) = http(port, "GET", "/", &[], "");
    assert_eq!(status, 200);
    assert!(Command::new("kill").arg(&pid).status().unwrap().success());
    let gone = Instant::now();
    while root.join(".planroom/serve.json").exists() && gone.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!root.join(".planroom/serve.json").exists(), "serve removes serve.json when it stops");
}

/// The status of a `GET /api/v1/stream` sent with `headers`, read from its status line.
fn stream_status(s: &Serve, headers: &[(&str, &str)]) -> u16 {
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let mut request = format!("GET /api/v1/stream HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n", s.port);
    for (k, v) in headers {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    line.split(' ').nth(1).unwrap_or_else(|| panic!("no status line: {line:?}")).parse().unwrap()
}

#[test]
fn a_same_origin_get_without_origin_reaches_the_stream_and_a_cross_origin_one_does_not() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let bearer = format!("Bearer {token}");
    // What Chromium sends for the page's own fetch of the stream: no Origin, Sec-Fetch-Site.
    assert_eq!(stream_status(&s, &[("Authorization", &bearer), ("Sec-Fetch-Site", "same-origin")]), 200);
    // A page on another site, or a request with neither header, is refused.
    for site in ["cross-site", "same-site", "none"] {
        assert_eq!(stream_status(&s, &[("Authorization", &bearer), ("Sec-Fetch-Site", site)]), 401, "{site}");
    }
    assert_eq!(stream_status(&s, &[("Authorization", &bearer)]), 401);
    let foreign = [
        ("Authorization", bearer.as_str()),
        ("Origin", "http://evil.test"),
        ("Sec-Fetch-Site", "same-origin"),
    ];
    assert_eq!(stream_status(&s, &foreign), 401);
    // Only a GET may leave Origin out: a POST needs the exact one.
    let (status, _) = http(
        s.port,
        "POST",
        "/api/v1/focus",
        &[("Authorization", &bearer), ("Sec-Fetch-Site", "same-origin")],
        "{}",
    );
    assert_eq!(status, 401);
}

#[test]
fn every_response_carries_exactly_the_csp_the_canvas_needs() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let mut stream = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    let request = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n", s.port);
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let csp = response
        .lines()
        .find_map(|l| l.strip_prefix("content-security-policy: "))
        .unwrap_or_else(|| panic!("no CSP: {response}"));
    assert_eq!(
        csp,
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; \
         font-src 'self'; connect-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; \
         frame-ancestors 'none'"
    );
}

#[test]
fn the_control_token_registers_a_login_code_by_its_digest() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let control = format!("Bearer {}", control_token(&root.join(".data"), s.port));
    let code = "0123456789abcdef0123456789abcdef";
    let body = json!({"code_sha256": planroom::server::auth::code_digest(code)}).to_string();
    let register =
        |headers: &[(&str, &str)], body: &str| http(s.port, "POST", "/api/v1/code", headers, body).0;
    assert_eq!(register(&[("Authorization", "Bearer nope")], &body), 401);
    assert_eq!(register(&[("Authorization", &control), ("Origin", "http://evil.test")], &body), 401);
    assert_eq!(register(&[("Authorization", &control)], r#"{"code_sha256": "short"}"#), 400);
    assert_eq!(register(&[("Authorization", &control)], &body), 204);
    let exchange = || {
        let body = json!({"code": code}).to_string();
        http(s.port, "POST", "/api/v1/session", &[("Origin", &origin(s.port))], &body)
    };
    let (status, reply) = exchange();
    assert_eq!(status, 200, "{reply}");
    assert_eq!(exchange().0, 401, "a registered code works once too");
}

#[test]
fn scan_over_http_takes_the_served_root_and_no_other_dir() {
    let (_tmp, root) = workspace();
    let (s, _) = serve(&root);
    let token = login(&s);
    let at = |dir: &Path| json!({"dir": dir.display().to_string()}).to_string();
    let (status, body) = api(&s, &token, "POST", "/api/v1/scan", &at(&root));
    assert_eq!(status, 200, "{body}");
    let (status, body) = api(&s, &token, "POST", "/api/v1/scan", &at(&root.join("orders")));
    assert_eq!(status, 400, "{body}");
}

/// Every file under `dir`, recursively.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn mode(path: &Path) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions()) & 0o777
}

/// The workspace's `serve.json`, parsed.
fn read_serve_json(root: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root.join(".planroom/serve.json")).unwrap()).unwrap()
}

/// Wait up to 5 s for `done`.
fn eventually(done: impl Fn() -> bool) -> bool {
    let started = Instant::now();
    while !done() && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

#[test]
fn the_control_token_lives_under_the_data_root_and_no_tree_read_of_the_workspace_reaches_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, data) = (tmp.path().join("shop"), tmp.path().join("data"));
    workspace_at(&root, &data);
    let (pid, port) = served(&detach(&root, &data));
    let _running = Detached(pid);

    // serve.json keeps the rest, and no control token.
    let serve_json = read_serve_json(&root);
    assert_eq!(serve_json["port"], port);
    assert_eq!(serve_json["pid"], pid);
    assert!(serve_json.get("control_token").is_none(), "{serve_json}");
    // The token is in the server's record under the data root, 0600 in a 0700 folder.
    let record = data.join("serve").join(format!("{port}.json"));
    assert_eq!(mode(&record), 0o600);
    assert_eq!(mode(&data.join("serve")), 0o700);
    let control = control_token(&data, port);
    assert_eq!(control.len(), 64);
    // Nothing a tree read of the workspace, such as `grep -r x .`, reads holds the token.
    for file in files_under(&root) {
        let text = String::from_utf8_lossy(&std::fs::read(&file).unwrap()).into_owned();
        assert!(!text.contains(&control), "{} holds the control token", file.display());
    }

    // The token from the data root registers a login code, as `open` registers one.
    let code = "fedcba9876543210fedcba9876543210";
    let body = json!({"code_sha256": planroom::server::auth::code_digest(code)}).to_string();
    let bearer = format!("Bearer {control}");
    assert_eq!(http(port, "POST", "/api/v1/code", &[("Authorization", &bearer)], &body).0, 204);
    let exchange = json!({"code": code}).to_string();
    assert_eq!(http(port, "POST", "/api/v1/session", &[("Origin", &origin(port))], &exchange).0, 200);
    // A later serve reuses this one through the token, and drains it when it moves the port.
    assert_eq!(served(&detach(&root, &data)), (pid, port));
    let free = TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
    let out = planroom_in(&root, &data)
        .args(["serve", "--detach", "--json", "--port", &free.to_string()])
        .output()
        .unwrap();
    let moved: Value = serde_json::from_slice(&out.stdout).unwrap();
    let (new, again) = served(&moved);
    let _moved = Detached(new);
    assert_eq!(again, free);
    assert!(exits(pid), "the drain reached the first server through its token");
    // Each server's record goes with it; the new one's is there, and serve.json names it.
    assert!(eventually(|| !record.exists()), "the drained server removes its record");
    assert!(data.join("serve").join(format!("{free}.json")).exists());
    assert_eq!(read_serve_json(&root)["pid"], new);
}

#[test]
fn a_serve_of_an_older_engine_that_kept_its_token_in_serve_json_is_still_drained() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, data) = (tmp.path().join("shop"), tmp.path().join("data"));
    workspace_at(&root, &data);
    let (old, port) = served(&detach(&root, &data));
    let _old = Detached(old);
    // An older engine kept the control token in serve.json and wrote no record under the data root.
    let token = control_token(&data, port);
    std::fs::remove_file(data.join("serve").join(format!("{port}.json"))).unwrap();
    edit_serve_json(&root, &data, json!({"version": "0.0.1", "control_token": token}));
    let (new, again) = served(&detach(&root, &data));
    let _new = Detached(new);
    assert_ne!(new, old);
    assert_eq!(again, port);
    assert!(exits(old), "the older server drained and exited");
    assert!(read_serve_json(&root).get("control_token").is_none());
}

#[test]
fn serve_starts_before_the_first_scan_and_the_empty_map_comes_from_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, data) = (tmp.path().join("shop"), tmp.path().join("data"));
    // A directory no scan has touched: the fixture writes no state directory.
    fixture(&root);
    let first = detach(&root, &data);
    let (pid, port) = served(&first);
    let running = Detached(pid);
    // Serving makes no workspace: a scan still does that.
    assert!(!root.join(".planroom").exists(), "serve made a state directory");
    let root_key = root.canonicalize().unwrap().display().to_string();
    assert_eq!(recorded_ports(&data), vec![(root_key.clone(), port)]);
    // A second serve reuses it, as in a scanned workspace.
    assert_eq!(served(&detach(&root, &data)), (pid, port));

    let page = std::fs::read_to_string(first["result"]["login_page"].as_str().unwrap()).unwrap();
    let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
    let s = Serve { child: Command::new("true").spawn().unwrap(), port, code };
    let token = login(&s);
    let (status, body) = api(&s, &token, "POST", "/api/v1/map", r#"{"detail":"graph"}"#);
    assert_eq!(status, 200, "{body}");
    let map: Value = serde_json::from_str(&body).unwrap();
    let next = &map["result"]["empty"]["next"];
    assert_eq!(map["result"]["empty"]["reason"], "not_scanned", "{map}");
    assert_eq!(next["command"], "scan", "{map}");
    assert_eq!(next["args"], json!([root_key]));
    // A layout before the first scan has nowhere to go, and is no error: it's a cache.
    let put = json!({"view_key": "map:system:all", "structure_hash": "abc", "positions": {}});
    assert_eq!(api(&s, &token, "PUT", "/api/v1/layout", &put.to_string()).0, 204);
    assert!(!root.join(".planroom").exists());

    // The stream opens on the empty map, and the scan its next action runs reaches it.
    let mut stream = open_stream(&s, &token);
    let (status, body) = api(&s, &token, "POST", "/api/v1/scan", &json!({"dir": root_key}).to_string());
    assert_eq!(status, 200, "{body}");
    let frames = read_stream_until(&mut stream, Duration::from_secs(5), |t| {
        t.contains(r#""target":"orders:repo:orders""#) && t.contains(r#""target":"worker:repo:worker""#)
    });
    assert!(frames.is_some(), "the first scan's frames reach a stream opened before it");
    let (status, body) = api(&s, &token, "POST", "/api/v1/map", r#"{"detail":"graph"}"#);
    assert_eq!(status, 200, "{body}");
    let map: Value = serde_json::from_str(&body).unwrap();
    assert!(map["result"]["empty"].is_null(), "{map}");
    // Once the scan made the state directory, serve.json joins the record there, with no token.
    assert!(eventually(|| root.join(".planroom/serve.json").exists()));
    let serve_json = read_serve_json(&root);
    assert_eq!(serve_json["pid"], pid);
    assert!(serve_json.get("control_token").is_none());
    // The layout cache takes the new store.
    assert_eq!(api(&s, &token, "PUT", "/api/v1/layout", &put.to_string()).0, 204);
    let store = Store::open_existing(&root.join(".planroom/store.db")).unwrap().unwrap();
    assert!(store.layout("map:system:all", "abc").unwrap().is_some());
    // The next serve after the scan records the workspace_id with the port it keeps.
    drop(running);
    assert!(exits(pid));
    let (pid, again) = served(&detach(&root, &data));
    let _again = Detached(pid);
    assert_eq!(again, port);
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("workspaces.json")).unwrap()).unwrap();
    let id = store.meta("workspace_id").unwrap().unwrap();
    assert_eq!(doc["workspaces"][0]["workspace_id"], id.as_str(), "{doc}");
}

#[test]
fn serve_removes_its_records_when_a_newer_engine_migrates_the_store() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, data) = (tmp.path().join("shop"), tmp.path().join("data"));
    workspace_at(&root, &data);
    let mut child =
        planroom_in(&root, &data).args(["serve", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap();
    let port = env["result"]["port"].as_u64().unwrap();
    let record = data.join("serve").join(format!("{port}.json"));
    assert!(record.exists() && root.join(".planroom/serve.json").exists());
    // A newer engine's migration, as an older reader sees it: `user_version` past this build's.
    let script = format!(
        "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); \
         c.execute(\"UPDATE meta SET value='9.9.9' WHERE key='engine_version'\"); \
         c.execute('PRAGMA user_version = {}'); c.commit()",
        planroom_store::STORE_VERSION + 1
    );
    let store = root.join(".planroom/store.db");
    let out = Command::new("python3").args(["-c", &script]).arg(store).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = child.try_wait().unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(status.and_then(|s| s.code()), Some(7), "serve exits with schema_mismatch");
    assert!(!root.join(".planroom/serve.json").exists(), "serve.json goes with it");
    assert!(!record.exists(), "and so does its record under the data root");
}

#[test]
fn a_login_page_whose_code_expires_unused_is_removed() {
    let (_tmp, root) = workspace();
    let (_s, env) = serve(&root);
    let page = PathBuf::from(env["result"]["login_page"].as_str().unwrap());
    assert!(page.exists());
    // Its code's 60 s pass with nobody signing in: the page is as old as an expired code.
    let past = std::time::SystemTime::now() - Duration::from_secs(120);
    std::fs::File::options().write(true).open(&page).unwrap().set_modified(past).unwrap();
    let started = Instant::now();
    while page.exists() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!page.exists(), "the sweep removes a page whose code expired unused");
}

// --- `open`: the person's way in (DR-29, DR-64, DR-75). ---

/// A test person key from a fixed scalar, registered for the workspace at `root` with a token
/// under itself; returns it.
fn person_key(root: &Path, seed: u8) -> p256::ecdsa::SigningKey {
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    bytes[31] = seed;
    let key = p256::ecdsa::SigningKey::from_slice(&bytes).unwrap();
    let public = b64url(&key.verifying_key().to_sec1_point(false).to_bytes());
    let mut child = planroom(root)
        .args(["person", "init", "--json"])
        .env("PLANROOM_PERSON_TOKEN", press(&key, "person init", 1))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(public.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    key
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

/// A press's token over `command` with no arguments, signed `ago` seconds back.
fn press(key: &p256::ecdsa::SigningKey, command: &str, ago: u64) -> String {
    use p256::ecdsa::signature::Signer;
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - ago;
    let sig: p256::ecdsa::Signature =
        key.sign(&planroom_kernel::keys::person_token_message(command, &[], "", "", at));
    format!("{at}.{}", b64url(&sig.to_bytes()))
}

/// A stand-in for the browser opener: it writes its argv, one word per line, and its environment
/// into `dir`, keeps a copy of the page it was given, and opens nothing.
fn stub_opener(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("opener");
    let script = format!(
        "#!/bin/sh\nfor a in \"$0\" \"$@\"; do echo \"$a\"; done > '{d}/argv'\nenv > '{d}/env'\ncat \"$1\" > '{d}/page'\n",
        d = dir.display()
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    path
}

/// `planroom open --json` as a press, with `opener` standing in for the browser's, a display to
/// open on and the caller's `PATH` set to `path`; its envelope, and the server it started in the
/// background, stopped when the guard drops.
fn open_with(root: &Path, key: &p256::ecdsa::SigningKey, opener: &Path, path: &str) -> (Value, Detached) {
    let out = planroom(root)
        .args(["open", "--json"])
        .env("PLANROOM_PERSON_TOKEN", press(key, "open", 0))
        .env("PLANROOM_TEST_OPENER", opener)
        .env("DISPLAY", ":0")
        .env("PATH", path)
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_TTY")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let env: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    let record = std::fs::read_dir(root.join(".data/serve")).unwrap().next().unwrap().unwrap().path();
    let record: Value = serde_json::from_str(&std::fs::read_to_string(record).unwrap()).unwrap();
    (env, Detached(record["pid"].as_u64().unwrap() as u32))
}

#[test]
fn the_opener_gets_a_fixed_path() {
    let (tmp, root) = workspace();
    let key = person_key(&root, 31);
    // The caller's PATH starts with a folder that holds an opener of each name of its own.
    let shims = tmp.path().join("shims");
    std::fs::create_dir_all(&shims).unwrap();
    for name in ["xdg-open", "open", "wslview"] {
        std::fs::write(shims.join(name), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(shims.join(name), std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
    }
    let caller_path = format!("{}:{}", shims.display(), std::env::var("PATH").unwrap());
    let seen = tmp.path().join("opener");
    let (env, _served) = open_with(&root, &key, &stub_opener(&seen), &caller_path);
    assert_eq!(env["result"]["opened"], true, "{env}");
    let opener_env = std::fs::read_to_string(seen.join("env")).unwrap();
    // The fixed system PATH, never the caller's, and nothing beyond the allow-list: no token.
    assert!(opener_env.lines().any(|l| l == "PATH=/usr/bin:/bin"), "{opener_env}");
    assert!(!opener_env.contains(&shims.display().to_string()), "{opener_env}");
    // What the shell running the stand-in adds itself is its own, not the opener's environment.
    let allowed = [
        "PATH",
        "HOME",
        "LC_ALL",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "PWD",
        "OLDPWD",
        "SHLVL",
        "_",
    ];
    for var in opener_env.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k)) {
        assert!(allowed.contains(&var), "the opener got {var}: {opener_env}");
    }
    assert!(!opener_env.contains("PLANROOM_"), "{opener_env}");
}

#[test]
fn the_code_is_never_in_an_argv() {
    let (tmp, root) = workspace();
    let key = person_key(&root, 32);
    let seen = tmp.path().join("opener");
    let (env, served) = open_with(&root, &key, &stub_opener(&seen), &std::env::var("PATH").unwrap());
    assert_eq!(env["result"]["opened"], true, "{env}");
    // The opener's argv is the program and the login page's path under DATA_ROOT/open/.
    let argv: Vec<String> =
        std::fs::read_to_string(seen.join("argv")).unwrap().lines().map(str::to_owned).collect();
    assert_eq!(argv.len(), 2, "{argv:?}");
    let open_dir = root.join(".data/open").canonicalize().unwrap();
    let page_dir = Path::new(&argv[1]).parent().map(|p| p.canonicalize().unwrap());
    assert_eq!(page_dir, Some(open_dir), "{argv:?}");
    // The code is in the page, and in no argv: not the opener's, not `open`'s, not the server's.
    let html = std::fs::read_to_string(seen.join("page")).unwrap();
    let code = html.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
    assert_eq!(code.len(), 32);
    assert!(argv.iter().all(|a| !a.contains(&code)), "{argv:?}");
    if let Ok(cmdline) = std::fs::read(format!("/proc/{}/cmdline", served.0)) {
        assert!(!String::from_utf8_lossy(&cmdline).contains(&code));
    }
    assert!(!env.to_string().contains(&code), "{env}");
    // And it signs a tab in, once.
    let record = std::fs::read_dir(root.join(".data/serve")).unwrap().next().unwrap().unwrap().path();
    let port: u16 = record.file_stem().unwrap().to_str().unwrap().parse().unwrap();
    let body = json!({"code": code}).to_string();
    assert_eq!(http(port, "POST", "/api/v1/session", &[("Origin", &origin(port))], &body).0, 200);
    assert_eq!(http(port, "POST", "/api/v1/session", &[("Origin", &origin(port))], &body).0, 401);
}

#[test]
fn an_expired_or_reused_code_is_refused() {
    use std::sync::{Arc, Mutex};

    use planroom::server::auth::{Auth, CODE_LIFE, Refused, code_digest};

    // The code's clock, which the test moves: no real wait.
    let start = Instant::now();
    let offset = Arc::new(Mutex::new(Duration::ZERO));
    let clock = {
        let offset = offset.clone();
        Arc::new(move || start + *offset.lock().unwrap())
    };
    let mut auth = Auth::with_clock(41_873, clock);
    let advance = |by: Duration| *offset.lock().unwrap() += by;

    // A code 61 s old is refused, and stays refused.
    auth.add_code(code_digest("old-code"));
    advance(CODE_LIFE + Duration::from_secs(1));
    assert_eq!(auth.exchange("old-code"), Err(Refused::Expired));
    assert_eq!(auth.exchange("old-code"), Err(Refused::Expired));

    // A fresh one works at 59 s, once. A reuse is refused and revokes the token the first try
    // issued, and the code never works again, though its 60 s haven't passed.
    auth.add_code(code_digest("new-code"));
    advance(Duration::from_secs(59));
    let token = auth.exchange("new-code").unwrap();
    assert!(auth.bearer_ok(Some(&format!("Bearer {token}"))));
    assert_eq!(auth.exchange("new-code"), Err(Refused::UsedTwice));
    assert!(!auth.bearer_ok(Some(&format!("Bearer {token}"))), "the reuse revoked the first token");
    assert_eq!(auth.exchange("new-code"), Err(Refused::Expired));
}
