//! Serializes the whole dump→diff→sync cycle per `cache_key`, for backends
//! that fold every sync into one shared full document (apisix-standalone).
//!
//! That backend's own per-`cache_key` lock only guards the final read-modify-
//! write of its cache — it doesn't cover the `diff` a caller computed
//! *before* calling `sync`. Two concurrent `/sync` requests for the same
//! `cache_key` can each `dump` the same base document, diff against it
//! independently, and then apply their own diff one after the other; the
//! second one's diff was computed against a document that's already stale
//! by the time it writes, so it silently drops whatever the first one just
//! added. Locking here — around `dump`+`diff`+`sync` together, before either
//! request's `dump` runs — removes the window entirely: a `dump` can only
//! ever be diffed against the document that's still current at apply time.
use std::sync::{Arc, LazyLock};

use dashmap::DashMap;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

// A sharded registry (same shape as `adc_backend_apisix_standalone::cache::Cache`)
// of per-cache_key locks, each held by its caller across the `.await`s in
// its dump -> diff -> sync cycle.
static LOCKS: LazyLock<DashMap<String, Arc<AsyncMutex<()>>>> = LazyLock::new(DashMap::new);

/// Holds the lock for `cache_key` until the returned guard is dropped.
///
/// The registry only ever grows (entries are never evicted) — harmless in
/// practice, since `cache_key` tracks a small, effectively fixed set of real
/// backend clusters for the life of an `ingress-server` process.
pub async fn lock(cache_key: &str) -> OwnedMutexGuard<()> {
    let entry = LOCKS
        .entry(cache_key.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone();
    entry.lock_owned().await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    /// Two holders of the same key's lock must never be inside their
    /// critical section at the same time — `busy` catches an overlap.
    #[tokio::test]
    async fn two_locks_on_the_same_key_never_overlap() {
        let busy = Arc::new(AtomicBool::new(false));

        let mut tasks = Vec::new();
        for _ in 0..8 {
            let busy = busy.clone();
            tasks.push(tokio::spawn(async move {
                let _guard = lock("shared").await;
                assert!(
                    !busy.swap(true, Ordering::SeqCst),
                    "another holder was already in the critical section"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
                busy.store(false, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn locks_on_different_keys_do_not_block_each_other() {
        let start = tokio::time::Instant::now();
        let _guard = lock("one").await;
        lock("two").await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }
}
