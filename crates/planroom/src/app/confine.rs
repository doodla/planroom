//! Reads confined to the workspace (architecture.md, Security and privacy: Reads by auto-allowed
//! tools; DR-52): the global `--workspace DIR`, and the paths `ask` and `scan` are named.
//!
//! - A read (`map`, `find`, `ask`, `focus`, `watch`) with `--workspace DIR` needs DIR to be in a
//!   workspace with a store, else `workspace_not_found` naming DIR, and nothing is created. Every
//!   other leaf runs in DIR as from its own directory; `version` and `schema` ignore it.
//! - An argument of `ask` is a path when it starts with `/`, `~`, `file:`, `./` or `../`, is `.`
//!   or `..`, or holds a `/` with no `:` before its first `/` (a node ID such as
//!   `storefront:dir:src/cart` has its `:` first). Each is checked as given and as resolved,
//!   symbolic links followed, a missing path through its longest existing parent.
//! - A credential path needs a person: a dotfile (a file whose name starts with `.`), anything
//!   under `~/.ssh`, `~/.aws`, `~/.gnupg` or `~/.password-store`, `~/.config/*/credentials`, and a
//!   final name matching `*.pem` or `.env*`. A hidden directory is no credential path by its name
//!   alone: a temporary root or a `.github` holds what the rest of the list names. `scan` checks
//!   the directory it reads, the one it's named or, for a plain `scan`, the one it runs in, and a
//!   `--workspace` as given; a relative one is placed from the process's own directory.
//! - Anything else outside the workspace's repos is `outside_workspace`: until `scan FILE` makes a
//!   `file:` snapshot (M4), no read reaches a file outside them. A repo's git directory and the
//!   state directory hold no repo content, so they count as outside too.
//! - Neither refusal has a hint: no command a person could run would read the path in M1.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use planroom_contract::{AskReq, Command, ErrorCode, ScanReq};
use planroom_store::Store;
use serde_json::json;

use crate::app::actor::{self, Bound, Caller, Terminal, TerminalPath};
use crate::app::error::AppError;
use crate::app::workspace;
use crate::{clock, text};

/// The leaves that only read, which `--workspace` never makes a store for.
const READS: &[&str] = &["map", "find", "ask", "focus", "watch"];
/// The leaves that ignore `--workspace`.
const IGNORES: &[&str] = &["version", "schema"];

/// The directory `leaf` runs in: `current`, the process's own, or the `--workspace` directory.
pub fn cwd(
    workspace: Option<&Path>,
    leaf: &str,
    current: std::io::Result<PathBuf>,
) -> Result<PathBuf, AppError> {
    let current = current.map_err(AppError::internal);
    let Some(dir) = workspace.filter(|_| !IGNORES.contains(&leaf)) else { return current };
    let base = if dir.is_absolute() { PathBuf::new() } else { current? };
    let named = base.join(dir);
    // No directory there: no command makes it a workspace, so the refusal names none.
    let missing = || AppError::workspace_not_found(dir).without_hint();
    let resolved = std::fs::canonicalize(&named).map_err(|_| missing())?;
    if !resolved.is_dir() {
        return Err(missing());
    }
    if READS.contains(&leaf) && !has_store(&resolved) {
        // A read creates nothing; the scan that makes DIR a workspace runs from right here.
        let line = format!("planroom scan {}", text::shell_word(&resolved.display().to_string()));
        return Err(AppError::workspace_not_found(dir).with_hint(line));
    }
    Ok(resolved)
}

/// Whether `dir` is in a workspace whose state directory holds a store, or the pointer to one.
fn has_store(dir: &Path) -> bool {
    workspace::find_root(dir).is_some_and(|root| {
        let state = workspace::state_dir(&root);
        state.join(workspace::STORE).is_file() || state.join("store.pointer").is_file()
    })
}

/// Whether an argument names a path rather than an ID or a word.
pub fn path_shaped(arg: &str) -> bool {
    ["/", "~", "file:", "./", "../"].iter().any(|p| arg.starts_with(p))
        || arg == "."
        || arg == ".."
        || arg.find('/').is_some_and(|slash| !arg[..slash].contains(':'))
}

/// Check `ask`'s path-shaped arguments before the question is looked up: a credential path needs
/// a person, and every path must lie in the workspace's repos.
pub fn ask(req: &AskReq, cwd: &Path, caller: &Caller, terminal: &mut dyn Terminal) -> Result<(), AppError> {
    let spec = Command::Ask(req.clone()).spec();
    let mut person: Option<bool> = None;
    for arg in req.args.iter().chain(req.question.as_ref()).filter(|a| path_shaped(a)) {
        let named = Named::of(arg, cwd);
        if named.is_credential() && !*person.get_or_insert_with(|| is_person(spec, cwd, caller, terminal)) {
            return Err(refused(arg, "credential"));
        }
        if !named.in_repos(cwd) {
            return Err(refused(arg, "outside_workspace"));
        }
    }
    Ok(())
}

/// Check the directory `scan` reads, by its argument, by `--workspace` or as the one it runs in:
/// a credential path needs a person. A person's scan of one runs as any scan does.
pub fn scan(
    req: &ScanReq,
    cwd: &Path,
    flag: Option<&Path>,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<(), AppError> {
    let spec = Command::Scan(req.clone()).spec();
    let here = std::env::current_dir();
    let mut named: Vec<(String, Named)> = Vec::new();
    match &req.dir {
        Some(dir) => named.push((dir.clone(), Named::of(dir, cwd))),
        // A plain `scan` reads the directory it runs in.
        None => named.push((cwd.display().to_string(), Named::of(".", cwd))),
    }
    if let Some(flag) = flag {
        // `cwd` is the flag's directory already resolved, so a relative flag is placed from the
        // process's own directory, as `cwd()` placed it; one that can't be placed is a credential
        // path.
        let shown = flag.display().to_string();
        let given = match &here {
            Ok(here) => Named::of(&shown, here),
            Err(_) if flag.is_absolute() => Named::of(&shown, cwd),
            Err(_) => Named::unplaced(),
        };
        named.push((shown, given));
    }
    if let Some((arg, _)) = named.iter().find(|(_, n)| n.is_credential())
        && !is_person(spec, cwd, caller, terminal)
    {
        return Err(refused(arg, "credential"));
    }
    Ok(())
}

/// Whether the caller is a person, as `open` resolves one: a token under the workspace's
/// registered key, spent once, or a confirmation on the terminal.
fn is_person(
    spec: &'static planroom_contract::CommandSpec,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> bool {
    let command = spec.name();
    let mut store = workspace::find_root(cwd).and_then(|root| {
        Store::open_existing(&workspace::store_path(&root)).ok().flatten().map(|s| (root, s))
    });
    let keys = match &store {
        Some((root, store)) => match actor::registered_keys(store, root, caller, &command) {
            Ok(keys) => keys,
            Err(_) => return false,
        },
        None => vec![],
    };
    let keys: Vec<_> = keys.iter().collect();
    let now = clock::unix_micros() / 1_000_000;
    let Ok(acting) =
        actor::person(caller, spec, &Bound::default(), &keys, now, TerminalPath::Takes, terminal)
    else {
        return false;
    };
    match (&acting.digest, store.as_mut()) {
        (Some(digest), Some((_, store))) => {
            actor::spend(store, digest, now, &command, TerminalPath::Takes).is_ok()
        }
        // A token with no store to check it against signs nothing here.
        (Some(_), None) => false,
        (None, _) => true,
    }
}

/// `path_refused` for `path`, with no hint: no command a person runs reads it yet.
fn refused(path: &str, reason: &str) -> AppError {
    let why = text::get(&format!("confine.{reason}"), &[]);
    AppError::new(ErrorCode::PathRefused, &[("path", &path), ("why", &why)])
        .details(json!({"path": path, "reason": reason}))
}

/// A path as given (made absolute and folded, links not followed) and as resolved.
struct Named {
    given: Option<PathBuf>,
    resolved: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl Named {
    fn of(arg: &str, cwd: &Path) -> Named {
        let home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute());
        let arg = arg.strip_prefix("file://").or_else(|| arg.strip_prefix("file:")).unwrap_or(arg);
        let absolute = if arg == "~" || arg.starts_with("~/") {
            home.as_ref().map(|h| h.join(arg.trim_start_matches('~').trim_start_matches('/')))
        } else if arg.starts_with('~') {
            // Another user's home: never this workspace's.
            None
        } else {
            Some(cwd.join(arg))
        };
        let given = absolute.as_deref().map(fold);
        let resolved = given.as_deref().map(resolve);
        Named { given, resolved, home }
    }

    /// A path that can't be placed, which may be anything.
    fn unplaced() -> Named {
        Named { given: None, resolved: None, home: None }
    }

    fn forms(&self) -> impl Iterator<Item = &PathBuf> {
        self.given.iter().chain(self.resolved.iter())
    }

    fn is_credential(&self) -> bool {
        // A path that can't be placed may be anything.
        self.given.is_none() || self.forms().any(|p| credential(p, self.home.as_deref()))
    }

    /// Whether both forms lie in a repo of the workspace `cwd` belongs to, outside its git
    /// directory and the state directory.
    fn in_repos(&self, cwd: &Path) -> bool {
        let Ok(ws) = workspace::discover(cwd) else { return false };
        let state = canonical_or(&ws.state_dir());
        let inside = |p: &Path| {
            ws.repos.iter().any(|repo| {
                let repo = canonical_or(&repo.path);
                p.starts_with(&repo) && !p.starts_with(repo.join(".git"))
            }) && !p.starts_with(&state)
        };
        self.given.is_some() && self.forms().all(|p| inside(p))
    }
}

fn canonical_or(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `path` with `.` dropped and each `..` taken back, without touching the filesystem.
fn fold(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// `path` with its longest existing parent resolved, symbolic links followed, and the rest
/// folded on.
fn resolve(path: &Path) -> PathBuf {
    let names: Vec<&OsStr> = path.iter().collect();
    for keep in (1..=names.len()).rev() {
        let existing: PathBuf = names[..keep].iter().collect();
        if let Ok(mut real) = std::fs::canonicalize(&existing) {
            real.extend(&names[keep..]);
            return fold(&real);
        }
    }
    fold(path)
}

/// The home's folders that hold keys or secrets, everything under them a credential path.
const STORES: &[&str] = &[".ssh", ".aws", ".gnupg", ".password-store"];

/// Whether `path` is a credential path (architecture.md, the `scan` row of the command catalogue).
fn credential(path: &Path, home: Option<&Path>) -> bool {
    let name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
    // A dotfile, not a hidden directory such as a temporary one: the stores kept as directories are
    // named below, and a directory holds what the rest of the list names.
    if (name.starts_with('.') && !path.is_dir()) || name.ends_with(".pem") {
        return true;
    }
    let Some(home) = home else {
        // With no home to place `~` by, any store's folder, and any `.config/*/credentials`, may
        // be the home's: fail closed.
        let parts: Vec<&OsStr> = path.iter().collect();
        return parts.iter().any(|p| STORES.iter().any(|s| p == s))
            || (name == "credentials" && parts.len() >= 3 && parts[parts.len() - 3] == ".config");
    };
    if STORES.iter().any(|store| path.starts_with(home.join(store))) {
        return true;
    }
    name == "credentials" && path.parent().and_then(Path::parent) == Some(home.join(".config").as_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_id_is_no_path_and_a_relative_path_is() {
        for arg in
            ["/etc/passwd", "~/.ssh/id_rsa", "file:///x", "./a", "../a", ".", "..", "src/a.ts", "a/b:c"]
        {
            assert!(path_shaped(arg), "{arg}");
        }
        for arg in ["storefront:dir:src/cart", "kinds", "storefront#12", "a:b/c"] {
            assert!(!path_shaped(arg), "{arg}");
        }
    }

    #[test]
    fn credential_paths_follow_the_catalogue_list() {
        let home = Path::new("/h");
        for path in [
            "/w/.env",
            "/w/.env.local",
            "/h/.ssh/id_rsa",
            "/h/.aws/config",
            "/h/.gnupg/pubring.kbx",
            "/h/.password-store/site.gpg",
            "/h/.config/gh/credentials",
            "/w/key.pem",
        ] {
            assert!(credential(Path::new(path), Some(home)), "{path}");
        }
        for path in ["/w/src/a.ts", "/h/.config/gh/hosts.yml", "/w/credentials"] {
            assert!(!credential(Path::new(path), Some(home)), "{path}");
        }
        // With no home, any home's counts: fail closed.
        for path in [
            "/root/.ssh/id_rsa",
            "/home/x/.aws/config",
            "/home/x/.password-store/site.gpg",
            "/home/x/.config/gh/credentials",
        ] {
            assert!(credential(Path::new(path), None), "{path}");
        }
        assert!(!credential(Path::new("/w/src/a.ts"), None));
    }

    #[test]
    fn a_missing_path_resolves_through_its_longest_existing_parent() {
        assert_eq!(
            resolve(Path::new("/nonexistent-planroom/a/../b")),
            PathBuf::from("/nonexistent-planroom/b")
        );
        assert_eq!(fold(Path::new("/a/b/../../../c")), PathBuf::from("/c"));
    }
}
