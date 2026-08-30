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
use crate::operator::{Operator, SyncOutcome, SyncedState, WireVersions};
use crate::utils::stable_timestamp;

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
    /// as `servers` (paired up positionally).
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
/// `Server` header) — high enough to unlock every version-gated code path.
const UNKNOWN_VERSION: Version = Version::new(999, 999, 999);

pub struct Backend {
    servers: Vec<StandaloneServer>,
    cache_key: String,
    bypass_cache: bool,
    version: OnceCell<Version>,
    /// Pinned from this instance's own `dump`, if it ran one — so a later
    /// `sync` on the *same* instance reconstructs the desired config
    /// against the exact baseline `events` were diffed against, not a
    /// fresh (possibly since-changed by a concurrent caller) read of the
    /// global cache. See `sync`'s doc comment. A caller can legitimately
    /// `dump`/`sync` more than once on the same instance, so every call
    /// overwrites this with its own result. Always set together (same
    /// moment, same lock) with `dumped_versions` below — both come from
    /// the same `CachedEntry`.
    dumped_desired: std::sync::Mutex<Option<Configuration>>,
    /// The `modifiedIndex`/`*_conf_version` values the wire document last
    /// actually written for this `cache_key` carried — what
    /// `crate::operator::stamp_versions` carries forward for a resource
    /// `events` doesn't touch. Pinned for the same reason, and under the
    /// same rules, as `dumped_desired`; derived from
    /// `crate::cache::CachedEntry::versions` (or the freshly-written wire
    /// document, after a `sync`) rather than held onto directly — nothing
    /// here needs the wire document itself, only what it carried.
    dumped_versions: std::sync::Mutex<Option<WireVersions>>,
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
        // are the same length; otherwise every server shares `tokens[0]`.
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
            dumped_desired: std::sync::Mutex::new(None),
            dumped_versions: std::sync::Mutex::new(None),
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
        // fallback.
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
    ///
    /// Also pins `config`/derived versions onto this instance
    /// (`dumped_desired`/`dumped_versions`) — see `sync`'s doc comment for
    /// why a later `sync` on this same instance prefers those pinned copies
    /// over a fresh cache read.
    async fn dump(&self) -> Result<Configuration, BackendError> {
        if self.bypass_cache {
            Cache::global().invalidate(&self.cache_key).await;
        }
        // Cheap fast path: no lock held across anything, so it can't block
        // a concurrent `sync` on this key. `config` and `versions` must
        // come from the same moment, or the values pinned below could
        // mismatch what's cached.
        if let Some(entry) = Cache::global().get_live(&self.cache_key).await
            && let Some(config) = entry.config
        {
            *self.dumped_desired.lock().unwrap() = Some(config.clone());
            *self.dumped_versions.lock().unwrap() = entry.versions;
            return Ok(config);
        }

        // Resolved before the lock below: it may itself read or write this
        // same cache_key's cached `version` field, which would deadlock
        // against a lock this call is about to hold.
        let version = self.resolved_version().await?;

        // Held across the re-check, the fetch, and the commit below —
        // `Backend::sync` takes this same per-key lock for its own
        // read-then-write, so a slow fetch here, started from data read
        // before this point, can never land after (and silently overwrite)
        // a `sync` that already committed newer data in the meantime.
        let mut entry = Cache::global().lock(&self.cache_key).await;
        if Cache::global().is_fresh(&entry) && let Some(config) = entry.config.clone() {
            // Someone else populated the cache while this call resolved
            // the version or waited for this lock.
            *self.dumped_desired.lock().unwrap() = Some(config.clone());
            *self.dumped_versions.lock().unwrap() = entry.versions.clone();
            return Ok(config);
        }

        // `config` here is a re-modelled view of the dumped wire document
        // (`to_adc`), not a previously-cached *desired* config — this only
        // runs on the bootstrap (or a `bypass_cache`/`InvalidateADCCache`
        // re-seed), and there is no prior desired config to prefer over it
        // yet. See `crate::transformer::transform_to_wire`'s doc comment
        // for why every *later* `sync` caches the desired config directly
        // instead. `raw_config` itself is only needed transiently, to seed
        // `versions` — nothing here holds onto the wire document itself.
        let (config, raw_config) = Fetcher::new(self.servers.clone(), version).dump().await?;
        let versions = WireVersions::from_wire(&raw_config);
        let highest = versions.highest_conf_version();
        // Built in full before it touches `entry`, then swapped in as one
        // assignment — a panic while computing the new state leaves the
        // old entry untouched instead of a torn mix of old and new fields.
        // Applied directly to the guard already held above, not through
        // `Cache::set_dump_result` — that would try to lock this same key
        // again and deadlock.
        let new_entry = CachedEntry {
            version: entry.version.clone(),
            latest_version: Some(entry.latest_version.map_or(highest, |current| current.max(highest))),
            config: Some(config.clone()),
            versions: Some(versions.clone()),
            updated_at: Some(stable_timestamp()),
        };
        *entry = new_entry;
        drop(entry);
        *self.dumped_desired.lock().unwrap() = Some(config.clone());
        *self.dumped_versions.lock().unwrap() = Some(versions);
        Ok(config)
    }

    /// When this instance ran its own `dump` first — the normal case: a
    /// caller diffs `events` against whatever `dump` just returned, then
    /// syncs them on the same instance — prefers that `dump`'s pinned
    /// `dumped_desired`/`dumped_versions` over a fresh read of the global
    /// cache: a concurrent caller (a `bypass_cache` dump, or another sync)
    /// could have changed the cached entry for this key in the time between
    /// this instance's `dump` and this `sync` call, and reconstructing the
    /// desired config against a baseline other than the one `events` were
    /// diffed against would produce a wrong result. Without a pinned
    /// snapshot — `events` built from something other than this instance's
    /// own `dump` (a caller applying a known change directly, say) — falls
    /// back to whatever's in the global cache for this key, or an empty
    /// document if there's nothing there yet; this is a documented
    /// convention, not something enforced here.
    async fn sync(
        &self,
        events: Vec<Event>,
        opts: BackendSyncOptions,
    ) -> Result<Vec<BackendSyncResult>, BackendError> {
        // Held for the whole read-modify-write span, not just the read
        // below — see `Cache::lock`'s doc comment for why two concurrent
        // syncs on the same key can't just read-then-write independently.
        let mut entry = Cache::global().lock(&self.cache_key).await;
        let prior_desired = match self.dumped_desired.lock().unwrap().clone() {
            Some(desired) => desired,
            None => entry.config.clone().unwrap_or_default(),
        };
        let old_versions = match self.dumped_versions.lock().unwrap().clone() {
            Some(versions) => versions,
            None => entry.versions.clone().unwrap_or_default(),
        };
        let latest_known_version = entry.latest_version;

        match Operator::new(self.servers.clone(), prior_desired, old_versions, latest_known_version)
            .sync(events, opts)
            .await
        {
            Ok(SyncOutcome { results, new_state: Some(SyncedState { timestamp, desired, wire }) }) => {
                // `wire` (the document actually PUT) is only needed
                // transiently, to derive what gets cached — nothing here
                // holds onto it past this point.
                let versions = WireVersions::from_wire(&wire);
                // Pinned before `desired`/`versions` are moved into
                // `new_entry` below, so a later `sync`/`dump` on this same
                // instance builds on what this call actually wrote, not the
                // pre-sync snapshot it started from.
                *self.dumped_desired.lock().unwrap() = Some(desired.clone());
                *self.dumped_versions.lock().unwrap() = Some(versions.clone());
                // Built in full before it touches `entry`, then swapped in
                // as one assignment — a panic while computing the new state
                // leaves the old entry untouched instead of a torn mix of
                // old and new fields. `tokio::sync::Mutex` never poisons on
                // a panicked guard holder, so nothing else would catch a
                // partial write here.
                let new_entry = CachedEntry {
                    version: entry.version.clone(),
                    latest_version: Some(entry.latest_version.map_or(timestamp, |current| current.max(timestamp))),
                    config: Some(desired),
                    versions: Some(versions),
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
                // stale cache. This instance's own pinned snapshots are now
                // equally untrustworthy, so they're cleared too — a retry
                // on the same instance must go through a fresh `dump`.
                *entry = CachedEntry::default();
                *self.dumped_desired.lock().unwrap() = None;
                *self.dumped_versions.lock().unwrap() = None;
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
