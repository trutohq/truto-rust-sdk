//! Rust SDK for the [Truto](https://truto.one) REST API. Guide: <https://truto.one/docs/sdks/rust>.
//!
//! ```no_run
//! use futures_util::TryStreamExt;
//! use serde_json::json;
//!
//! # async fn run() -> Result<(), truto::Error> {
//! let client = truto::Client::new(std::env::var("TRUTO_API_TOKEN").unwrap());
//!
//! // Admin resources return typed models.
//! let accounts = client.integrated_accounts().list().collect_all().await?;
//!
//! // Data-plane APIs return dynamic records, paginated lazily.
//! let mut contacts = client
//!     .unified_api()
//!     .list("crm", "contacts", "integrated-account-id")
//!     .query(json!({ "limit": 50 }))
//!     .into_stream();
//! while let Some(contact) = contacts.try_next().await? {
//!     println!("{:?}", contact.get("name"));
//! }
//! # Ok(()) }
//! ```
//!
//! Every method returns a builder: chain `.query(..)`, `.timeout(..)`,
//! `.no_retry()` and friends, then `.await` it. List methods return a
//! [`Pager`].
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod client;
mod error;
mod pager;
mod query;
mod request;

pub mod models;
pub mod resources;

pub use client::{
    Client, ClientBuilder, RateLimitHook, RequestOptions, RetryHook, DEFAULT_BASE_URL,
    DEFAULT_MAX_RETRIES, DEFAULT_RETRY_AFTER, DEFAULT_TIMEOUT, RETRY_STATUS_CODES,
};
pub use error::{ApiError, Error, ErrorBody, Result};
pub use pager::{ListResponse, Page, Pager};
pub use query::{encode as encode_query, IntoQuery, Query};
pub use request::{RawResponse, Request};
pub use resources::Uninstall;

/// Re-exported so callers can build JSON without adding the dependency.
pub use serde_json;

/// A dynamic JSON object: the item type of the unified, proxy and custom
/// APIs, and of resources the spec does not describe.
pub type Record = serde_json::Map<String, serde_json::Value>;

/// The result of a delete on resources typed as [`Record`].
pub type DeleteResult = Record;

/// Decode a [`Record`] (or any JSON value) into your own type.
///
/// ```
/// #[derive(serde::Deserialize)]
/// struct Contact { id: String }
/// let mut record = truto::Record::new();
/// record.insert("id".into(), "c1".into());
/// let c: Contact = truto::decode(record).unwrap();
/// assert_eq!(c.id, "c1");
/// ```
pub fn decode<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> Result<T> {
    let value = serde_json::to_value(value).map_err(|e| Error::InvalidRequest(e.to_string()))?;
    serde_json::from_value(value).map_err(|e| Error::Decode {
        message: e.to_string(),
        source: Some(e),
    })
}

/// Compiles the README's examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
