//! Request builders.

use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;

use reqwest::header::HeaderMap;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::client::{content_type, Call, Client, RequestOptions};
use crate::error::Error;
use crate::query::Query;
use crate::Record;

/// Adds the per-request settings shared by [`Request`], [`crate::Pager`] and
/// the other builders. Requires fields `query: Query`, `opts: RequestOptions`
/// and `error: Option<Error>`.
macro_rules! request_settings {
    () => {
        /// Add query parameters (a [`Query`](crate::Query) or a JSON object).
        /// Repeated calls merge; later keys win.
        pub fn query(mut self, query: impl $crate::query::IntoQuery) -> Self {
            match query.into_query() {
                Ok(map) => self.query.merge(map),
                Err(e) => {
                    self.error.get_or_insert(e);
                }
            }
            self
        }

        /// Add a header to the request.
        pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
            self.opts.headers.push((name.into(), value.into()));
            self
        }

        /// Override the per-attempt timeout.
        pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
            self.opts.timeout = Some(timeout);
            self
        }

        /// Override the maximum number of retries.
        pub fn max_retries(mut self, n: u32) -> Self {
            self.opts.max_retries = Some(n);
            self
        }

        /// Disable retries for this request.
        pub fn no_retry(mut self) -> Self {
            self.opts.no_retry = true;
            self
        }

        /// Also retry on these statuses.
        pub fn retry_status_codes(mut self, codes: impl IntoIterator<Item = u16>) -> Self {
            self.opts.retry_status_codes.extend(codes);
            self
        }

        /// Called before every retry with `(retry_count, status)`.
        pub fn on_retry(mut self, hook: impl Fn(u32, Option<u16>) + Send + Sync + 'static) -> Self {
            self.opts.on_retry = Some(std::sync::Arc::new(hook));
            self
        }

        /// Called on a 429 before waiting, with `(retry_after_seconds, retry_count)`.
        pub fn on_rate_limited(mut self, hook: impl Fn(u64, u32) + Send + Sync + 'static) -> Self {
            self.opts.on_rate_limited = Some(std::sync::Arc::new(hook));
            self
        }

        /// Replace all per-request settings with `options`.
        pub fn options(mut self, options: $crate::RequestOptions) -> Self {
            self.opts = options;
            self
        }
    };
}
pub(crate) use request_settings;

/// A single API call. Configure it with the chainable methods, then `.await`
/// it (or call [`Request::send`]) to get the decoded response.
///
/// `T` is the response type: a generated model, [`Record`], or any type you
/// choose with [`Request::typed`].
#[must_use = "a request does nothing until it is awaited"]
pub struct Request<T = Record> {
    client: Client,
    method: Method,
    path: String,
    query: Query,
    body: Option<Value>,
    opts: RequestOptions,
    error: Option<Error>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("body", &self.body)
            .field("options", &self.opts)
            .finish()
    }
}

impl<T> Request<T> {
    pub(crate) fn new(client: Client, method: Method, path: String) -> Self {
        Self {
            client,
            method,
            path,
            query: Query::new(),
            body: None,
            opts: RequestOptions::default(),
            error: None,
            _marker: PhantomData,
        }
    }

    request_settings!();

    /// Set the JSON body. A value that serialises to `null` (such as `()`)
    /// sends no body.
    pub fn body(mut self, body: impl Serialize) -> Self {
        match serde_json::to_value(body) {
            Ok(Value::Null) => self.body = None,
            Ok(v) => self.body = Some(v),
            Err(e) => {
                self.error
                    .get_or_insert(Error::InvalidRequest(format!("cannot serialise body: {e}")));
            }
        }
        self
    }

    /// Decode the response as `U` instead (e.g. your own struct for a unified
    /// API record).
    pub fn typed<U>(self) -> Request<U> {
        Request {
            client: self.client,
            method: self.method,
            path: self.path,
            query: self.query,
            body: self.body,
            opts: self.opts,
            error: self.error,
            _marker: PhantomData,
        }
    }

    pub(crate) fn query_ref(&mut self) -> &mut Query {
        &mut self.query
    }

    async fn execute(self) -> Result<crate::client::Response, Error> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let call = Call::new(self.method, self.path, &self.query, self.body);
        self.client.execute(&call, &self.opts).await
    }

    /// Send the request and return the raw response, whatever its content
    /// type (use this for files and other non-JSON bodies).
    pub async fn raw(self) -> Result<RawResponse, Error> {
        let resp = self.execute().await?;
        Ok(RawResponse {
            status: resp.status.as_u16(),
            headers: resp.headers,
            body: resp.body,
        })
    }
}

impl<T: DeserializeOwned> Request<T> {
    /// Send the request and decode the response. Same as `.await`.
    pub async fn send(self) -> Result<T, Error> {
        self.execute().await?.decode()
    }
}

impl<T: DeserializeOwned + Send + 'static> IntoFuture for Request<T> {
    type Output = Result<T, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// An undecoded successful response, from [`Request::raw`].
#[derive(Debug, Clone)]
pub struct RawResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: HeaderMap,
    /// Response body bytes.
    pub body: Vec<u8>,
}

impl RawResponse {
    /// The `Content-Type` header, if any.
    pub fn content_type(&self) -> Option<&str> {
        content_type(&self.headers)
    }

    /// The body as text (invalid UTF-8 is replaced).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Decode the body as JSON.
    pub fn json<U: DeserializeOwned>(&self) -> Result<U, Error> {
        serde_json::from_slice(&self.body).map_err(|e| Error::decode(e.to_string(), Some(e)))
    }
}
