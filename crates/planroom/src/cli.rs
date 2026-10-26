//! The CLI: one subcommand per catalogue leaf this build serves, and the `--json` envelope.
//!
//! With `--json`, every command prints exactly one JSON document on stdout, on one line, except
//! `watch`, which prints one `StreamFrame` per line until stopped.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{CommandFactory, Parser, Subcommand};
use planroom_contract::{
    Command, CommandName, Envelope, ErrorCode, FocusReq, Handle, ImportReq, MapReq, Meta, PersonInitReq,
    SCHEMA_VERSION, ScanReq, SchemaReq, Seq, ServeReq, ShareReq, Size, StreamFrame, VersionReq, Warning,
    WarningCode, WatchReq,
};
use planroom_kernel::display::{sanitize, sanitize_lines};
use planroom_store::{Store, Tail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::app::actor::Caller;
use crate::app::error::AppError;
use crate::app::{self, Reply, workspace};
use crate::live::Live;
use crate::text;

/// A shared, living map of a software system for people and agents.
#[derive(Parser)]
#[command(name = "planroom", version)]
struct Cli {
    /// Print one JSON document on stdout (for `watch`, one JSON frame per line).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Discover, extract and link the workspace's repos.
    Scan(ScanReq),
    /// The map at a level through a lens.
    Map(MapReq),
    /// Where you are, the one open decision and the next action.
    Focus(FocusReq),
    /// Stream the workspace's committed events as NDJSON frames.
    Watch(WatchReq),
    /// The contract schema's summary, or the command catalogue with --commands.
    Schema(SchemaReq),
    /// This engine's version, target, schema version and catalogue hash.
    Version(VersionReq),
    /// Serve the canvas and the HTTP API on this workspace's port.
    Serve(ServeReq),
    /// The person key that signs a person's actions.
    #[command(subcommand)]
    Person(PersonCmd),
    /// Write the canvas, its snapshot and the first live head for an Artifact publish.
    Share(ShareReq),
    /// Import Artifact inbox rows from stdin, one {path, row} per line; answers read requests.
    Import(ImportReq),
}

#[derive(Subcommand)]
enum PersonCmd {
    /// Register this workspace's person key, read from stdin; --rekey replaces it.
    Init(PersonInitReq),
}

pub fn main() -> ExitCode {
    let caller = Caller::from_env();
    let args: Vec<OsString> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => return usage_error(&e, &args),
    };
    let Some(command) = cli.command else {
        // A closed pipe, as in `planroom | head`, isn't an error.
        let mut out = std::io::stdout().lock();
        let _ = write!(out, "{}", Cli::command().render_help()).and_then(|()| out.flush());
        return ExitCode::SUCCESS;
    };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => return finish::<()>(cli.json, "scan", Err(AppError::internal(e)), Instant::now()),
    };
    let started = Instant::now();
    match command {
        Cmd::Scan(req) => {
            let name = Command::Scan(req.clone()).name();
            finish(cli.json, &name.0, app::scan::run(&req, &cwd), started)
        }
        Cmd::Map(req) => {
            let name = Command::Map(req.clone()).name();
            finish(cli.json, &name.0, app::map::run(&req, &cwd), started)
        }
        Cmd::Focus(req) => {
            let name = Command::Focus(req.clone()).name();
            finish(cli.json, &name.0, app::focus::run(&req, &cwd), started)
        }
        Cmd::Watch(req) => watch(cli.json, &req, &cwd),
        Cmd::Schema(req) => {
            let name = Command::Schema(req.clone()).name();
            finish(cli.json, &name.0, app::version::schema(&req), started)
        }
        Cmd::Person(PersonCmd::Init(req)) => {
            let name = Command::PersonInit(req.clone()).name();
            let reply = app::person::run(&req, &cwd, &caller, &mut std::io::stdin().lock());
            finish(cli.json, &name.0, reply, started)
        }
        Cmd::Serve(req) => crate::server::run(&req, &cwd, cli.json, started),
        Cmd::Share(req) => {
            let name = Command::Share(req.clone()).name();
            finish(cli.json, &name.0, app::share::run(&req, &cwd), started)
        }
        Cmd::Import(req) => {
            let name = Command::Import(req.clone()).name();
            let reply = app::import::run(&req, &cwd, &caller, &mut std::io::stdin().lock());
            finish(cli.json, &name.0, reply, started)
        }
        Cmd::Version(req) => {
            let name = Command::Version(req).name();
            finish(cli.json, &name.0, app::version::version(), started)
        }
    }
}

/// Print a reply or an error, and exit with the error's category's code. Text mode and stderr
/// are display-sanitized; `--json` stays data, but for its engine-authored `summary` line.
pub(crate) fn finish<T: Serialize>(
    json: bool,
    command: &str,
    reply: Result<Reply<T>, AppError>,
    started: Instant,
) -> ExitCode {
    let took_ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    let (code, out) = match reply {
        Ok(reply) => {
            let line = if json {
                let summary = summary_of(&reply.text);
                envelope(command, Some(&reply.result), None, reply.seq, took_ms, reply.warnings, summary)
            } else {
                sanitize_lines(&reply.text)
            };
            (ExitCode::SUCCESS, line)
        }
        Err(e) => {
            let exit = ExitCode::from(e.code.exit_code() as u8);
            let line = if json {
                let e = with_usage(e, command);
                envelope::<T>(command, None, Some(&e), 0, took_ms, vec![], None)
            } else {
                let hint = e.hint.as_deref().map(|h| format!("\n  {}", sanitize(h))).unwrap_or_default();
                let mut err = std::io::stderr().lock();
                let _ = writeln!(err, "{}{hint}", sanitize(&e.message));
                return exit;
            };
            (exit, line)
        }
    };
    // A closed pipe, as in `planroom map | head`, isn't an error.
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{out}").and_then(|()| stdout.flush());
    code
}

/// The envelope's `summary`: the first line text mode prints, display-sanitized.
pub(crate) fn summary_of(text: &str) -> Option<String> {
    text.lines().next().map(sanitize).filter(|line| !line.trim().is_empty())
}

/// The envelope as one line, with `meta.size` measuring the line itself. `meta.truncated` is set
/// exactly when a `truncated` warning says what the budget cut.
pub(crate) fn envelope<T: Serialize>(
    command: &str,
    result: Option<&T>,
    error: Option<&AppError>,
    seq: Seq,
    took_ms: u32,
    warnings: Vec<Warning>,
    summary: Option<String>,
) -> String {
    let truncated = warnings.iter().any(|w| w.code == WarningCode::Truncated);
    let mut env = Envelope {
        schema_version: SCHEMA_VERSION,
        ok: error.is_none(),
        command: CommandName(command.into()),
        summary,
        result,
        error: error.map(AppError::object),
        meta: Meta { seq, took_ms, size: Size::default(), truncated, warnings, ..Meta::default() },
    };
    // The size counts its own digits, so settle it: at most a few rounds.
    let mut line = serde_json::to_string(&env).expect("an envelope serializes");
    for _ in 0..4 {
        let bytes = line.len() as u32;
        let size = Size { bytes, tokens_est: bytes.div_ceil(4) };
        if env.meta.size == size {
            break;
        }
        env.meta.size = size;
        line = serde_json::to_string(&env).expect("an envelope serializes");
    }
    line
}

/// `planroom watch`: frames until stdout closes, or one error envelope when the stream can't
/// start or a newer engine migrates the store under it (`schema_mismatch`, DR-78).
fn watch(json: bool, req: &WatchReq, cwd: &Path) -> ExitCode {
    let started = Instant::now();
    let opened = workspace::find_root(cwd)
        .ok_or_else(|| AppError::workspace_not_found(cwd))
        .and_then(|root| {
            let path = workspace::store_path(&root);
            // Opening the store migrates an older one and refuses a newer one, as every command
            // does at open; the stream itself reads only through its read-only connection.
            drop(Store::open_existing(&path)?.ok_or_else(|| AppError::workspace_not_found(&root))?);
            Tail::open(&path)?.ok_or_else(|| AppError::workspace_not_found(&root))
        })
        .and_then(|tail| Ok(Live::new(tail, req.since, None)?));
    let mut live = match opened {
        Ok(live) => live,
        Err(e) => return finish::<()>(json, "watch", Err(e), started),
    };
    let stdout = std::io::stdout();
    let mut emit = |frame: &StreamFrame| {
        let line =
            if json { serde_json::to_string(frame).expect("a frame serializes") } else { frame_text(frame) };
        let mut out = stdout.lock();
        writeln!(out, "{line}").and_then(|_| out.flush()).is_ok()
    };
    match live.run(&mut emit) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => finish::<()>(json, "watch", Err(e.into()), started),
    }
}

fn frame_text(frame: &StreamFrame) -> String {
    match frame {
        StreamFrame::Event { seq, summary, .. } => format!("{seq} {}", sanitize(summary)),
        StreamFrame::Reset { seq } => format!("{seq} reset"),
        StreamFrame::Ping => "ping".into(),
    }
}

/// A clap usage error. `--help` and `--version` print as clap prints them. Otherwise, with `--json`
/// in argv, one `invalid_argument` envelope naming the leaf, with its synopsis and its `schema
/// --command LEAF` handle (DR-21, DR-66); without it, clap's text on stderr, display-sanitized.
/// Either way, exit 2.
fn usage_error(e: &clap::Error, args: &[OsString]) -> ExitCode {
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        e.exit();
    }
    let json = args.iter().skip(1).take_while(|a| *a != "--").any(|a| a == "--json");
    let rendered = e.render().to_string();
    if !json {
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "{}", sanitize_lines(&rendered));
        return ExitCode::from(ErrorCode::InvalidArgument.exit_code() as u8);
    }
    let path = leaf_path(args);
    let command = if path.is_empty() { "planroom".to_owned() } else { path.join(" ") };
    let why = rendered.lines().next().unwrap_or_default();
    let why = why.strip_prefix("error: ").unwrap_or(why);
    let what = text::get("argument.usage", &[("why", &why)]);
    let error = AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({ "field": usage_field(e) }));
    finish::<()>(true, &command, Err(error), Instant::now())
}

/// The subcommand path argv names, as far as it names one: `["person", "init"]`.
fn leaf_path(args: &[OsString]) -> Vec<String> {
    let mut cmd = Cli::command();
    let mut path = Vec::new();
    for arg in args.iter().skip(1).filter_map(|a| a.to_str()) {
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') {
            continue;
        }
        let Some(sub) = cmd.find_subcommand(arg).cloned() else { break };
        path.push(sub.get_name().to_owned());
        cmd = sub;
    }
    path
}

/// The argument a clap error names, as its request field: `--level <LEVEL>` is `level`.
fn usage_field(e: &clap::Error) -> Value {
    if e.kind() == ErrorKind::InvalidSubcommand {
        return json!("command");
    }
    let named = match e.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(arg)) => Some(arg.as_str()),
        Some(ContextValue::Strings(args)) => args.first().map(String::as_str),
        _ => None,
    };
    let Some(named) = named else { return Value::Null };
    let field = match named.strip_prefix("--") {
        Some(long) => long.split([' ', '=']).next().unwrap_or_default().to_owned(),
        None => named.trim_matches(['<', '>', '[', ']', '.']).to_lowercase(),
    };
    json!(sanitize(&field.replace('-', "_")))
}

/// An `invalid_argument` envelope carries the leaf's synopsis and its `schema --command LEAF`
/// handle in `details`, and names that handle as the hint, so a wrong argument costs one small
/// read. With no leaf to name, the handle is `schema --commands`.
fn with_usage(e: AppError, command: &str) -> AppError {
    if e.code != ErrorCode::InvalidArgument || e.details.get("synopsis").is_some() {
        return e;
    }
    let leaf = Command::SPECS.iter().any(|s| s.name() == command);
    let mut cmd = Cli::command();
    if leaf {
        for word in command.split(' ') {
            if let Some(sub) = cmd.find_subcommand(word).cloned() {
                cmd = sub;
            }
        }
    }
    let schema = if leaf {
        Handle {
            command: CommandName("schema".into()),
            args: vec!["--command".into(), command.into()],
            label: text::get("argument.schema_label", &[("leaf", &command)]),
        }
    } else {
        Handle {
            command: CommandName("schema".into()),
            args: vec!["--commands".into()],
            label: text::get("schema.commands_label", &[]),
        }
    };
    let words: Vec<String> =
        schema.args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect();
    let hint = format!("planroom schema {}", words.join(" "));
    let mut details = match e.details.clone() {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    details.insert("synopsis".into(), json!(synopsis(&cmd, if leaf { command } else { "" })));
    details.insert("schema".into(), serde_json::to_value(&schema).unwrap_or(Value::Null));
    e.details(Value::Object(details)).with_hint(hint)
}

/// A leaf's one-line synopsis from its clap arguments, long flags only, such as
/// `planroom map [--level intent|system|container|component|code] [--lens LENS] ... [--json]`.
fn synopsis(cmd: &clap::Command, path: &str) -> String {
    let mut words =
        vec!["planroom".to_owned(), if path.is_empty() { "COMMAND".into() } else { path.to_owned() }];
    for arg in cmd.get_arguments() {
        let id = arg.get_id().as_str();
        if matches!(id, "help" | "version" | "json") || arg.is_hide_set() {
            continue;
        }
        let value = || {
            let values: Vec<String> =
                arg.get_possible_values().iter().map(|v| v.get_name().to_owned()).collect();
            if values.is_empty() {
                arg.get_value_names()
                    .and_then(|names| names.first())
                    .map_or_else(|| id.to_uppercase(), |n| n.as_str().to_owned())
            } else {
                values.join("|")
            }
        };
        let word = match arg.get_long() {
            Some(long) if arg.get_action().takes_values() => format!("--{long} {}", value()),
            Some(long) => format!("--{long}"),
            None => value(),
        };
        words.push(if arg.is_required_set() { word } else { format!("[{word}]") });
    }
    words.push("[--json]".into());
    words.join(" ")
}
