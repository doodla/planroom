//! Browser login (DR-29, DR-64): a one-time code, valid once for 60 s in this `serve` process,
//! exchanged for a random bearer token. A second try at a code revokes the token the first issued.
//! Every API request needs that token, an exact `Origin` and a matching `Host`. A browser sends no
//! `Origin` on a same-origin `GET`, so the stream's `GET` may prove its origin with
//! `Sec-Fetch-Site: same-origin` instead.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::time::{Duration, Instant};

use planroom_kernel::keys;

/// How long a login code stays good.
pub const CODE_LIFE: Duration = Duration::from_secs(60);

/// `n` random bytes from the system's generator, as lowercase hex.
pub fn random_hex(n: usize) -> std::io::Result<String> {
    let mut bytes = vec![0u8; n];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The sha256 a code is registered under; the code itself is never kept.
pub fn code_digest(code: &str) -> String {
    keys::sha256_plain_hex(code.as_bytes())
}

struct Code {
    expires: Instant,
    /// The token the first exchange issued.
    issued: Option<String>,
}

/// Login codes and the tokens they issued.
pub struct Auth {
    pub port: u16,
    codes: HashMap<String, Code>,
    tokens: HashSet<String>,
}

/// Why an exchange failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Unknown,
    Expired,
    UsedTwice,
}

impl Auth {
    pub fn new(port: u16) -> Auth {
        Auth { port, codes: HashMap::new(), tokens: HashSet::new() }
    }

    /// Register a code by its digest, good once for [`CODE_LIFE`].
    pub fn add_code(&mut self, digest: String) {
        self.codes.insert(digest, Code { expires: Instant::now() + CODE_LIFE, issued: None });
    }

    /// Exchange a code for a new bearer token.
    pub fn exchange(&mut self, code: &str) -> Result<String, Refused> {
        let entry = self.codes.get_mut(&code_digest(code)).ok_or(Refused::Unknown)?;
        if let Some(token) = entry.issued.take() {
            // Used twice: both tabs lose their sessions; a fresh code is one press away.
            self.tokens.remove(&token);
            return Err(Refused::UsedTwice);
        }
        if entry.expires < Instant::now() {
            return Err(Refused::Expired);
        }
        let token = random_hex(32).map_err(|_| Refused::Unknown)?;
        entry.issued = Some(token.clone());
        entry.expires = Instant::now();
        self.tokens.insert(token.clone());
        Ok(token)
    }

    /// Whether `Authorization: Bearer <token>` names a token this server issued.
    pub fn bearer_ok(&self, authorization: Option<&str>) -> bool {
        authorization.and_then(|h| h.strip_prefix("Bearer ")).is_some_and(|t| self.tokens.contains(t.trim()))
    }

    /// The one `Host` this server answers to: `127.0.0.1:PORT` or `localhost:PORT`.
    pub fn host_ok(&self, host: Option<&str>) -> bool {
        host.is_some_and(|h| {
            h == format!("127.0.0.1:{}", self.port) || h == format!("localhost:{}", self.port)
        })
    }

    /// An exact `Origin` of `http://127.0.0.1:PORT` or `http://localhost:PORT`; a missing one is refused.
    pub fn origin_ok(&self, origin: Option<&str>) -> bool {
        origin.is_some_and(|o| {
            o == format!("http://127.0.0.1:{}", self.port) || o == format!("http://localhost:{}", self.port)
        })
    }

    /// Whether a request comes from this server's own page: an exact `Origin`, or, on a `GET`,
    /// which a browser sends without one when it is same-origin (Fetch), no `Origin` and
    /// `Sec-Fetch-Site: same-origin`. A foreign `Origin`, and a `GET` marked `cross-site`,
    /// `same-site` or `none`, or with neither header, are refused.
    pub fn same_origin(&self, get: bool, origin: Option<&str>, sec_fetch_site: Option<&str>) -> bool {
        match origin {
            Some(_) => self.origin_ok(origin),
            None => get && sec_fetch_site == Some("same-origin"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_works_once_and_a_second_try_revokes_its_token() {
        let mut auth = Auth::new(41_873);
        auth.add_code(code_digest("c0de"));
        let token = auth.exchange("c0de").unwrap();
        assert!(auth.bearer_ok(Some(&format!("Bearer {token}"))));
        assert_eq!(auth.exchange("c0de"), Err(Refused::UsedTwice));
        assert!(!auth.bearer_ok(Some(&format!("Bearer {token}"))));
        assert_eq!(auth.exchange("c0de"), Err(Refused::Expired));
        assert_eq!(auth.exchange("other"), Err(Refused::Unknown));
    }

    #[test]
    fn origin_and_host_must_name_this_port_exactly() {
        let auth = Auth::new(41_873);
        assert!(auth.origin_ok(Some("http://127.0.0.1:41873")));
        assert!(auth.origin_ok(Some("http://localhost:41873")));
        assert!(!auth.origin_ok(Some("http://127.0.0.1:41874")));
        assert!(!auth.origin_ok(Some("https://127.0.0.1:41873")));
        assert!(!auth.origin_ok(None));
        assert!(auth.host_ok(Some("localhost:41873")));
        assert!(!auth.host_ok(Some("evil.test:41873")));
        assert_eq!(random_hex(16).unwrap().len(), 32);
    }

    #[test]
    fn a_get_without_origin_passes_only_as_same_origin() {
        let auth = Auth::new(41_873);
        assert!(auth.same_origin(true, None, Some("same-origin")));
        assert!(auth.same_origin(false, Some("http://127.0.0.1:41873"), Some("same-origin")));
        assert!(!auth.same_origin(false, None, Some("same-origin")), "only a GET may leave Origin out");
        assert!(!auth.same_origin(true, None, None));
        for site in ["cross-site", "same-site", "none"] {
            assert!(!auth.same_origin(true, None, Some(site)), "{site}");
        }
        assert!(!auth.same_origin(true, Some("http://evil.test"), Some("same-origin")));
    }
}
