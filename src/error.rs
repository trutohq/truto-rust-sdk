//! The SDK's error type.

use reqwest::header::HeaderMap;
use serde_json::Value;

/// The body of an error response: parsed JSON when the server sent JSON,
/// otherwise the text as received.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorBody {
    /// The body was JSON.
    Json(Value),
    /// The body was not JSON (or was empty).
    Text(String),
}

impl ErrorBody {
    /// The JSON body, if the response was JSON.
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            ErrorBody::Json(v) => Some(v),
            ErrorBody::Text(_) => None,
        }
    }

    /// The body as text (JSON bodies are re-serialised).
    pub fn to_text(&self) -> String {
        match self {
            ErrorBody::Json(v) => v.to_string(),
            ErrorBody::Text(s) => s.clone(),
        }
    }
}

/// A non-2xx response from the Truto API.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// HTTP reason phrase, e.g. `Not Found`.
    pub status_text: String,
    /// The parsed response body.
    pub body: ErrorBody,
    /// The response headers.
    pub headers: HeaderMap,
}

/// Every error the SDK returns.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with a non-2xx status (after any retries).
    #[error("truto API error: {} {}", .0.status, .0.status_text)]
    Api(Box<ApiError>),

    /// The request did not complete within its timeout.
    #[error("truto request timed out")]
    Timeout,

    /// The request could not be sent or the response could not be read
    /// (DNS, connection refused, TLS, broken connection, ...).
    #[error("truto transport error: {0}")]
    Transport(#[source] reqwest::Error),

    /// A 2xx response body could not be decoded into the requested type.
    #[error("failed to decode truto response: {message}")]
    Decode {
        /// What went wrong.
        message: String,
        /// The underlying JSON error, when there is one.
        #[source]
        source: Option<serde_json::Error>,
    },

    /// The request could not be built (invalid body, query or configuration).
    #[error("invalid truto request: {0}")]
    InvalidRequest(String),
}

impl Error {
    /// The HTTP status of an [`Error::Api`] error (`408` for [`Error::Timeout`]).
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(e) => Some(e.status),
            Error::Timeout => Some(408),
            _ => None,
        }
    }

    /// The API error details, if this is an [`Error::Api`].
    pub fn api_error(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e),
            _ => None,
        }
    }

    /// The parsed error body, if this is an [`Error::Api`].
    pub fn body(&self) -> Option<&ErrorBody> {
        self.api_error().map(|e| &e.body)
    }

    /// `true` for a 404 response.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::Api(e) if e.status == 404)
    }

    /// `true` for a 429 response.
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Error::Api(e) if e.status == 429)
    }

    /// `true` for a 401 response.
    pub fn is_unauthorized(&self) -> bool {
        matches!(self, Error::Api(e) if e.status == 401)
    }

    /// `true` when the request timed out.
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout)
    }

    pub(crate) fn decode(message: impl Into<String>, source: Option<serde_json::Error>) -> Self {
        Error::Decode {
            message: message.into(),
            source,
        }
    }
}

/// Shorthand for `Result<T, truto::Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;
