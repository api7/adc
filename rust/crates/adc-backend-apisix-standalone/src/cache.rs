//! Cross-request cache of each standalone target's resolved state, keyed by
//! a caller-supplied `cache_key` (typically derived from its server list).
//! A real process-wide cache, not per-`Backend`-instance state, so that
//! long-lived callers (e.g. an ingress-server handling many requests
//! against the same standalone target) don't pay for a fresh
//! "find the latest server + full dump" bootstrap on every request.
//!
//! One entry per `cache_key`, holding all four cached values together
//! (version/latest_version/config/raw_config) behind a single per-key
//! `tokio::sync::Mutex` — they're always read and written in the same
//! lifecycle anyway (see `crate::backend::Backend::dump`/`sync`), so
//! there's no case where one legitimately needs to expire, evict, or lock
//! independently of the others.
//!
//! `Backend::sync` locks the whole entry for its read-modify-write span via
//! [`Cache::lock`], so two concurrent syncs on the same key can't both read
//! the same starting snapshot and silently discard one another's changes.
//! Every other accessor below takes the same per-key lock too, just briefly
//! (one field, not a multi-step operation) — which also means a `dump`
//! reading through `config`/`raw_config` while a `sync` is in flight for
//! the same key naturally waits for it, rather than reading a half-applied
//! state.
//!
//! Eviction checks `Arc::strong_count`, not the entry's own lock state: an
//! entry with a strong count of 1 has no clone anywhere outside this map,
//! so nobody could be about to lock it either — this also catches a
//! caller that already holds a clone but hasn't called `.lock()` yet,
//! which a lock-state check alone would miss. The scan for a candidate and
//! the actual removal are two separate steps, but the removal re-checks
//! the count under `DashMap::remove_if`'s single shard-lock acquisition,
//! so nothing can acquire a new clone in the gap between "this looked
//! idle" and "so I removed it".

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use adc_sdk::resources::Configuration;
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use semver::Version;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::typing::ApisixStandalone;
use crate::utils::stable_timestamp;

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
pub(crate) struct CachedEntry {
    pub(crate) version: Option<Version>,
    pub(crate) latest_version: Option<i64>,
    pub(crate) config: Option<Configuration>,
    pub(crate) raw_config: Option<ApisixStandalone>,
    /// Milliseconds since the Unix epoch, from [`stable_timestamp`] — the
    /// same clock `Operator::sync`'s own conf-version timestamps come from,
    /// so a successful sync's write-back reuses that call's timestamp
    /// directly instead of taking a second, separate "now" reading. Also
    /// what eviction ranks candidates by (older wins).
    pub(crate) updated_at: Option<i64>,
}

impl CachedEntry {
    fn is_expired(&self, ttl: Duration) -> bool {
        // Compared in u128 throughout: a `ttl.as_millis()` (u128) cast down
        // to i64 would silently wrap for a configured TTL above i64::MAX
        // ms, making every entry look immediately expired.
        self.updated_at.is_none_or(|at| stable_timestamp().saturating_sub(at).max(0) as u128 > ttl.as_millis())
    }
}

pub struct Cache {
    entries: DashMap<String, Arc<AsyncMutex<CachedEntry>>>,
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

    fn entry(&self, key: &str) -> Arc<AsyncMutex<CachedEntry>> {
        self.entries.entry(key.to_string()).or_default().clone()
    }

    /// Locks the whole cached entry for `key`, for a caller that needs to
    /// read then later write it as one atomic step spanning more than a
    /// single field access — see `Backend::sync`'s use of this. Every
    /// other method here (`raw_config`, `set_raw_config`, ...) takes and
    /// releases this same lock internally, just for the one field it
    /// touches, so ordinary callers never need this directly. Not exposed
    /// outside the crate: `CachedEntry`'s fields are crate-internal, so a
    /// caller elsewhere couldn't do anything with the guard anyway.
    ///
    /// Resets an expired entry before returning it, so a caller through
    /// here sees the same "expired == absent" state `get_live` does,
    /// rather than reading stale data past its TTL.
    pub(crate) async fn lock(&self, key: &str) -> OwnedMutexGuard<CachedEntry> {
        let mut entry = self.entry(key).lock_owned().await;
        if entry.is_expired(self.ttl) {
            *entry = CachedEntry::default();
        }
        entry
    }

    /// Every field, read under one lock acquisition, so a caller pinning
    /// more than one of them (`Backend::dump` does, for `config` and
    /// `raw_config`) gets values guaranteed to be from the same moment.
    pub(crate) async fn get_live(&self, key: &str) -> Option<CachedEntry> {
        // A plain `get`, not `entry()` — reading a key that was never
        // cached must not materialize a fresh entry.
        let arc = self.entries.get(key)?.value().clone();
        let entry = arc.lock().await;
        if entry.is_expired(self.ttl) { None } else { Some(entry.clone()) }
    }

    pub async fn version(&self, key: &str) -> Option<Version> {
        self.get_live(key).await?.version
    }

    // Only reached through the `test-utils`-gated re-export (see
    // `crate::tests`) — dead as far as a plain `--lib` build (no consumer
    // outside this crate's own tests) can tell.
    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn latest_version(&self, key: &str) -> Option<i64> {
        self.get_live(key).await?.latest_version
    }

    // Only reached through the `test-utils`-gated re-export (see
    // `crate::tests`) now that `dump` reads `get_live` directly instead —
    // dead as far as a plain `--lib` build (no consumer outside this
    // crate's own tests) can tell.
    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn config(&self, key: &str) -> Option<Configuration> {
        self.get_live(key).await?.config
    }

    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn raw_config(&self, key: &str) -> Option<ApisixStandalone> {
        self.get_live(key).await?.raw_config
    }

    async fn touch(&self, key: &str, apply: impl FnOnce(&mut CachedEntry)) {
        {
            let entry = self.entry(key);
            let mut entry = entry.lock().await;
            entry.updated_at = Some(stable_timestamp());
            apply(&mut entry);
        }
        self.evict_if_over_capacity();
    }

    pub async fn set_version(&self, key: &str, version: Version) {
        self.touch(key, |entry| entry.version = Some(version)).await;
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
    // Only reached through the `test-utils`-gated re-export (see
    // `crate::tests`) now that `Backend::dump` writes all three fields
    // together via `set_dump_result` instead — dead as far as a plain
    // `--lib` build (no consumer outside this crate's own tests) can tell.
    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn set_latest_version(&self, key: &str, value: i64) {
        self.touch(key, |entry| {
            entry.latest_version = Some(
                entry
                    .latest_version
                    .map_or(value, |current| current.max(value)),
            );
        })
        .await;
    }

    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn set_config(&self, key: &str, config: Configuration) {
        self.touch(key, |entry| entry.config = Some(config)).await;
    }

    #[cfg_attr(not(feature = "test-utils"), allow(dead_code))]
    pub async fn set_raw_config(&self, key: &str, raw_config: ApisixStandalone) {
        self.touch(key, |entry| entry.raw_config = Some(raw_config))
            .await;
    }

    /// `latest_version`/`config`/`raw_config` together, under one lock
    /// acquisition, so a concurrent reader (another `dump`'s `get_live`)
    /// can never observe just some of the three updated and not the
    /// others.
    pub async fn set_dump_result(&self, key: &str, latest_version: i64, config: Configuration, raw_config: ApisixStandalone) {
        self.touch(key, |entry| {
            entry.latest_version = Some(entry.latest_version.map_or(latest_version, |current| current.max(latest_version)));
            entry.config = Some(config);
            entry.raw_config = Some(raw_config);
        })
        .await;
    }

    /// No-op for a never-cached key — checked via `entries.entry`, whose
    /// `Occupied`/`Vacant` match settles that under one shard-lock
    /// acquisition rather than a separate lookup-then-decide. For a cached
    /// key, resets its contents in place rather than removing the map
    /// entry: a concurrent `Backend::sync` that already holds this same
    /// `Arc` (e.g. mid-write) must keep writing into the entry this call
    /// reset, not one orphaned from the map.
    pub async fn invalidate(&self, key: &str) {
        let entry = match self.entries.entry(key.to_string()) {
            Entry::Occupied(occupied) => occupied.get().clone(),
            Entry::Vacant(_) => return,
        };
        let mut entry = entry.lock().await;
        *entry = CachedEntry::default();
    }

    /// Best-effort: if every entry is currently referenced by something
    /// other than this map (an in-flight `lock`/`get_live`/...), this
    /// leaves the map over `max_entries` rather than evicting something
    /// still in use. Long-running processes with pathologically many
    /// distinct `cache_key`s and constant concurrent access could keep the
    /// map slightly over budget indefinitely, which is an acceptable
    /// trade against ever corrupting a key's cached state.
    fn evict_if_over_capacity(&self) {
        while self.entries.len() > self.max_entries {
            let oldest = self
                .entries
                .iter()
                .filter_map(|entry| {
                    if Arc::strong_count(entry.value()) != 1 {
                        return None;
                    }
                    let updated_at = entry.value().try_lock().ok()?.updated_at;
                    Some((entry.key().clone(), updated_at))
                })
                .min_by_key(|(_, updated_at)| *updated_at)
                .map(|(key, _)| key);
            let Some(key) = oldest else { break };
            // Re-checked here, under the removal's own shard-lock
            // acquisition, in case something acquired a clone in the gap
            // since the scan above read this key's count.
            if self.entries.remove_if(&key, |_, arc| Arc::strong_count(arc) == 1).is_none() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::*;

    #[tokio::test]
    async fn a_fresh_cache_has_nothing_cached() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        assert!(cache.version("k").await.is_none());
        assert!(cache.config("k").await.is_none());
    }

    #[tokio::test]
    async fn set_then_get_round_trips_within_ttl() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("k", 42).await;
        assert_eq!(cache.latest_version("k").await, Some(42));
    }

    #[tokio::test]
    async fn writing_a_smaller_version_never_regresses_the_cached_one() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("k", 100).await;
        // Simulates a slower concurrent `Operator::sync` call that decided
        // on an older timestamp before a faster one raced ahead and wrote a
        // newer value, only landing its own write afterward.
        cache.set_latest_version("k", 50).await;
        assert_eq!(cache.latest_version("k").await, Some(100));
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
        let cache = Arc::new(Cache::with_limits(16, Duration::from_secs(3600)));
        let values: Vec<i64> = (0..200).map(|i| (i * 7919) % 1000).collect();
        let expected_max = *values.iter().max().unwrap();

        let mut tasks = tokio::task::JoinSet::new();
        for value in values {
            let cache = cache.clone();
            tasks.spawn(async move {
                // Jitter completion order so writers genuinely interleave
                // instead of running in the order they were spawned.
                tokio::time::sleep(Duration::from_micros((value as u64 * 37) % 500)).await;
                cache.set_latest_version("k", value).await;
            });
        }
        tasks.join_all().await;

        assert_eq!(cache.latest_version("k").await, Some(expected_max));
    }

    /// `Cache::lock` must give real mutual exclusion, not just a fresh
    /// mutex each call: many tasks racing a read-increment-write on a
    /// value protected by nothing but this lock must never lose an
    /// update. Runs on a genuine multi-threaded runtime so the critical
    /// sections actually interleave across OS threads.
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn lock_serializes_concurrent_read_modify_write_on_the_same_key() {
        let cache = Arc::new(Cache::with_limits(16, Duration::from_secs(3600)));
        // A bare atomic, read and written non-atomically below (individual
        // `load`/`store` calls, not a `fetch_add`) — its own type gives no
        // mutual exclusion, so correctness here depends entirely on
        // `Cache::lock` closing the race window between the two.
        let counter = Arc::new(std::sync::atomic::AtomicI64::new(0));

        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..200 {
            let cache = cache.clone();
            let counter = counter.clone();
            tasks.spawn(async move {
                let _guard = cache.lock("k").await;
                let read = counter.load(std::sync::atomic::Ordering::Relaxed);
                tokio::task::yield_now().await;
                counter.store(read + 1, std::sync::atomic::Ordering::Relaxed);
            });
        }
        tasks.join_all().await;

        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 200);
    }

    #[tokio::test]
    async fn an_entry_older_than_the_ttl_is_treated_as_absent() {
        let cache = Cache::with_limits(16, Duration::from_millis(10));
        cache.set_latest_version("k", 1).await;
        sleep(Duration::from_millis(30));
        assert!(cache.latest_version("k").await.is_none());
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

    #[tokio::test]
    async fn invalidate_clears_every_cached_field_for_that_key() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_version("k", Version::new(1, 0, 0)).await;
        cache.set_latest_version("k", 1).await;
        cache.set_config("k", empty_configuration()).await;
        cache.set_raw_config("k", ApisixStandalone::default()).await;
        cache.invalidate("k").await;
        assert!(cache.version("k").await.is_none());
        assert!(cache.latest_version("k").await.is_none());
        assert!(cache.config("k").await.is_none());
        assert!(cache.raw_config("k").await.is_none());
    }

    #[tokio::test]
    async fn invalidate_on_a_never_cached_key_does_not_materialize_an_entry() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.invalidate("k").await;
        assert_eq!(cache.entries.len(), 0);
    }

    /// A holder that grabbed this entry's `Arc` before `invalidate` ran
    /// (e.g. a concurrent `sync` mid-write) must still be holding the same
    /// entry `invalidate` reset, not one orphaned from the map — exercised
    /// here with a task actually holding the lock while `invalidate` runs,
    /// not just two sequential, non-overlapping lookups.
    #[tokio::test]
    async fn invalidate_keeps_the_same_entry_identity_for_concurrent_holders() {
        let cache = Arc::new(Cache::with_limits(16, Duration::from_secs(3600)));
        cache.set_version("k", Version::new(1, 0, 0)).await;
        let before = cache.entry("k");

        let (holding, release) = (
            tokio::sync::oneshot::channel(),
            tokio::sync::oneshot::channel(),
        );
        let (holding_tx, holding_rx) = holding;
        let (release_tx, release_rx) = release;
        let holder_cache = cache.clone();
        let holder = tokio::spawn(async move {
            let _guard = holder_cache.lock("k").await;
            holding_tx.send(()).unwrap();
            release_rx.await.unwrap();
        });
        holding_rx.await.unwrap(); // the spawned task now holds the lock

        let invalidate_cache = cache.clone();
        let invalidating = tokio::spawn(async move { invalidate_cache.invalidate("k").await });
        release_tx.send(()).unwrap(); // let the holder drop its guard; invalidate can now proceed
        holder.await.unwrap();
        invalidating.await.unwrap();

        let after = cache.entry("k");
        assert!(Arc::ptr_eq(&before, &after));
        assert!(cache.version("k").await.is_none());
    }

    #[tokio::test]
    async fn different_keys_are_cached_independently() {
        let cache = Cache::with_limits(16, Duration::from_secs(3600));
        cache.set_latest_version("a", 1).await;
        cache.set_latest_version("b", 2).await;
        assert_eq!(cache.latest_version("a").await, Some(1));
        assert_eq!(cache.latest_version("b").await, Some(2));
    }

    #[tokio::test]
    async fn inserting_past_capacity_evicts_the_least_recently_touched_key() {
        let cache = Cache::with_limits(2, Duration::from_secs(3600));
        cache.set_latest_version("a", 1).await;
        sleep(Duration::from_millis(5));
        cache.set_latest_version("b", 2).await;
        sleep(Duration::from_millis(5));
        cache.set_latest_version("c", 3).await;

        assert_eq!(cache.entries.len(), 2);
        assert!(cache.latest_version("a").await.is_none());
        assert_eq!(cache.latest_version("b").await, Some(2));
        assert_eq!(cache.latest_version("c").await, Some(3));
    }

    #[tokio::test]
    async fn re_touching_a_key_protects_it_from_eviction() {
        let cache = Cache::with_limits(2, Duration::from_secs(3600));
        cache.set_latest_version("a", 1).await;
        sleep(Duration::from_millis(5));
        cache.set_latest_version("b", 2).await;
        sleep(Duration::from_millis(5));
        cache.set_latest_version("a", 10).await; // "a" is now the most recently touched
        sleep(Duration::from_millis(5));
        cache.set_latest_version("c", 3).await; // "b" should be evicted instead

        assert_eq!(cache.entries.len(), 2);
        assert_eq!(cache.latest_version("a").await, Some(10));
        assert!(cache.latest_version("b").await.is_none());
        assert_eq!(cache.latest_version("c").await, Some(3));
    }

    /// An entry that's locked (held by an in-flight caller) must never be
    /// picked as an eviction victim, even while enough other keys pass
    /// through to blow well past capacity — the whole point of checking
    /// `Arc::strong_count` instead of just scanning by recency.
    #[tokio::test]
    async fn a_locked_entry_survives_eviction_pressure_from_other_keys() {
        let cache = Arc::new(Cache::with_limits(2, Duration::from_secs(3600)));
        cache.set_latest_version("a", 1).await;

        let holder_cache = cache.clone();
        let (holding_tx, holding_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let holder = tokio::spawn(async move {
            let _guard = holder_cache.lock("a").await;
            holding_tx.send(()).unwrap();
            release_rx.await.unwrap();
        });
        holding_rx.await.unwrap(); // the spawned task now holds "a"'s lock

        for i in 0..10 {
            cache.set_latest_version(&format!("k{i}"), i).await;
        }

        release_tx.send(()).unwrap();
        holder.await.unwrap();

        assert_eq!(cache.latest_version("a").await, Some(1));
    }
}
