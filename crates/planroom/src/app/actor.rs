//! Actor resolution for person-only commands: a person is a call that carries a valid
//! `PLANROOM_PERSON_TOKEN` (architecture.md, Actors; DR-64, DR-72, DR-75).
//!
//! The token is `<at>.<sig>`: `at` the Unix time in seconds, and `sig` the base64url of the raw
//! `r ‖ s` ECDSA P-256 SHA-256 signature, by the person key, over
//! `kernel::keys::person_token_message(command, args, idem, base, at)`. The engine refuses a token
//! older than 30 s (or more than 30 s ahead), one for another command or other arguments, and one
//! already spent, and records each spent token's digest in `meta.spent_tokens` until it expires.

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use planroom_contract::{Actor, Client, CommandSpec, ErrorCode, Surface};
use planroom_kernel::keys;
use planroom_store::Store;
use serde_json::json;

use crate::app::error::AppError;

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

/// Why a token was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Missing,
    Malformed,
    Stale,
    BadSignature,
    Spent,
    /// `PLANROOM_ACTOR` named someone other than a person with no token.
    NotAPerson,
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
        return Err(if caller.actor.as_deref().is_some_and(|a| !a.starts_with("person:")) {
            Refused::NotAPerson
        } else {
            Refused::Missing
        });
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

/// Spend a checked token's digest; a second use is refused.
pub fn spend(store: &mut Store, digest: &str, now: u64, command: &str) -> Result<(), AppError> {
    // Kept past both ends of the window, so a token can't come back after its digest expires.
    if store.spend_token(digest, now, now + 2 * TOKEN_WINDOW_SECS + 1)? {
        Ok(())
    } else {
        Err(person_required(command, Refused::Spent))
    }
}

/// The person this OS user is.
pub fn person() -> Actor {
    let user =
        std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_else(|_| "unknown".into());
    Actor::Person { id: user, name: None }
}

/// `person_required`, naming the command a person can run and why the call wasn't one.
pub fn person_required(command: &str, why: Refused) -> AppError {
    AppError::new(ErrorCode::PersonRequired, &[("command", &command)])
        .details(json!({"command": command, "reason": why.as_str()}))
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
}
