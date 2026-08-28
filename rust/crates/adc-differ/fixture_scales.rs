// Shared between `examples/gen_fixtures.rs` (which generates these fixtures)
// and `tests/fixtures_sanity.rs` (which checks them), via `include!`, so the
// two can't drift apart. Not part of the crate's public API — this is dev
// tooling data, pulled in as source text rather than a `mod`.

const SCALES: &[(&str, usize)] = &[("small", 100), ("medium", 1_000), ("large", 10_000)];
const CHANGE_RATIOS: &[(&str, f64)] = &[("none", 0.0), ("few", 0.05), ("many", 0.5)];
