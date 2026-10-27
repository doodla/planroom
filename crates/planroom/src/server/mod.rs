//! `planroom serve [--detach] [--port N]`: the canvas and the HTTP API on `127.0.0.1`, at the
//! workspace's recorded port (architecture.md, Local server and live stream; DR-29, DR-58, DR-78).
//!
//! Before it serves, it settles the version handshake with any `serve` already running for the
//! workspace (`handshake`), binds the port, mints the one-time login code, writes the login page
//! and its record with the control token under the data root (mode 0600), and `serve.json`
//! without the token in the state directory (mode 0600), and prints one envelope. It starts in a
//! directory no scan has made a workspace yet, and serves the empty map until a scan makes one.
//! `--detach` forks after binding, so the port already accepts connections, and the child takes a
//! session of its own with `setsid`; the parent prints and returns at once.

pub mod auth;
pub mod canvas;
mod routes;

use std::cmp::Ordering;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use planroom_contract::format::{self, Document, Extra};
use planroom_contract::{CodeReq, DrainReq, ErrorCode, ServeInfo, ServeReq, Timestamp};
use planroom_store::Store;
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};

use crate::app::error::AppError;
use crate::app::{Reply, data, version, workspace};
use crate::{cli, clock, text};

/// A login page older than its code is deleted: the code in it no longer works.
const PAGE_LIFE: Duration = auth::CODE_LIFE;
/// How often the server sweeps the data root's `open/` for login pages past their life.
const PAGE_SWEEP: Duration = Duration::from_secs(5);
/// A log file past this size is rotated once at start.
const LOG_CAP: u64 = 1 << 20;
/// How long a request to a running `serve`, to register a code or to drain it, may take.
const PEER_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a drained `serve` has to let go of the port: a request in flight runs at most 10 s.
const DRAIN_WAIT: Duration = Duration::from_secs(15);

/// Everything `serve` settles before it forks or serves.
struct Prepared {
    root: PathBuf,
    store_path: PathBuf,
    workspace_id: String,
    listener: TcpListener,
    port: u16,
    auth: auth::Auth,
    control_token: String,
    page: PathBuf,
    /// The sha256 of the code in `page`, whose exchange deletes the page.
    page_code: String,
    data_root: PathBuf,
}

/// What `prepare` settled: a server of this process's own to start, or one already running that
/// this `serve` uses.
enum Start {
    New(Box<Prepared>),
    Reused(Reply<ServeInfo>),
}

pub fn run(req: &ServeReq, cwd: &Path, json: bool, started: Instant) -> ExitCode {
    let prepared = match prepare(req, cwd) {
        Ok(Start::New(p)) => *p,
        Ok(Start::Reused(reply)) => return cli::finish(json, "serve", Ok(reply), started),
        Err(e) => return cli::finish::<()>(json, "serve", Err(e), started),
    };
    if !req.detach {
        let reply = announce(&prepared, std::process::id());
        let code = cli::finish(json, "serve", reply, started);
        if code != ExitCode::SUCCESS {
            return code;
        }
        return match serve(prepared) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => cli::finish::<()>(json, "serve", Err(e), started),
        };
    }
    // SAFETY: no thread has started yet (the runtime starts after the fork), so the child is a
    // whole copy of this one-threaded process.
    match unsafe { libc::fork() } {
        -1 => cli::finish::<()>(json, "serve", Err(AppError::internal("fork failed")), started),
        0 => {
            // SAFETY: the child is one-threaded; setsid only makes it a session leader.
            unsafe { libc::setsid() };
            let log = log_path(&prepared.workspace_id);
            detach_stdio(log.as_deref());
            if let Err(e) = serve(prepared) {
                eprintln!("planroom serve: {}", log_line(&e.message));
                std::process::exit(e.code.exit_code());
            }
            std::process::exit(0)
        }
        child => {
            let reply = announce(&prepared, child as u32);
            cli::finish(json, "serve", reply, started)
        }
    }
}

/// Settle the handshake with a running `serve`, then bind the port, mint the code and write the
/// login page; or hand back the running server this one uses.
fn prepare(req: &ServeReq, cwd: &Path) -> Result<Start, AppError> {
    // A directory no scan has made a workspace yet is served as the root a scan would take, so the
    // canvas shows the empty map with its next action from the real server (DR-82).
    let root = match workspace::find_root(cwd) {
        Some(root) => root,
        None => fs::canonicalize(cwd).map_err(|_| AppError::workspace_not_found(cwd))?,
    };
    let store_path = workspace::store_path(&root);
    // Read what the workspace records, and close the store before any fork. With no store yet
    // there's no `workspace_id`: the port is recorded for the root alone until a later `serve`.
    let workspace_id = match Store::open_existing(&store_path)? {
        Some(store) => {
            store.meta("workspace_id")?.ok_or_else(|| AppError::internal("the store has no workspace_id"))?
        }
        None => String::new(),
    };
    let data_root = data::root()?;
    // Only a server on the port this workspace records is this workspace's: a copied workspace
    // brings the original's `serve.json` along, and must neither reuse nor drain that server.
    let recorded = {
        let all = data::workspaces(&data_root)?;
        let root_key = root.display().to_string();
        let moved = moved_from(&all, &root_key, &workspace_id);
        all.iter().find(|w| w.root == root_key || Some(&w.root) == moved.as_ref()).map(|w| w.port)
    };
    let mut drained = None;
    if let Some(running) = recorded.and_then(|port| read_running(&data_root, &root, port)) {
        match handshake(&running, req.port, &data_root)? {
            Handshake::Reuse(reply) => return Ok(Start::Reused(reply)),
            Handshake::Drained => drained = Some(running.port),
            Handshake::Gone => {}
        }
    }
    let (listener, port) = bind(&data_root, &root, &workspace_id, req.port, drained)?;
    let mut auth = auth::Auth::new(port);
    let (code, page) = login_page(&data_root, port)?;
    let page_code = auth::code_digest(&code);
    auth.add_code(page_code.clone());
    let control_token = auth::random_hex(32).map_err(AppError::internal)?;
    Ok(Start::New(Box::new(Prepared {
        root,
        store_path,
        workspace_id,
        listener,
        port,
        auth,
        control_token,
        page,
        page_code,
        data_root,
    })))
}

/// A fresh one-time login code for the server on `port`, and the 0600 redirect page under the
/// data root that opens the canvas with it. The code is in no output; only the page's path is.
fn login_page(data_root: &Path, port: u16) -> Result<(String, PathBuf), AppError> {
    let code = auth::random_hex(16).map_err(AppError::internal)?;
    let open = data_root.join("open");
    data::ensure_dir(&open)?;
    let page = open.join(format!("{}.html", auth::random_hex(16).map_err(AppError::internal)?));
    let html = format!(
        "<!doctype html>\n<meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\">\
         <title>Planroom</title>\n<script>location.replace(\"http://127.0.0.1:{port}/#code={code}\")</script>\n"
    );
    data::write_private(&page, html.as_bytes())?;
    Ok((code, page))
}

/// What the version handshake decided about the `serve` that `serve.json` names.
enum Handshake {
    /// It runs this engine's version and catalogue, or another build this binary isn't newer
    /// than: this `serve` uses it.
    Reuse(Reply<ServeInfo>),
    /// It was older, or another build older than this binary, and has been asked to drain.
    Drained,
    /// It no longer answers as itself: `serve.json` is left from a server that died.
    Gone,
}

/// The version handshake (architecture.md, Version handshake; DR-58, DR-69). A running `serve`
/// is used only when its version and catalogue hash equal this engine's. An older one is asked
/// to drain, so a newer mod never talks to an older API. One of the same version with another
/// catalogue hash (a dev build) is drained only when this binary is newer by mtime; otherwise it
/// is used, and the reply says it runs another build, so two sessions on two builds don't drain
/// each other in turn. A newer one keeps running, and this engine refuses with
/// `schema_mismatch`, naming the update. `--port N` that moves the workspace drains any server
/// that isn't newer, since that server can't serve the new port.
fn handshake(running: &ServeFile, asked: Option<u16>, data_root: &Path) -> Result<Handshake, AppError> {
    let own = version::info();
    let order = format::compare_versions(&own.version, &running.version);
    if order == Some(Ordering::Less) {
        return match register_code(running, data_root)? {
            Some(page) => {
                let _ = fs::remove_file(page);
                Err(newer_running(running, &own.version))
            }
            None => Ok(Handshake::Gone),
        };
    }
    let moving = asked.is_some_and(|port| port != running.port);
    let same_build = order == Some(Ordering::Equal) && running.catalogue_hash == own.catalogue_hash;
    let drain = moving
        || match order {
            Some(Ordering::Greater) => true,
            _ if same_build => false,
            // Another build: drain it only when this binary is the newer one.
            _ => binary_mtime_ms().unwrap_or(0) > running.binary_mtime_ms.unwrap_or(0),
        };
    if drain {
        let token = running.control_token.clone().unwrap_or_default();
        let body = serde_json::to_string(&DrainReq { token }).map_err(AppError::internal)?;
        return Ok(match post(running.port, "/api/v1/drain", None, &body) {
            Some(202) => Handshake::Drained,
            _ => Handshake::Gone,
        });
    }
    let Some(page) = register_code(running, data_root)? else {
        return Ok(Handshake::Gone);
    };
    let url = format!("http://127.0.0.1:{}/", running.port);
    let key = if same_build { "serve.reused" } else { "serve.reused_other_build" };
    let summary = text::get(key, &[("url", &url)]);
    let result = ServeInfo {
        summary: summary.clone(),
        url,
        port: running.port,
        pid: running.pid,
        login_page: page.display().to_string(),
    };
    Ok(Handshake::Reuse(Reply { result, seq: 0, warnings: vec![], text: summary }))
}

/// Register a fresh login code with the running `serve` through its control token, as `open`
/// does, and write its login page. `None` when that server doesn't answer as itself: it died and
/// left `serve.json`, or another program holds its port now. A page whose code didn't register
/// is removed at once.
fn register_code(running: &ServeFile, data_root: &Path) -> Result<Option<PathBuf>, AppError> {
    let (code, page) = login_page(data_root, running.port)?;
    let body = serde_json::to_string(&CodeReq { code_sha256: auth::code_digest(&code) })
        .map_err(AppError::internal)?;
    let token = running.control_token.as_deref().unwrap_or_default();
    if post(running.port, "/api/v1/code", Some(token), &body) == Some(204) {
        return Ok(Some(page));
    }
    let _ = fs::remove_file(&page);
    Ok(None)
}

/// `schema_mismatch` for a newer `serve` already running: it keeps running, and the line names
/// the update that catches this engine up.
fn newer_running(running: &ServeFile, own: &str) -> AppError {
    let message = text::get("serve.newer_running", &[("running", &running.version), ("own", &own)]);
    AppError::newer_file(Path::new("serve.json"), &running.version, own)
        .details(json!({
            "found": running.version,
            "supported": own,
            "writer_version": running.version,
            "file": "serve.json",
        }))
        .with_message(message)
}

/// One `POST` to a `serve` on this machine, with `bearer` as its `Authorization` when given; the
/// reply's status, or `None` when nothing answers within [`PEER_TIMEOUT`].
fn post(port: u16, path: &str, bearer: Option<&str>, body: &str) -> Option<u16> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, PEER_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(PEER_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PEER_TIMEOUT)).ok()?;
    let auth = bearer.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
         Content-Type: application/json\r\n{auth}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).ok()?;
    std::str::from_utf8(&head).ok()?.strip_prefix("HTTP/1.1 ")?.trim().parse().ok()
}

/// This binary's mtime in milliseconds since the epoch, which `serve.json` records so a later
/// `serve` of the same version can tell which build is newer.
fn binary_mtime_ms() -> Option<u64> {
    let modified = std::env::current_exe().ok()?.metadata().ok()?.modified().ok()?;
    u64::try_from(modified.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}

/// The workspace's port: `--port N`, recorded; else the recorded one, which never moves; else a
/// free one no other workspace records. Another program on it is `port_in_use`. A workspace that
/// moved keeps its port: an entry with this `workspace_id` whose root no longer holds a workspace
/// is this one's, and its root is updated; a copy, whose original still stands, gets its own.
/// After a drain, the port is tried until the drained server lets go of it.
fn bind(
    data_root: &Path,
    root: &Path,
    workspace_id: &str,
    asked: Option<u16>,
    drained: Option<u16>,
) -> Result<(TcpListener, u16), AppError> {
    let root_key = root.display().to_string();
    let all = data::workspaces(data_root)?;
    let moved = moved_from(&all, &root_key, workspace_id);
    let mine = |w: &data::WorkspaceEntry| w.root == root_key || Some(&w.root) == moved.as_ref();
    let others: Vec<u16> = all.iter().filter(|w| !mine(w)).map(|w| w.port).collect();
    let entry = all.iter().find(|w| mine(w));
    let recorded = entry.map(|w| w.port);
    // A root first served before its first scan was recorded with no `workspace_id`; the first
    // `serve` after the scan records it, so a later move of the workspace keeps its port.
    let unrecorded_id = !workspace_id.is_empty() && entry.is_some_and(|w| w.workspace_id != workspace_id);
    if let Some(port) = asked.or(recorded) {
        if others.contains(&port) {
            return Err(port_in_use(port));
        }
        let wait = if drained == Some(port) { DRAIN_WAIT } else { Duration::ZERO };
        let listener = bind_port(port, wait)?;
        if recorded != Some(port) || moved.is_some() || unrecorded_id {
            claim(data_root, &root_key, workspace_id, moved.as_deref(), port)?;
        }
        return Ok((listener, port));
    }
    // A free port no other workspace records, claimed under the lock, so two first serves can't
    // record one port between them.
    for _ in 0..32 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(AppError::internal)?;
        let port = listener.local_addr().map_err(AppError::internal)?.port();
        if others.contains(&port) {
            continue;
        }
        match claim(data_root, &root_key, workspace_id, moved.as_deref(), port) {
            Ok(()) => return Ok((listener, port)),
            Err(e) if e.code == ErrorCode::PortInUse => continue,
            Err(e) => return Err(e),
        }
    }
    Err(AppError::internal("found no free port that no other workspace records"))
}

/// Bind `127.0.0.1:port`, trying again for up to `wait` while another process lets go of it.
fn bind_port(port: u16, wait: Duration) -> Result<TcpListener, AppError> {
    let deadline = Instant::now() + wait;
    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if Instant::now() >= deadline {
                    return Err(port_in_use(port));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(AppError::internal(format!("127.0.0.1:{port}: {e}"))),
        }
    }
}

/// The root this workspace had before it moved: an entry with its `workspace_id` whose root no
/// longer holds a workspace. None when the workspace's own root is recorded, or when it has no
/// store yet, and so no `workspace_id` to know it by.
fn moved_from(all: &[data::WorkspaceEntry], root_key: &str, workspace_id: &str) -> Option<String> {
    if workspace_id.is_empty() || all.iter().any(|w| w.root == root_key) {
        return None;
    }
    all.iter()
        .find(|w| w.workspace_id == workspace_id && !workspace::is_root(Path::new(&w.root)))
        .map(|w| w.root.clone())
}

/// Record `port` for the workspace at `root_key`, replacing its own entry and the one it moved
/// from, under `workspaces.json`'s lock. Refused with `port_in_use` when another workspace
/// records that port by now.
fn claim(
    data_root: &Path,
    root_key: &str,
    workspace_id: &str,
    moved: Option<&str>,
    port: u16,
) -> Result<(), AppError> {
    data::update(data_root, |entries| {
        let mine = |w: &data::WorkspaceEntry| w.root == root_key || Some(w.root.as_str()) == moved;
        if entries.iter().any(|w| !mine(w) && w.port == port) {
            return Err(port_in_use(port));
        }
        let mut entry = data::WorkspaceEntry::new(root_key.into(), workspace_id.into(), port);
        for old in entries.iter().filter(|w| mine(w)) {
            if entry.workspace_id.is_empty() {
                entry.workspace_id.clone_from(&old.workspace_id);
            }
            for (key, value) in &old.extra {
                entry.extra.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        entries.retain(|w| !mine(w));
        entries.push(entry);
        Ok(())
    })
}

fn port_in_use(port: u16) -> AppError {
    AppError::new(ErrorCode::PortInUse, &[("port", &port)])
        .details(json!({"port": port, "fix": text::get("hint.port_in_use", &[])}))
}

/// Write the records of the serving process `pid`, and the reply that announces it: its record
/// with the control token under the data root, and `serve.json`, the same without the token, in
/// the state directory once there is one.
fn announce(prepared: &Prepared, pid: u32) -> Result<Reply<ServeInfo>, AppError> {
    let info = version::info();
    let record = ServeFile {
        format: ServeFile::format_tag(),
        port: prepared.port,
        pid,
        version: info.version,
        catalogue_hash: info.catalogue_hash,
        control_token: Some(prepared.control_token.clone()),
        started_at: clock::now(),
        binary_mtime_ms: binary_mtime_ms(),
        extra: Extra::new(),
    };
    data::ensure_dir(&prepared.data_root.join(RECORDS))?;
    let bytes = serde_json::to_vec(&record).map_err(AppError::internal)?;
    data::write_private(&record_path(&prepared.data_root, prepared.port), &bytes)?;
    write_serve_json(&prepared.root, &record)?;
    let url = format!("http://127.0.0.1:{}/", prepared.port);
    let summary = text::get("serve.summary", &[("url", &url)]);
    let login_page = prepared.page.display().to_string();
    let result = ServeInfo { summary: summary.clone(), url, port: prepared.port, pid, login_page };
    Ok(Reply { result, seq: 0, warnings: vec![], text: summary })
}

/// A running server's record (`planroom.serve/v1`), for a later `serve` and for `open`
/// (contracts.md, HTTP API and live stream). It is kept twice. `DATA_ROOT/serve/<port>.json`
/// (mode 0600 in a 0700 folder) holds the control token, under the port `workspaces.json` gives
/// this workspace alone, so a tree read of the workspace (`grep -r x .`) never reaches the token
/// (DR-88). `serve.json` in the state directory holds the rest, and only once the state directory
/// exists: a `serve` started before the first scan makes no workspace of its directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServeFile {
    pub format: String,
    pub port: u16,
    pub pid: u32,
    pub version: String,
    pub catalogue_hash: String,
    /// Only in the record under the data root. An older engine's `serve.json` still holds it, and
    /// a newer `serve` reads it there to reuse or drain that server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_token: Option<String>,
    pub started_at: Timestamp,
    /// This binary's mtime, in milliseconds since the epoch, when the server started, so a later
    /// `serve` of the same version and another catalogue drains it only when its own binary is
    /// newer (DR-69). A `serve.json` without it counts as the oldest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_mtime_ms: Option<u64>,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Map::is_empty")]
    pub extra: Extra,
}

impl Document for ServeFile {
    const NAME: &'static str = "serve";
}

/// The `serve.json` at `path`, when it holds one this engine reads.
fn read_serve_json(path: &Path) -> Option<ServeFile> {
    format::read_str(&fs::read_to_string(path).ok()?).ok()
}

fn serve_json_path(root: &Path) -> PathBuf {
    workspace::state_dir(root).join("serve.json")
}

/// The data root's folder of running servers' records.
const RECORDS: &str = "serve";

/// The record, with its control token, of the server on `port`, which `workspaces.json` gives to
/// one workspace alone.
pub fn record_path(data_root: &Path, port: u16) -> PathBuf {
    data_root.join(RECORDS).join(format!("{port}.json"))
}

/// The running server this workspace records on `port`, with its control token: its record under
/// the data root, or, from an older engine that kept the token there, the state directory's
/// `serve.json`. Only a server on the port the workspace records counts, so a copied workspace,
/// which brings the original's `serve.json` along, neither reuses nor drains the original's.
fn read_running(data_root: &Path, root: &Path, port: u16) -> Option<ServeFile> {
    let has_token =
        |r: &ServeFile| r.port == port && r.control_token.as_deref().is_some_and(|t| !t.is_empty());
    read_serve_json(&record_path(data_root, port))
        .filter(has_token)
        .or_else(|| read_serve_json(&serve_json_path(root)).filter(has_token))
}

/// Write `serve.json`, `record` without its control token, when the state directory exists.
fn write_serve_json(root: &Path, record: &ServeFile) -> Result<(), AppError> {
    if !workspace::state_dir(root).is_dir() {
        return Ok(());
    }
    let shown = ServeFile { control_token: None, ..record.clone() };
    let bytes = serde_json::to_vec(&shown).map_err(AppError::internal)?;
    data::write_private(&serve_json_path(root), &bytes)
}

/// Remove this process's records: each that still names it. One a later `serve` wrote, after it
/// drained this one or took over from it, isn't this server's, and is left alone.
fn remove_own_records(records: &[PathBuf], pid: u32) {
    for path in records {
        if read_serve_json(path).is_some_and(|s| s.pid == pid) {
            let _ = fs::remove_file(path);
        }
    }
}

/// `DATA_ROOT/logs/<workspace_id>/serve.log`, rotated once when it passes 1 MiB.
fn log_path(workspace_id: &str) -> Option<PathBuf> {
    let dir = data::root().ok()?.join("logs").join(workspace_id);
    data::ensure_dir(&dir).ok()?;
    let log = dir.join("serve.log");
    if fs::metadata(&log).is_ok_and(|m| m.len() > LOG_CAP) {
        let _ = fs::rename(&log, dir.join("serve.log.1"));
    }
    Some(log)
}

/// Point stdin and stdout at `/dev/null` and stderr at the log, so the caller's pipes close and
/// a detached `serve` returns at once. Closing 0 to 2 first makes each open take the lowest free
/// descriptor, so no `dup2` is needed.
fn detach_stdio(log: Option<&Path>) {
    for fd in 0..3 {
        // SAFETY: the standard descriptors belong to this process; nothing else owns them.
        drop(unsafe { OwnedFd::from_raw_fd(fd) });
    }
    let _ = File::open("/dev/null").map(IntoRawFd::into_raw_fd);
    let _ = OpenOptions::new().write(true).open("/dev/null").map(IntoRawFd::into_raw_fd);
    let stderr = log.map_or_else(
        || OpenOptions::new().write(true).open("/dev/null"),
        |log| OpenOptions::new().create(true).append(true).mode(0o600).open(log),
    );
    let _ = stderr.map(IntoRawFd::into_raw_fd);
}

/// Serve until a signal or a drain: the live loop on its own thread, the routes on a Tokio
/// runtime. Either way the server stops taking connections, ends every live stream, lets the
/// requests in flight finish, and removes `serve.json` while it is still this server's.
fn serve(prepared: Prepared) -> Result<(), AppError> {
    let runtime =
        tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(AppError::internal)?;
    // None before the first scan: the layout cache opens the store once a scan has made it.
    let store = Store::open_existing(&prepared.store_path)?;
    let (frames, _) = tokio::sync::broadcast::channel(1024);
    let (own_tx, own_rx) = std::sync::mpsc::channel();
    crate::app::announce_commits_to(own_tx);
    let pid = std::process::id();
    let record = record_path(&prepared.data_root, prepared.port);
    let records = vec![serve_json_path(&prepared.root), record.clone()];
    let latest = Arc::new(AtomicU64::new(0));
    let on_mismatch = {
        let records = records.clone();
        move || remove_own_records(&records, pid)
    };
    // The first scan made the state directory: `serve.json` joins the record there.
    let on_store = {
        let root = prepared.root.clone();
        move || {
            if let Some(own) = read_serve_json(&record).filter(|r| r.pid == pid) {
                let _ = write_serve_json(&root, &own);
            }
        }
    };
    let live = routes::Hooks { on_mismatch: Box::new(on_mismatch), on_store: Box::new(on_store) };
    routes::start_live(&prepared.store_path, own_rx, frames.clone(), latest.clone(), live)?;
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let state = Arc::new(routes::State {
        root: prepared.root.clone(),
        store_path: prepared.store_path.clone(),
        auth: Mutex::new(prepared.auth),
        control_token: prepared.control_token.clone(),
        store: Mutex::new(store),
        frames,
        latest,
        clients: AtomicUsize::new(0),
        stop,
        login_page: Mutex::new(Some((prepared.page_code.clone(), prepared.page.clone()))),
    });
    let open = prepared.data_root.join("open");
    let result = runtime.block_on(async move {
        tokio::spawn(async move {
            loop {
                sweep_login_pages(&open);
                tokio::time::sleep(PAGE_SWEEP).await;
            }
        });
        prepared.listener.set_nonblocking(true).map_err(AppError::internal)?;
        let listener = tokio::net::TcpListener::from_std(prepared.listener).map_err(AppError::internal)?;
        let stop = state.stop.clone();
        let mut drained = stopped.clone();
        let shutdown = async move {
            tokio::select! {
                () = signalled() => {}
                _ = drained.wait_for(|stop| *stop) => {}
            }
            // Every live stream ends on this, so the graceful shutdown waits only for requests.
            let _ = stop.send(true);
        };
        let served = axum::serve(listener, routes::router(state)).with_graceful_shutdown(shutdown);
        // A request runs at most 10 s, so a shutdown that takes longer is stuck: stop anyway.
        let mut stuck = stopped;
        tokio::select! {
            served = served => served.map_err(AppError::internal),
            () = async {
                let _ = stuck.wait_for(|stop| *stop).await;
                tokio::time::sleep(DRAIN_WAIT).await;
            } => Ok(()),
        }
    });
    remove_own_records(&records, pid);
    result
}

/// Delete the login pages in `open/` older than a code's life: the code in each no longer works.
/// This covers the page of a code that was never used, and the pages a later `serve` wrote for
/// this one when it reused it.
fn sweep_login_pages(open: &Path) {
    let Ok(entries) = fs::read_dir(open) else { return };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "html") {
            continue;
        }
        let expired = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age > PAGE_LIFE));
        if expired {
            let _ = fs::remove_file(path);
        }
    }
}

/// Resolves on SIGINT or SIGTERM.
async fn signalled() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(term) => term,
        Err(_) => return std::future::pending().await,
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

/// A line for the server's log: a detached server's stderr is its log file under the data root,
/// which keeps what it is given, so the line is masked as every output is (DR-64) and
/// display-sanitized.
pub(crate) fn log_line(line: &str) -> String {
    planroom_kernel::display::sanitize(&planroom_kernel::redact::text(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_line_masks_the_repo_text_it_names() {
        let line = log_line("git can't read db_PASSWORD=hunter2 here.\u{202e}");
        assert!(!line.contains("hunter2"), "{line}");
        assert!(line.starts_with("git can't read db_PASSWORD=[redacted] here."), "{line}");
    }
}
