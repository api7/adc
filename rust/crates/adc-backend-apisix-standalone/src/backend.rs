//! The apisix-standalone `Backend`: unlike `adc-backend-apisix`/
//! `adc-backend-api7` (one admin API, one target), a standalone cluster is
//! *n* independently-addressed servers that must all end up holding the
//! same declarative config document — `dump` picks whichever one has the
//! most recently accepted write, `sync` writes the new document to every
//! one of them.

use std::time::Duration;

use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::resources::Configuration;
use adc_sdk::{
    BackendError, BackendMetadata, BackendSyncOptions, BackendSyncResult, BackendValidateResult,
    DefaultValue, Event,
};
use async_trait::async_trait;
use semver::Version;
use tokio::sync::OnceCell;

use crate::cache::Cache;
use crate::fetcher::Fetcher;
use crate::operator::Operator;
use crate::typing::ApisixStandalone;

/// One target server plus the client already configured with its own
/// (server-specific, since standalone clusters can each carry a different
/// admin API token) auth header.
#[derive(Clone)]
pub struct StandaloneServer {
    pub server: String,
    pub client: HttpClient,
}

pub struct BackendOptions {
    /// At least one entry, each a full admin API base URL
    /// (`http://host:port`) — every entry gets written to on `sync` and is
    /// a candidate for `dump`'s "most recently updated" pick.
    pub servers: Vec<String>,
    /// Either one token shared by every server, or exactly as many tokens
    /// as `servers` (paired up positionally) — matches the TS backend's own
    /// `opts.token.split(',')` convention.
    pub tokens: Vec<String>,
    /// Identifies this backend's entry in the process-wide config cache
    /// (`crate::cache::Cache`) — callers targeting the same standalone
    /// cluster across multiple `Backend` instances should pass the same
    /// key, so they share cached state instead of each re-bootstrapping it.
    pub cache_key: String,
    /// Forces the next `dump` to discard whatever's cached for `cache_key`
    /// and re-fetch from the cluster, instead of trusting the cache.
    pub bypass_cache: bool,
    pub timeout: Option<Duration>,
    pub tls: TlsConfig,
}

pub struct Backend {
    servers: Vec<StandaloneServer>,
    cache_key: String,
    bypass_cache: bool,
    version: OnceCell<Version>,
}

impl Backend {
    pub fn new(opts: BackendOptions) -> Result<Self, BackendError> {
        if opts.servers.is_empty() {
            return Err(BackendError::Other(
                "apisix-standalone backend requires at least one server".into(),
            ));
        }
        let servers_count = opts.servers.len();
        // A `token` per `server`, positionally paired, when the two lists
        // are the same length; otherwise every server shares `tokens[0]` —
        // matches the TS backend's own `opts.token.split(',')` convention.
        let paired_tokens = opts.tokens.len() == servers_count;

        let servers = opts
            .servers
            .into_iter()
            .enumerate()
            .map(|(index, server)| {
                let token = if paired_tokens { opts.tokens.get(index) } else { opts.tokens.first() }
                    .cloned()
                    .ok_or_else(|| BackendError::Other("apisix-standalone backend requires at least one token".into()))?;
                let client = HttpClient::new(HttpClientConfig {
                    server: server.clone(),
                    token,
                    timeout: opts.timeout,
                    tls: opts.tls.clone(),
                })?
                .with_log_scope(vec!["APISIX".to_string()]);
                Ok(StandaloneServer { server, client })
            })
            .collect::<Result<Vec<_>, BackendError>>()?;

        Ok(Self {
            servers,
            cache_key: opts.cache_key,
            bypass_cache: opts.bypass_cache,
            version: OnceCell::new(),
        })
    }

    async fn resolved_version(&self) -> Result<Version, BackendError> {
        if let Some(version) = self.version.get() {
            return Ok(version.clone());
        }
        if let Some(version) = Cache::global().version(&self.cache_key) {
            let _ = self.version.set(version.clone());
            return Ok(version);
        }

        let version = self
            .version
            .get_or_try_init(|| async {
                let primary = &self.servers[0];
                // HEAD support on the config document endpoint is itself
                // version-gated (see `Fetcher::find_latest`'s
                // `version_supports_head`), so the version probe uses the
                // admin root instead, which has none of that ambiguity.
                let request = primary.client.request(Method::HEAD, "/apisix/admin")?;
                let response = primary.client.send(request).await?;

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
        // Only cache a genuinely observed version, not the "couldn't tell"
        // fallback — matches the TS backend's own `semverEQ(version,
        // mockVersion)` guard.
        if *version != Version::new(999, 999, 999) {
            Cache::global().set_version(&self.cache_key, version.clone());
        }
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
        let primary = &self.servers[0];
        let request = primary.client.request(Method::HEAD, "/apisix/admin")?;
        primary.client.send(request).await?;
        Ok(())
    }

    async fn version(&self) -> Result<Version, BackendError> {
        self.resolved_version().await
    }

    async fn default_value(&self) -> Result<DefaultValue, BackendError> {
        Ok(DefaultValue::default())
    }

    async fn dump(&self) -> Result<Configuration, BackendError> {
        if self.bypass_cache {
            Cache::global().invalidate(&self.cache_key);
        }
        if let Some(config) = Cache::global().config(&self.cache_key) {
            return Ok(config);
        }

        let version = self.resolved_version().await?;
        let (config, raw_config) = Fetcher::new(self.servers.clone(), version).dump().await?;

        Cache::global().set_latest_version(&self.cache_key, highest_conf_version(&raw_config));
        Cache::global().set_config(&self.cache_key, config.clone());
        Cache::global().set_raw_config(&self.cache_key, raw_config);
        Ok(config)
    }

    async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        let old_raw_config = Cache::global().raw_config(&self.cache_key).unwrap_or_default();
        Operator::new(self.servers.clone(), self.cache_key.clone(), old_raw_config)
            .sync(events, opts)
            .await
    }

    async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        adc_backend_apisix::Validator::new(self.servers[0].client.clone())
            .validate(events)
            .await
    }
}

/// The version cache primes off whichever `*_conf_version` field is
/// numerically highest across the whole document — there's no single
/// document-wide version, just one counter per resource collection, and the
/// highest one is what a subsequent `sync` must not regress below (see
/// `crate::operator::Operator::sync`'s clock-rollback guard).
fn highest_conf_version(raw_config: &ApisixStandalone) -> i64 {
    [
        raw_config.routes_conf_version,
        raw_config.services_conf_version,
        raw_config.consumers_conf_version,
        raw_config.ssls_conf_version,
        raw_config.global_rules_conf_version,
        raw_config.plugin_metadata_conf_version,
        raw_config.upstreams_conf_version,
        raw_config.stream_routes_conf_version,
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(0)
}
