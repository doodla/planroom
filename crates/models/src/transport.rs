//! The transport seam: the one place a model request leaves the process. The adapters build an
//! [`HttpRequest`] that holds no credential, check its host against the allow-list, and hand the
//! credential to the transport beside it, so a request can be recorded as it stands.
//!
//! [`Live`] sends over `reqwest` with rustls and the ring provider, never OpenSSL: no redirects, `https` only, the
//! credential as a sensitive header. A transport failure keeps none of `reqwest`'s text, so no
//! URL, header or provider string reaches an error.

use std::fmt;
use std::time::Duration;

use crate::error::ModelError;

/// A credential: never printed, logged, recorded or put in an error.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential(String);

impl Credential {
    /// A credential from a non-empty value.
    pub fn new(value: &str) -> Option<Credential> {
        (!value.is_empty()).then(|| Credential(value.to_owned()))
    }

    /// The value, for the one header that carries it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential(..)")
    }
}

/// A request as sent, less its credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    /// Headers that never hold a credential.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A response as received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// Names lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Why a request got no response. It carries no text from the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    Timeout,
    Connect,
    Other,
}

/// What sends a request. Tests stand in for the network with one of their own.
pub trait Transport {
    /// Send `request`, with `credential` as its auth header when there is one.
    fn send(
        &self,
        request: &HttpRequest,
        credential: Option<&Credential>,
    ) -> Result<HttpResponse, TransportError>;
}

/// The live transport.
pub struct Live {
    client: reqwest::blocking::Client,
}

impl Live {
    /// A client with no redirects, `https` only, and `timeout` per call. The proxy comes from the
    /// environment, as for any program; it never stands in for a credential on its own, since
    /// the gate sends nothing without a live flag.
    pub fn new(timeout: Duration) -> Result<Live, ModelError> {
        // reqwest is built without a provider of its own, so it takes the process default: ring's
        // (DR-39). An earlier call has already installed it when this one finds one in place.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .timeout(timeout)
            .build()
            .map_err(|_| ModelError::internal())?;
        Ok(Live { client })
    }
}

impl Transport for Live {
    fn send(
        &self,
        request: &HttpRequest,
        credential: Option<&Credential>,
    ) -> Result<HttpResponse, TransportError> {
        let method =
            reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|_| TransportError::Other)?;
        let mut builder = self.client.request(method, &request.url).body(request.body.clone());
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(credential) = credential {
            let mut value =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", credential.expose()))
                    .map_err(|_| TransportError::Other)?;
            value.set_sensitive(true);
            builder = builder.header(reqwest::header::AUTHORIZATION, value);
        }
        let response = builder.send().map_err(|e| {
            if e.is_timeout() {
                TransportError::Timeout
            } else if e.is_connect() {
                TransportError::Connect
            } else {
                TransportError::Other
            }
        })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_ascii_lowercase(), v.to_owned())))
            .collect();
        let body = response.bytes().map_err(|_| TransportError::Other)?.to_vec();
        Ok(HttpResponse { status, headers, body })
    }
}
