use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use adc_backend_core::{concurrent_map, concurrent_map_until_err, concurrent_map_until_ok};

#[tokio::test]
async fn respects_the_concurrency_bound() {
    let current = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);

    let items: Vec<u32> = (0..20).collect();
    let results = concurrent_map(items.clone(), Some(4), |item| {
        let current = &current;
        let peak = &peak;
        async move {
            let now = current.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(5)).await;
            current.fetch_sub(1, Ordering::SeqCst);
            item
        }
    })
    .await;

    assert_eq!(results.len(), items.len());
    assert!(peak.load(Ordering::SeqCst) <= 4, "peak concurrency was {}", peak.load(Ordering::SeqCst));

    let mut sorted = results;
    sorted.sort();
    assert_eq!(sorted, items);
}

#[tokio::test]
async fn unbounded_when_concurrency_is_none() {
    let current = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);

    let items: Vec<u32> = (0..10).collect();
    let results = concurrent_map(items.clone(), None, |item| {
        let current = &current;
        let peak = &peak;
        async move {
            let now = current.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(5)).await;
            current.fetch_sub(1, Ordering::SeqCst);
            item
        }
    })
    .await;

    assert_eq!(results.len(), items.len());
    assert_eq!(peak.load(Ordering::SeqCst), items.len());
}

#[tokio::test]
async fn until_err_stops_pulling_new_work_after_the_first_failure_but_finishes_in_flight_items() {
    let started = AtomicUsize::new(0);
    let items: Vec<u32> = (0..20).collect();

    // Item 0 fails immediately; everything else in flight takes long enough
    // that item 0's failure is guaranteed to be observed first.
    let result = concurrent_map_until_err(items, Some(4), |item| {
        let started = &started;
        async move {
            started.fetch_add(1, Ordering::SeqCst);
            if item == 0 {
                return Err("boom");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(item)
        }
    })
    .await;

    assert_eq!(result, Err("boom"));
    // Only the initial batch (bounded by concurrency) was ever started —
    // nothing queued behind the limit was pulled once item 0 failed.
    assert_eq!(started.load(Ordering::SeqCst), 4, "no new work should start once a failure is observed");
}

#[tokio::test]
async fn until_err_returns_all_results_when_nothing_fails() {
    let items: Vec<u32> = (0..10).collect();
    let result = concurrent_map_until_err(items.clone(), Some(3), |item| async move { Ok::<u32, &str>(item) }).await;

    let mut results = result.unwrap();
    results.sort();
    assert_eq!(results, items);
}

#[tokio::test]
async fn until_ok_stops_pulling_new_work_after_the_first_success_but_finishes_in_flight_items() {
    let started = AtomicUsize::new(0);
    let items: Vec<u32> = (0..20).collect();

    // Nothing resolves synchronously, so the whole initial batch gets polled at least once
    // before any of them can win -- item 0's shorter sleep is what makes it win.
    let result = concurrent_map_until_ok(items, Some(4), |item| {
        let started = &started;
        async move {
            started.fetch_add(1, Ordering::SeqCst);
            if item == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
                return Ok::<u32, &str>(item);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            Err("boom")
        }
    })
    .await;

    assert_eq!(result, Ok(0));
    // Only the initial batch (bounded by concurrency) was ever started -- nothing queued
    // behind the limit was pulled once a success was observed.
    assert_eq!(started.load(Ordering::SeqCst), 4, "no new work should start once a success is observed");
}

#[tokio::test]
async fn until_ok_fails_only_once_everything_has() {
    let items: Vec<u32> = (0..10).collect();
    let result = concurrent_map_until_ok(items, Some(3), |_| async move { Err::<u32, &str>("boom") }).await;

    assert_eq!(result, Err("boom"));
}
