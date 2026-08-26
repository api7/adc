use std::future::Future;
use std::time::Duration;

use adc_sdk::BackendError;

/// A fixed-delay retry policy: on a retriable failure, wait `delay` and try
/// again, up to `retries` additional attempts (so `retries + 1` attempts
/// total). Non-retriable errors return immediately. What counts as
/// retriable isn't decided here — [`RetryPolicy::run`] takes that as a
/// predicate, since it varies by backend (an APISIX dependency-ordering
/// conflict, say, isn't something this backend-agnostic crate can know
/// about).
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub retries: usize,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    /// Matches the APISIX operator's hardcoded `retry({ count: 3, delay: 100 })`
    /// for mutating requests (PUT/DELETE against `/apisix/admin/*`).
    fn default() -> Self {
        Self { retries: 3, delay: Duration::from_millis(100) }
    }
}

impl RetryPolicy {
    pub async fn run<T, F, Fut>(
        &self,
        is_retriable: impl Fn(&BackendError) -> bool,
        mut f: F,
    ) -> Result<T, BackendError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, BackendError>>,
    {
        let mut attempt = 0;
        loop {
            match f().await {
                Ok(value) => return Ok(value),
                Err(err) if is_retriable(&err) && attempt < self.retries => {
                    attempt += 1;
                    tokio::time::sleep(self.delay).await;
                }
                Err(err) => return Err(err),
            }
        }
    }
}
