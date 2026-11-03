//! Cursor pagination.

use std::collections::VecDeque;
use std::marker::PhantomData;

use futures_core::Stream;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Call, Client, RequestOptions};
use crate::error::Error;
use crate::query::Query;
use crate::request::request_settings;
use crate::Record;

/// One page of a list endpoint, as the API sends it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    /// The items on this page (some endpoints call this `results`).
    #[serde(alias = "results")]
    pub result: Vec<T>,
    /// Cursor for the next page; `None` or empty on the last page.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// One fetched page, from [`Pager::next_page`].
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    /// The items on this page.
    pub items: Vec<T>,
    /// Cursor for the next page (`None` on the last page).
    pub next_cursor: Option<String>,
}

/// A lazy pager over a cursor-paginated list endpoint.
///
/// Nothing is fetched until you call [`next_page`](Pager::next_page),
/// [`collect_all`](Pager::collect_all), [`first`](Pager::first) or iterate
/// [`into_stream`](Pager::into_stream). The query you pass is copied; each
/// page request adds `next_cursor` to that copy.
#[must_use = "a pager does nothing until you fetch a page"]
pub struct Pager<T = Record> {
    client: Client,
    path: String,
    query: Query,
    opts: RequestOptions,
    error: Option<Error>,
    cursor: Option<String>,
    started: bool,
    done: bool,
    _marker: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Pager<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pager")
            .field("path", &self.path)
            .field("query", &self.query)
            .field("cursor", &self.cursor)
            .field("done", &self.done)
            .finish()
    }
}

impl<T> Pager<T> {
    pub(crate) fn new(client: Client, path: String) -> Self {
        Self {
            client,
            path,
            query: Query::new(),
            opts: RequestOptions::default(),
            error: None,
            cursor: None,
            started: false,
            done: false,
            _marker: PhantomData,
        }
    }

    request_settings!();

    /// Decode items as `U` instead (e.g. your own struct for unified API records).
    pub fn typed<U>(self) -> Pager<U> {
        Pager {
            client: self.client,
            path: self.path,
            query: self.query,
            opts: self.opts,
            error: self.error,
            cursor: self.cursor,
            started: self.started,
            done: self.done,
            _marker: PhantomData,
        }
    }

    pub(crate) fn query_ref(&mut self) -> &mut Query {
        &mut self.query
    }

    /// `true` once the last page has been fetched.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

impl<T: DeserializeOwned> Pager<T> {
    /// Fetch the next page. Returns `Ok(None)` once every page has been read.
    pub async fn next_page(&mut self) -> Result<Option<Page<T>>, Error> {
        if self.done {
            return Ok(None);
        }
        if let Some(e) = self.error.take() {
            self.done = true;
            return Err(e);
        }
        let mut query = self.query.clone();
        if !self.started {
            // Only forward a caller-supplied cursor when it is non-empty.
            if query.get("next_cursor").is_some_and(is_blank) {
                query.remove("next_cursor");
            }
        } else if let Some(c) = &self.cursor {
            query.insert("next_cursor", c.clone());
        }
        self.started = true;

        let call = Call::new(Method::GET, self.path.clone(), &query, None);
        let page = match self.client.execute(&call, &self.opts).await {
            Ok(resp) => resp.decode::<Value>().and_then(parse_page::<T>),
            Err(e) => Err(e),
        };
        let page = match page {
            Ok(p) => p,
            Err(e) => {
                self.done = true;
                return Err(e);
            }
        };
        self.cursor = page.next_cursor.clone();
        if self.cursor.is_none() {
            self.done = true;
        }
        Ok(Some(page))
    }

    /// Fetch every page and return all items.
    pub async fn collect_all(mut self) -> Result<Vec<T>, Error> {
        let mut all = Vec::new();
        while let Some(page) = self.next_page().await? {
            all.extend(page.items);
        }
        Ok(all)
    }

    /// The first item, if any (skips empty pages that still carry a cursor).
    pub async fn first(mut self) -> Result<Option<T>, Error> {
        while let Some(page) = self.next_page().await? {
            if let Some(item) = page.items.into_iter().next() {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    /// Iterate over every item, fetching pages as needed. The stream ends
    /// after yielding an error. It is `Unpin`, so `TryStreamExt::try_next`
    /// works on it directly.
    pub fn into_stream(self) -> impl Stream<Item = Result<T, Error>> + Unpin {
        Box::pin(futures_util::stream::unfold(
            (self, VecDeque::new()),
            |(mut pager, mut buf)| async move {
                loop {
                    if let Some(item) = buf.pop_front() {
                        return Some((Ok(item), (pager, buf)));
                    }
                    match pager.next_page().await {
                        Ok(Some(page)) => buf.extend(page.items),
                        Ok(None) => return None,
                        Err(e) => return Some((Err(e), (pager, buf))),
                    }
                }
            },
        ))
    }
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn parse_page<T: DeserializeOwned>(value: Value) -> Result<Page<T>, Error> {
    let (items, cursor) = match value {
        Value::Object(mut obj) => {
            let items = obj
                .remove("result")
                .filter(|v| !v.is_null())
                .or_else(|| obj.remove("results"))
                .unwrap_or(Value::Array(Vec::new()));
            (items, obj.remove("next_cursor"))
        }
        // Tolerate endpoints that return a bare array.
        arr @ Value::Array(_) => (arr, None),
        other => {
            return Err(Error::decode(
                format!("expected a list response object, got {other}"),
                None,
            ))
        }
    };
    let items: Vec<T> = serde_json::from_value(items)
        .map_err(|e| Error::decode(format!("list items: {e}"), Some(e)))?;
    let next_cursor = match cursor {
        Some(Value::String(s)) if !s.is_empty() => Some(s),
        _ => None,
    };
    Ok(Page { items, next_cursor })
}
