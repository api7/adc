//! Generates layered synthetic ADC configuration fixtures for benchmarking
//! `DifferV4`. Written once to disk so both the Rust (criterion) benchmark
//! and an equivalent TS benchmark can load the exact same inputs and produce
//! comparable numbers.
//!
//! Each "service" is a `route + service + upstream` bundle: one service body
//! (with its own plugin + inline `upstream`) and two nested routes (each with
//! their own plugin). Scale is expressed as the number of such bundles, so
//! the actual resource count is roughly 3x the scale number.
//!
//! Run with: `cargo run -p adc-differ --example gen_fixtures`

use std::path::PathBuf;

use adc_sdk::utils::generate_id;
use serde_json::{Value, json};

const SCALES: &[(&str, usize)] = &[("small", 100), ("medium", 1_000), ("large", 10_000)];
const CHANGE_RATIOS: &[(&str, f64)] = &[("none", 0.0), ("few", 0.05), ("many", 0.5)];

fn route(name: &str, plugin_value: &str) -> Value {
    json!({
        "name": name,
        "uris": [format!("/{name}")],
        "methods": ["GET"],
        "plugins": { "test-plugin": { "value": plugin_value } },
    })
}

fn service(index: usize, mutated: bool) -> Value {
    let name = format!("svc-{index}");
    let suffix = if mutated { "v2" } else { "v1" };
    json!({
        "name": name,
        "description": if mutated { format!("desc for {name} (updated)") } else { format!("desc for {name}") },
        "plugins": { "limit-count": { "count": 100, "time_window": 60 } },
        "upstream": { "nodes": [{ "host": format!("10.0.{}.{}", index / 256, index % 256), "port": 8080, "weight": 1 }] },
        "routes": [route("route-a", suffix), route("route-b", "v1")],
    })
}

/// Recursively strip `id` fields, mimicking a hand-written local config file
/// that doesn't pin resource ids (ids get derived from names on sync).
fn strip_ids(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("id");
            for value in map.values_mut() {
                strip_ids(value);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                strip_ids(item);
            }
        }
        _ => {}
    }
}

/// Assign server-side ids the way a real backend would after a prior sync:
/// service id = hash(name), route id = hash(service_name.route_name).
fn assign_remote_ids(services: &mut [Value]) {
    for svc in services.iter_mut() {
        let name = svc["name"].as_str().unwrap().to_string();
        svc["id"] = json!(generate_id(&name));
        if let Some(routes) = svc["routes"].as_array_mut() {
            for route in routes.iter_mut() {
                let route_name = route["name"].as_str().unwrap().to_string();
                route["id"] = json!(generate_id(&format!("{name}.{route_name}")));
            }
        }
    }
}

fn build_remote(count: usize) -> Value {
    let mut services: Vec<Value> = (0..count).map(|i| service(i, false)).collect();
    assign_remote_ids(&mut services);
    json!({ "services": services })
}

fn build_local(count: usize, change_ratio: f64) -> Value {
    let touched = (count as f64 * change_ratio).round() as usize;
    let mut services: Vec<Value> = (0..count).map(|i| service(i, i < touched)).collect();
    for s in &mut services {
        strip_ids(s);
    }
    json!({ "services": services })
}

fn main() {
    let out_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benches/fixtures");
    std::fs::create_dir_all(&out_dir).expect("create fixtures dir");

    for &(scale_name, count) in SCALES {
        let remote = build_remote(count);
        let remote_path = out_dir.join(format!("{scale_name}.remote.json"));
        std::fs::write(&remote_path, serde_json::to_vec(&remote).unwrap()).unwrap();
        println!("wrote {} ({} services)", remote_path.display(), count);

        for &(scenario_name, ratio) in CHANGE_RATIOS {
            let local = build_local(count, ratio);
            let local_path = out_dir.join(format!("{scale_name}.{scenario_name}.local.json"));
            std::fs::write(&local_path, serde_json::to_vec(&local).unwrap()).unwrap();
            println!("wrote {}", local_path.display());
        }
    }
}
