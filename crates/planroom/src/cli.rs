//! The CLI: one subcommand per catalogue leaf this build serves, and the `--json` envelope.
//!
//! With `--json`, every command prints exactly one JSON document on stdout, on one line, except
//! `watch`, which prints one `StreamFrame` per line until stopped.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use clap::{CommandFactory, Parser, Subcommand};
use planroom_contract::{
    Command, CommandName, Envelope, FocusReq, MapReq, Meta, PersonInitReq, SCHEMA_VERSION, ScanReq,
    SchemaReq, Seq, ServeReq, Size, StreamFrame, VersionReq, Warning, WatchReq,
};
use planroom_store::Tail;
use serde::Serialize;

use crate::app::actor::Caller;
use crate::app::error::AppError;
use crate::app::{self, Reply, workspace};
use crate::live::Live;

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
}

#[derive(Subcommand)]
enum PersonCmd {
    /// Register this workspace's person key, read from stdin; --rekey replaces it.
    Init(PersonInitReq),
}

pub fn main() -> ExitCode {
    let caller = Caller::from_env();
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help().expect("help is written to stdout");
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
        Cmd::Version(req) => {
            let name = Command::Version(req).name();
            finish(cli.json, &name.0, app::version::version(), started)
        }
    }
}

/// Print a reply or an error, and exit with the error's category's code.
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
                envelope(command, Some(&reply.result), None, reply.seq, took_ms, reply.warnings)
            } else {
                reply.text
            };
            (ExitCode::SUCCESS, line)
        }
        Err(e) => {
            let line = if json {
                envelope::<T>(command, None, Some(&e), 0, took_ms, vec![])
            } else {
                eprintln!(
                    "{}{}",
                    e.message,
                    e.hint.as_deref().map(|h| format!("\n  {h}")).unwrap_or_default()
                );
                return ExitCode::from(e.code.exit_code() as u8);
            };
            (ExitCode::from(e.code.exit_code() as u8), line)
        }
    };
    println!("{out}");
    code
}

/// The envelope as one line, with `meta.size` measuring the line itself.
pub(crate) fn envelope<T: Serialize>(
    command: &str,
    result: Option<&T>,
    error: Option<&AppError>,
    seq: Seq,
    took_ms: u32,
    warnings: Vec<Warning>,
) -> String {
    let mut env = Envelope {
        schema_version: SCHEMA_VERSION,
        ok: error.is_none(),
        command: CommandName(command.into()),
        result,
        error: error.map(AppError::object),
        meta: Meta { seq, took_ms, size: Size::default(), warnings, ..Meta::default() },
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

/// `planroom watch`: frames until stdout closes.
fn watch(json: bool, req: &WatchReq, cwd: &Path) -> ExitCode {
    let started = Instant::now();
    let opened = workspace::find_root(cwd)
        .ok_or_else(|| AppError::workspace_not_found(cwd))
        .and_then(|root| {
            Tail::open(&workspace::store_path(&root))?.ok_or_else(|| AppError::workspace_not_found(&root))
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
        StreamFrame::Event { seq, summary, .. } => format!("{seq} {summary}"),
        StreamFrame::Reset { seq } => format!("{seq} reset"),
        StreamFrame::Ping => "ping".into(),
    }
}
