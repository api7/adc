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

use crate::cache::{Cache, CachedEntry};
use crate::fetcher::Fetcher;
use crate::operator::{Operator, SyncOutcome};
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

/// Stands in for a version that couldn't be determined (missing/unparseable
/// `Server` header) — high enough to unlock every version-gated code path,
/// matching the TS backend's own `mockVersion` convention.
const UNKNOWN_VERSION: Version = Version::new(999, 999, 999);

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
        if let Some(version) = Cache::global().version(&self.cache_key).await {
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
                    _ => UNKNOWN_VERSION,
                })
            })
            .await?;
        // Only cache a genuinely observed version, not the "couldn't tell"
        // fallback — matches the TS backend's own `semverEQ(version,
        // mockVersion)` guard.
        if *version != UNKNOWN_VERSION {
            Cache::global().set_version(&self.cache_key, version.clone()).await;
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

    /// The real cross-server "find the latest state" bootstrap
    /// (`Fetcher::dump`) only ever runs for the first `dump` of a given
    /// `cache_key`. Every call after that, and `sync` below, both just read
    /// whatever's cached — `sync` writes its own result back into the same
    /// cache at the end, so a `dump` right before it always sees fresh data
    /// without needing to hit the servers again.
    async fn dump(&self) -> Result<Configuration, BackendError> {
        if self.bypass_cache {
            Cache::global().invalidate(&self.cache_key);
        }
        if let Some(config) = Cache::global().config(&self.cache_key).await {
            return Ok(config);
        }

        let version = self.resolved_version().await?;
        let (config, raw_config) = Fetcher::new(self.servers.clone(), version).dump().await?;

        Cache::global().set_latest_version(&self.cache_key, highest_conf_version(&raw_config)).await;
        Cache::global().set_config(&self.cache_key, config.clone()).await;
        Cache::global().set_raw_config(&self.cache_key, raw_config).await;
        Ok(config)
    }

    /// Relies on a `dump` having already primed the cache for this
    /// `cache_key` — this is why `dump`'s own doc comment calls out that
    /// contract. The lock below guards against a *concurrent* `sync` for
    /// the same key, not against a cold cache: every caller (the CLI's
    /// `dump`-then-`sync` pipeline, and any other caller expected to
    /// follow the same convention) dumps before it syncs.
    async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        // Held for the whole read-modify-write span, not just the read
        // below — see `Cache::lock`'s doc comment for why two concurrent
        // syncs on the same key can't just read-then-write independently.
        let mut entry = Cache::global().lock(&self.cache_key).await;
        let old_raw_config = entry.raw_config.clone().unwrap_or_default();
        let latest_known_version = entry.latest_version;

        match Operator::new(self.servers.clone(), old_raw_config, latest_known_version).sync(events, opts).await {
            Ok(SyncOutcome { results, new_state: Some((timestamp, new_config)) }) => {
                // Built in full before it touches `entry`, then swapped in
                // as one assignment — a panic while computing the new state
                // (e.g. inside `to_adc`) leaves the old entry untouched
                // instead of a torn mix of old and new fields.
                // `tokio::sync::Mutex` never poisons on a panicked guard
                // holder, so nothing else would catch a partial write here.
                let new_entry = CachedEntry {
                    version: entry.version.clone(),
                    latest_version: Some(entry.latest_version.map_or(timestamp, |current| current.max(timestamp))),
                    config: Some(crate::transformer::to_adc(&new_config)),
                    raw_config: Some(new_config),
                    updated_at: Some(timestamp),
                };
                *entry = new_entry;
                Ok(results)
            }
            Ok(SyncOutcome { results, new_state: None }) => Ok(results),
            Err(error) => {
                // A server earlier in the batch may have already accepted
                // the new document before a later one failed — the cache
                // can't be trusted either way at that point, so this
                // resets the entry we're already holding the lock for
                // (same effect as `Cache::invalidate`, just through the
                // guard instead of a second lookup) rather than leaving it
                // pointing at data a live server may have moved past. The
                // next dump() re-fetches and re-runs `find_latest` to
                // discover the cluster's real state instead of trusting
                // stale cache.
                *entry = CachedEntry::default();
                Err(error)
            }
        }
    }

    async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        let version = self.resolved_version().await?;
        adc_backend_apisix::Validator::new(self.servers[0].client.clone(), version)
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
