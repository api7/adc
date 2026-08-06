//! Cross-request cache of each standalone target's resolved state, keyed by
//! a caller-supplied `cache_key` (typically derived from its server list).
//! Mirrors the TS backend's module-level `lru-cache` singletons — a real
//! process-wide cache, not per-`Backend`-instance state, so that
//! long-lived callers (e.g. an ingress-server handling many requests
//! against the same standalone target) don't pay for a fresh
//! "find the latest server + full dump" bootstrap on every request.
//!
//! Structurally simpler than the TS version: one entry per `cache_key`
//! holding all four cached values together (version/latest_version/
//! config/raw_config), rather than four independent `lru-cache` instances —
//! they're always read and written in the same lifecycle anyway (see
//! `crate::backend::Backend::dump`/`sync`), so there's no case where one
//! would legitimately expire or evict independently of the others.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use adc_sdk::resources::Configuration;
use dashmap::DashMap;
use semver::Version;

use crate::typing::ApisixStandalone;

const DEFAULT_MAX_ENTRIES: usize = 16;
const DEFAULT_TTL_MS: u64 = 3_600_000;

fn env_max_entries() -> usize {
    std::env::var("ADC_APISIX_STANDALONE_CACHE_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(DEFAULT_MAX_ENTRIES)
}

fn env_ttl() -> Duration {
    let ms = std::env::var("ADC_APISIX_STANDALONE_CACHE_TTL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &u64| *v >= 1)
        .unwrap_or(DEFAULT_TTL_MS);
    Duration::from_millis(ms)
}

#[derive(Clone, Default)]
struct CachedEntry {
    version: Option<Version>,
    latest_version: Option<i64>,
    config: Option<Configuration>,
    raw_config: Option<ApisixStandalone>,
    updated_at: Option<Instant>,
}

pub struct Cache {
    entries: DashMap<String, CachedEntry>,
    max_entries: usize,
    ttl: Duration,
}

static GLOBAL: LazyLock<Cache> = LazyLock::new(|| Cache::with_limits(env_max_entries(), env_ttl()));

impl Cache {
    pub fn with_limits(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: DashMap::new(),
            max_entries: max_entries.max(1),
            ttl,
        }
    }

    /// The process-wide singleton every `Backend` instance reads/writes by
    /// default. Test code should use [`Self::with_limits`] instead — an
    /// isolated instance, not this one, since tests running in the same
    /// process would otherwise share (and race on) cache state keyed by
    /// whatever `cache_key` happens to collide.
    pub fn global() -> &'static Cache {
        &GLOBAL
    }

    fn get_live(&self, key: &str) -> Option<CachedEntry> {
        let entry = self.entries.get(key)?;
        let expired = entry.updated_at.is_none_or(|at| at.elapsed() > self.ttl);
        if expired {
            drop(entry);
            self.entries.remove(key);
            return None;
        }
        Some(entry.clone())
    }

    pub fn version(&self, key: &str) -> Option<Version> {
        self.get_live(key)?.version
    }

    pub fn latest_version(&self, key: &str) -> Option<i64> {
        self.get_live(key)?.latest_version
    }

    pub fn config(&self, key: &str) -> Option<Configuration> {
        self.get_live(key)?.config
    }

    pub fn raw_config(&self, key: &str) -> Option<ApisixStandalone> {
        self.get_live(key)?.raw_config
    }

    fn touch(&self, key: &str, apply: impl FnOnce(&mut CachedEntry)) {
        {
            let mut entry = self.entries.entry(key.to_string()).or_default();
            entry.updated_at = Some(Instant::now());
            apply(&mut entry);
        }
        self.evict_if_over_capacity();
    }

    pub fn set_version(&self, key: &str, version: Version) {
        self.touch(key, |entry| entry.version = Some(version));
    }

    /// Bumps the cached version to `value`, never below whatever's already
    /// there. A plain overwrite would let a slower concurrent
    /// `Operator::sync` call — one that read an older `latest_version`
    /// before a faster call raced ahead and wrote a newer one — regress the
    /// cache back down once *it* finishes and writes its own (smaller)
    /// value. That regression isn't just a stale cache entry: a later
    /// sync's clock-rollback guard reads this value to pick its own
    /// timestamp, so a regressed value here could produce a
    /// `*_conf_version` the data plane has already seen (and rejects).
    pub fn set_latest_version(&self, key: &str, value: i64) {
        self.touch(key, |entry| {
            entry.latest_version = Some(entry.latest_version.map_or(value, |current| current.max(value)));
        });
    }

    pub fn set_config(&self, key: &str, config: Configuration) {
        self.touch(key, |entry| entry.config = Some(config));
    }

    pub fn set_raw_config(&self, key: &str, raw_config: ApisixStandalone) {
        self.touch(key, |entry| entry.raw_config = Some(raw_config));
    }

    pub fn invalidate(&self, key: &str) {
        self.entries.remove(key);
    }

    /// A soft, best-effort cap: eviction reads `updated_at` timestamps
    /// without coordinating with concurrent inserts, so under concurrent
    /// writers the map can transiently hold a couple more entries than
    /// `max_entries` before the next call trims it back down. That's fine —
    /// this exists to bound long-run memory growth, not to enforce an exact
    /// invariant.
    fn evict_if_over_capacity(&self) {
        while self.entries.len() > self.max_entries {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|entry| entry.updated_at)
                .map(|entry| entry.key().clone());
            match oldest {
                Some(key) => {
                    self.entries.remove(&key);
                }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    #[test]
    fn a_fresh_cache_has_nothing_cached() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        assert!(cache.version("k").is_none());
        assert!(cache.config("k").is_none());
    }

    #[test]
    fn set_then_get_round_trips_within_ttl() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("k", 42);
        assert_eq!(cache.latest_version("k"), Some(42));
    }

    #[test]
    fn writing_a_smaller_version_never_regresses_the_cached_one() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("k", 100);
        // Simulates a slower concurrent `Operator::sync` call that decided
        // on an older timestamp before a faster one raced ahead and wrote a
        // newer value, only landing its own write afterward.
        cache.set_latest_version("k", 50);
        assert_eq!(cache.latest_version("k"), Some(100));
    }

    /// Regression coverage for a real bug this crate's design guards
    /// against: many `Operator::sync` calls racing on the same `cache_key`
    /// must leave the cached `latest_version` at the highest timestamp any
    /// of them ever wrote, regardless of which one's write happens to land
    /// last. Runs on a genuine multi-threaded runtime (not
    /// `current_thread`) so the writes actually interleave across OS
    /// threads rather than just cooperatively yielding on one.
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_writers_racing_on_the_same_key_never_regress_the_cached_version() {
        let cache = std::sync::Arc::new(Cache::with_limits(16, Duration::from_secs(3600)));
        let values: Vec<i64> = (0..200).map(|i| (i * 7919) % 1000).collect();
        let expected_max = *values.iter().max().unwrap();

        let mut tasks = tokio::task::JoinSet::new();
        for value in values {
            let cache = cache.clone();
            tasks.spawn(async move {
                // Jitter completion order so writers genuinely interleave
                // instead of running in the order they were spawned.
                tokio::time::sleep(Duration::from_micros((value as u64 * 37) % 500)).await;
                cache.set_latest_version("k", value);
            });
        }
        tasks.join_all().await;

        assert_eq!(cache.latest_version("k"), Some(expected_max));
    }

    #[test]
    fn an_entry_older_than_the_ttl_is_treated_as_absent() {
        let cache = Cache::with_limits(16, Duration::from_millis(10));
        cache.set_latest_version("k", 1);
        sleep(Duration::from_millis(30));
        assert!(cache.latest_version("k").is_none());
    }

    fn empty_configuration() -> Configuration {
        Configuration {
            services: None,
            ssls: None,
            consumers: None,
            consumer_groups: None,
            global_rules: None,
            plugin_metadata: None,
        }
    }

    #[test]
    fn invalidate_removes_every_cached_field_for_that_key() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("k", 1);
        cache.set_config("k", empty_configuration());
        cache.invalidate("k");
        assert!(cache.latest_version("k").is_none());
        assert!(cache.config("k").is_none());
    }

    #[test]
    fn different_keys_are_cached_independently() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("a", 1);
        cache.set_latest_version("b", 2);
        assert_eq!(cache.latest_version("a"), Some(1));
        assert_eq!(cache.latest_version("b"), Some(2));
    }

    #[test]
    fn inserting_past_capacity_evicts_the_least_recently_touched_key() {
        let cache = Cache::with_limits(2, Duration::from_secs(3600));
        cache.set_latest_version("a", 1);
        sleep(Duration::from_millis(5));
        cache.set_latest_version("b", 2);
        sleep(Duration::from_millis(5));
        cache.set_latest_version("c", 3);

        assert!(cache.latest_version("a").is_none());
        assert_eq!(cache.latest_version("b"), Some(2));
        assert_eq!(cache.latest_version("c"), Some(3));
    }

    #[test]
    fn re_touching_a_key_protects_it_from_eviction() {
        let cache = Cache::with_limits(2, Duration::from_secs(3600));
        cache.set_latest_version("a", 1);
        sleep(Duration::from_millis(5));
        cache.set_latest_version("b", 2);
        sleep(Duration::from_millis(5));
        // Re-touch "a" so "b" becomes the least recently touched instead.
        cache.set_latest_version("a", 10);
        sleep(Duration::from_millis(5));
        cache.set_latest_version("c", 3);

        assert_eq!(cache.latest_version("a"), Some(10));
        assert!(cache.latest_version("b").is_none());
        assert_eq!(cache.latest_version("c"), Some(3));
    }
}
