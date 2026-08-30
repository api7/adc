//! Isolates the equality-fast-path idea from everything else `DifferV4::diff`
//! also pays for (per-item cloning, id-stripping, tuple/id-map construction)
//! to find its ceiling: how much does *just* the comparison step save when
//! the two inputs are 100% identical, with none of the surrounding pipeline
//! overhead in the mix? Run with:
//! `cargo bench -p adc-differ --bench ideal_equal_bench`.

use std::path::PathBuf;

use adc_sdk::value_diff::diff_value;
use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benches/fixtures")
}

fn load_value(name: &str) -> Value {
    let path = fixtures_dir().join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("read {}: {e} (run `cargo run -p adc-differ --example gen_fixtures` first)", path.display())
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn bench_identical(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_value_on_identical_tree");

    for scale in ["small", "medium", "large", "xlarge"] {
        // The whole `remote` document (every service/route/upstream), diffed
        // against a byte-identical clone of itself — the absolute best case
        // for the equality fast path: nothing anywhere in the tree differs.
        let doc = load_value(&format!("{scale}.remote.json"));
        let clone = doc.clone();
        group.bench_function(scale, |b| {
            b.iter(|| diff_value(std::hint::black_box(&doc), std::hint::black_box(&clone)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_identical);
criterion_main!(benches);
