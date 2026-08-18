use adc_backend_core::{HttpClient, Method, ResourceFilter};
use adc_sdk::resources::Configuration;
use adc_sdk::{
    BackendError, BackendMetadata, BackendSyncOptions, BackendSyncResult, BackendValidateResult,
    DefaultValue, Event,
};
use async_trait::async_trait;
use semver::Version;
use tokio::sync::OnceCell;

use crate::fetcher::Fetcher;
use crate::operator::Operator;
use crate::validator::Validator;

/// Shared by `ping` and `resolved_version` — neither needs the response body.
const PROBE_PATH: &str = "/apisix/admin/routes?page=1&page_size=1";

pub struct Backend {
    client: HttpClient,
    filter: ResourceFilter,
    version: OnceCell<Version>,
    fetch_concurrency: usize,
}

impl Backend {
    pub fn new(client: HttpClient, filter: ResourceFilter, fetch_concurrency: usize) -> Self {
        Self {
            client: client.with_log_scope(vec!["APISIX".to_string()]),
            filter,
            version: OnceCell::new(),
            fetch_concurrency,
        }
    }

    /// APISIX has no dedicated "get version" endpoint; every admin API
    /// response carries it in the `Server` response header instead
    /// (`APISIX/3.9.0`, confirmed strict `MAJOR.MINOR.PATCH` against a real
    /// instance). Falls back to a version high enough to unlock every
    /// version-gated feature when the header is missing or unparseable,
    /// rather than failing outright — matches the TS backend's own
    /// fallback. Fetched once and cached for the lifetime of this `Backend`.
    async fn resolved_version(&self) -> Result<Version, BackendError> {
        let version = self
            .version
            .get_or_try_init(|| async {
                let request = self.client.request(Method::GET, PROBE_PATH)?;
                let response = self.client.send(request).await?;

                let header = response
                    .headers()
                    .get("server")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.strip_prefix("APISIX/"));
                Ok::<_, BackendError>(match header.map(Version::parse) {
                    Some(Ok(version)) => version,
                    _ => Version::new(999, 999, 999),
                })
            })
            .await?;
        Ok(version.clone())
    }
}

#[async_trait]
impl adc_sdk::Backend for Backend {
    fn metadata(&self) -> BackendMetadata {
        BackendMetadata {
            log_scope: vec!["APISIX".to_string()],
        }
    }

    async fn ping(&self) -> Result<(), BackendError> {
        let request = self.client.request(Method::GET, PROBE_PATH)?;
        self.client.send(request).await?;
        Ok(())
    }

    async fn version(&self) -> Result<Version, BackendError> {
        self.resolved_version().await
    }

    async fn default_value(&self) -> Result<DefaultValue, BackendError> {
        Ok(DefaultValue::default())
    }

    async fn dump(&self) -> Result<Configuration, BackendError> {
        let version = self.resolved_version().await?;
        Fetcher::new(
            self.client.clone(),
            version,
            self.filter.clone(),
            self.fetch_concurrency,
        )
        .dump()
        .await
    }

    async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        let version = self.resolved_version().await?;
        Operator::new(self.client.clone(), version)
            .sync(events, opts)
            .await
    }

    async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        let version = self.resolved_version().await?;
        Validator::new(self.client.clone(), version).validate(events).await
    }
}
