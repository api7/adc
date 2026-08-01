use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use adc_backend_core::concurrent_map;

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
