//! `planroom serve [--detach] [--port N]`: the canvas and the HTTP API on `127.0.0.1`, at the
//! workspace's recorded port (architecture.md, Local server and live stream; DR-29, DR-58, DR-78).
//!
//! Before it serves, it binds the port, mints the one-time login code, writes the login page
//! under the data root (mode 0600) and `serve.json` in the state directory (mode 0600), and prints
//! one envelope. `--detach` forks after binding, so the port already accepts connections, and the
//! child takes a session of its own with `setsid`; the parent prints and returns at once.

pub mod auth;
pub mod canvas;
mod routes;

use std::fs::{self, File, OpenOptions};
use std::net::TcpListener;
use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use planroom_contract::{ErrorCode, ServeInfo, ServeReq};
use planroom_store::Store;
use serde_json::json;

use crate::app::error::AppError;
use crate::app::{Reply, data, version, workspace};
use crate::{cli, clock, text};

/// A logged-out canvas has this long to use its login page before the page is deleted.
const PAGE_LIFE: std::time::Duration = auth::CODE_LIFE;
/// A log file past this size is rotated once at start.
const LOG_CAP: u64 = 1 << 20;

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
}

pub fn run(req: &ServeReq, cwd: &Path, json: bool, started: Instant) -> ExitCode {
    let prepared = match prepare(req, cwd) {
        Ok(p) => p,
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
                eprintln!("planroom serve: {}", e.message);
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

/// Bind the port, mint the code and write the login page.
fn prepare(req: &ServeReq, cwd: &Path) -> Result<Prepared, AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let store_path = workspace::store_path(&root);
    // Read what the workspace records, and close the store before any fork.
    let workspace_id = {
        let store = Store::open_existing(&store_path)?.ok_or_else(|| AppError::workspace_not_found(&root))?;
        store.meta("workspace_id")?.ok_or_else(|| AppError::internal("the store has no workspace_id"))?
    };
    let data_root = data::root();
    let (listener, port) = bind(&data_root, &root, &workspace_id, req.port)?;
    let mut auth = auth::Auth::new(port);
    let code = auth::random_hex(16).map_err(AppError::internal)?;
    auth.add_code(auth::code_digest(&code));
    let control_token = auth::random_hex(32).map_err(AppError::internal)?;
    let open = data_root.join("open");
    data::ensure_dir(&open)?;
    let page = open.join(format!("{}.html", auth::random_hex(16).map_err(AppError::internal)?));
    let html = format!(
        "<!doctype html>\n<meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\">\
         <title>Planroom</title>\n<script>location.replace(\"http://127.0.0.1:{port}/#code={code}\")</script>\n"
    );
    data::write_private(&page, html.as_bytes())?;
    Ok(Prepared { root, store_path, workspace_id, listener, port, auth, control_token, page })
}

/// The workspace's port: `--port N`, recorded; else the recorded one, which never moves; else a
/// free one no other workspace records. Another program on it is `port_in_use`.
fn bind(
    data_root: &Path,
    root: &Path,
    workspace_id: &str,
    asked: Option<u16>,
) -> Result<(TcpListener, u16), AppError> {
    let all = data::workspaces(data_root)?;
    let root_key = root.display().to_string();
    let others: Vec<u16> = all.iter().filter(|w| w.root != root_key).map(|w| w.port).collect();
    let recorded = all.iter().find(|w| w.root == root_key).map(|w| w.port);
    let listener = match asked.or(recorded) {
        Some(port) if others.contains(&port) => return Err(port_in_use(port)),
        Some(port) => TcpListener::bind(("127.0.0.1", port)).map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrInUse => port_in_use(port),
            _ => AppError::internal(format!("127.0.0.1:{port}: {e}")),
        })?,
        None => loop {
            let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(AppError::internal)?;
            let port = listener.local_addr().map_err(AppError::internal)?.port();
            if !others.contains(&port) {
                break listener;
            }
        },
    };
    let port = listener.local_addr().map_err(AppError::internal)?.port();
    if recorded != Some(port) {
        data::record(
            data_root,
            data::WorkspaceEntry { root: root_key, workspace_id: workspace_id.into(), port },
        )?;
    }
    Ok((listener, port))
}

fn port_in_use(port: u16) -> AppError {
    AppError::new(ErrorCode::PortInUse, &[("port", &port)])
        .details(json!({"port": port, "fix": text::get("hint.port_in_use", &[])}))
}

/// Write `serve.json` for the serving process `pid`, and the reply that announces it.
fn announce(prepared: &Prepared, pid: u32) -> Result<Reply<ServeInfo>, AppError> {
    let info = version::info();
    let serve_json = json!({
        "format": "planroom.serve/v1",
        "port": prepared.port,
        "pid": pid,
        "version": info.version,
        "catalogue_hash": info.catalogue_hash,
        "control_token": prepared.control_token,
        "started_at": clock::now(),
    });
    data::write_private(&serve_json_path(&prepared.root), serve_json.to_string().as_bytes())?;
    let url = format!("http://127.0.0.1:{}/", prepared.port);
    let summary = text::get("serve.summary", &[("url", &url)]);
    let login_page = prepared.page.display().to_string();
    let result = ServeInfo { summary: summary.clone(), url, port: prepared.port, pid, login_page };
    Ok(Reply { result, seq: 0, warnings: vec![], text: summary })
}

fn serve_json_path(root: &Path) -> PathBuf {
    root.join(workspace::DIR).join("serve.json")
}

/// `DATA_ROOT/logs/<workspace_id>/serve.log`, rotated once when it passes 1 MiB.
fn log_path(workspace_id: &str) -> Option<PathBuf> {
    let dir = data::root().join("logs").join(workspace_id);
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

/// Serve until a signal: the live loop on its own thread, the routes on a Tokio runtime.
fn serve(prepared: Prepared) -> Result<(), AppError> {
    let runtime =
        tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(AppError::internal)?;
    let store = Store::open_existing(&prepared.store_path)?
        .ok_or_else(|| AppError::workspace_not_found(&prepared.root))?;
    let (frames, _) = tokio::sync::broadcast::channel(1024);
    let (own_tx, own_rx) = std::sync::mpsc::channel();
    crate::app::announce_commits_to(own_tx);
    routes::start_live(&prepared.store_path, own_rx, frames.clone())?;
    let state = Arc::new(routes::State {
        root: prepared.root.clone(),
        store_path: prepared.store_path.clone(),
        auth: Mutex::new(prepared.auth),
        control_token: prepared.control_token.clone(),
        store: Mutex::new(store),
        frames,
        login_page: Mutex::new(Some(prepared.page.clone())),
    });
    let page = prepared.page.clone();
    let serve_json = serve_json_path(&prepared.root);
    let pid = std::process::id();
    let result = runtime.block_on(async move {
        tokio::spawn(async move {
            tokio::time::sleep(PAGE_LIFE).await;
            let _ = fs::remove_file(page);
        });
        prepared.listener.set_nonblocking(true).map_err(AppError::internal)?;
        let listener = tokio::net::TcpListener::from_std(prepared.listener).map_err(AppError::internal)?;
        axum::serve(listener, routes::router(state))
            .with_graceful_shutdown(shutdown())
            .await
            .map_err(AppError::internal)
    });
    // Leave serve.json only when another serve has taken it over since.
    if fs::read_to_string(&serve_json).is_ok_and(|t| t.contains(&format!("\"pid\":{pid},"))) {
        let _ = fs::remove_file(&serve_json);
    }
    result
}

/// Resolves on SIGINT or SIGTERM.
async fn shutdown() {
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
