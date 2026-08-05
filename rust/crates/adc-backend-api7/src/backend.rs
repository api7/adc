use adc_backend_core::{HttpClient, Method, ResourceFilter};
use adc_sdk::resources::Configuration;
use adc_sdk::{
    BackendError, BackendMetadata, BackendSyncOptions, BackendSyncResult, BackendValidateResult,
    DefaultValue, Event,
};
use async_trait::async_trait;
use semver::Version;
use tokio::sync::OnceCell;

use crate::default_value;
use crate::fetcher::Fetcher;
use crate::gateway_group::GatewayGroupResolver;
use crate::operator::Operator;
use crate::typing;
use crate::validator::Validator;

pub struct Backend {
    client: HttpClient,
    gateway_group: GatewayGroupResolver,
    filter: ResourceFilter,
    version: OnceCell<Version>,
    default_value: OnceCell<DefaultValue>,
}

impl Backend {
    pub fn new(
        client: HttpClient,
        gateway_group_name: String,
        token: &str,
        filter: ResourceFilter,
    ) -> Self {
        let client = client.with_log_scope(vec!["API7".to_string()]);
        let gateway_group = GatewayGroupResolver::new(client.clone(), gateway_group_name, token);
        Self {
            client,
            gateway_group,
            filter,
            version: OnceCell::new(),
            default_value: OnceCell::new(),
        }
    }

    /// Fetched once and cached for this `Backend`'s lifetime. `"dev"` is a
    /// known placeholder for an unreleased build and maps to a version
    /// high enough to unlock every version-gated feature; any other value
    /// that doesn't coerce to a semver is unexpected and falls back to
    /// `0.0.0` instead — deliberately the *conservative* direction, since
    /// assuming the oldest possible version is safer than assuming the
    /// newest when the actual version genuinely can't be determined.
    async fn resolved_version(&self) -> Result<Version, BackendError> {
        let version = self
            .version
            .get_or_try_init(|| async {
                let request = self.client.request(Method::GET, "/api/version")?;
                let body: typing::ValueResponse<String> = self.client.send_json(request).await?;
                Ok::<_, BackendError>(if body.value == "dev" {
                    Version::new(999, 999, 999)
                } else {
                    coerce_version(&body.value).unwrap_or_else(|| Version::new(0, 0, 0))
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
            log_scope: vec!["API7".to_string()],
        }
    }

    async fn ping(&self) -> Result<(), BackendError> {
        let request = self.client.request(Method::GET, "/api/gateway_groups")?;
        self.client.send(request).await?;
        Ok(())
    }

    async fn version(&self) -> Result<Version, BackendError> {
        self.resolved_version().await
    }

    /// Fetched once and cached for this `Backend`'s lifetime. See
    /// `crate::default_value` for the actual derivation.
    async fn default_value(&self) -> Result<DefaultValue, BackendError> {
        let value = self
            .default_value
            .get_or_try_init(|| default_value::fetch(&self.client))
            .await?;
        Ok(value.clone())
    }

    async fn dump(&self) -> Result<Configuration, BackendError> {
        let version = self.resolved_version().await?;
        let gateway_group_id = self.gateway_group.resolve().await?;
        Fetcher::new(
            self.client.clone(),
            version,
            gateway_group_id,
            self.filter.clone(),
        )
        .dump()
        .await
    }

    async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        let gateway_group_id = self.gateway_group.resolve().await?;
        Operator::new(self.client.clone(), gateway_group_id)
            .sync(events, opts)
            .await
    }

    async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        let version = self.resolved_version().await?;
        let gateway_group_id = self.gateway_group.resolve().await?;
        Validator::new(self.client.clone(), version, gateway_group_id)
            .validate(events)
            .await
    }
}

/// A lenient version parser for values [`Version::parse`] rejects outright:
/// extracts the first run of digits after any non-digit prefix
/// (`"v3.9.10"` -> `3.9.10`) and tolerates a short `major[.minor[.patch]]`
/// (missing components default to `0`) — the dashboard's `/api/version`
/// endpoint has been observed returning both a `v`-prefixed value and the
/// literal string `"dev"` (handled separately, before this is called).
fn coerce_version(value: &str) -> Option<Version> {
    let digits_and_dots: String = value
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let mut parts = digits_and_dots.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Some(Version::new(major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerces_a_clean_version_string() {
        assert_eq!(coerce_version("3.9.10"), Some(Version::new(3, 9, 10)));
    }

    #[test]
    fn coerces_a_version_string_with_a_leading_prefix() {
        assert_eq!(coerce_version("v3.9.10"), Some(Version::new(3, 9, 10)));
    }

    #[test]
    fn fills_in_missing_minor_and_patch_components() {
        assert_eq!(coerce_version("3"), Some(Version::new(3, 0, 0)));
        assert_eq!(coerce_version("3.9"), Some(Version::new(3, 9, 0)));
    }

    #[test]
    fn returns_none_for_a_value_with_no_digits_at_all() {
        assert_eq!(coerce_version("dev"), None);
    }
}
