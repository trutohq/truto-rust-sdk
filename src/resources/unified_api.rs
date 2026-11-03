//! Unified API: one schema across every integration in a category.

use reqwest::Method;
use serde::Serialize;

use super::seg;
use crate::models::{UnifiedApiMetaResponse, UnifiedMetaGetResponse};
use crate::{Client, Pager, Record, Request};

/// The Unified API (`/unified/{model}/{resource}`).
///
/// Items are [`Record`]s; call `.typed::<YourType>()` on any request or
/// pager to decode into your own type instead.
#[derive(Debug, Clone, Copy)]
pub struct UnifiedApi<'a> {
    client: &'a Client,
}

impl<'a> UnifiedApi<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    fn path(model: &str, resource: &str) -> String {
        format!("unified/{}/{}", seg(model), seg(resource))
    }

    fn req(&self, method: Method, path: String, integrated_account_id: &str) -> Request<Record> {
        let mut req = self.client.request(method, path);
        req.query_ref()
            .insert("integrated_account_id", integrated_account_id);
        req
    }

    /// List records (`GET /unified/{model}/{resource}`).
    pub fn list(&self, model: &str, resource: &str, integrated_account_id: &str) -> Pager<Record> {
        let mut pager = self.client.pager(Self::path(model, resource));
        pager
            .query_ref()
            .insert("integrated_account_id", integrated_account_id);
        pager
    }

    /// Get one record.
    pub fn get(
        &self,
        model: &str,
        resource: &str,
        id: &str,
        integrated_account_id: &str,
    ) -> Request<Record> {
        let path = format!("{}/{}", Self::path(model, resource), seg(id));
        self.req(Method::GET, path, integrated_account_id)
    }

    /// Create a record.
    pub fn create(
        &self,
        model: &str,
        resource: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        self.req(
            Method::POST,
            Self::path(model, resource),
            integrated_account_id,
        )
        .body(body)
    }

    /// Update a record (PATCH).
    pub fn update(
        &self,
        model: &str,
        resource: &str,
        id: &str,
        integrated_account_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        let path = format!("{}/{}", Self::path(model, resource), seg(id));
        self.req(Method::PATCH, path, integrated_account_id)
            .body(body)
    }

    /// Delete a record.
    pub fn delete(
        &self,
        model: &str,
        resource: &str,
        id: &str,
        integrated_account_id: &str,
    ) -> Request<Record> {
        let path = format!("{}/{}", Self::path(model, resource), seg(id));
        self.req(Method::DELETE, path, integrated_account_id)
    }

    /// Call a custom method (`POST /unified/{model}/{resource}/{method}`).
    /// Add a body with `.body(..)` if the method takes one.
    pub fn custom_method(
        &self,
        model: &str,
        resource: &str,
        method: &str,
        integrated_account_id: &str,
    ) -> Request<Record> {
        let path = format!("{}/{}", Self::path(model, resource), seg(method));
        self.req(Method::POST, path, integrated_account_id)
    }

    /// An integration's mapping metadata for one unified method
    /// (`GET /unified/{model}/{resource}/{integration_name}/meta/{method}`).
    pub fn meta(
        &self,
        model: &str,
        resource: &str,
        integration_name: &str,
        method: &str,
    ) -> Request<UnifiedApiMetaResponse> {
        self.client.request(
            Method::GET,
            format!(
                "{}/{}/meta/{}",
                Self::path(model, resource),
                seg(integration_name),
                seg(method)
            ),
        )
    }

    /// Resources and methods of a unified model that an integration supports
    /// (`GET /unified/meta/{model}/{integration_name}`).
    pub fn model_meta(
        &self,
        model: &str,
        integration_name: &str,
    ) -> Request<UnifiedMetaGetResponse> {
        self.client.request(
            Method::GET,
            format!("unified/meta/{}/{}", seg(model), seg(integration_name)),
        )
    }
}
