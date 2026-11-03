//! Resource accessors. Get one from the client, e.g.
//! `client.integrated_accounts()`, then call a method to build a request.

use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;

use percent_encoding::{utf8_percent_encode, AsciiSet};
use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::client::{Client, RequestOptions};
use crate::error::Error;
use crate::query::{Query, STRICT};
use crate::request::request_settings;
use crate::Record;

mod admin;
mod custom_api;
mod integrated_accounts;
mod integrations;
mod mcp;
mod proxy_api;
mod unified_api;

pub use admin::*;
pub use custom_api::CustomApi;
pub use integrated_accounts::IntegratedAccounts;
pub use integrations::{Integrations, UnifiedModels};
pub use mcp::Mcp;
pub use proxy_api::ProxyApi;
pub use unified_api::UnifiedApi;

/// Percent-encode one path segment taken from a caller-supplied ID.
pub(crate) fn seg(s: &str) -> String {
    utf8_percent_encode(s, STRICT).to_string()
}

/// Characters escaped in a caller-supplied multi-segment path: everything a
/// path cannot carry literally. `/` and existing `%XX` escapes are kept.
const PATH: &AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

pub(crate) fn path(s: &str) -> String {
    utf8_percent_encode(s.trim_start_matches('/'), PATH).to_string()
}

/// Declare a resource accessor struct with its base path.
macro_rules! resource {
    ($(#[$meta:meta])* $name:ident, $path:literal) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("Base path: `/", $path, "`.")]
        #[derive(Debug, Clone, Copy)]
        pub struct $name<'a> {
            client: &'a $crate::Client,
        }

        impl<'a> $name<'a> {
            #[allow(dead_code)]
            const PATH: &'static str = $path;

            pub(crate) fn new(client: &'a $crate::Client) -> Self {
                Self { client }
            }
        }
    };
}
pub(crate) use resource;

macro_rules! list {
    ($item:ty) => {
        /// List items. Returns a lazy [`Pager`](crate::Pager).
        pub fn list(&self) -> $crate::Pager<$item> {
            self.client.pager(Self::PATH)
        }
    };
}
pub(crate) use list;

macro_rules! get {
    ($item:ty) => {
        /// Get one item by ID.
        pub fn get(&self, id: &str) -> $crate::Request<$item> {
            self.client.request(
                reqwest::Method::GET,
                format!("{}/{}", Self::PATH, $crate::resources::seg(id)),
            )
        }
    };
}
pub(crate) use get;

macro_rules! create {
    (any => $out:ty) => {
        /// Create an item from any serialisable JSON object.
        pub fn create(&self, body: impl serde::Serialize) -> $crate::Request<$out> {
            self.client
                .request(reqwest::Method::POST, Self::PATH)
                .body(body)
        }
    };
    ($body:ty => $out:ty) => {
        /// Create an item.
        pub fn create(&self, body: &$body) -> $crate::Request<$out> {
            self.client
                .request(reqwest::Method::POST, Self::PATH)
                .body(body)
        }
    };
}
pub(crate) use create;

macro_rules! update {
    (any => $out:ty) => {
        /// Update an item (PATCH) from any serialisable JSON object.
        pub fn update(&self, id: &str, body: impl serde::Serialize) -> $crate::Request<$out> {
            self.client
                .request(
                    reqwest::Method::PATCH,
                    format!("{}/{}", Self::PATH, $crate::resources::seg(id)),
                )
                .body(body)
        }
    };
    ($body:ty => $out:ty) => {
        /// Update an item (PATCH).
        pub fn update(&self, id: &str, body: &$body) -> $crate::Request<$out> {
            self.client
                .request(
                    reqwest::Method::PATCH,
                    format!("{}/{}", Self::PATH, $crate::resources::seg(id)),
                )
                .body(body)
        }
    };
}
pub(crate) use update;

macro_rules! delete {
    ($out:ty) => {
        /// Delete an item by ID.
        pub fn delete(&self, id: &str) -> $crate::Request<$out> {
            self.client.request(
                reqwest::Method::DELETE,
                format!("{}/{}", Self::PATH, $crate::resources::seg(id)),
            )
        }
    };
}
pub(crate) use delete;

/// `POST {path}/{id}/schedule`.
macro_rules! schedule {
    () => {
        /// Trigger the cron trigger now (`POST {id}/schedule`).
        pub fn schedule(&self, id: &str) -> $crate::Request<$crate::Record> {
            self.client.request(
                reqwest::Method::POST,
                format!("{}/{}/schedule", Self::PATH, $crate::resources::seg(id)),
            )
        }
    };
}
pub(crate) use schedule;

/// Removes every environment-level install of an integration or unified
/// model: lists the environment records that point at it, then deletes each.
/// Await it to get the delete results.
#[must_use = "an uninstall does nothing until it is awaited"]
pub struct Uninstall<D> {
    client: Client,
    path: &'static str,
    filter_key: &'static str,
    id: String,
    query: Query,
    opts: RequestOptions,
    error: Option<Error>,
    _marker: PhantomData<fn() -> D>,
}

impl<D> std::fmt::Debug for Uninstall<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Uninstall")
            .field("path", &self.path)
            .field(self.filter_key, &self.id)
            .finish()
    }
}

impl<D> Uninstall<D> {
    pub(crate) fn new(
        client: &Client,
        path: &'static str,
        filter_key: &'static str,
        id: &str,
    ) -> Self {
        Self {
            client: client.clone(),
            path,
            filter_key,
            id: id.to_string(),
            query: Query::new(),
            opts: RequestOptions::default(),
            error: None,
            _marker: PhantomData,
        }
    }

    request_settings!();
}

impl<D: DeserializeOwned + Send + 'static> Uninstall<D> {
    /// Run the uninstall. Same as `.await`.
    pub async fn send(self) -> Result<Vec<D>, Error> {
        if let Some(e) = self.error {
            return Err(e);
        }
        let mut query = self.query.clone();
        query.insert(self.filter_key, self.id.clone());
        let installs: Vec<Record> = self
            .client
            .pager::<Record>(self.path)
            .query(query)
            .options(self.opts.clone())
            .collect_all()
            .await?;
        let mut results = Vec::with_capacity(installs.len());
        for install in installs {
            let id = match install.get("id") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => continue,
            };
            let deleted = self
                .client
                .request::<D>(Method::DELETE, format!("{}/{}", self.path, seg(&id)))
                .options(self.opts.clone())
                .send()
                .await?;
            results.push(deleted);
        }
        Ok(results)
    }
}

impl<D: DeserializeOwned + Send + 'static> IntoFuture for Uninstall<D> {
    type Output = Result<Vec<D>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_and_paths_encode() {
        assert_eq!(seg("a/b c"), "a%2Fb%20c");
        assert_eq!(path("/files/a b/report.csv"), "files/a%20b/report.csv");
    }
}

macro_rules! accessors {
    ($($(#[$meta:meta])* $fn:ident => $ty:ident;)*) => {
        impl Client {
            $(
                $(#[$meta])*
                pub fn $fn(&self) -> $ty<'_> {
                    $ty::new(self)
                }
            )*
        }
    };
}

accessors! {
    /// API tokens (`/api-token`).
    api_tokens => ApiTokens;
    /// Environments (`/environment`).
    environments => Environments;
    /// Integration categories (`/category`).
    categories => Categories;
    /// Teams (`/team`).
    teams => Teams;
    /// Users (`/user`).
    users => Users;
    /// Tenants (`/tenant`).
    tenants => Tenants;
    /// Link tokens (`/link-token`).
    link_tokens => LinkTokens;
    /// Session tokens (`/session-token`).
    session_tokens => SessionTokens;
    /// Integrated accounts (`/integrated-account`).
    integrated_accounts => IntegratedAccounts;
    /// Sandbox integrated accounts (`/sandbox-integrated-account`).
    sandbox_integrated_accounts => SandboxIntegratedAccounts;
    /// Integrations (`/integration`).
    integrations => Integrations;
    /// Environment integrations (`/environment-integration`).
    environment_integrations => EnvironmentIntegrations;
    /// Unified models (`/unified-model`).
    unified_models => UnifiedModels;
    /// Environment unified models (`/environment-unified-model`).
    environment_unified_models => EnvironmentUnifiedModels;
    /// Unified model resource methods (`/unified-model-resource-method`).
    unified_model_resource_methods => UnifiedModelResourceMethods;
    /// Environment unified model resource methods (`/environment-unified-model-resource-method`).
    environment_unified_model_resource_methods => EnvironmentUnifiedModelResourceMethods;
    /// Sync jobs (`/sync-job`).
    sync_jobs => SyncJobs;
    /// Sync job runs (`/sync-job-run`).
    sync_job_runs => SyncJobRuns;
    /// Sync job cron triggers (`/sync-job-cron-trigger`).
    sync_job_cron_triggers => SyncJobCronTriggers;
    /// Sync job templates (`/sync-job-template`).
    sync_job_templates => SyncJobTemplates;
    /// Sync job run states (`/sync-job-run-state`).
    sync_job_run_states => SyncJobRunStates;
    /// Daemons (`/daemon`).
    daemons => Daemons;
    /// Daemon jobs (`/daemon-job`).
    daemon_jobs => DaemonJobs;
    /// Daemon job runs (`/daemon-job-run`).
    daemon_job_runs => DaemonJobRuns;
    /// Daemon job cron triggers (`/daemon-job-cron-trigger`).
    daemon_job_cron_triggers => DaemonJobCronTriggers;
    /// Alarms (`/alarm`).
    alarms => Alarms;
    /// Workflows (`/workflow`).
    workflows => Workflows;
    /// Workflow runs (`/workflow-run`).
    workflow_runs => WorkflowRuns;
    /// Webhooks (`/webhook`).
    webhooks => Webhooks;
    /// Datastores (`/datastore`).
    datastores => Datastores;
    /// Documentation (`/documentation`).
    documentation => DocumentationResource;
    /// Logs (`/log`).
    logs => Logs;
    /// Static gates (`/static-gate`).
    static_gates => StaticGates;
    /// Observability destinations (`/observability-destination`).
    observability_destinations => ObservabilityDestinations;
    /// Analytics (`/analytics`).
    analytics => Analytics;
    /// Batch requests (`/batch-request`).
    batch_requests => BatchRequests;
    /// Environment integration webhooks (`/environment-integration-webhook`).
    environment_integration_webhooks => EnvironmentIntegrationWebhooks;
    /// Integrated account webhooks (`/integrated-account-webhook`).
    integrated_account_webhooks => IntegratedAccountWebhooks;
    /// The Unified API (`/unified`).
    unified_api => UnifiedApi;
    /// The Proxy API (`/proxy`).
    proxy_api => ProxyApi;
    /// The Custom API (`/custom`).
    custom_api => CustomApi;
    /// Truto MCP servers (`/mcp`).
    mcp => Mcp;
}

impl Client {
    /// Check the API is reachable (`GET /ping`).
    pub fn ping(&self) -> crate::Request<crate::models::Ping> {
        self.request(Method::GET, "ping")
    }
}
