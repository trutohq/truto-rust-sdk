//! Integrated accounts (connected end-customer accounts).

use reqwest::Method;
use serde::Serialize;
use serde_json::json;

use super::{create, delete, get, list, resource, seg, update};
use crate::models::*;
use crate::{Pager, Record, Request};

resource!(
    /// Integrated accounts: your customers' connected accounts.
    IntegratedAccounts,
    "integrated-account"
);

impl IntegratedAccounts<'_> {
    list!(IntegratedAccountResponse);
    get!(IntegratedAccountResponse);
    create!(IntegratedAccountCreate => IntegratedAccountResponse);
    update!(IntegratedAccountUpdate => IntegratedAccountResponse);
    delete!(DeleteResponse);

    fn item(&self, id: &str, suffix: &str) -> String {
        format!("{}/{}/{}", Self::PATH, seg(id), suffix)
    }

    /// Replace the account's credentials (`PATCH /integrated-account/{id}/credentials`).
    pub fn update_credentials(
        &self,
        id: &str,
        body: &IntegratedAccountCredentialsUpdateRequest,
    ) -> Request<IntegratedAccountResponse> {
        self.client
            .request(Method::PATCH, self.item(id, "credentials"))
            .body(body)
    }

    /// Delete many accounts at once, by `ids` or `tenant_id`
    /// (`POST /integrated-account/bulk-delete`).
    pub fn bulk_delete(&self, body: impl Serialize) -> Request<Record> {
        self.client
            .request(Method::POST, "integrated-account/bulk-delete")
            .body(body)
    }

    /// Create an access token scoped to the account (`POST /integrated-account/token`).
    pub fn create_token(&self, id: &str) -> Request<IntegratedAccountTokenResponse> {
        self.client
            .request(Method::POST, "integrated-account/token")
            .body(json!({ "integrated_account_id": id }))
    }

    /// Create a token for the post-connect form (`POST /integrated-account/form-token`).
    pub fn create_form_token(&self, id: &str) -> Request<IntegratedAccountFormTokenResponse> {
        self.client
            .request(Method::POST, "integrated-account/form-token")
            .body(json!({ "integrated_account_id": id }))
    }

    /// Refresh the account's credentials now (`POST /integrated-account/refresh-credentials`).
    pub fn refresh_credentials(
        &self,
        id: &str,
    ) -> Request<IntegratedAccountRefreshCredentialsResponse> {
        self.client
            .request(Method::POST, "integrated-account/refresh-credentials")
            .body(json!({ "id": id }))
    }

    /// Re-run post-install actions (`POST /integrated-account/run-post-install-actions`).
    pub fn run_post_install_actions(
        &self,
        id: &str,
    ) -> Request<IntegratedAccountRunPostInstallActionsResponse> {
        self.client
            .request(Method::POST, "integrated-account/run-post-install-actions")
            .body(json!({ "id": id }))
    }

    /// Refresh cached unified API mappings (`POST /integrated-account/refresh-mapping-cache`).
    pub fn refresh_mapping_cache(
        &self,
        body: &IntegratedAccountRefreshMappingCacheRequest,
    ) -> Request<IntegratedAccountRefreshMappingCacheResponse> {
        self.client
            .request(Method::POST, "integrated-account/refresh-mapping-cache")
            .body(body)
    }

    /// Validate the account's connection now (`POST /integrated-account/{id}/validate`).
    pub fn validate(&self, id: &str) -> Request<IntegratedAccountValidateResponse> {
        self.client.request(Method::POST, self.item(id, "validate"))
    }

    /// Tools available on the account (`GET /integrated-account/{id}/tools`),
    /// optionally filtered by methods (`get`, `list`, `read`, `write`, `custom`, ...)
    /// and tags. Empty slices send no filter. Both are sent as arrays
    /// (`methods[0]=get&methods[1]=list`): the server does not split a
    /// comma-joined value.
    pub fn tools(&self, id: &str, methods: &[&str], tags: &[&str]) -> Request<ToolsResponseSchema> {
        let mut req = self.client.request(Method::GET, self.item(id, "tools"));
        if !methods.is_empty() {
            req.query_ref().insert("methods", methods.to_vec());
        }
        if !tags.is_empty() {
            req.query_ref().insert("tags", tags.to_vec());
        }
        req
    }

    /// Resources and methods available on the account (`GET /integrated-account/{id}/capabilities`).
    pub fn capabilities(&self, id: &str) -> Request<CapabilitiesResponse> {
        self.client
            .request(Method::GET, self.item(id, "capabilities"))
    }

    /// MCP servers created for the account (`GET /integrated-account/{id}/mcp`).
    pub fn list_mcp_servers(&self, id: &str) -> Pager<MCP> {
        self.client.pager(self.item(id, "mcp"))
    }

    /// One MCP server of the account.
    pub fn get_mcp_server(&self, id: &str, mcp_id: &str) -> Request<MCP> {
        self.client.request(
            Method::GET,
            format!("{}/{}", self.item(id, "mcp"), seg(mcp_id)),
        )
    }

    /// Create an MCP server for the account.
    pub fn create_mcp_server(
        &self,
        id: &str,
        body: &IntegratedAccountMcpCreateRequest,
    ) -> Request<MCPCreateResponse> {
        self.client
            .request(Method::POST, self.item(id, "mcp"))
            .body(body)
    }

    /// Update an MCP server of the account.
    pub fn update_mcp_server(
        &self,
        id: &str,
        mcp_id: &str,
        body: &IntegratedAccountMcpUpdateRequest,
    ) -> Request<MCP> {
        self.client
            .request(
                Method::PATCH,
                format!("{}/{}", self.item(id, "mcp"), seg(mcp_id)),
            )
            .body(body)
    }

    /// Delete an MCP server of the account.
    pub fn delete_mcp_server(&self, id: &str, mcp_id: &str) -> Request<DeleteResponse> {
        self.client.request(
            Method::DELETE,
            format!("{}/{}", self.item(id, "mcp"), seg(mcp_id)),
        )
    }
}
