//! URL and navigation networking for Lex.
//!
//! This crate owns browser-facing navigation policy while delegating HTTP,
//! TLS, connection pooling, and content decoding to mature infrastructure.
//! It has no dependency on Lex's UI or renderer.

mod cache;

use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use cache::{CacheEntry, ResponseCache};
pub use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{
    blocking::{Client, Response},
    header::{CONTENT_LENGTH, LOCATION},
    StatusCode,
};
use thiserror::Error;
use url::Url;

/// Default maximum decoded document size (16 MiB).
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
/// Default maximum number of followed redirects.
pub const DEFAULT_REDIRECT_LIMIT: usize = 10;

/// A validated network URL accepted by Lex navigation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LexUrl(Url);

impl LexUrl {
    /// Parses and validates an absolute HTTP or HTTPS URL.
    ///
    /// # Errors
    /// Returns [`NetworkError::MalformedUrl`] for malformed or non-absolute
    /// input and [`NetworkError::UnsupportedScheme`] for non-HTTP schemes.
    pub fn parse(input: &str) -> Result<Self, NetworkError> {
        let parsed = Url::parse(input).map_err(|error| NetworkError::MalformedUrl {
            input: input.into(),
            reason: error.to_string(),
        })?;
        Self::validate(parsed)
    }

    fn validate(mut url: Url) -> Result<Self, NetworkError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(NetworkError::UnsupportedScheme(url.scheme().into()));
        }
        if url.host_str().is_none() {
            return Err(NetworkError::MalformedUrl {
                input: url.into(),
                reason: "URL has no host".into(),
            });
        }
        // Fragments are document-local and must never be sent over HTTP or
        // create separate cache entries.
        url.set_fragment(None);
        Ok(Self(url))
    }

    /// Resolves a redirect location relative to this URL.
    ///
    /// # Errors
    /// Returns an error when the location is malformed or uses an unsupported
    /// scheme.
    pub fn join(&self, location: &str) -> Result<Self, NetworkError> {
        let joined = self
            .0
            .join(location)
            .map_err(|error| NetworkError::MalformedRedirect {
                location: location.into(),
                reason: error.to_string(),
            })?;
        Self::validate(joined)
    }

    /// Returns the normalized serialized URL.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the URL scheme.
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }
}

impl std::fmt::Display for LexUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Browser navigation request passed to the network layer.
#[derive(Clone, Debug)]
pub struct NavigationRequest {
    /// Initial validated URL.
    pub url: LexUrl,
    /// Additional request headers. Restricted headers remain controlled by
    /// the HTTP implementation.
    pub headers: HeaderMap,
    /// Maximum decoded body size.
    pub max_response_bytes: usize,
    /// Maximum redirects to follow.
    pub redirect_limit: usize,
    /// Whether a fresh cached response may be returned.
    pub use_cache: bool,
}

impl NavigationRequest {
    /// Creates a document GET navigation with safe defaults.
    #[must_use]
    pub fn get(url: LexUrl) -> Self {
        Self {
            url,
            headers: HeaderMap::new(),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            redirect_limit: DEFAULT_REDIRECT_LIMIT,
            use_cache: true,
        }
    }
}

/// Fully decoded navigation response ready for the future HTML parser.
#[derive(Clone, Debug)]
pub struct NavigationResponse {
    /// Final status after redirects.
    pub status: u16,
    /// Final response URL.
    pub final_url: LexUrl,
    /// Response headers supplied by the server.
    pub headers: HeaderMap,
    /// Decoded response body.
    pub body: Vec<u8>,
    /// URLs visited before the final response.
    pub redirect_chain: Vec<LexUrl>,
    /// Whether this result came from Lex's response cache.
    pub from_cache: bool,
}

/// Networking and navigation failures surfaced without panicking.
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("malformed URL `{input}`: {reason}")]
    MalformedUrl { input: String, reason: String },
    #[error("unsupported URL scheme `{0}`")]
    UnsupportedScheme(String),
    #[error("malformed redirect location `{location}`: {reason}")]
    MalformedRedirect { location: String, reason: String },
    #[error("redirect response did not contain a valid Location header")]
    MissingRedirectLocation,
    #[error("redirect limit of {limit} exceeded")]
    TooManyRedirects { limit: usize },
    #[error("redirect loop detected at `{0}`")]
    RedirectLoop(LexUrl),
    #[error("refused HTTPS to HTTP redirect from `{from}` to `{to}`")]
    InsecureRedirect { from: LexUrl, to: LexUrl },
    #[error("HTTP transport or TLS validation failed: {0}")]
    Transport(#[source] reqwest::Error),
    #[error("response declared {declared} bytes, exceeding limit {limit}")]
    OversizedResponse { declared: u64, limit: usize },
    #[error("decoded response exceeded limit {limit}")]
    ResponseLimitExceeded { limit: usize },
    #[error("failed while reading response: {0}")]
    ResponseRead(#[source] std::io::Error),
    #[error("network cache lock was poisoned")]
    CacheUnavailable,
}

/// Reusable HTTP client and Lex response cache.
///
/// The underlying client pools connections. Rustls validates certificates
/// against the platform-independent WebPKI root set; invalid certificates are
/// never accepted by configuration in this crate.
pub struct NetworkClient {
    client: Client,
    cache: Arc<Mutex<ResponseCache>>,
}

impl NetworkClient {
    /// Creates a network client with automatic gzip, Brotli, deflate, and zstd
    /// response decoding. Redirects are disabled in the HTTP crate so Lex can
    /// enforce its own redirect policy.
    ///
    /// # Errors
    /// Returns an error if the reusable HTTP client cannot be constructed.
    pub fn new() -> Result<Self, NetworkError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Lex/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(NetworkError::Transport)?;
        Ok(Self {
            client,
            cache: Arc::new(Mutex::new(ResponseCache::default())),
        })
    }

    /// Performs a synchronous document navigation.
    ///
    /// This API is intentionally synchronous for the first networking
    /// milestone. It must be called away from the Windows event thread.
    ///
    /// # Errors
    /// Returns structured errors for URL policy, redirect policy, transport,
    /// TLS, cache, and response size failures.
    pub fn navigate(
        &self,
        request: &NavigationRequest,
    ) -> Result<NavigationResponse, NetworkError> {
        if request.use_cache {
            if let Some(cached) = self
                .cache
                .lock()
                .map_err(|_| NetworkError::CacheUnavailable)?
                .get_fresh(&request.url, SystemTime::now())
            {
                return Ok(cached);
            }
        }

        let mut current = request.url.clone();
        let mut visited = HashSet::from([current.clone()]);
        let mut redirect_chain = Vec::new();

        loop {
            let response = self
                .client
                .get(current.as_str())
                .headers(request.headers.clone())
                .send()
                .map_err(NetworkError::Transport)?;

            if is_redirect(response.status()) {
                if redirect_chain.len() >= request.redirect_limit {
                    return Err(NetworkError::TooManyRedirects {
                        limit: request.redirect_limit,
                    });
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .ok_or(NetworkError::MissingRedirectLocation)?
                    .to_str()
                    .map_err(|_| NetworkError::MissingRedirectLocation)?;
                let next = current.join(location)?;
                if current.scheme() == "https" && next.scheme() == "http" {
                    return Err(NetworkError::InsecureRedirect {
                        from: current,
                        to: next,
                    });
                }
                if !visited.insert(next.clone()) {
                    return Err(NetworkError::RedirectLoop(next));
                }
                redirect_chain.push(current);
                current = next;
                continue;
            }

            return self.finish_response(
                response,
                current,
                redirect_chain,
                request.max_response_bytes,
                request.use_cache,
            );
        }
    }

    fn finish_response(
        &self,
        mut response: Response,
        final_url: LexUrl,
        redirect_chain: Vec<LexUrl>,
        limit: usize,
        cache_enabled: bool,
    ) -> Result<NavigationResponse, NetworkError> {
        if let Some(declared) = response.content_length() {
            let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
            if declared > limit_u64 {
                return Err(NetworkError::OversizedResponse { declared, limit });
            }
        }
        // Content-Length can be absent or describe compressed bytes, so the
        // decoded stream is independently bounded.
        let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
        let mut body = Vec::with_capacity(limit.min(64 * 1024));
        response
            .by_ref()
            .take(read_limit)
            .read_to_end(&mut body)
            .map_err(NetworkError::ResponseRead)?;
        if body.len() > limit {
            return Err(NetworkError::ResponseLimitExceeded { limit });
        }

        let result = NavigationResponse {
            status: response.status().as_u16(),
            final_url: final_url.clone(),
            headers: response.headers().clone(),
            body,
            redirect_chain,
            from_cache: false,
        };
        if cache_enabled {
            let entry = CacheEntry::from_response(&result, SystemTime::now());
            if let Some(entry) = entry {
                self.cache
                    .lock()
                    .map_err(|_| NetworkError::CacheUnavailable)?
                    .insert(final_url, entry);
            }
        }
        Ok(result)
    }
}

impl Default for NetworkClient {
    fn default() -> Self {
        Self::new().expect("the built-in HTTP client configuration is valid")
    }
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

#[cfg(test)]
mod tests {
    use super::{LexUrl, NetworkError};

    #[test]
    fn parses_and_normalizes_http_urls() {
        let url = LexUrl::parse("https://Example.COM:443/a/../b?q=1#part").unwrap();
        assert_eq!(url.as_str(), "https://example.com/b?q=1");
    }

    #[test]
    fn rejects_relative_and_privileged_urls() {
        assert!(matches!(LexUrl::parse("/relative"), Err(NetworkError::MalformedUrl { .. })));
        assert!(matches!(LexUrl::parse("file:///secret"), Err(NetworkError::UnsupportedScheme(_))));
    }

    #[test]
    fn resolves_relative_redirects() {
        let base = LexUrl::parse("https://example.test/one/page").unwrap();
        assert_eq!(base.join("../two").unwrap().as_str(), "https://example.test/two");
    }
}
