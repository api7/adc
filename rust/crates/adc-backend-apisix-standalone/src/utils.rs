use std::sync::LazyLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static ORIGIN: LazyLock<(SystemTime, Instant)> = LazyLock::new(|| (SystemTime::now(), Instant::now()));

pub fn stable_timestamp() -> i64 {
    let (origin_wall, origin_monotonic) = *ORIGIN;
    let wall = origin_wall + origin_monotonic.elapsed();
    wall.duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_calls_never_decrease() {
        let mut previous = stable_timestamp();
        for _ in 0..1000 {
            let current = stable_timestamp();
            assert!(current >= previous, "{current} < {previous}");
            previous = current;
        }
    }
}
