//! Proxy API: the integration's native resources, passed through.

use reqwest::Method;
use serde::Serialize;

use super::seg;
use crate::{Client, Pager, Record, Request};

/// The Proxy API (`/proxy/{resource}`).
///
/// Items are [`Record`]s; call `.typed::<YourType>()` to decode into your own type.
#[derive(Debug, Clone, Copy)]
pub struct ProxyApi<'a> {
    client: &'a Client,
}

impl<'a> ProxyApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    fn req(&self, method: Method, path: String, integrated_account_id: &str) -> Request<Record> {
        let mut req = self.client.request(method, path);
        req.query_ref()
            .insert("integrated_account_id", integrated_account_id);
        req
    }

    /// List records (`GET /proxy/{resource}`).
    pub fn list(&self, resource: &str, integrated_account_id: &str) -> Pager<Record> {
        let mut pager = self.client.pager(format!("proxy/{}", seg(resource)));
        pager
            .query_ref()
            .insert("integrated_account_id", integrated_account_id);
        pager
    }

    /// Get one record.
    pub fn get(&self, resource: &str, id: &str, integrated_account_id: &str) -> Request<Record> {
        let path = format!("proxy/{}/{}", seg(resource), seg(id));
        self.req(Method::GET, path, integrated_account_id)
    }

    /// Create a record.
    pub fn create(
        &self,
        resource: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        let path = format!("proxy/{}", seg(resource));
        self.req(Method::POST, path, integrated_account_id)
            .body(body)
    }

    /// Update a record (PATCH).
    pub fn update(
        &self,
        resource: &str,
        id: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        let path = format!("proxy/{}/{}", seg(resource), seg(id));
        self.req(Method::PATCH, path, integrated_account_id)
            .body(body)
    }

    /// Delete a record.
    pub fn delete(&self, resource: &str, id: &str, integrated_account_id: &str) -> Request<Record> {
        let path = format!("proxy/{}/{}", seg(resource), seg(id));
        self.req(Method::DELETE, path, integrated_account_id)
    }

    /// Call a custom method (`POST /proxy/{resource}/{method}`). Add a body
    /// with `.body(..)` if the method takes one.
    pub fn custom_method(
        &self,
        resource: &str,
        method: &str,
        integrated_account_id: &str,
    ) -> Request<Record> {
        let path = format!("proxy/{}/{}", seg(resource), seg(method));
        self.req(Method::POST, path, integrated_account_id)
    }
}
