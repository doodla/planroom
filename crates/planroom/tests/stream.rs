//! The live stream over `serve` (architecture.md, Local server and live stream; DR-1, DR-61):
//! two SSE clients on one `serve` with no other writer, each writing through `POST /api/v1/scan`;
//! each sees the other's write as a frame naming the repo's node ID. A tab's `POST /api/v1/edit`
//! shows in the very next read on that server. HTTP is spoken by hand over `TcpStream`, as in
//! `server_auth.rs`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use planroom_testkit::fixture;
use serde_json::{Value, json};

fn planroom(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_planroom"));
    cmd.current_dir(root).env("PLANROOM_DATA", root.join(".data"));
    cmd
}

/// A foreground `serve` on a scanned fixture workspace, killed when dropped.
struct Serve {
    child: Child,
    port: u16,
    root: PathBuf,
    /// The code in the login page `serve` wrote.
    code: String,
    _tmp: tempfile::TempDir,
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn serve() -> Serve {
    let tmp = tempfile::tempdir().unwrap();
    let root = scanned(&tmp);
    serve_on(tmp, root)
}

/// The fixture workspace, scanned, in `tmp`.
fn scanned(tmp: &tempfile::TempDir) -> PathBuf {
    let root = tmp.path().join("shop");
    fixture(&root);
    assert!(planroom(&root).args(["scan", "--json"]).output().unwrap().status.success());
    root
}

/// A foreground `serve` on the scanned workspace at `root`, inside `tmp`.
fn serve_on(tmp: tempfile::TempDir, root: PathBuf) -> Serve {
    let mut child = planroom(&root).args(["serve", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let env: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(env["ok"], true, "{env}");
    let port = env["result"]["port"].as_u64().unwrap() as u16;
    let page = std::fs::read_to_string(env["result"]["login_page"].as_str().unwrap()).unwrap();
    let code = page.split("#code=").nth(1).unwrap().split('"').next().unwrap().to_owned();
    Serve { child, port, root, code, _tmp: tmp }
}

fn origin(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// One HTTP/1.1 request from the page's origin; returns the status and the body.
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

/// A signed-in tab: the bearer token its login code bought.
fn sign_in(s: &Serve, code: &str) -> String {
    let body = json!({"code": code}).to_string();
    let (status, reply) = http(s.port, "POST", "/api/v1/session", &[("Origin", &origin(s.port))], &body);
    assert_eq!(status, 200, "{reply}");
    let env: Value = serde_json::from_str(&reply).unwrap();
    env["result"]["token"].as_str().unwrap().to_owned()
}

/// A second tab's code, registered with the control token in the server's record under the data
/// root, as `planroom open` does.
fn second_code(s: &Serve) -> String {
    let record = s.root.join(".data/serve").join(format!("{}.json", s.port));
    let record: Value = serde_json::from_str(&std::fs::read_to_string(record).unwrap()).unwrap();
    let control = format!("Bearer {}", record["control_token"].as_str().unwrap());
    let code = "00112233445566778899aabbccddeeff";
    let body = json!({"code_sha256": planroom::server::auth::code_digest(code)}).to_string();
    assert_eq!(http(s.port, "POST", "/api/v1/code", &[("Authorization", &control)], &body).0, 204);
    code.to_owned()
}

/// One tab's live stream, read as it arrives.
struct Stream {
    socket: TcpStream,
    seen: String,
}

impl Stream {
    fn open(s: &Serve, token: &str) -> Stream {
        let mut socket = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
        let request = format!(
            "GET /api/v1/stream HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nAuthorization: Bearer {token}\r\n\r\n",
            s.port,
            origin(s.port)
        );
        socket.write_all(request.as_bytes()).unwrap();
        let mut stream = Stream { socket, seen: String::new() };
        assert!(stream.wait_for("event: ping"), "the stream opens with a ping");
        stream
    }

    /// Read until the text so far contains `needle`, for up to 10 s.
    fn wait_for(&mut self, needle: &str) -> bool {
        self.wait_until(|seen| seen.contains(needle))
    }

    /// Read until `done` holds for the text so far, for up to 10 s.
    fn wait_until(&mut self, done: impl Fn(&str) -> bool) -> bool {
        let started = Instant::now();
        let mut buf = [0u8; 4096];
        while started.elapsed() < Duration::from_secs(10) {
            if done(&self.seen) {
                return true;
            }
            self.socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
            match self.socket.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => self.seen.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(_) => {}
            }
        }
        done(&self.seen)
    }
}

/// Move a repo's head, so the next scan of it records a new scan event.
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

/// A tab's write through `POST /api/v1/scan`.
fn scan(s: &Serve, token: &str) {
    let bearer = format!("Bearer {token}");
    let headers = [("Origin", origin(s.port)), ("Authorization", bearer)];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, body) = http(s.port, "POST", "/api/v1/scan", &headers, "{}");
    assert_eq!(status, 200, "{body}");
}

#[test]
fn two_sse_clients_on_one_serve_see_each_others_writes() {
    let s = serve();
    let first = sign_in(&s, &s.code.clone());
    let second = sign_in(&s, &second_code(&s));
    let mut a = Stream::open(&s, &first);
    let mut b = Stream::open(&s, &second);

    // The first tab writes: both streams carry its frame, which names the repo's node ID.
    commit_in(&s.root.join("orders"));
    scan(&s, &first);
    for (name, stream) in [("the writer", &mut a), ("the other tab", &mut b)] {
        assert!(stream.wait_for("Scanned orders at"), "{name} sees the first tab's write: {}", stream.seen);
        assert!(stream.seen.contains(r#""target":"orders:repo:orders""#), "{}", stream.seen);
    }

    // The second tab writes: the first tab sees it too.
    commit_in(&s.root.join("worker"));
    scan(&s, &second);
    for (name, stream) in [("the other tab", &mut a), ("the writer", &mut b)] {
        assert!(stream.wait_for("Scanned worker at"), "{name} sees the second tab's write: {}", stream.seen);
        assert!(stream.seen.contains(r#""target":"worker:repo:worker""#), "{}", stream.seen);
    }

    // Both saw the same frames, in the same order, each once.
    let ids = |seen: &str| -> Vec<String> {
        seen.lines().filter_map(|l| l.strip_prefix("id: ")).map(str::to_owned).collect()
    };
    assert_eq!(ids(&a.seen), ids(&b.seen));
    let mut unique = ids(&a.seen);
    unique.dedup();
    assert_eq!(unique, ids(&a.seen), "no frame twice");
}

/// A tab's `POST` of `body` to the command at `path`, as JSON; the status and the envelope.
fn post(s: &Serve, token: &str, path: &str, body: &Value) -> (u16, Value) {
    let bearer = format!("Bearer {token}");
    let headers = [("Origin", origin(s.port)), ("Authorization", bearer)];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, body) = http(s.port, "POST", path, &headers, &body.to_string());
    (status, serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}")))
}

#[test]
fn a_serve_write_then_read_shows_the_write() {
    let s = serve();
    let tab = sign_in(&s, &s.code.clone());
    let mut stream = Stream::open(&s, &tab);
    let (status, map) = post(&s, &tab, "/api/v1/map", &json!({"level": "container"}));
    assert_eq!(status, 200, "{map}");
    let base = map["meta"]["seq"].as_u64().unwrap();

    let rename = json!({
        "op": "rename", "id": "orders:container:orders", "name": "order-service", "base": base, "client": "tab_1",
    });
    let (status, written) = post(&s, &tab, "/api/v1/edit", &rename);
    assert_eq!(status, 200, "{written}");
    let seq = written["meta"]["seq"].as_u64().unwrap();
    assert!(seq > base, "{written}");

    // The very next read, on the same server, folds the write in and says so in meta.seq.
    let (status, map) = post(&s, &tab, "/api/v1/map", &json!({"level": "container"}));
    assert_eq!(status, 200, "{map}");
    assert!(map["meta"]["seq"].as_u64().unwrap() >= seq, "{map}");
    let name = map["result"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "orders:container:orders")
        .map(|n| n["name"].clone());
    assert_eq!(name, Some(json!("order-service")), "{map}");
    // And the stream carries the write's frame, naming its target.
    assert!(stream.wait_for(&format!("id: {seq}")), "{}", stream.seen);
    assert!(stream.seen.contains(r#""target":"orders:container:orders""#), "{}", stream.seen);
}

/// A `watch --json --session SESSION` as `USER=ann`, from `client` (`PLANROOM_CLIENT`, or none for
/// the CLI's own `cli:<pid>`), with a channel of its stdout lines; killed when dropped.
struct Watch {
    child: Child,
    lines: mpsc::Receiver<String>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn watch_as(root: &Path, session: &str, client: Option<&str>) -> Watch {
    let mut cmd = planroom(root);
    cmd.args(["watch", "--json", "--session", session])
        .env("USER", "ann")
        .env("LOGNAME", "ann")
        .env_remove("PLANROOM_ACTOR")
        .stdout(Stdio::piped());
    match client {
        Some(id) => cmd.env("PLANROOM_CLIENT", id),
        None => cmd.env_remove("PLANROOM_CLIENT"),
    };
    let mut child = cmd.spawn().unwrap();
    let out = child.stdout.take().unwrap();
    let (tx, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    Watch { child, lines }
}

impl Watch {
    /// The rows of the first `presence` frame that `ok` accepts, within a minute.
    fn presence_until(&self, ok: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = self.lines.recv_timeout(left).expect("watch sends the presence frame looked for");
            let frame: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
            if frame["type"] != "presence" {
                continue;
            }
            let actors = frame["actors"].as_array().unwrap().clone();
            if ok(&actors) {
                return actors;
            }
        }
    }
}

fn client_ids(rows: &[Value]) -> Vec<String> {
    let mut ids: Vec<String> = rows.iter().map(|r| r["client_id"].as_str().unwrap().to_owned()).collect();
    ids.sort();
    ids
}

#[test]
fn presence_keeps_a_row_per_surface() {
    let tmp = tempfile::tempdir().unwrap();
    let root = scanned(&tmp);

    // One person in two places: the mod's watch and a terminal's.
    let first = watch_as(&root, "s1", Some("mod:s1"));
    let second = watch_as(&root, "s1", None);
    let both = first.presence_until(|rows| rows.len() == 2);
    assert_eq!(both[0]["actor"], both[1]["actor"], "{both:?}");
    assert_eq!(both[0]["actor"]["kind"], "person");
    assert_eq!(both[0]["actor"]["id"], "ann");
    let mut surfaces: Vec<&str> = both.iter().map(|r| r["surface"].as_str().unwrap()).collect();
    surfaces.sort();
    assert_eq!(surfaces, ["cli", "mod"]);
    let ids = client_ids(&both);
    assert_ne!(ids[0], ids[1]);
    assert!(ids.contains(&"mod:s1".to_owned()), "{ids:?}");

    // The mod's own focus leaves out its own row; the CLI's keeps every row.
    let who = |args: &[&str]| -> Vec<Value> {
        let out = planroom(&root).args(args).output().unwrap();
        let env: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(env["ok"], true, "{env}");
        env["result"]["who"].as_array().unwrap().clone()
    };
    let own = who(&["focus", "--session", "s1", "--json"]);
    assert_eq!(own.len(), 1, "{own:?}");
    assert_eq!(own[0]["surface"], "cli");
    assert_eq!(client_ids(&who(&["focus", "--json"])), ids);

    // A second session adds its row and drops none.
    let third = watch_as(&root, "s2", Some("mod:s2"));
    let all = first.presence_until(|rows| rows.len() == 3);
    let mut want = ids.clone();
    want.push("mod:s2".into());
    want.sort();
    assert_eq!(client_ids(&all), want);

    // A serve started after them sends the present set right after its ping.
    let s = serve_on(tmp, root);
    let token = sign_in(&s, &s.code.clone());
    let mut stream = Stream::open(&s, &token);
    // The first presence event, once its data line is whole.
    let presence = |seen: &str| -> Option<String> {
        let rest = &seen[seen.find("event: presence\n")?..];
        let whole = &rest[..rest.find("\n\n")?];
        whole.lines().find_map(|l| l.strip_prefix("data: ")).map(str::to_owned)
    };
    assert!(stream.wait_until(|seen| presence(seen).is_some()), "{}", stream.seen);
    let kinds: Vec<&str> = stream.seen.lines().filter_map(|l| l.strip_prefix("event: ")).collect();
    assert_eq!(kinds[..2], ["ping", "presence"], "{}", stream.seen);
    let frame: Value = serde_json::from_str(&presence(&stream.seen).unwrap()).unwrap();
    assert_eq!(client_ids(frame["actors"].as_array().unwrap()), want);

    drop(stream);
    drop((first, second, third));
    drop(s);
}
