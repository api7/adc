use std::future::Future;

use futures::stream::{self, StreamExt};

/// Runs `f` over `items` with at most `concurrency` in flight at once.
/// `None` means unbounded — every item starts immediately, matching RxJS
/// `mergeMap(fn)` called with no concurrency argument.
///
/// Results come back in completion order, not input order (same as
/// `mergeMap`'s emission order): fine for callers like `Backend::sync`,
/// where each output already carries its own identity (the `Event` it came
/// from) rather than relying on positional correlation with `items`.
pub async fn concurrent_map<T, U, F, Fut>(items: Vec<T>, concurrency: Option<usize>, f: F) -> Vec<U>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = U>,
{
    let concurrency = concurrency.unwrap_or(items.len()).max(1);
    stream::iter(items).map(f).buffer_unordered(concurrency).collect().await
}
