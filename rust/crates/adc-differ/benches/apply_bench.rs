//! Benchmarks `apply` against the same fixtures/scales `differ_bench.rs`
//! uses for `DifferV4::diff`, so the two numbers are directly comparable —
//! this is specifically to answer "is skipping `apply` after already having
//! `events` worth the complexity it'd take on", not to characterize `apply`
//! in isolation. Run with: `cargo bench -p adc-differ --bench apply_bench`.

use std::path::PathBuf;

use adc_differ::DifferV4;
use adc_sdk::resources::FlatConfiguration;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SCALES: &[(&str, u64)] = &[("small", 100), ("medium", 1_000), ("large", 10_000), ("xlarge", 50_000)];
const SCENARIOS: &[&str] = &["none", "few", "many"];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benches/fixtures")
}

fn load(name: &str) -> FlatConfiguration {
    let path = fixtures_dir().join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("read {}: {e} (run `cargo run -p adc-differ --example gen_fixtures` first)", path.display())
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn bench_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply");

    for &(scale_name, service_count) in SCALES {
        let remote = load(&format!("{scale_name}.remote.json"));
        group.throughput(Throughput::Elements(service_count));

        for &scenario in SCENARIOS {
            let local = load(&format!("{scale_name}.{scenario}.local.json"));
            let events = DifferV4::diff(&local, &remote, None);
            group.bench_with_input(BenchmarkId::new(scenario, scale_name), &(events, remote.clone()), |b, (events, remote)| {
                b.iter(|| adc_differ::apply(std::hint::black_box(events), std::hint::black_box(remote)));
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_apply);
criterion_main!(benches);
