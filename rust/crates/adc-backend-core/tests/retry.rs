use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use adc_backend_core::RetryPolicy;
use adc_sdk::BackendError;

#[tokio::test]
async fn succeeds_after_transient_failures_within_budget() {
    let attempts = AtomicUsize::new(0);
    let policy = RetryPolicy { retries: 3, delay: Duration::from_millis(1) };

    let result: Result<&str, BackendError> = policy
        .run(BackendError::is_retriable, || async {
            if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
                Err(BackendError::Transport("not yet".into()))
            } else {
                Ok("done")
            }
        })
        .await;

    assert_eq!(result.unwrap(), "done");
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn gives_up_after_exhausting_retries() {
    let attempts = AtomicUsize::new(0);
    let policy = RetryPolicy { retries: 2, delay: Duration::from_millis(1) };

    let result: Result<&str, BackendError> = policy
        .run(BackendError::is_retriable, || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Transport("always fails".into()))
        })
        .await;

    assert!(matches!(result, Err(BackendError::Transport(msg)) if msg == "always fails"));
    // 1 initial attempt + 2 retries = 3.
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn does_not_retry_a_non_retriable_error() {
    let attempts = AtomicUsize::new(0);
    let policy = RetryPolicy { retries: 3, delay: Duration::from_millis(1) };

    let result: Result<&str, BackendError> = policy
        .run(BackendError::is_retriable, || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(BackendError::Api { status: 400, message: "bad config".into() })
        })
        .await;

    assert!(matches!(result, Err(BackendError::Api { status: 400, .. })));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
