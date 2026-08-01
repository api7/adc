use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use adc_backend_core::RetryPolicy;

#[tokio::test]
async fn succeeds_after_transient_failures_within_budget() {
    let attempts = AtomicUsize::new(0);
    let policy = RetryPolicy { retries: 3, delay: Duration::from_millis(1) };

    let result: Result<&str, &str> = policy
        .run(|| async {
            if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                Err("not yet")
            } else {
                Ok("done")
            }
        })
        .await;

    assert_eq!(result, Ok("done"));
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn gives_up_after_exhausting_retries() {
    let attempts = AtomicUsize::new(0);
    let policy = RetryPolicy { retries: 2, delay: Duration::from_millis(1) };

    let result: Result<&str, &str> = policy
        .run(|| async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err("always fails")
        })
        .await;

    assert_eq!(result, Err("always fails"));
    // 1 initial attempt + 2 retries = 3.
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}
