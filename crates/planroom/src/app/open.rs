//! `planroom open`: the canvas in the person's browser, signed in with a one-time code (DR-29,
//! DR-64, DR-75, DR-81).
//!
//! - Person only: a mod press's token, or, in a workspace no mod has registered a key for, a
//!   `/dev/tty` confirmation that names it. Once a key is registered, the terminal path is refused.
//! - It starts this workspace's `serve` in the background, or reuses the running one, which writes
//!   a fresh code into a 0600 login page under `DATA_ROOT/open/`. The opener gets that page's
//!   path, so the code is in no process's argv, and nothing `open` prints holds it.
//! - That `serve` is a fresh process, `planroom serve --detach --json` run from this executable
//!   with `PLANROOM_PERSON_TOKEN` and `PLANROOM_RAW` left out of its environment, never a fork of
//!   this one: a fork would keep the environment this process started with, token and all, for the
//!   server's whole life (DR-64).
//! - The opener runs with a cleared environment: the fixed system `PATH`, `/usr/bin:/bin`, never
//!   the caller's, so a shim earlier on the caller's `PATH` never gets the page, plus `HOME`,
//!   `LC_ALL` and what reaches the person's display. So no person token reaches it.
//! - Where no browser can be seen (no `DISPLAY` or `WAYLAND_DISPLAY` on Linux, an SSH session, no
//!   opener, or one that fails), it opens nothing and returns `opened: false` with `share` as the
//!   next action. It starts no server where it finds no opener, and removes the page an opener
//!   failed on. On WSL it opens through `wslview`.

use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};

use planroom_contract::{
    Command, CommandName, Empty, EmptyReason, Envelope, Handle, OpenReq, OpenResult, ServeInfo,
};
use planroom_store::Store;

use crate::app::actor::{self, Bound, Caller, Terminal};
use crate::app::error::AppError;
use crate::app::{Reply, workspace};
use crate::{clock, text};

/// The `PATH` the opener runs with, whatever the caller's is (DR-75).
pub const OPENER_PATH: &str = "/usr/bin:/bin";
/// The caller's variables the opener keeps besides `PATH`: its home and locale, and what reaches
/// the person's display and desktop session.
const OPENER_KEEPS: &[&str] = &[
    "HOME",
    "LC_ALL",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];
/// How long the opener may take to hand the page to a browser.
const OPENER_WAIT: Duration = Duration::from_secs(10);

pub fn run(
    req: &OpenReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Reply<OpenResult>, AppError> {
    let spec = Command::Open(req.clone()).spec();
    let command = spec.name();
    // Before the first scan there's no store, and so no key: `serve` serves the empty map there.
    let root = match workspace::find_root(cwd) {
        Some(root) => root,
        None => std::fs::canonicalize(cwd).map_err(|_| AppError::workspace_not_found(cwd))?,
    };
    let mut store = Store::open_existing(&workspace::store_path(&root))?;
    let registered = match &store {
        Some(store) => store.meta("person_pubkey")?,
        None => None,
    };
    let keys: Vec<_> = registered.as_deref().and_then(actor::parse_key).into_iter().collect();
    let keys: Vec<_> = keys.iter().collect();
    let now = clock::unix_micros() / 1_000_000;
    let path = actor::terminal_path(&command, registered.is_some());
    let acting = actor::person(caller, spec, &Bound::default(), &keys, now, path, terminal)?;
    if let (Some(digest), Some(store)) = (&acting.digest, store.as_mut()) {
        actor::spend(store, digest, now, &command, path)?;
    }
    drop(store);

    // Where no browser can be seen, no server starts for nothing.
    let opened = match Opener::find() {
        Some(opener) => {
            let served = start_serve(&root)?;
            let page = PathBuf::from(&served.login_page);
            let opened = opener.open(&page);
            if !opened {
                // The code in it opens nothing now; a later `open` writes a fresh one.
                let _ = std::fs::remove_file(&page);
            }
            opened
        }
        None => false,
    };
    if !opened {
        let line = text::get("open.no_browser", &[]);
        let result = OpenResult { opened: false, empty: Some(no_browser(&root, line.clone())) };
        return Ok(Reply { result, seq: 0, warnings: vec![], text: line });
    }
    let line = text::get("open.opened", &[]);
    Ok(Reply { result: OpenResult { opened: true, empty: None }, seq: 0, warnings: vec![], text: line })
}

/// This workspace's `serve`, started in the background or the running one reused, as
/// `planroom serve --detach --json` run from `root` replies. A fresh process of this executable,
/// whose environment leaves out the person token and `PLANROOM_RAW`, so the server that outlives
/// this action holds neither.
fn start_serve(root: &Path) -> Result<ServeInfo, AppError> {
    let exe = std::env::current_exe().map_err(AppError::internal)?;
    let out = Process::new(exe)
        .args(["serve", "--detach", "--json"])
        .current_dir(root)
        .env_remove("PLANROOM_PERSON_TOKEN")
        .env_remove("PLANROOM_RAW")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(AppError::internal)?;
    let reply: Envelope<ServeInfo> = serde_json::from_slice(&out.stdout)
        .map_err(|e| AppError::internal(format!("serve --detach replied with no envelope: {e}")))?;
    match (reply.result, reply.error) {
        (Some(served), _) if reply.ok => Ok(served),
        (_, Some(error)) => Err(AppError::from_object(error)),
        _ => Err(AppError::internal("serve --detach replied with neither a result nor an error")),
    }
}

/// The empty state where no browser can be seen: sharing the canvas as an Artifact shows it on
/// any device, so `share` is the next action, which the pane offers as `/room share` (DR-29). Its
/// `--out` is the state directory's `share`, absolute, so it runs from any directory: the pane
/// runs a handle from the session's, which may be inside a repo.
fn no_browser(root: &Path, line: String) -> Empty {
    let out = workspace::state_dir(root).join("share");
    Empty {
        reason: EmptyReason("no_browser".into()),
        line: Some(line),
        next: Handle {
            command: CommandName("share".into()),
            args: vec!["--out".into(), out.display().to_string()],
            label: text::label("open.share_label", &[]),
        },
    }
}

/// The program that hands a page to the person's browser.
struct Opener {
    program: PathBuf,
}

impl Opener {
    /// This machine's opener, where a browser can be seen: `open` on macOS, `wslview` on WSL, and
    /// `xdg-open` on Linux with a display; looked up only in [`OPENER_PATH`]. None in an SSH
    /// session, or on Linux with no display, as on a container host.
    fn find() -> Option<Opener> {
        let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        if set("SSH_CONNECTION") || set("SSH_TTY") {
            return None;
        }
        let name = if cfg!(target_os = "macos") {
            "open"
        } else if wsl() {
            "wslview"
        } else if set("DISPLAY") || set("WAYLAND_DISPLAY") {
            "xdg-open"
        } else {
            return None;
        };
        #[cfg(debug_assertions)]
        if let Some(program) = test_opener() {
            return Some(Opener { program });
        }
        OPENER_PATH
            .split(':')
            .map(|dir| Path::new(dir).join(name))
            .find(|p| p.is_file())
            .map(|program| Opener { program })
    }

    /// Hand `page` to the browser; whether the opener says it did, within [`OPENER_WAIT`].
    fn open(&self, page: &Path) -> bool {
        let mut cmd = Process::new(&self.program);
        cmd.arg(page).env_clear().env("PATH", OPENER_PATH);
        for name in OPENER_KEEPS {
            if let Some(value) = std::env::var_os(name) {
                cmd.env(name, value);
            }
        }
        let Ok(mut child) = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        else {
            return false;
        };
        let deadline = Instant::now() + OPENER_WAIT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }
}

/// Whether this Linux runs under WSL, where `wslview` opens the Windows browser.
fn wsl() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WSL_DISTRO_NAME").is_some()
            || Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists())
}

/// The test seam for the opener: in a debug build only, `PLANROOM_TEST_OPENER` names an absolute
/// program that stands in for the system opener, so a test can record the argv and environment
/// the opener gets without a browser. Release builds compile this out, so no setting a person or
/// an agent can reach picks what opens the login page.
#[cfg(debug_assertions)]
fn test_opener() -> Option<PathBuf> {
    let program = PathBuf::from(std::env::var_os("PLANROOM_TEST_OPENER")?);
    program.is_absolute().then_some(program)
}
