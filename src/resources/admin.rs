//! Admin resources: tokens, environments, tenants, sync jobs, webhooks, ...

use reqwest::Method;
use serde::Serialize;

use super::{create, delete, get, list, resource, schedule, seg, update};
use crate::models::*;
use crate::{Pager, Record, Request};

resource!(
    /// API tokens.
    ApiTokens,
    "api-token"
);
impl ApiTokens<'_> {
    list!(ApiTokenResponse);
    get!(ApiTokenResponse);
}

resource!(
    /// Environments.
    Environments,
    "environment"
);
impl Environments<'_> {
    list!(EnvironmentResponse);
    get!(EnvironmentResponse);
    update!(any => EnvironmentResponse);
}

resource!(
    /// Integration categories.
    Categories,
    "category"
);
impl Categories<'_> {
    list!(CategorySchema);
}

resource!(
    /// Teams.
    Teams,
    "team"
);
impl Teams<'_> {
    list!(Record);
    get!(Record);
    update!(any => Record);
}

resource!(
    /// Users.
    Users,
    "user"
);
impl Users<'_> {
    list!(Record);
    get!(Record);
}

resource!(
    /// Tenants.
    Tenants,
    "tenant"
);
impl Tenants<'_> {
    list!(TenantResponse);
    get!(TenantResponse);
    create!(TenantCreateRequest => TenantResponse);
    update!(TenantUpdateRequest => TenantResponse);
    delete!(TenantResponse);

    /// Create many tenants at once (`POST /tenant/bulk`).
    pub fn bulk_create(&self, body: &TenantBulkRequest) -> Request<TenantBulkCreateResponse> {
        self.client.request(Method::POST, "tenant/bulk").body(body)
    }
}

resource!(
    /// Link tokens for the connection flow.
    LinkTokens,
    "link-token"
);
impl LinkTokens<'_> {
    create!(LinkTokenRequest => LinkToken);
}

resource!(
    /// Session tokens.
    SessionTokens,
    "session-token"
);
impl SessionTokens<'_> {
    create!(SessionTokenRequest => SessionTokenResponse);

    /// Revoke a session token (`DELETE /session-token` with a JSON body).
    pub fn revoke(&self, body: &SessionTokenDeleteRequest) -> Request<SessionTokenDeleteResponse> {
        self.client.request(Method::DELETE, Self::PATH).body(body)
    }

    /// Renew the calling session token (`POST /session-token/renew`).
    pub fn renew(&self) -> Request<SessionTokenRenewResponse> {
        self.client.request(Method::POST, "session-token/renew")
    }

    /// Describe the calling session token (`GET /session-token/me`).
    pub fn me(&self) -> Request<SessionTokenMeResponse> {
        self.client.request(Method::GET, "session-token/me")
    }
}

resource!(
    /// Sandbox integrated accounts.
    SandboxIntegratedAccounts,
    "sandbox-integrated-account"
);
impl SandboxIntegratedAccounts<'_> {
    list!(Record);
    get!(Record);
    create!(any => Record);
    delete!(Record);
}

resource!(
    /// Integrations installed in an environment.
    EnvironmentIntegrations,
    "environment-integration"
);
impl EnvironmentIntegrations<'_> {
    list!(EnvironmentIntegrationResponse);
    get!(EnvironmentIntegrationResponse);
    create!(EnvironmentIntegrationCreateRequest => EnvironmentIntegrationResponse);
    update!(EnvironmentIntegrationUpdateRequest => EnvironmentIntegrationResponse);
    delete!(EnvironmentIntegrationDeleteResponse);
}

resource!(
    /// Unified models installed in an environment.
    EnvironmentUnifiedModels,
    "environment-unified-model"
);
impl EnvironmentUnifiedModels<'_> {
    list!(EnvironmentUnifiedModelResponse);
    get!(EnvironmentUnifiedModelResponse);
    create!(EnvironmentUnifiedModelCreateRequest => EnvironmentUnifiedModelResponse);
    update!(EnvironmentUnifiedModelUpdateRequest => EnvironmentUnifiedModelResponse);
    delete!(EnvironmentUnifiedModelDeleteResponse);
}

resource!(
    /// Unified model resource methods.
    UnifiedModelResourceMethods,
    "unified-model-resource-method"
);
impl UnifiedModelResourceMethods<'_> {
    list!(UnifiedModelResourceMethodRowResponse);
    get!(UnifiedModelResourceMethodRowResponse);
    create!(UnifiedModelResourceMethodCreateRequest => UnifiedModelResourceMethodRowResponse);
    update!(UnifiedModelResourceMethodUpdateRequest => UnifiedModelResourceMethodRowResponse);
    delete!(UnifiedModelResourceMethodDeleteResponse);
}

resource!(
    /// Environment overrides of unified model resource methods.
    EnvironmentUnifiedModelResourceMethods,
    "environment-unified-model-resource-method"
);
impl EnvironmentUnifiedModelResourceMethods<'_> {
    list!(EnvironmentUnifiedModelResourceMethodRowResponse);
    get!(EnvironmentUnifiedModelResourceMethodRowResponse);
    create!(EnvironmentUnifiedModelResourceMethodCreateRequest => EnvironmentUnifiedModelResourceMethodRowResponse);
    update!(EnvironmentUnifiedModelResourceMethodUpdateRequest => EnvironmentUnifiedModelResourceMethodRowResponse);
    delete!(EnvironmentUnifiedModelResourceMethodDeleteResponse);
}

resource!(
    /// Sync jobs.
    SyncJobs,
    "sync-job"
);
impl SyncJobs<'_> {
    list!(SyncJobSchema);
    get!(SyncJobSchema);
    create!(SyncJobCreateRequest => SyncJobSchema);
    update!(SyncJobUpdateRequest => SyncJobSchema);
    delete!(DeleteResponse);
}

resource!(
    /// Sync job runs.
    SyncJobRuns,
    "sync-job-run"
);
impl SyncJobRuns<'_> {
    list!(SyncJobRunResponse);
    get!(SyncJobRunResponse);
    create!(SyncJobRunCreateRequest => SyncJobRunResponse);
    update!(any => SyncJobRunResponse);
    delete!(SyncJobRunResponse);
}

resource!(
    /// Sync job cron triggers.
    SyncJobCronTriggers,
    "sync-job-cron-trigger"
);
impl SyncJobCronTriggers<'_> {
    list!(SyncJobCronTriggerSchema);
    get!(SyncJobCronTriggerSchema);
    create!(SyncJobCronTriggerCreateRequest => SyncJobCronTriggerSchema);
    update!(SyncJobCronTriggerUpdateRequest => SyncJobCronTriggerSchema);
    delete!(DeleteResponse);
    schedule!();
}

resource!(
    /// Sync job templates.
    SyncJobTemplates,
    "sync-job-template"
);
impl SyncJobTemplates<'_> {
    list!(SyncJobTemplateSchema);
    get!(SyncJobTemplateSchema);
    create!(SyncJobTemplateCreate => SyncJobTemplateSchema);
    update!(SyncJobTemplatePatch => SyncJobTemplateSchema);
    delete!(SyncJobTemplateDeleteResponse);
}

resource!(
    /// Sync job run states.
    SyncJobRunStates,
    "sync-job-run-state"
);
impl SyncJobRunStates<'_> {
    list!(SyncJobRunState);
    get!(SyncJobRunState);
    create!(SyncJobRunStateCreateRequest => SyncJobRunState);
    update!(SyncJobRunStateUpdateRequest => SyncJobRunState);
    delete!(SyncJobRunState);
}

macro_rules! record_crud {
    ($(#[$meta:meta])* $name:ident, $path:literal) => {
        resource!($(#[$meta])* $name, $path);
        impl $name<'_> {
            list!(Record);
            get!(Record);
            create!(any => Record);
            update!(any => Record);
            delete!(Record);
        }
    };
}

record_crud!(
    /// Daemons.
    Daemons,
    "daemon"
);
record_crud!(
    /// Daemon jobs.
    DaemonJobs,
    "daemon-job"
);
record_crud!(
    /// Daemon job runs.
    DaemonJobRuns,
    "daemon-job-run"
);
record_crud!(
    /// Alarms.
    Alarms,
    "alarm"
);

resource!(
    /// Daemon job cron triggers.
    DaemonJobCronTriggers,
    "daemon-job-cron-trigger"
);
impl DaemonJobCronTriggers<'_> {
    list!(Record);
    get!(Record);
    create!(any => Record);
    update!(any => Record);
    delete!(Record);
    schedule!();
}

resource!(
    /// Workflow runs.
    WorkflowRuns,
    "workflow-run"
);
impl WorkflowRuns<'_> {
    list!(WorkflowRunResponse);
    get!(WorkflowRunResponse);
    create!(any => Record);
    update!(any => Record);
    delete!(Record);
}

resource!(
    /// Workflows.
    Workflows,
    "workflow"
);
impl Workflows<'_> {
    list!(WorkflowSchema);
    get!(WorkflowSchema);
    create!(WorkflowCreateSchema => WorkflowSchema);
    update!(any => WorkflowSchema);
    delete!(WorkflowDeleteResponse);
}

resource!(
    /// Webhooks.
    Webhooks,
    "webhook"
);
impl Webhooks<'_> {
    list!(WebhookSchema);
    get!(WebhookSchema);
    create!(WebhookCreateRequest => WebhookCreateResponse);
    update!(WebhookUpdateRequest => WebhookSchema);
    delete!(DeleteResponse);

    /// Send a test event to a webhook (`POST /webhook/test`).
    pub fn test(&self, body: &WebhookTestRequest) -> Request<WebhookTestResponse> {
        self.client.request(Method::POST, "webhook/test").body(body)
    }
}

resource!(
    /// Datastores.
    Datastores,
    "datastore"
);
impl Datastores<'_> {
    list!(DatastoreSchema);
    get!(DatastoreSchema);
    create!(DatastoreCreateRequest => DatastoreSchema);
    update!(DatastoreUpdateRequest => DatastoreSchema);
    delete!(DatastoreDeleteResponse);

    /// Test a datastore operation (`POST /datastore/{id}/test/{method}`).
    pub fn test(&self, id: &str, method: &str, body: impl Serialize) -> Request<Record> {
        self.client
            .request(
                Method::POST,
                format!("{}/{}/test/{}", Self::PATH, seg(id), seg(method)),
            )
            .body(body)
    }
}

resource!(
    /// Documentation records.
    DocumentationResource,
    "documentation"
);
impl DocumentationResource<'_> {
    list!(crate::models::Documentation);
    get!(crate::models::Documentation);
    create!(DocumentationCreateRequest => crate::models::Documentation);
    update!(DocumentationPatchRequest => crate::models::Documentation);
    delete!(());
}

resource!(
    /// Logs.
    Logs,
    "log"
);
impl Logs<'_> {
    /// List logs of one type: `unified_proxy_api`, `rapid_bridge`, `webhook`,
    /// `mcp` or `sync_job_cron_trigger` (the API rejects anything else). The
    /// server requires `log_type`, so it is a parameter here; add filters with
    /// `.query(..)`.
    pub fn list(&self, log_type: &str) -> Pager<Log> {
        let mut pager = self.client.pager(Self::PATH);
        pager.query_ref().insert("log_type", log_type);
        pager
    }
}

resource!(
    /// Static gates.
    StaticGates,
    "static-gate"
);
impl StaticGates<'_> {
    list!(StaticGateResponse);
    get!(StaticGateResponse);
    create!(StaticGateCreateRequest => StaticGateCreateResponse);
    update!(StaticGateUpdateRequest => StaticGateResponse);
    delete!(DeleteResponse);
}

resource!(
    /// Observability destinations.
    ObservabilityDestinations,
    "observability-destination"
);
impl ObservabilityDestinations<'_> {
    list!(ObservabilityDestinationResponse);
    get!(ObservabilityDestinationResponse);
    create!(ObservabilityDestinationCreateRequest => ObservabilityDestinationResponse);
    update!(ObservabilityDestinationUpdateRequest => ObservabilityDestinationResponse);
    delete!(DeleteResponse);

    /// Send a test event (`POST /observability-destination/{id}/test`).
    pub fn test(
        &self,
        id: &str,
        body: &ObservabilityDestinationTestRequest,
    ) -> Request<ObservabilityDestinationTestResponse> {
        self.client
            .request(Method::POST, format!("{}/{}/test", Self::PATH, seg(id)))
            .body(body)
    }
}

resource!(
    /// Usage analytics. Pass filters with `.query(..)`.
    Analytics,
    "analytics"
);
impl Analytics<'_> {
    /// `GET /analytics/status`.
    pub fn status(&self) -> Request<AnalyticsStatusResponse> {
        self.client.request(Method::GET, "analytics/status")
    }

    /// `GET /analytics/metrics`.
    pub fn metrics(&self) -> Request<AnalyticsMetricsResponse> {
        self.client.request(Method::GET, "analytics/metrics")
    }

    /// `GET /analytics/dashboard`.
    pub fn dashboard(&self) -> Request<AnalyticsDashboardResponse> {
        self.client.request(Method::GET, "analytics/dashboard")
    }

    /// `GET /analytics/timeseries`.
    pub fn timeseries(&self) -> Request<AnalyticsTimeseriesResponse> {
        self.client.request(Method::GET, "analytics/timeseries")
    }

    /// `GET /analytics/breakdown`.
    pub fn breakdown(&self) -> Request<AnalyticsBreakdownResponse> {
        self.client.request(Method::GET, "analytics/breakdown")
    }
}

resource!(
    /// Batch requests.
    BatchRequests,
    "batch-request"
);
impl BatchRequests<'_> {
    create!(BatchRequestSchema => BatchRequestResponse);
}

resource!(
    /// Forward webhook payloads to an environment integration.
    EnvironmentIntegrationWebhooks,
    "environment-integration-webhook"
);
impl EnvironmentIntegrationWebhooks<'_> {
    /// `POST /environment-integration-webhook/{environment_integration_id}`.
    pub fn process(
        &self,
        environment_integration_id: &str,
        body: impl Serialize,
    ) -> Request<Record> {
        self.client
            .request(
                Method::POST,
                format!("{}/{}", Self::PATH, seg(environment_integration_id)),
            )
            .body(body)
    }
}

resource!(
    /// Forward webhook payloads to an integrated account.
    IntegratedAccountWebhooks,
    "integrated-account-webhook"
);
impl IntegratedAccountWebhooks<'_> {
    /// `POST /integrated-account-webhook/{integrated_account_id}`.
    pub fn process(&self, integrated_account_id: &str, body: impl Serialize) -> Request<Record> {
        self.client
            .request(
                Method::POST,
                format!("{}/{}", Self::PATH, seg(integrated_account_id)),
            )
            .body(body)
    }
}
