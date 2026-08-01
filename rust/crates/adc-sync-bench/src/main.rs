//! Theoretical-equivalent Rust reimplementation of `backend-apisix`'s sync
//! HTTP request pattern (one PUT/DELETE per resource event), built to
//! directly compare against the TS/axios+RxJS measurement in
//! `apps/cli/bench/e2e-isolated.bench.ts` — same fixture, same mock server
//! (`mock-apisix-standalone-server.mjs`), same "one request per diff event"
//! shape, same retry wrapper shape (count=3, delay=100ms, inert against a
//! healthy mock). Not a full backend-apisix port: skips the SERVICE-with-
//! inline-upstream double-request special case and the real transformer
//! logic, since the point is comparing *per-request client-side overhead*
//! (reqwest+tokio vs axios+RxJS), not re-deriving business rules already
//! covered by the differ port.
//!
//! Usage:
//!   adc-sync-bench <fixture.json> <server_base_url> [concurrency] [iterations] [runtime: current|multi]

use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use adc_differ::DifferV4;
use adc_sdk::{Event, EventType, InternalConfiguration, ResourceType};
use cpu_time::ProcessTime;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde_json::Value;

fn api_name(rt: ResourceType) -> &'static str {
    match rt {
        ResourceType::Route => "routes",
        ResourceType::Service => "services",
        ResourceType::Upstream => "upstreams",
        ResourceType::Ssl => "ssls",
        ResourceType::GlobalRule => "global_rules",
        ResourceType::PluginConfig => "plugin_configs",
        ResourceType::PluginMetadata => "plugin_metadata",
        ResourceType::Consumer => "consumers",
        ResourceType::ConsumerGroup => "consumer_groups",
        ResourceType::ConsumerCredential => "consumer_credentials", // overridden below
        ResourceType::StreamRoute => "stream_routes",
        ResourceType::InternalStreamService => unreachable!(),
    }
}

fn request_path(event: &Event) -> String {
    if event.resource_type == ResourceType::ConsumerCredential {
        return format!(
            "/apisix/admin/consumers/{}/credentials/{}",
            event.parent_id.as_deref().unwrap_or(""),
            event.resource_id
        );
    }
    format!("/apisix/admin/{}/{}", api_name(event.resource_type), event.resource_id)
}

async fn send_with_retry(client: &Client, base_url: &str, event: &Event) -> Result<(), reqwest::Error> {
    let path = request_path(event);
    let url = format!("{base_url}{path}");
    let is_delete = event.event_type == EventType::Delete;
    let body: Value = event.new_value.clone().unwrap_or(Value::Null);

    let mut attempt = 0;
    loop {
        let result = if is_delete {
            client.delete(&url).send().await
        } else {
            client.put(&url).json(&body).send().await
        };
        match result {
            Ok(resp) => {
                let _ = resp.bytes().await; // drain body, mirrors axios awaiting the full response
                return Ok(());
            }
            Err(e) => {
                attempt += 1;
                if attempt >= 3 {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn run_sync(events: Vec<Event>, base_url: String, concurrency: usize) {
    let client = Arc::new(Client::builder().build().expect("build reqwest client"));
    let base_url = Arc::new(base_url);

    stream::iter(events)
        .map(|event| {
            let client = client.clone();
            let base_url = base_url.clone();
            async move {
                send_with_retry(&client, &base_url, &event)
                    .await
                    .expect("mock server request failed");
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let fixture_path = args.get(1).expect("usage: adc-sync-bench <fixture.json> <server_base_url> [concurrency] [iterations] [runtime]");
    let base_url = args.get(2).expect("missing server_base_url").clone();
    let concurrency: usize = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(10);
    let iterations: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(3);
    let runtime_flavor = args.get(5).map(String::as_str).unwrap_or("current");

    let bytes = std::fs::read(fixture_path).expect("read fixture");
    let local_value: Value = serde_json::from_slice(&bytes).expect("parse fixture json");
    let local: InternalConfiguration = local_value.as_object().cloned().unwrap();
    let empty = InternalConfiguration::new();

    let events = DifferV4::diff(&local, &empty, None, None);
    eprintln!("diff events: {}", events.len());

    let rt = match runtime_flavor {
        "multi" => tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap(),
        _ => tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap(),
    };

    let mut cpu_ms_samples = Vec::with_capacity(iterations);
    let mut wall_ms_samples = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let events = events.clone();
        let base_url = base_url.clone();
        let cpu_start = ProcessTime::now();
        let wall_start = Instant::now();
        rt.block_on(run_sync(events, base_url, concurrency));
        wall_ms_samples.push(wall_start.elapsed().as_secs_f64() * 1000.0);
        cpu_ms_samples.push(cpu_start.elapsed().as_secs_f64() * 1000.0);
    }

    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let median = |v: &[f64]| {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s[s.len() / 2]
    };

    println!(
        "\n=== rust reqwest+tokio sync ({runtime_flavor}-thread runtime, concurrency={concurrency}, n={iterations}, events={}) ===",
        events.len()
    );
    println!(
        "cpu: mean={:.2}ms median={:.2}ms   wall: mean={:.2}ms median={:.2}ms   cpu/wall={:.0}%",
        mean(&cpu_ms_samples),
        median(&cpu_ms_samples),
        mean(&wall_ms_samples),
        median(&wall_ms_samples),
        mean(&cpu_ms_samples) / mean(&wall_ms_samples) * 100.0,
    );
}
