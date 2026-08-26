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
    concurrency: usize,
}

impl Backend {
    pub fn new(
        client: HttpClient,
        gateway_group_name: String,
        token: &str,
        filter: ResourceFilter,
        concurrency: usize,
    ) -> Self {
        let client = client.with_log_scope(vec!["API7".to_string()]);
        let gateway_group = GatewayGroupResolver::new(client.clone(), gateway_group_name, token);
        Self {
            client,
            gateway_group,
            filter,
            version: OnceCell::new(),
            default_value: OnceCell::new(),
            concurrency,
        }
    }

    /// Fetched once and cached for this `Backend`'s lifetime. Falls back to
    /// a version high enough to unlock every version-gated feature whenever
    /// the value doesn't coerce to a semver — covers the literal `"dev"`
    /// placeholder API7 returns for an unreleased build, and anything else
    /// unexpected the same way.
    async fn resolved_version(&self) -> Result<Version, BackendError> {
        let version = self
            .version
            .get_or_try_init(|| async {
                let request = self.client.request(Method::GET, "/api/version")?;
                let body: typing::ValueResponse<String> = self.client.send_json(request).await?;
                Ok::<_, BackendError>(coerce_version(&body.value).unwrap_or_else(|| Version::new(999, 999, 999)))
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
            self.concurrency,
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
/// literal string `"dev"`, neither of which has anything for this to
/// extract, so both return `None` like any other unparseable value.
fn coerce_version(value: &str) -> Option<Version> {
    let digits_and_dots: String = value
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let mut parts = digits_and_dots.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    // A component that's *present* but doesn't parse (e.g. the empty string
    // between two dots, or truncated as `digits_and_dots` cuts the value off
    // at its first non-digit/non-dot char) fails the whole thing rather than
    // silently defaulting to 0 — that would misreport a garbled minor/patch
    // as a clean, low version instead of an unparseable one. Only a
    // genuinely *omitted* trailing component (`parts.next()` returning
    // `None`) defaults.
    let minor = match parts.next() {
        None => 0,
        Some(s) => s.parse().ok()?,
    };
    let patch = match parts.next() {
        None => 0,
        Some(s) => s.parse().ok()?,
    };
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

    /// A present-but-garbled minor component (here, `digits_and_dots` cuts
    /// the value off right after the dot) must not be mistaken for an
    /// *omitted* one — that would misreport this as the clean version 3.0.0
    /// instead of an unparseable value. `resolved_version` treats `None` the
    /// same way as any other unparseable string: it falls back to
    /// `Version::new(999, 999, 999)` rather than guessing low.
    #[test]
    fn returns_none_for_a_present_but_invalid_minor_component() {
        assert_eq!(coerce_version("3.dev"), None);
    }
}
