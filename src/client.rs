//! The HTTP client: configuration, retries, and response handling.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER,
    USER_AGENT,
};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::{ApiError, Error, ErrorBody};
use crate::query::{self, Query};

/// Default API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.truto.one";
/// Default per-attempt timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// Default number of retries after the first attempt.
pub const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default wait after a 429 without a usable `Retry-After` header.
pub const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(10);
/// Statuses that are always retried (unless retries are disabled).
pub const RETRY_STATUS_CODES: [u16; 7] = [408, 409, 425, 429, 502, 503, 504];

const SDK_USER_AGENT: &str = concat!("truto-rust-sdk/", env!("CARGO_PKG_VERSION"));
const BACKOFF_BASE: Duration = Duration::from_millis(500);
const BACKOFF_CAP: Duration = Duration::from_secs(10);

/// Called before each retry with `(retry_count, status)`. `retry_count` starts
/// at 1; `status` is `None` when the retry follows a connection error.
pub type RetryHook = Arc<dyn Fn(u32, Option<u16>) + Send + Sync>;
/// Called on a 429 before waiting, with `(retry_after_seconds, retry_count)`.
/// `retry_count` is the number of retries made so far.
pub type RateLimitHook = Arc<dyn Fn(u64, u32) + Send + Sync>;

/// Per-request overrides. Every request builder exposes the same settings as
/// chainable methods (`.timeout(..)`, `.no_retry()`, ...); this struct is for
/// reusing one set of overrides across calls via `.options(..)`.
#[derive(Clone, Default)]
pub struct RequestOptions {
    /// Extra headers sent with the request.
    pub headers: Vec<(String, String)>,
    /// Per-attempt timeout (client default when `None`).
    pub timeout: Option<Duration>,
    /// Maximum retries (client default when `None`).
    pub max_retries: Option<u32>,
    /// Disable retries entirely.
    pub no_retry: bool,
    /// Statuses to retry in addition to [`RETRY_STATUS_CODES`].
    pub retry_status_codes: Vec<u16>,
    /// Retry hook (overrides the client's).
    pub on_retry: Option<RetryHook>,
    /// Rate-limit hook (overrides the client's).
    pub on_rate_limited: Option<RateLimitHook>,
}

impl RequestOptions {
    /// Empty overrides.
    pub fn new() -> Self {
        Self::default()
    }
}

impl fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestOptions")
            .field("headers", &self.headers)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("no_retry", &self.no_retry)
            .field("retry_status_codes", &self.retry_status_codes)
            .field("on_retry", &self.on_retry.is_some())
            .field("on_rate_limited", &self.on_rate_limited.is_some())
            .finish()
    }
}

struct Inner {
    http: reqwest::Client,
    token: String,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
    default_retry_after: Duration,
    on_retry: Option<RetryHook>,
    on_rate_limited: Option<RateLimitHook>,
}

/// A Truto API client. Cheap to clone: clones share one connection pool.
///
/// ```no_run
/// # async fn run() -> Result<(), truto::Error> {
/// let client = truto::Client::new("my-api-token");
/// let accounts = client.integrated_accounts().list().collect_all().await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base_url)
            .field("timeout", &self.inner.timeout)
            .field("max_retries", &self.inner.max_retries)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client with default settings.
    ///
    /// # Panics
    ///
    /// Panics if the TLS backend cannot be initialised (as `reqwest::Client::new` does).
    /// Use [`Client::builder`] to handle that as an error instead.
    pub fn new(token: impl Into<String>) -> Self {
        Self::builder()
            .token(token)
            .build()
            .expect("failed to build the truto client")
    }

    /// A builder for a configured client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The base URL requests are sent to.
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    pub(crate) fn request<T>(&self, method: Method, path: impl Into<String>) -> crate::Request<T> {
        crate::Request::new(self.clone(), method, path.into())
    }

    pub(crate) fn pager<T>(&self, path: impl Into<String>) -> crate::Pager<T> {
        crate::Pager::new(self.clone(), path.into())
    }

    fn url(&self, path: &str, query: &Map<String, Value>) -> String {
        let mut url = format!("{}/{}", self.inner.base_url, path.trim_start_matches('/'));
        let qs = query::encode(query);
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs);
        }
        url
    }

    /// Send a request with retries and return the successful response.
    pub(crate) async fn execute(
        &self,
        call: &Call,
        opts: &RequestOptions,
    ) -> Result<Response, Error> {
        let inner = &self.inner;
        let url = self.url(&call.path, &call.query);
        let body = match &call.body {
            Some(v) => {
                Some(serde_json::to_vec(v).map_err(|e| Error::InvalidRequest(e.to_string()))?)
            }
            None => None,
        };
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        for (k, v) in &opts.headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| Error::InvalidRequest(format!("invalid header name {k:?}: {e}")))?;
            let value = HeaderValue::from_str(v).map_err(|e| {
                Error::InvalidRequest(format!("invalid header value for {k:?}: {e}"))
            })?;
            headers.insert(name, value);
        }
        let auth = HeaderValue::from_str(&format!("Bearer {}", inner.token))
            .map_err(|e| Error::InvalidRequest(format!("invalid token: {e}")))?;
        headers.insert(AUTHORIZATION, auth);
        headers.insert(USER_AGENT, HeaderValue::from_static(SDK_USER_AGENT));
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }

        let timeout = opts.timeout.unwrap_or(inner.timeout);
        let max_retries = if opts.no_retry {
            0
        } else {
            opts.max_retries.unwrap_or(inner.max_retries)
        };
        let on_retry = opts.on_retry.as_ref().or(inner.on_retry.as_ref());
        let on_rate_limited = opts
            .on_rate_limited
            .as_ref()
            .or(inner.on_rate_limited.as_ref());
        let retryable =
            |s: u16| RETRY_STATUS_CODES.contains(&s) || opts.retry_status_codes.contains(&s);

        let mut retry_count: u32 = 0;
        loop {
            let mut req = inner
                .http
                .request(call.method.clone(), &url)
                .headers(headers.clone());
            if let Some(b) = &body {
                req = req.body(b.clone());
            }

            let attempt = async {
                let resp = req.send().await?;
                let status = resp.status();
                let headers = resp.headers().clone();
                let body = resp.bytes().await?.to_vec();
                Ok::<_, reqwest::Error>(Response {
                    status,
                    headers,
                    body,
                })
            };

            let resp = match tokio::time::timeout(timeout, attempt).await {
                Err(_) => return Err(Error::Timeout),
                Ok(Err(e)) if e.is_timeout() => return Err(Error::Timeout),
                Ok(Err(e)) => {
                    if e.is_connect() && retry_count < max_retries {
                        tokio::time::sleep(backoff(retry_count)).await;
                        retry_count += 1;
                        if let Some(hook) = on_retry {
                            hook(retry_count, None);
                        }
                        continue;
                    }
                    return Err(Error::Transport(e));
                }
                Ok(Ok(resp)) => resp,
            };

            if resp.status.is_success() {
                return Ok(resp);
            }

            let status = resp.status.as_u16();
            if retryable(status) && retry_count < max_retries {
                if status == 429 {
                    let wait = retry_after(&resp.headers).unwrap_or(inner.default_retry_after);
                    if let Some(hook) = on_rate_limited {
                        hook(wait.as_secs(), retry_count);
                    }
                    tokio::time::sleep(wait).await;
                } else {
                    tokio::time::sleep(backoff(retry_count)).await;
                }
                retry_count += 1;
                if let Some(hook) = on_retry {
                    hook(retry_count, Some(status));
                }
                continue;
            }

            return Err(api_error(resp));
        }
    }
}

/// Builder for [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    token: Option<String>,
    base_url: Option<String>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    default_retry_after: Option<Duration>,
    http: Option<reqwest::Client>,
    on_retry: Option<RetryHook>,
    on_rate_limited: Option<RateLimitHook>,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("default_retry_after", &self.default_retry_after)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// The API token (required).
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// The API base URL (default `https://api.truto.one`).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Per-attempt timeout (default 30s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Maximum retries after the first attempt (default 3).
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = Some(n);
        self
    }

    /// Wait after a 429 that has no usable `Retry-After` header (default 10s).
    pub fn default_retry_after(mut self, wait: Duration) -> Self {
        self.default_retry_after = Some(wait);
        self
    }

    /// Use your own `reqwest::Client` (proxies, custom TLS, ...).
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Called before every retry with `(retry_count, status)`.
    pub fn on_retry(mut self, hook: impl Fn(u32, Option<u16>) + Send + Sync + 'static) -> Self {
        self.on_retry = Some(Arc::new(hook));
        self
    }

    /// Called on a 429 before waiting, with `(retry_after_seconds, retry_count)`.
    pub fn on_rate_limited(mut self, hook: impl Fn(u64, u32) + Send + Sync + 'static) -> Self {
        self.on_rate_limited = Some(Arc::new(hook));
        self
    }

    /// Build the client. Fails if no token was set or the HTTP client cannot be created.
    pub fn build(self) -> Result<Client, Error> {
        let token = self
            .token
            .filter(|t| !t.is_empty())
            .ok_or_else(|| Error::InvalidRequest("an API token is required".into()))?;
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .build()
                .map_err(Error::Transport)?,
        };
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                token,
                base_url,
                timeout: self.timeout.unwrap_or(DEFAULT_TIMEOUT),
                max_retries: self.max_retries.unwrap_or(DEFAULT_MAX_RETRIES),
                default_retry_after: self.default_retry_after.unwrap_or(DEFAULT_RETRY_AFTER),
                on_retry: self.on_retry,
                on_rate_limited: self.on_rate_limited,
            }),
        })
    }
}

/// One HTTP call, before retries.
#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub method: Method,
    pub path: String,
    pub query: Map<String, Value>,
    pub body: Option<Value>,
}

impl Call {
    pub(crate) fn new(method: Method, path: String, query: &Query, body: Option<Value>) -> Self {
        Self {
            method,
            path,
            query: query.as_map().clone(),
            body,
        }
    }
}

/// A successful response.
pub(crate) struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    fn is_empty(&self) -> bool {
        self.status == StatusCode::NO_CONTENT || self.body.iter().all(u8::is_ascii_whitespace)
    }

    fn is_json(&self) -> bool {
        content_type(&self.headers).is_some_and(|c| c.contains("json"))
    }

    /// Decode the body into `T` (see the contract's response rules).
    pub(crate) fn decode<T: DeserializeOwned>(&self) -> Result<T, Error> {
        if self.is_empty() {
            return decode_empty();
        }
        if !self.is_json() {
            return Err(Error::decode(
                format!(
                    "expected a JSON response, got content-type {:?}",
                    content_type(&self.headers).unwrap_or("")
                ),
                None,
            ));
        }
        serde_json::from_slice(&self.body).map_err(|e| Error::decode(e.to_string(), Some(e)))
    }
}

/// Decode an empty response: `null` for types that accept it (unit, `Option`,
/// `Value`), otherwise `{}` (records and models).
pub(crate) fn decode_empty<T: DeserializeOwned>() -> Result<T, Error> {
    serde_json::from_value(Value::Null)
        .or_else(|_| serde_json::from_value(Value::Object(Map::new())))
        .map_err(|e| Error::decode(format!("empty response: {e}"), Some(e)))
}

pub(crate) fn content_type(headers: &HeaderMap) -> Option<&str> {
    headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok())
}

fn api_error(resp: Response) -> Error {
    let text = String::from_utf8_lossy(&resp.body).into_owned();
    let body = if resp.is_json() {
        match serde_json::from_slice(&resp.body) {
            Ok(v) => ErrorBody::Json(v),
            Err(_) => ErrorBody::Text(text),
        }
    } else {
        ErrorBody::Text(text)
    };
    Error::Api(Box::new(ApiError {
        status: resp.status.as_u16(),
        status_text: resp.status.canonical_reason().unwrap_or("").to_string(),
        body,
        headers: resp.headers,
    }))
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let v = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    v.parse::<u64>().ok().map(Duration::from_secs)
}

/// Exponential backoff with full jitter: uniform in `[0, min(cap, base * 2^n)]`.
fn backoff(retry_count: u32) -> Duration {
    let ceiling = BACKOFF_BASE
        .saturating_mul(1u32 << retry_count.min(16))
        .min(BACKOFF_CAP);
    let millis = ceiling.as_millis() as u64;
    Duration::from_millis(random_u64() % (millis + 1))
}

fn random_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_stays_within_bounds() {
        for n in 0..20 {
            let d = backoff(n);
            assert!(d <= BACKOFF_CAP);
            if n == 0 {
                assert!(d <= BACKOFF_BASE);
            }
        }
    }

    #[test]
    fn builder_requires_token_and_trims_base_url() {
        assert!(Client::builder().build().is_err());
        let c = Client::builder()
            .token("t")
            .base_url("http://localhost:1/")
            .build()
            .unwrap();
        assert_eq!(c.base_url(), "http://localhost:1");
    }
}
