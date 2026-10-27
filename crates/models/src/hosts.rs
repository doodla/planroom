//! The host allow-list (architecture.md, What leaves the machine; DR-48): a credential goes only
//! to `ai-gateway.vercel.sh`, `api.typesafe.ai`, or the host of the `base_url` your user-level
//! config sets, which the engine's config loader takes from no other place. Every request is
//! checked against it before a transport sees the request or the credential.

use reqwest::Url;

use crate::error::ModelError;

/// The Vercel AI Gateway.
pub const GATEWAY_HOST: &str = "ai-gateway.vercel.sh";
/// TypeSafe's direct API.
pub const TYPESAFE_HOST: &str = "api.typesafe.ai";

/// The field a refused host names.
const FIELD: &str = "claude.base_url";

/// The hosts a credential may go to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowed {
    hosts: Vec<String>,
}

impl Allowed {
    /// The two built-in hosts and the host of `base_url`, which must be a plain `https` URL: no
    /// user info, query or fragment, so the URL can't hide another host or carry a secret.
    pub fn new(base_url: &str) -> Result<Allowed, ModelError> {
        let host = plain_https_host(base_url)?;
        let mut hosts = vec![GATEWAY_HOST.to_owned(), TYPESAFE_HOST.to_owned()];
        if !hosts.contains(&host) {
            hosts.push(host);
        }
        Ok(Allowed { hosts })
    }

    /// Refuse `url` unless it is a plain `https` URL to an allowed host, on the default port.
    pub fn check(&self, url: &str) -> Result<(), ModelError> {
        let host = plain_https_host(url)?;
        if self.hosts.contains(&host) {
            Ok(())
        } else {
            Err(ModelError::invalid_argument(FIELD, "credential_host"))
        }
    }
}

/// The lowercase host of a plain `https` URL on the default port.
fn plain_https_host(url: &str) -> Result<String, ModelError> {
    let refused = || ModelError::invalid_argument(FIELD, "credential_host");
    let parsed = Url::parse(url).map_err(|_| refused())?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.port().is_some()
    {
        return Err(refused());
    }
    match parsed.host_str() {
        Some(host) if !host.is_empty() => Ok(host.trim_end_matches('.').to_ascii_lowercase()),
        _ => Err(refused()),
    }
}
