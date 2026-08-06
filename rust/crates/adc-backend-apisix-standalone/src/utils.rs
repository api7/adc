use std::time::{SystemTime, UNIX_EPOCH};

/// A millisecond wall-clock timestamp, used as a synced config's version
/// number. Not truly monotonic on its own (wall-clock time can roll back);
/// [`crate::operator::Operator::sync`] additionally clamps it against the
/// last version accepted by the data plane so the number it actually sends
/// never regresses.
pub fn stable_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_millis() as i64
}
