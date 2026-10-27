//! Actor resolution and the actor rules (architecture.md, Actors and Actor rules; DR-4, DR-64,
//! DR-72, DR-75, DR-81). The engine resolves who is acting before anything else:
//!
//! - A read needs no actor, and `scan`, which only records what it observed, is the system's.
//! - An agent names itself with `PLANROOM_ACTOR=agent:<id>:<role>`, and is never a person,
//!   whatever else its call carries.
//! - A person is a call that carries a valid `PLANROOM_PERSON_TOKEN`, or a terminal write the
//!   person confirms on `/dev/tty`, for every command but those that refuse that path. A terminal
//!   alone never makes a person.
//! - A write with no actor, no token and no terminal is `actor_required`.
//!
//! The token is `<at>.<sig>`: `at` the Unix time in seconds, and `sig` the base64url of the raw
//! `r ‖ s` ECDSA P-256 SHA-256 signature, by the person key, over
//! `kernel::keys::person_token_message(command, args, idem, base, at)`. The engine refuses a token
//! older than 30 s (or more than 30 s ahead), one for another command or other arguments, and one
//! already spent, and records each spent token's digest in `meta.spent_tokens` until it expires.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, IsTerminal, Write};

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use planroom_contract::{Actor, ActorRule, Client, Command, CommandSpec, ErrorCode, Surface};
use planroom_kernel::keys;
use planroom_store::Store;
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

/// How long a token is good for, either side of its `at`.
pub const TOKEN_WINDOW_SECS: u64 = 30;

/// Who called and how: what actor resolution reads, captured once at start, before the token
/// leaves the process's environment.
#[derive(Debug, Clone, Default)]
pub struct Caller {
    /// `PLANROOM_PERSON_TOKEN`.
    pub token: Option<String>,
    /// `PLANROOM_ACTOR`.
    pub actor: Option<String>,
    /// `PLANROOM_CLIENT`.
    pub client: Option<String>,
    /// The process's arguments after the program name.
    pub argv: Vec<String>,
}

impl Caller {
    /// Read the caller from the environment, and remove the token from this process's
    /// environment so no child inherits it (DR-64).
    pub fn from_env() -> Caller {
        let token = std::env::var("PLANROOM_PERSON_TOKEN").ok();
        if token.is_some() {
            // SAFETY: called from `main` before any thread starts, so nothing reads the
            // environment concurrently.
            unsafe { std::env::remove_var("PLANROOM_PERSON_TOKEN") };
        }
        Caller {
            token,
            actor: std::env::var("PLANROOM_ACTOR").ok(),
            client: std::env::var("PLANROOM_CLIENT").ok(),
            argv: std::env::args().skip(1).collect(),
        }
    }

    /// The writer's client: `PLANROOM_CLIENT` (`mod:<session>` is the mod), else this process.
    pub fn client(&self) -> Client {
        match &self.client {
            Some(id) if id.starts_with("mod:") => Client { surface: Surface::Mod, id: id.clone() },
            Some(id) => Client { surface: Surface::Cli, id: id.clone() },
            None => Client { surface: Surface::Cli, id: format!("cli:{}", std::process::id()) },
        }
    }

    /// The leaf's own arguments: the argv after the leaf's path words, without `--json`.
    pub fn leaf_args(&self, spec: &CommandSpec) -> Option<Vec<String>> {
        let args: Vec<&String> = self.argv.iter().filter(|a| *a != "--json").collect();
        let path = spec.path;
        if args.len() < path.len() || args.iter().zip(path).any(|(a, p)| a.as_str() != *p) {
            return None;
        }
        Some(args[path.len()..].iter().map(|a| (*a).clone()).collect())
    }
}

/// The key a person action may be signed with.
pub fn parse_key(text: &str) -> Option<VerifyingKey> {
    let bytes = base64url_decode(text.trim())?;
    if bytes.len() != 65 || bytes[0] != 4 {
        return None;
    }
    VerifyingKey::from_sec1_bytes(&bytes).ok()
}

/// The first 16 hex characters of the sha256 of a key's SEC1 bytes.
pub fn fingerprint(text: &str) -> String {
    let bytes = base64url_decode(text.trim()).unwrap_or_default();
    keys::sha256_plain_hex(&bytes)[..16].to_owned()
}

/// Why a call wasn't a person's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Missing,
    Malformed,
    Stale,
    BadSignature,
    Spent,
    /// `PLANROOM_ACTOR` named an agent: never a person, whatever else the call carries.
    NotAPerson,
    /// A terminal confirmation, for a command that refuses that path (DR-64, DR-75).
    TerminalRefused,
    /// The person at the terminal didn't confirm.
    NotConfirmed,
}

impl Refused {
    fn as_str(self) -> &'static str {
        match self {
            Refused::Missing => "missing",
            Refused::Malformed => "malformed",
            Refused::Stale => "stale",
            Refused::BadSignature => "bad_signature",
            Refused::Spent => "spent",
            Refused::NotAPerson => "not_a_person",
            Refused::TerminalRefused => "terminal_refused",
            Refused::NotConfirmed => "not_confirmed",
        }
    }
}

/// The request fields a token binds, besides the command.
#[derive(Debug, Clone, Default)]
pub struct Bound<'a> {
    pub idem: Option<&'a str>,
    pub base: Option<u64>,
}

/// Check the caller's token for `spec` against `keys` (any of them may have signed), at Unix time
/// `now`, without spending it. Returns the digest to spend.
pub fn check(
    caller: &Caller,
    spec: &CommandSpec,
    bound: &Bound<'_>,
    keys_allowed: &[&VerifyingKey],
    now: u64,
) -> Result<String, Refused> {
    let Some(token) = caller.token.as_deref() else {
        return Err(Refused::Missing);
    };
    let (at, sig) = token.split_once('.').ok_or(Refused::Malformed)?;
    let at: u64 = at.parse().map_err(|_| Refused::Malformed)?;
    let sig = base64url_decode(sig).ok_or(Refused::Malformed)?;
    let sig = Signature::from_slice(&sig).map_err(|_| Refused::Malformed)?;
    if now.abs_diff(at) > TOKEN_WINDOW_SECS {
        return Err(Refused::Stale);
    }
    let args = caller.leaf_args(spec).ok_or(Refused::BadSignature)?;
    let base = bound.base.map(|b| b.to_string()).unwrap_or_default();
    let msg = keys::person_token_message(&spec.name(), &args, bound.idem.unwrap_or(""), &base, at);
    if !keys_allowed.iter().any(|k| k.verify(&msg, &sig).is_ok()) {
        return Err(Refused::BadSignature);
    }
    Ok(keys::sha256_plain_hex(token.as_bytes()))
}

/// Spend a checked token's digest; a second use is refused, naming the way a person runs
/// `command` along `path`, the terminal path the command took.
pub fn spend(
    store: &mut Store,
    digest: &str,
    now: u64,
    command: &str,
    path: TerminalPath,
) -> Result<(), AppError> {
    // Kept past both ends of the window, so a token can't come back after its digest expires.
    if store.spend_token(digest, now, now + 2 * TOKEN_WINDOW_SECS + 1)? {
        Ok(())
    } else {
        Err(person_required(command, Refused::Spent, path))
    }
}

/// The person this OS user is.
pub fn os_person() -> Actor {
    let user =
        std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_else(|_| "unknown".into());
    Actor::Person { id: user, name: None }
}

/// Whether a command takes a `/dev/tty` confirmation in place of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPath {
    Takes,
    /// Anyone who controls a pty can confirm a prompt there, so the commands that change repos,
    /// pair a device or hand out the authority the others check take only a press or a canvas tab
    /// (DR-64, DR-75).
    Refused,
}

/// The terminal path for `command`, in a workspace that has a person key registered or not:
///
/// - `handoff start`, `handoff merge`, `handoff revert` and `person pair` change repos or pair a
///   device, and refuse it (DR-64).
/// - `person init` refuses it: a first registration proves it holds the key with a token under the
///   key on its stdin, and `--rekey` takes a token under the old key or a press (DR-75).
/// - `open` refuses it once a mod has registered a key; before that no mod can press, so a
///   confirmation that names it serves (DR-81).
/// - Every other person-only command or input takes it.
pub fn terminal_path(command: &str, key_registered: bool) -> TerminalPath {
    match command {
        "handoff start" | "handoff merge" | "handoff revert" | "person pair" | "person init" => {
            TerminalPath::Refused
        }
        "open" if key_registered => TerminalPath::Refused,
        _ => TerminalPath::Takes,
    }
}

/// Where a person confirms an action: the process's terminal, or none.
pub trait Terminal {
    /// Whether there is a terminal to ask.
    fn present(&mut self) -> bool;
    /// Ask `question`, a yes or no; `None` where there is no terminal.
    fn confirm(&mut self, question: &str) -> Option<bool>;
}

/// The process's own terminal: `/dev/tty`, and only when its stdin or its stdout is a terminal
/// too, so `planroom import - < rows` run at a terminal can ask. A process the Bash tool runs has
/// neither on a terminal, so an agent's unnamed write never reaches a prompt (A-3); one run under
/// a pty does, and only a confirmation there makes it a person's.
pub struct Tty;

impl Tty {
    fn attached() -> bool {
        std::io::stdin().is_terminal() || std::io::stdout().is_terminal()
    }
}

impl Terminal for Tty {
    fn present(&mut self) -> bool {
        Tty::attached() && OpenOptions::new().read(true).write(true).open("/dev/tty").is_ok()
    }

    fn confirm(&mut self, question: &str) -> Option<bool> {
        if !Tty::attached() {
            return None;
        }
        let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty").ok()?;
        write!(tty, "{question} [y/N] ").ok()?;
        tty.flush().ok()?;
        let mut answer = String::new();
        BufReader::new(tty).read_line(&mut answer).ok()?;
        Some(matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
    }
}

/// No terminal: for a caller that never prompts, such as a request over HTTP.
pub struct NoTerminal;

impl Terminal for NoTerminal {
    fn present(&mut self) -> bool {
        false
    }

    fn confirm(&mut self, _question: &str) -> Option<bool> {
        None
    }
}

/// Who acts, and the token digest to spend when the action commits: none for an agent or for a
/// terminal confirmation, which carry no token.
#[derive(Debug, Clone)]
pub struct Acting {
    pub actor: Actor,
    pub digest: Option<String>,
}

/// Resolve the caller of a person-only command or input as a person, or refuse it with
/// `person_required` naming why (DR-4, DR-64, DR-75):
///
/// - `PLANROOM_ACTOR` naming anyone but a person is never a person, even with a token beside it,
///   so a tool handler's call can't act as one.
/// - A token decides when there is one; `keys_allowed` are the public keys that may have signed it.
/// - `PLANROOM_ACTOR` naming a person without a token is refused: a claim isn't a key.
/// - With neither, a terminal confirmation that names the action serves where `path` takes it.
pub fn person(
    caller: &Caller,
    spec: &CommandSpec,
    bound: &Bound<'_>,
    keys_allowed: &[&VerifyingKey],
    now: u64,
    path: TerminalPath,
    terminal: &mut dyn Terminal,
) -> Result<Acting, AppError> {
    let command = spec.name();
    if caller.actor.as_deref().is_some_and(|a| !a.starts_with("person:")) {
        return Err(person_required(&command, Refused::NotAPerson, path));
    }
    if caller.token.is_some() {
        let digest = check(caller, spec, bound, keys_allowed, now)
            .map_err(|why| person_required(&command, why, path))?;
        return Ok(Acting { actor: os_person(), digest: Some(digest) });
    }
    if caller.actor.is_some() {
        return Err(person_required(&command, Refused::Missing, path));
    }
    if path == TerminalPath::Refused {
        let why = if terminal.present() { Refused::TerminalRefused } else { Refused::Missing };
        return Err(person_required(&command, why, path));
    }
    match terminal.confirm(&question(&command)) {
        None => Err(person_required(&command, Refused::Missing, path)),
        Some(false) => Err(person_required(&command, Refused::NotConfirmed, path)),
        Some(true) => Ok(Acting { actor: os_person(), digest: None }),
    }
}

/// Who writes to user-authored state, and the token digest to spend when it commits
/// (architecture.md, Actors; DR-4, DR-60). A person-only command resolves as [`person`] does.
/// Otherwise the agent `PLANROOM_ACTOR` names, a person's token, or a person who confirms on the
/// terminal; with none of them, `actor_required`, so every Bash agent names itself.
pub fn writer(
    caller: &Caller,
    spec: &CommandSpec,
    bound: &Bound<'_>,
    keys_allowed: &[&VerifyingKey],
    now: u64,
    terminal: &mut dyn Terminal,
) -> Result<Acting, AppError> {
    let command = spec.name();
    if spec.actor == ActorRule::PersonOnly {
        let path = terminal_path(&command, !keys_allowed.is_empty());
        return person(caller, spec, bound, keys_allowed, now, path, terminal);
    }
    match caller.actor.as_deref() {
        Some(named) if named.starts_with("person:") => {
            return person(caller, spec, bound, keys_allowed, now, TerminalPath::Takes, terminal);
        }
        Some(named) => {
            return agent(named)
                .map(|actor| Acting { actor, digest: None })
                .ok_or_else(|| AppError::invalid_argument("PLANROOM_ACTOR", "argument.actor"));
        }
        None => {}
    }
    if caller.token.is_some() {
        return person(caller, spec, bound, keys_allowed, now, TerminalPath::Takes, terminal);
    }
    match terminal.confirm(&question(&command)) {
        Some(true) => Ok(Acting { actor: os_person(), digest: None }),
        // No terminal, or nobody there confirmed: nobody stands behind the write.
        None | Some(false) => Err(actor_required(&command)),
    }
}

/// The terminal confirmation's question, naming the action and the person it acts as.
fn question(command: &str) -> String {
    let who = match os_person() {
        Actor::Person { id, .. } => id,
        _ => String::new(),
    };
    text::get("actor.confirm", &[("command", &command), ("person", &who)])
}

/// `PLANROOM_ACTOR=agent:<id>:<role>` as an actor; none for any other spelling.
pub fn agent(named: &str) -> Option<Actor> {
    let mut parts = named.split(':');
    let (Some("agent"), Some(id), Some(role), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let word = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b));
    (word(id) && word(role)).then(|| Actor::Agent { session: id.into(), role: role.into(), key: None })
}

/// `actor_required` for `command`: its hint shows how an agent names itself.
pub fn actor_required(command: &str) -> AppError {
    AppError::new(ErrorCode::ActorRequired, &[("command", &command)])
}

/// `person_required`, naming the command a person can run and why the call wasn't one. Its hint
/// names a way a person runs the command that exists in this build, along `path`, the terminal
/// path the refused call met, from [`person_path`].
pub fn person_required(command: &str, why: Refused, path: TerminalPath) -> AppError {
    AppError::new(ErrorCode::PersonRequired, &[("command", &command)])
        .with_hint(person_path(command, path))
        .details(json!({"command": command, "reason": why.as_str()}))
}

/// [`person_required`] for a leaf whose operands come from the call, such as `review`: its hint
/// names `line`, the exact command a person runs (see [`person_path_with`]), and its details the
/// card a person can pick, when the refused call was about one.
pub fn person_required_with(
    command: &str,
    why: Refused,
    path: TerminalPath,
    line: &str,
    card: Option<&planroom_contract::EventId>,
) -> AppError {
    let mut details = json!({"command": command, "reason": why.as_str()});
    if let Some(card) = card {
        details["card"] = json!(card.0);
    }
    person_required(command, why, path).with_hint(person_path_with(command, path, line)).details(details)
}

/// The ways a person runs `command` where its terminal path is `path` (see [`terminal_path`]):
///
/// - `person init`: the mod's own start in this workspace, which registers its key, and whose
///   mismatch pane's button runs `--rekey`.
/// - A leaf `/room` serves: `/room COMMAND`, which the mod runs as the person's own action.
/// - Where the terminal path is taken: the exact line from the person's own terminal as well,
///   which asks them to confirm it. Where it is refused, never the terminal.
/// - A leaf with neither: the action's button in the Planroom pane.
pub fn person_path(command: &str, path: TerminalPath) -> String {
    person_path_with(command, path, &terminal_line(command))
}

/// [`person_path`], naming `line` as the terminal command: for a leaf whose operands come from
/// the call, such as `edit`, the caller's own command line.
pub fn person_path_with(command: &str, path: TerminalPath, line: &str) -> String {
    let room = Command::SPECS.iter().any(|s| s.name() == command && s.room);
    let typed = format!("/room {command}");
    match (command, path) {
        ("person init", _) => text::get("hint.person_required_key", &[]),
        (_, TerminalPath::Refused) if room => text::get("hint.person_required_room", &[("line", &typed)]),
        (_, TerminalPath::Refused) => text::get("hint.person_required", &[]),
        (_, TerminalPath::Takes) if room => {
            text::get("hint.person_required_room_or_terminal", &[("room", &typed), ("line", &line)])
        }
        (_, TerminalPath::Takes) => text::get("hint.person_required_terminal", &[("line", &line)]),
    }
}

/// `e`, when it is a `person_required` refusal of `command`, with its hint naming `line` as the
/// terminal command (see [`person_path_with`]); any other error as it is.
pub fn naming_line(e: AppError, command: &str, path: TerminalPath, line: &str) -> AppError {
    if e.code == ErrorCode::PersonRequired { e.with_hint(person_path_with(command, path, line)) } else { e }
}

/// The exact line that runs `command` from a terminal: its path, and `-` for `import`, whose rows
/// come on stdin.
pub fn terminal_line(command: &str) -> String {
    match command {
        "import" => "planroom import -".to_owned(),
        _ => format!("planroom {command}"),
    }
}

/// base64url without padding (RFC 4648 §5), as `crypto.subtle` keys and signatures travel.
/// Hand-rolled: a dozen lines, where a crate would add a dependency the boundary lists don't allow.
pub fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.trim_end_matches('=');
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    (acc == 0).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_decodes_without_padding() {
        assert_eq!(base64url_decode("aGk").unwrap(), b"hi");
        assert_eq!(base64url_decode("_-8").unwrap(), [0xff, 0xef]);
        assert_eq!(base64url_decode("aGk=").unwrap(), b"hi");
        assert!(base64url_decode("a+b").is_none());
    }

    #[test]
    fn leaf_args_drop_the_path_and_json() {
        let caller = Caller {
            argv: ["person", "init", "--json", "--rekey"].map(String::from).to_vec(),
            ..Caller::default()
        };
        let spec = planroom_contract::Command::PersonInit(Default::default()).spec();
        assert_eq!(caller.leaf_args(spec).unwrap(), ["--rekey"]);
        let other = Caller { argv: vec!["map".into()], ..Caller::default() };
        assert!(other.leaf_args(spec).is_none());
    }

    #[test]
    fn only_a_well_formed_agent_is_an_agent() {
        assert_eq!(
            agent("agent:1a2b3c4d:chat"),
            Some(Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None })
        );
        for named in ["agent:1a2b3c4d", "agent::chat", "agent:a b:chat", "agent:a:b:c", "system:scan"] {
            assert_eq!(agent(named), None, "{named}");
        }
    }
}
