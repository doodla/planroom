//! The HTTP routes (contracts.md, HTTP API and live stream): the read commands, the live stream,
//! the layout cache, the login exchange and the embedded canvas.

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as UrlPath, Query, State as Shared};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use planroom_contract::{
    Category, ErrorCode, FocusReq, LayoutPut, MapReq, ScanReq, Seq, SessionReq, SessionToken, StreamFrame,
};
use planroom_store::{Store, Tail};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::app::error::AppError;
use crate::app::{self, Reply};
use crate::live::Live;
use crate::server::{auth, canvas};
use crate::{cli, text};

/// The largest request body.
const BODY_LIMIT: usize = 1 << 20;
const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
                   connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// What every request shares.
pub struct State {
    pub root: PathBuf,
    pub store_path: PathBuf,
    pub auth: Mutex<auth::Auth>,
    /// The connection for the layout cache, which isn't an event and makes no frame.
    pub store: Mutex<Store>,
    /// Every frame the live loop sends, for each stream client.
    pub frames: broadcast::Sender<StreamFrame>,
    /// The login page, deleted once its code is exchanged.
    pub login_page: Mutex<Option<PathBuf>>,
}

type Shared_ = Shared<Arc<State>>;

pub fn router(state: Arc<State>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/api/v1/session", post(session))
        .route("/api/v1/stream", get(stream))
        .route("/api/v1/layout", put(layout))
        .route("/api/v1/{*command}", post(command))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::map_response(secure))
        .with_state(state)
}

/// Headers on every response: a strict CSP, no sniffing, no referrer, and no CORS headers at all.
async fn secure(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// The live loop on its own thread: it polls `data_version` and hears this process's own
/// commits, and sends each frame to every stream client. When a newer engine migrates the store,
/// the whole server exits with `schema_mismatch`, as an older `watch` does (DR-73, DR-78).
pub fn start_live(
    store_path: &Path,
    own: Receiver<Seq>,
    frames: broadcast::Sender<StreamFrame>,
) -> Result<(), AppError> {
    let tail = Tail::open(store_path)?.ok_or_else(|| AppError::internal("the store went away"))?;
    let mut live = Live::new(tail, None, Some(own))?;
    std::thread::Builder::new()
        .name("planroom-live".into())
        .spawn(move || {
            let mut emit = |frame: &StreamFrame| {
                let _ = frames.send(frame.clone());
                true
            };
            if let Err(e) = live.run(&mut emit) {
                eprintln!("planroom serve: the live loop stopped: {e}");
                if e.code() == ErrorCode::SchemaMismatch {
                    std::process::exit(e.code().exit_code());
                }
            }
        })
        .map_err(AppError::internal)?;
    Ok(())
}

/// Why a request was refused before it ran.
fn unauthorized() -> Response {
    let error = AppError::new(ErrorCode::Unauthorized, &[]);
    reply::<()>("serve", Err(error), Instant::now())
}

/// Whether a request passes the checks every API request passes: the `Host`, an exact `Origin`,
/// and, unless `bearer` is false, a token this server issued.
fn guard(state: &State, headers: &HeaderMap, bearer: bool) -> bool {
    let get = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let auth = state.auth.lock().expect("the auth lock is never poisoned");
    auth.host_ok(get(header::HOST))
        && auth.origin_ok(get(header::ORIGIN))
        && (!bearer || auth.bearer_ok(get(header::AUTHORIZATION)))
}

/// An envelope with the HTTP status its error's category maps to.
fn reply<T: Serialize>(command: &str, result: Result<Reply<T>, AppError>, started: Instant) -> Response {
    let took_ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    let (status, body) = match result {
        Ok(r) => (StatusCode::OK, cli::envelope(command, Some(&r.result), None, r.seq, took_ms, r.warnings)),
        Err(e) => {
            let status = match (e.code, e.code.category()) {
                (ErrorCode::Unauthorized, _) => StatusCode::UNAUTHORIZED,
                (_, Category::Usage) => StatusCode::BAD_REQUEST,
                (_, Category::NotFound) => StatusCode::NOT_FOUND,
                (_, Category::Conflict) => StatusCode::CONFLICT,
                (_, Category::Permission) => StatusCode::FORBIDDEN,
                (_, Category::Budget) => StatusCode::TOO_MANY_REQUESTS,
                (_, Category::Unavailable) => StatusCode::SERVICE_UNAVAILABLE,
                (_, Category::Failed) => StatusCode::UNPROCESSABLE_ENTITY,
                (_, Category::Internal) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, cli::envelope::<T>(command, None, Some(&e), 0, took_ms, vec![]))
        }
    };
    (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
}

/// `GET /`: the canvas, or the page naming the fix when this binary has none.
async fn index(Shared(state): Shared_, headers: HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if !state.auth.lock().expect("the auth lock is never poisoned").host_ok(host) {
        return unauthorized();
    }
    match canvas::file("index.html") {
        Some((bytes, kind)) => ([(header::CONTENT_TYPE, kind)], bytes).into_response(),
        None => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], canvas::fix_page()).into_response(),
    }
}

/// `GET /assets/*`: one file of the embedded canvas.
async fn asset(Shared(state): Shared_, headers: HeaderMap, UrlPath(path): UrlPath<String>) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if !state.auth.lock().expect("the auth lock is never poisoned").host_ok(host) {
        return unauthorized();
    }
    match canvas::file(&format!("assets/{path}")) {
        Some((bytes, kind)) => ([(header::CONTENT_TYPE, kind)], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /api/v1/session`: the login page's one-time code for a bearer token.
async fn session(Shared(state): Shared_, headers: HeaderMap, body: Bytes) -> Response {
    if !guard(&state, &headers, false) {
        return unauthorized();
    }
    let started = Instant::now();
    let Ok(req) = serde_json::from_slice::<SessionReq>(&body) else {
        return unauthorized();
    };
    let exchanged = state.auth.lock().expect("the auth lock is never poisoned").exchange(&req.code);
    match exchanged {
        Ok(token) => {
            if let Some(page) = state.login_page.lock().expect("the page lock is never poisoned").take() {
                let _ = std::fs::remove_file(page);
            }
            let text = text::get("serve.session", &[]);
            reply(
                "serve",
                Ok(Reply { result: SessionToken { token }, seq: 0, warnings: vec![], text }),
                started,
            )
        }
        Err(_) => unauthorized(),
    }
}

/// `PUT /api/v1/layout`: cache an auto-layout; last writer wins, and no frame follows.
async fn layout(Shared(state): Shared_, headers: HeaderMap, body: Bytes) -> Response {
    if !guard(&state, &headers, true) {
        return unauthorized();
    }
    let started = Instant::now();
    let put: LayoutPut = match serde_json::from_slice(&body) {
        Ok(put) => put,
        Err(e) => return reply::<()>("serve", Err(bad_body(&e)), started),
    };
    let positions = serde_json::to_string(&put.positions).expect("positions serialize");
    let stored = tokio::task::spawn_blocking(move || {
        let store = state.store.lock().expect("the store lock is never poisoned");
        store.put_layout(&put.view_key, &put.structure_hash, &positions)
    })
    .await;
    match stored {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(e)) => reply::<()>("serve", Err(e.into()), started),
        Err(e) => reply::<()>("serve", Err(AppError::internal(e)), started),
    }
}

fn bad_body(e: &serde_json::Error) -> AppError {
    AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get("argument.bad_body", &[("why", e)]))])
        .details(json!({"field": "body"}))
}

/// `POST /api/v1/{command}`: a catalogue command, run as the CLI runs it, in the workspace root.
async fn command(
    Shared(state): Shared_,
    headers: HeaderMap,
    UrlPath(path): UrlPath<String>,
    body: Bytes,
) -> Response {
    if !guard(&state, &headers, true) {
        return unauthorized();
    }
    let started = Instant::now();
    let name = path.replace('/', " ");
    let body: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => return reply::<()>(&name, Err(bad_body(&e)), started),
        }
    };
    let root = state.root.clone();
    let ran = tokio::task::spawn_blocking(move || dispatch(&name, body, &root, started)).await;
    ran.unwrap_or_else(|e| reply::<()>("serve", Err(AppError::internal(e)), started))
}

/// The read commands this server runs, and `scan`, whose commit reaches every stream through the
/// process's own live loop. `scan` over HTTP always scans the workspace root.
fn dispatch(name: &str, body: Value, root: &Path, started: Instant) -> Response {
    fn parse<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, AppError> {
        serde_json::from_value(body).map_err(|e| bad_body(&e))
    }
    match name {
        "map" => reply(name, parse::<MapReq>(body).and_then(|req| app::map::run(&req, root)), started),
        "focus" => reply(name, parse::<FocusReq>(body).and_then(|req| app::focus::run(&req, root)), started),
        "scan" => {
            let ran = parse::<ScanReq>(body).and_then(|req| {
                if req.dir.is_some() {
                    return Err(AppError::invalid_argument("dir", "argument.http_scan_dir"));
                }
                app::scan::run(&req, root)
            });
            reply(name, ran, started)
        }
        other => {
            let what = text::get("argument.http_command", &[("command", &other)]);
            let error =
                AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": null}));
            reply::<()>("serve", Err(error), started)
        }
    }
}

/// `GET /api/v1/stream?since=SEQ`: server-sent events, one per frame, `Last-Event-ID` resuming.
async fn stream(
    Shared(state): Shared_,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !guard(&state, &headers, true) {
        return unauthorized();
    }
    let since: Option<Seq> = query
        .get("since")
        .and_then(|s| s.parse().ok())
        .or_else(|| headers.get("last-event-id").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()));
    // Subscribe before the backlog is read, so nothing commits unseen between the two.
    let live = state.frames.subscribe();
    let store_path = state.store_path.clone();
    let backlog = tokio::task::spawn_blocking(move || backlog(&store_path, since)).await;
    let (backlog, cursor) = match backlog {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => return reply::<()>("watch", Err(e), Instant::now()),
        Err(e) => return reply::<()>("watch", Err(AppError::internal(e)), Instant::now()),
    };
    let first = std::iter::once(StreamFrame::Ping).chain(backlog);
    let later = BroadcastStream::new(live).filter_map(move |frame| match frame {
        Ok(StreamFrame::Event { seq, .. }) if seq <= cursor => None,
        Ok(frame) => Some(frame),
        Err(_) => None,
    });
    let events = tokio_stream::iter(first).chain(later).map(|frame| Ok::<_, Infallible>(sse(&frame)));
    Sse::new(events).into_response()
}

/// The events after `since`, or none without it, and the cursor the live frames continue from.
fn backlog(store_path: &Path, since: Option<Seq>) -> Result<(Vec<StreamFrame>, Seq), AppError> {
    let tail = Tail::open(store_path)?.ok_or_else(|| AppError::internal("the store went away"))?;
    let mut live = Live::new(tail, since, None)?;
    let mut frames = Vec::new();
    if since.is_some() {
        live.drain(&mut |frame| {
            frames.push(frame.clone());
            true
        })?;
    }
    Ok((frames, live.cursor()))
}

fn sse(frame: &StreamFrame) -> SseEvent {
    let data = serde_json::to_value(frame).expect("a frame serializes");
    let kind = data["type"].as_str().unwrap_or("event").to_owned();
    let event = SseEvent::default().event(kind).data(data.to_string());
    match frame {
        StreamFrame::Event { seq, .. } | StreamFrame::Reset { seq } => event.id(seq.to_string()),
        StreamFrame::Ping => event,
    }
}
