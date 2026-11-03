//! Integrations and unified models, with install/uninstall helpers.

use reqwest::Method;
use serde_json::json;

use super::{create, delete, get, list, resource, seg, update, Uninstall};
use crate::models::*;
use crate::Request;

resource!(
    /// Integrations.
    Integrations,
    "integration"
);

impl Integrations<'_> {
    list!(IntegrationResponse);
    get!(IntegrationResponse);
    create!(IntegrationCreateRequest => IntegrationResponse);
    update!(IntegrationUpdateRequest => IntegrationResponse);
    delete!(DeleteResponse);

    /// Install an integration in the token's environment
    /// (`POST /environment-integration`).
    pub fn install(
        &self,
        integration_id: &str,
        is_enabled: bool,
    ) -> Request<EnvironmentIntegrationResponse> {
        self.client
            .request(Method::POST, "environment-integration")
            .body(json!({ "integration_id": integration_id, "is_enabled": is_enabled }))
    }

    /// Uninstall an integration from the token's environment: lists the
    /// matching environment integrations and deletes each one.
    pub fn uninstall(
        &self,
        integration_id: &str,
    ) -> Uninstall<EnvironmentIntegrationDeleteResponse> {
        Uninstall::new(
            self.client,
            "environment-integration",
            "integration_id",
            integration_id,
        )
    }

    /// Tools an integration exposes (`GET /integration/{id}/tools`). Prefer
    /// [`capabilities`](Self::capabilities).
    pub fn tools(&self, id: &str) -> Request<IntegrationToolsResponse> {
        self.client
            .request(Method::GET, format!("{}/{}/tools", Self::PATH, seg(id)))
    }

    /// Unified APIs an integration supports (`GET /integration/{id}/unified-apis`).
    /// Prefer [`capabilities`](Self::capabilities).
    pub fn unified_apis(&self, id: &str) -> Request<IntegrationUnifiedApisResponse> {
        self.client.request(
            Method::GET,
            format!("{}/{}/unified-apis", Self::PATH, seg(id)),
        )
    }

    /// Resources and methods of an integration, by ID or name
    /// (`GET /integration/{id}/capabilities`).
    pub fn capabilities(&self, id: &str) -> Request<CapabilitiesResponse> {
        self.client.request(
            Method::GET,
            format!("{}/{}/capabilities", Self::PATH, seg(id)),
        )
    }
}

resource!(
    /// Unified models.
    UnifiedModels,
    "unified-model"
);

impl UnifiedModels<'_> {
    list!(UnifiedModelResponse);
    get!(UnifiedModelResponse);
    create!(UnifiedModelCreateRequest => UnifiedModelResponse);
    update!(UnifiedModelUpdateRequest => UnifiedModelResponse);
    delete!(UnifiedModelDeleteResponse);

    /// Install a unified model in the token's environment
    /// (`POST /environment-unified-model`).
    pub fn install(&self, unified_model_id: &str) -> Request<EnvironmentUnifiedModelResponse> {
        self.client
            .request(Method::POST, "environment-unified-model")
            .body(json!({ "unified_model_id": unified_model_id }))
    }

    /// Uninstall a unified model: lists the matching environment unified
    /// models and deletes each one.
    pub fn uninstall(
        &self,
        unified_model_id: &str,
    ) -> Uninstall<EnvironmentUnifiedModelDeleteResponse> {
        Uninstall::new(
            self.client,
            "environment-unified-model",
            "unified_model_id",
            unified_model_id,
        )
    }
}
