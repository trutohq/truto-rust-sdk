//! Custom API: endpoints you define on an integration.

use reqwest::Method;
use serde::Serialize;

use super::path;
use crate::{Client, Record, Request};

/// The Custom API (`/custom/{path}`).
///
/// `path` may contain `/` (it is not escaped). Responses decode as
/// [`Record`]; for non-JSON responses (files, CSV, ...) finish the request
/// with [`.raw()`](crate::Request::raw) instead of awaiting it.
#[derive(Debug, Clone, Copy)]
pub struct CustomApi<'a> {
    client: &'a Client,
}

impl<'a> CustomApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    fn req(&self, method: Method, p: &str, integrated_account_id: &str) -> Request<Record> {
        let mut req = self.client.request(method, format!("custom/{}", path(p)));
        req.query_ref()
            .insert("integrated_account_id", integrated_account_id);
        req
    }

    /// `GET /custom/{path}`.
    pub fn get(&self, path: &str, integrated_account_id: &str) -> Request<Record> {
        self.req(Method::GET, path, integrated_account_id)
    }

    /// `POST /custom/{path}`. Pass `()` for no body.
    pub fn post(
        &self,
        path: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        self.req(Method::POST, path, integrated_account_id)
            .body(body)
    }

    /// `PUT /custom/{path}`. Pass `()` for no body.
    pub fn put(
        &self,
        path: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        self.req(Method::PUT, path, integrated_account_id)
            .body(body)
    }

    /// `PATCH /custom/{path}`. Pass `()` for no body.
    pub fn patch(
        &self,
        path: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        self.req(Method::PATCH, path, integrated_account_id)
            .body(body)
    }

    /// `DELETE /custom/{path}`.
    pub fn delete(&self, path: &str, integrated_account_id: &str) -> Request<Record> {
        self.req(Method::DELETE, path, integrated_account_id)
    }
}
