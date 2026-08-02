use std::future::Future;

use futures::stream::{self, FuturesUnordered, StreamExt};

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

/// Like `concurrent_map`, but as soon as one item's future resolves to
/// `Err`, stops starting any new ones and returns that error, discarding
/// every `Ok` collected so far (from this batch and, per the caller's own
/// choice, typically from earlier batches too). Items already in flight
/// when that happens still run to completion — there's no way to cancel
/// them once started — but their outcomes are discarded either way.
///
/// Mirrors RxJS `mergeMap(project, concurrency)`'s behavior when a
/// projected Observable errors: the merged subscription tears down
/// immediately, which cascades an `unsubscribe()` to every currently active
/// inner subscription (their underlying work, if it's a `Promise`, can't
/// actually be cancelled — it keeps running, just unobserved) while
/// anything still queued behind the concurrency limit is dropped without
/// ever being subscribed to, i.e. `f` is never even called for it.
pub async fn concurrent_map_until_err<T, U, E, F, Fut>(items: Vec<T>, concurrency: Option<usize>, mut f: F) -> Result<Vec<U>, E>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<U, E>>,
{
    let concurrency = concurrency.unwrap_or(items.len()).max(1);
    let mut items = items.into_iter();
    let mut in_flight = FuturesUnordered::new();
    for item in items.by_ref().take(concurrency) {
        in_flight.push(f(item));
    }

    let mut results = Vec::new();
    let mut failure = None;
    while let Some(outcome) = in_flight.next().await {
        match outcome {
            Ok(value) => results.push(value),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        if failure.is_none()
            && let Some(item) = items.next()
        {
            in_flight.push(f(item));
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(results),
    }
}
