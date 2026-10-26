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
    drop(holder);

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
    let serve_json: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".planroom/serve.json")).unwrap()).unwrap();
    let control = format!("Bearer {}", serve_json["control_token"].as_str().unwrap());
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
