use std::future::Future;
use std::time::Duration;

/// A fixed-delay retry policy: on failure, wait `delay` and try again, up to
/// `retries` additional attempts (so `retries + 1` attempts total).
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
    pub async fn run<T, E, F, Fut>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut attempt = 0;
        loop {
            match f().await {
                Ok(value) => return Ok(value),
                Err(_) if attempt < self.retries => {
                    attempt += 1;
                    tokio::time::sleep(self.delay).await;
                }
                Err(err) => return Err(err),
            }
        }
    }
}
