//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_resource_service_upstream -- --ignored --test-threads=1`.

use adc_sdk::resources::Configuration;
use semver::Version;
use serde_json::json;

mod common;
use common::{assert_matches_object, dump_configuration, server_version, sync_events};

fn local_config(json: serde_json::Value) -> Configuration {
    serde_json::from_value(json).unwrap()
}

#[tokio::test]
#[ignore]
async fn service_with_multiple_named_upstreams() {
    if server_version() < Version::new(3, 5, 0) {
        eprintln!("skipping: only applies from 3.5.0");
        return;
    }
    let backend = common::backend().await;

    let upstream_nd1_name = "nd-upstream1";
    let upstream_nd1 = json!({ "name": upstream_nd1_name, "type": "roundrobin", "scheme": "https", "nodes": [{ "host": "1.1.1.1", "port": 443, "weight": 100 }] });
    let upstream_nd2_name = "nd-upstream2";
    let upstream_nd2 = json!({ "name": upstream_nd2_name, "type": "roundrobin", "scheme": "https", "nodes": [{ "host": "1.0.0.1", "port": 443, "weight": 100 }] });
    let service_name = "test";
    let service_base = json!({
        "name": service_name,
        "upstream": { "type": "roundrobin", "nodes": [{ "host": "httpbin.org", "port": 443, "weight": 100 }] },
        "path_prefix": "/test",
        "strip_path_prefix": true,
    });
    let mut service = service_base.clone();
    service["upstreams"] = json!([upstream_nd1, upstream_nd2]);

    let remote = dump_configuration(&backend).await.unwrap();
    let events = common::diff(
        &local_config(json!({ "services": [service] })),
        &remote,
        None,
    );
    sync_events(&backend, events).await.unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    assert_matches_object(&serde_json::to_value(&services[0]).unwrap(), &service_base);
    let mut upstreams = services[0].upstreams.clone().unwrap();
    assert_eq!(upstreams.len(), 2);
    upstreams.sort_by(|a, b| a.name.cmp(&b.name));
    assert_matches_object(&serde_json::to_value(&upstreams[0]).unwrap(), &upstream_nd1);
    assert_matches_object(&serde_json::to_value(&upstreams[1]).unwrap(), &upstream_nd2);

    let mut new_upstream_nd1 = upstream_nd1.clone();
    new_upstream_nd1["retry_timeout"] = json!(100);
    let mut service_with_updated_upstream = service_base.clone();
    service_with_updated_upstream["upstreams"] = json!([new_upstream_nd1, upstream_nd2]);
    let remote = dump_configuration(&backend).await.unwrap();
    let events = common::diff(
        &local_config(json!({ "services": [service_with_updated_upstream] })),
        &remote,
        None,
    );
    sync_events(&backend, events).await.unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let mut upstreams = dump.services.unwrap()[0].upstreams.clone().unwrap();
    assert_eq!(upstreams.len(), 2);
    upstreams.sort_by(|a, b| a.name.cmp(&b.name));
    assert_matches_object(
        &serde_json::to_value(&upstreams[0]).unwrap(),
        &new_upstream_nd1,
    );
    assert_matches_object(&serde_json::to_value(&upstreams[1]).unwrap(), &upstream_nd2);

    let mut service_with_one_upstream = service_base.clone();
    service_with_one_upstream["upstreams"] = json!([new_upstream_nd1]);
    let remote = dump_configuration(&backend).await.unwrap();
    let events = common::diff(
        &local_config(json!({ "services": [service_with_one_upstream] })),
        &remote,
        None,
    );
    sync_events(&backend, events).await.unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let services = dump.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    let upstreams = services[0].upstreams.as_ref().unwrap();
    assert_eq!(upstreams.len(), 1);
    assert_matches_object(
        &serde_json::to_value(&upstreams[0]).unwrap(),
        &new_upstream_nd1,
    );

    let remote = dump_configuration(&backend).await.unwrap();
    let events = common::diff(&local_config(json!({})), &remote, None);
    sync_events(&backend, events).await.unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.services.is_none_or(|s| s.is_empty()));
}
