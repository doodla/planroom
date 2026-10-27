//! The HTTP routes (contracts.md, HTTP API and live stream): the read commands, the live stream,
//! the layout cache, the login exchange, the drain and the embedded canvas.
//!
//! Limits (architecture.md, Local server and live stream): a request runs at most 10 s, a body
//! holds at most 1 MB, and at most 64 live streams are open at once; past a limit the reply is
//! `serve_limit`. A stream client its broadcast drops gets one `reset` frame and refetches.

use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as UrlPath, Query, State as Shared};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use planroom_contract::{
    Category, CodeReq, DrainReq, ErrorCode, FocusReq, LayoutPut, MapReq, Meta, ScanReq, Seq, SessionReq,
    SessionToken, StreamFrame,
};
use planroom_store::{Store, StoreError, Tail};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, WatchStream};

use crate::app::error::AppError;
use crate::app::{self, Paged, Reply};
use crate::live::Live;
use crate::server::{auth, canvas};
use crate::{cli, text};

/// The largest request body.
const BODY_LIMIT: usize = 1 << 20;
/// The longest a request runs before its reply is `serve_limit`.
const REQUEST_LIMIT: Duration = Duration::from_secs(10);
/// The most live streams open at once.
const STREAM_CLIENTS: usize = 64;
/// Exactly what the canvas needs: its own scripts, styles, fonts, images, ELK worker and API, plus
/// inline style attributes, which React Flow places every block and edge with. Nothing else loads,
/// nothing frames the page, and no form posts anywhere.
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
                   img-src 'self'; font-src 'self'; connect-src 'self'; worker-src 'self'; \
                   base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// What every request shares.
pub struct State {
    pub root: PathBuf,
    pub store_path: PathBuf,
    pub auth: Mutex<auth::Auth>,
    /// The control token in this server's record under the data root, which registers login codes
    /// and drains it.
    pub control_token: String,
    /// The connection for the layout cache, which isn't an event and makes no frame: none until
    /// a scan makes the store, when the first layout opens it.
    pub store: Mutex<Option<Store>>,
    /// Every frame the live loop sends, for each stream client.
    pub frames: broadcast::Sender<StreamFrame>,
    /// The `seq` of the last event the live loop sent, which a dropped client's `reset` names.
    pub latest: Arc<AtomicU64>,
    /// The live streams open now.
    pub clients: AtomicUsize,
    /// Set once the server stops, on a signal or a drain: every live stream ends on it.
    pub stop: watch::Sender<bool>,
    /// The login page this server wrote, with its code's sha256: the page is deleted once that
    /// code is exchanged. Pages for codes registered later are swept once their codes expire.
    pub login_page: Mutex<Option<(String, PathBuf)>>,
}

type Shared_ = Shared<Arc<State>>;

pub fn router(state: Arc<State>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/api/v1/session", post(session))
        .route("/api/v1/code", post(code))
        .route("/api/v1/stream", get(stream))
        .route("/api/v1/layout", put(layout))
        .route("/api/v1/drain", post(drain))
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

/// What the live loop calls back into `serve` for.
pub struct Hooks {
    /// A newer engine migrated the store: remove this server's records before it exits.
    pub on_mismatch: Box<dyn FnOnce() + Send>,
    /// A scan made the store this server started without.
    pub on_store: Box<dyn FnOnce() + Send>,
}

/// The live loop on its own thread: it polls `data_version` and hears this process's own
/// commits, and sends each frame to every stream client, keeping the last `seq` in `latest`. When
/// a newer engine migrates the store, the whole server exits with `schema_mismatch`, as an older
/// `watch` does, after `on_mismatch` removes its records (DR-73, DR-78, DR-82). A server started
/// before the first scan waits for the store, polling as often as for `data_version`, then sends
/// every event from the first, so a stream opened on the empty map sees that scan's frames.
pub fn start_live(
    store_path: &Path,
    own: Receiver<Seq>,
    frames: broadcast::Sender<StreamFrame>,
    latest: Arc<AtomicU64>,
    hooks: Hooks,
) -> Result<(), AppError> {
    let Hooks { on_mismatch, on_store } = hooks;
    let started = match Tail::open(store_path)? {
        Some(tail) => {
            let live = Live::new(tail, None, Some(own))?;
            latest.store(live.cursor(), Ordering::SeqCst);
            Ok(live)
        }
        None => Err(own),
    };
    let path = store_path.to_path_buf();
    std::thread::Builder::new()
        .name("planroom-live".into())
        .spawn(move || {
            let mut emit = |frame: &StreamFrame| {
                if let StreamFrame::Event { seq, .. } | StreamFrame::Reset { seq } = frame {
                    latest.store(*seq, Ordering::SeqCst);
                }
                let _ = frames.send(frame.clone());
                true
            };
            let ran = match started {
                Ok(mut live) => live.run(&mut emit),
                Err(own) => wait_for_store(&path, own).and_then(|mut live| {
                    on_store();
                    live.run(&mut emit)
                }),
            };
            if let Err(e) = ran {
                let shown = super::log_line(&e.to_string());
                eprintln!("planroom serve: the live loop stopped: {shown}");
                if e.code() == ErrorCode::SchemaMismatch {
                    on_mismatch();
                    std::process::exit(e.code().exit_code());
                }
            }
        })
        .map_err(AppError::internal)?;
    Ok(())
}

/// Wait for a scan to make the store at `path`, then stream it from its first event. A store
/// still being made, whose tables a read can't see yet, is waited for too; one a newer engine
/// made is `schema_mismatch`.
fn wait_for_store(path: &Path, own: Receiver<Seq>) -> Result<Live, StoreError> {
    loop {
        std::thread::sleep(crate::live::POLL);
        let Ok(Some(tail)) = Tail::open(path) else { continue };
        tail.check_version()?;
        if tail.seq().is_ok() {
            return Live::new(tail, Some(0), Some(own));
        }
    }
}

/// Why a request was refused before it ran.
fn unauthorized() -> Response {
    let error = AppError::new(ErrorCode::Unauthorized, &[]);
    reply::<()>("serve", Err(error), Instant::now())
}

/// Whether a request passes the checks every API request passes: the `Host`, this page's own
/// origin (an exact `Origin`, or `Sec-Fetch-Site: same-origin` on a `GET`, which carries none),
/// and, unless `bearer` is false, a token this server issued.
fn guard(state: &State, headers: &HeaderMap, get_request: bool, bearer: bool) -> bool {
    let get = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let auth = state.auth.lock().expect("the auth lock is never poisoned");
    auth.host_ok(get(header::HOST))
        && auth.same_origin(
            get_request,
            get(header::ORIGIN),
            get(header::HeaderName::from_static("sec-fetch-site")),
        )
        && (!bearer || auth.bearer_ok(get(header::AUTHORIZATION)))
}

/// An envelope with the HTTP status its error's category maps to.
fn reply<T: Serialize>(command: &str, result: Result<Reply<T>, AppError>, started: Instant) -> Response {
    reply_paged(command, result.map(Paged::whole), started)
}

/// [`reply`] for a page: `meta.next` carries the cursor of the page after it.
fn reply_paged<T: Serialize>(
    command: &str,
    result: Result<Paged<T>, AppError>,
    started: Instant,
) -> Response {
    let took_ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    let (status, body) = match result {
        Ok(Paged { reply: r, next, dirty }) => {
            let summary = cli::summary_of(&r.text);
            let meta = Meta {
                seq: r.seq,
                took_ms,
                next,
                warnings: r.warnings,
                dirty_repos: dirty.repos,
                more_dirty_repos: dirty.more,
                ..Meta::default()
            };
            (StatusCode::OK, cli::envelope_with(command, Some(&r.result), None, meta, summary))
        }
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
            (status, cli::envelope::<T>(command, None, Some(&e), 0, took_ms, vec![], None))
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
    if !guard(&state, &headers, false, false) {
        return unauthorized();
    }
    let started = Instant::now();
    let Ok(req) = serde_json::from_slice::<SessionReq>(&body) else {
        return unauthorized();
    };
    let exchanged = state.auth.lock().expect("the auth lock is never poisoned").exchange(&req.code);
    match exchanged {
        Ok(token) => {
            let mut page = state.login_page.lock().expect("the page lock is never poisoned");
            if page.as_ref().is_some_and(|(code, _)| *code == auth::code_digest(&req.code))
                && let Some((_, page)) = page.take()
            {
                let _ = std::fs::remove_file(page);
            }
            drop(page);
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

/// `POST /api/v1/code`: register a login code by its sha256, with the control token from this
/// server's record, `DATA_ROOT/serve/<port>.json`. Its caller is a local process, not a page, so it may send no `Origin`; a page's
/// `POST` always sends one, and a foreign one is refused.
async fn code(Shared(state): Shared_, headers: HeaderMap, body: Bytes) -> Response {
    let get = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let control = get(header::AUTHORIZATION)
        .and_then(|h| h.strip_prefix("Bearer "))
        .is_some_and(|t| auth::code_digest(t.trim()) == auth::code_digest(&state.control_token));
    let mut auth = state.auth.lock().expect("the auth lock is never poisoned");
    let origin = get(header::ORIGIN);
    if !control || !auth.host_ok(get(header::HOST)) || (origin.is_some() && !auth.origin_ok(origin)) {
        drop(auth);
        return unauthorized();
    }
    let digest = serde_json::from_slice::<CodeReq>(&body)
        .ok()
        .map(|req| req.code_sha256)
        .filter(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    match digest {
        Some(digest) => {
            auth.add_code(digest);
            StatusCode::NO_CONTENT.into_response()
        }
        None => {
            drop(auth);
            let e = AppError::new(
                ErrorCode::InvalidArgument,
                &[("what", &text::get("argument.bad_body", &[("why", &"code_sha256")]))],
            )
            .details(json!({"field": "code_sha256"}));
            reply::<()>("serve", Err(e), Instant::now())
        }
    }
}

/// `PUT /api/v1/layout`: cache an auto-layout; last writer wins, and no frame follows.
async fn layout(Shared(state): Shared_, headers: HeaderMap, body: Bytes) -> Response {
    if !guard(&state, &headers, false, true) {
        return unauthorized();
    }
    let started = Instant::now();
    let put: LayoutPut = match serde_json::from_slice(&body) {
        Ok(put) => put,
        Err(e) => return reply::<()>("serve", Err(bad_body(&e)), started),
    };
    let positions = serde_json::to_string(&put.positions).expect("positions serialize");
    let stored = within(REQUEST_LIMIT, move || {
        let mut store = state.store.lock().expect("the store lock is never poisoned");
        if store.is_none() {
            *store = Store::open_existing(&state.store_path)?;
        }
        // Before the first scan there's no store to keep a layout in, and a layout is only a
        // cache: the next look lays the view out again.
        match store.as_ref() {
            Some(store) => store.put_layout(&put.view_key, &put.structure_hash, &positions),
            None => Ok(()),
        }
    })
    .await;
    match stored {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(e)) => reply::<()>("serve", Err(e.into()), started),
        Err(e) => reply::<()>("serve", Err(e), started),
    }
}

/// Run `work` on the blocking pool for at most `limit`. Past it the reply is `serve_limit`, and
/// `work` runs on to its end: a command may still finish and commit, and its frame reaches the
/// stream as any other.
async fn within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError> {
    match tokio::time::timeout(limit, tokio::task::spawn_blocking(work)).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(e)) => Err(AppError::internal(e)),
        Err(_) => Err(self::limit("request_time", limit.as_secs())),
    }
}

/// `serve_limit`: the request ran past [`REQUEST_LIMIT`], or [`STREAM_CLIENTS`] streams are open.
fn limit(which: &str, max: u64) -> AppError {
    let what = text::get(&format!("serve.limit_{which}"), &[]);
    AppError::new(ErrorCode::ServeLimit, &[("what", &what)]).details(json!({"limit": which, "max": max}))
}

/// `POST /api/v1/drain`: the control token from this server's record, `DATA_ROOT/serve/<port>.json`,
/// asks it to finish its requests in flight and exit, so a newer engine can take the port (DR-58,
/// DR-69). Like `code`, its caller is a local process, so it may send no `Origin`, never a foreign
/// one.
async fn drain(Shared(state): Shared_, headers: HeaderMap, body: Bytes) -> Response {
    let get = |name| headers.get(name).and_then(|v| v.to_str().ok());
    let token = serde_json::from_slice::<DrainReq>(&body).ok().map(|req| req.token);
    let control =
        token.is_some_and(|t| auth::code_digest(t.trim()) == auth::code_digest(&state.control_token));
    let allowed = {
        let auth = state.auth.lock().expect("the auth lock is never poisoned");
        let origin = get(header::ORIGIN);
        control && auth.host_ok(get(header::HOST)) && (origin.is_none() || auth.origin_ok(origin))
    };
    if !allowed {
        return unauthorized();
    }
    let _ = state.stop.send(true);
    StatusCode::ACCEPTED.into_response()
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
    if !guard(&state, &headers, false, true) {
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
    match within(REQUEST_LIMIT, move || dispatch(&name, body, &root, started)).await {
        Ok(response) => response,
        Err(e) => reply::<()>("serve", Err(e), started),
    }
}

/// The read commands this server runs, and `scan`, whose commit reaches every stream through the
/// process's own live loop. `scan` over HTTP always scans the workspace root; a `dir` may only name
/// that root, as the empty map's next action does.
fn dispatch(name: &str, body: Value, root: &Path, started: Instant) -> Response {
    fn parse<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, AppError> {
        serde_json::from_value(body).map_err(|e| bad_body(&e))
    }
    match name {
        "map" => {
            let map = parse::<MapReq>(body).and_then(|req| app::map::for_canvas(&req, root));
            reply_paged(name, map.map(|page| app::dirty::attach(page, root)), started)
        }
        "focus" => {
            reply_paged(name, parse::<FocusReq>(body).and_then(|req| app::focus::run(&req, root)), started)
        }
        "scan" => {
            let ran = parse::<ScanReq>(body).and_then(|mut req| {
                if let Some(dir) = req.dir.take() {
                    let same = Path::new(&dir).canonicalize().ok().zip(root.canonicalize().ok());
                    if !same.is_some_and(|(dir, root)| dir == root) {
                        return Err(AppError::invalid_argument("dir", "argument.http_scan_dir"));
                    }
                }
                app::scan::run(&req, root)
            });
            reply_paged(name, ran, started)
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
    if !guard(&state, &headers, true, true) {
        return unauthorized();
    }
    let since: Option<Seq> = query
        .get("since")
        .and_then(|s| s.parse().ok())
        .or_else(|| headers.get("last-event-id").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()));
    let Some(slot) = ClientSlot::take(&state) else {
        return reply::<()>("watch", Err(limit("stream_clients", STREAM_CLIENTS as u64)), Instant::now());
    };
    // Subscribe before the backlog is read, so nothing commits unseen between the two.
    let live = state.frames.subscribe();
    let stopped = state.stop.subscribe();
    let store_path = state.store_path.clone();
    let backlog = tokio::task::spawn_blocking(move || backlog(&store_path, since)).await;
    let (backlog, cursor) = match backlog {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => return reply::<()>("watch", Err(e), Instant::now()),
        Err(e) => return reply::<()>("watch", Err(AppError::internal(e)), Instant::now()),
    };
    let first = std::iter::once(StreamFrame::Ping).chain(backlog);
    let later = live_after(live, cursor, state.latest.clone());
    let stop = WatchStream::new(stopped).filter(|stop| *stop).map(|_| None);
    let events =
        tokio_stream::iter(first).chain(later).map(Some).merge(stop).take_while(Option::is_some).map(
            move |frame| {
                let _open = &slot;
                Ok::<_, Infallible>(sse(&frame.expect("take_while keeps only frames")))
            },
        );
    Sse::new(events).into_response()
}

/// One of the [`STREAM_CLIENTS`] stream places, given back when its stream ends.
struct ClientSlot(Arc<State>);

impl ClientSlot {
    fn take(state: &Arc<State>) -> Option<ClientSlot> {
        let open = state.clients.fetch_add(1, Ordering::SeqCst);
        if open >= STREAM_CLIENTS {
            state.clients.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(ClientSlot(state.clone()))
    }
}

impl Drop for ClientSlot {
    fn drop(&mut self) {
        self.0.clients.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The broadcast's frames for one client, after the events its backlog already sent (`cursor`).
/// A client the broadcast drops, because it fell behind and frames were lost, gets one `reset`
/// naming the last `seq` sent, and refetches.
fn live_after(
    live: broadcast::Receiver<StreamFrame>,
    mut cursor: Seq,
    latest: Arc<AtomicU64>,
) -> impl tokio_stream::Stream<Item = StreamFrame> {
    BroadcastStream::new(live).filter_map(move |frame| match frame {
        Ok(StreamFrame::Event { seq, .. }) if seq <= cursor => None,
        Ok(frame) => Some(frame),
        Err(BroadcastStreamRecvError::Lagged(_)) => {
            cursor = cursor.max(latest.load(Ordering::SeqCst));
            Some(StreamFrame::Reset { seq: cursor })
        }
    })
}

/// The events after `since`, or none without it, and the cursor the live frames continue from.
/// Before the first scan there are no events, and the live frames start from the first.
fn backlog(store_path: &Path, since: Option<Seq>) -> Result<(Vec<StreamFrame>, Seq), AppError> {
    let Some(tail) = Tail::open(store_path)? else {
        return Ok((vec![], 0));
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap()
    }

    #[test]
    fn a_request_past_its_limit_is_serve_limit_and_the_limit_is_10_s() {
        assert_eq!(REQUEST_LIMIT, Duration::from_secs(10));
        let ran = runtime().block_on(within(Duration::from_millis(50), || {
            std::thread::sleep(Duration::from_millis(500));
        }));
        let e = ran.expect_err("the work ran past its limit");
        assert_eq!(e.code, ErrorCode::ServeLimit);
        assert_eq!(e.details["limit"], "request_time");
        let quick = runtime().block_on(within(Duration::from_secs(5), || 7));
        assert_eq!(quick.unwrap(), 7);
    }

    #[test]
    fn a_stream_client_the_broadcast_drops_gets_one_reset() {
        let frames = runtime().block_on(async {
            let (tx, rx) = broadcast::channel(2);
            let latest = Arc::new(AtomicU64::new(7));
            let later = live_after(rx, 3, latest);
            // Five frames into a channel of two: this client lost three of them.
            for _ in 0..5 {
                tx.send(StreamFrame::Ping).unwrap();
            }
            drop(tx);
            later.collect::<Vec<_>>().await
        });
        assert_eq!(frames, vec![StreamFrame::Reset { seq: 7 }, StreamFrame::Ping, StreamFrame::Ping]);
    }
}
