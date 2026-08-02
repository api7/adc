//! Ties the fetcher, operator, and validator together behind
//! `adc_sdk::Backend` — the interface the CLI actually dispatches through.

use adc_backend_core::{HttpClient, Method};
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

pub struct Backend {
    client: HttpClient,
    version: OnceCell<Version>,
}

impl Backend {
    pub fn new(client: HttpClient) -> Self {
        Self {
            client,
            version: OnceCell::new(),
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
                let request = self.client.request(Method::GET, "/apisix/admin/routes")?;
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
        // Bounds the response to a handful of routes rather than the
        // server's entire route table — a lighter probe on route-heavy
        // deployments. Unrecognized query params are ignored by APISIX
        // versions that predate pagination support, so this is safe across
        // the whole supported version range.
        let request = self.client.request(Method::GET, "/apisix/admin/routes?page=1&page_size=10")?;
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
        Fetcher::new(self.client.clone(), version).dump().await
    }

    async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<Vec<BackendSyncResult>, BackendError> {
        let version = self.resolved_version().await?;
        Operator::new(self.client.clone(), version).sync(events, opts).await
    }

    async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        Validator::new(self.client.clone()).validate(events).await
    }
}
