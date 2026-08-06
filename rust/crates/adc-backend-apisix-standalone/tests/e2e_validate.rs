//! Ported from `libs/backend-apisix-standalone/e2e/validate.e2e-spec.ts`.
//! Real network calls against a live standalone APISIX instance — see
//! `common`'s module doc for how to bring one up and run this file.
//!
//! Deliberately thin: `Backend::validate` is a straight delegation to
//! `adc_backend_apisix::Validator` against the first configured server (the
//! same `/apisix/admin/configs/validate` endpoint apisix's own `Backend`
//! uses) — every validation rule this could exercise is already covered by
//! `adc-backend-apisix`'s own `tests/e2e_validate.rs`. This file only checks
//! that standalone's wiring actually reaches it and reports what it says.
//!
//! Requires apisix >= 3.17.0 (the endpoint itself doesn't exist before
//! that — confirmed against a real instance by `adc-backend-apisix`'s own
//! e2e suite), not the 3.16.0 the TS suite's `validate.e2e-spec.ts` gates
//! on for this backend specifically.

use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;

mod common;
use common::{apisix_version, backend, diff, empty_configuration};

macro_rules! skip_below_3_17_0 {
    () => {
        if apisix_version() < semver::Version::new(3, 17, 0) {
            eprintln!("skipping: validate requires apisix >= 3.17.0");
            return;
        }
    };
}

#[tokio::test]
#[ignore]
async fn succeeds_with_an_empty_configuration() {
    skip_below_3_17_0!();
    let backend = backend("validate-e2e");

    let result = backend.validate(&[]).await.unwrap();
    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn succeeds_with_a_valid_service_and_route() {
    skip_below_3_17_0!();
    let backend = backend("validate-e2e");

    let service = adc::Service {
        id: None,
        name: "validate-test-svc".to_string(),
        description: None,
        labels: None,
        upstream: Some(adc::Upstream {
            id: None,
            name: None,
            description: None,
            labels: None,
            r#type: adc::UpstreamBalancer::default(),
            hash_on: None,
            key: None,
            checks: None,
            nodes: Some(vec![adc::UpstreamNode { host: "httpbin.org".to_string(), port: 80, weight: 100, priority: 0, metadata: None }]),
            scheme: adc::UpstreamScheme::Http,
            retries: None,
            retry_timeout: None,
            timeout: None,
            tls: None,
            keepalive_pool: None,
            pass_host: adc::UpstreamPassHost::default(),
            upstream_host: None,
            service_name: None,
            discovery_type: None,
            discovery_args: None,
        }),
        upstreams: None,
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: Some(adc::ServiceRoutes::Http {
            routes: vec![adc::Route {
                id: None,
                name: "validate-test-route".to_string(),
                description: None,
                labels: None,
                hosts: None,
                uris: vec!["/validate-test".to_string()],
                priority: None,
                timeout: None,
                vars: None,
                methods: Some(vec![adc::HttpMethod::Get]),
                enable_websocket: None,
                remote_addrs: None,
                plugins: None,
                filter_func: None,
            }],
        }),
    };
    let local = Configuration { services: Some(vec![service]), ..empty_configuration() };
    let events = diff(&local, &empty_configuration());

    let result = backend.validate(&events).await.unwrap();
    assert!(result.success, "{:?}", result.errors);
    assert!(result.errors.is_empty());
}

#[tokio::test]
#[ignore]
async fn fails_with_an_invalid_plugin_configuration() {
    skip_below_3_17_0!();
    let backend = backend("validate-e2e");

    let mut plugins = adc::Plugins::new();
    // limit-count requires `count`/`time_window`; both are missing.
    plugins.insert("limit-count".to_string(), serde_json::json!({}));
    let service = adc::Service {
        id: None,
        name: "validate-bad-plugin-svc".to_string(),
        description: None,
        labels: None,
        upstream: Some(adc::Upstream {
            id: None,
            name: None,
            description: None,
            labels: None,
            r#type: adc::UpstreamBalancer::default(),
            hash_on: None,
            key: None,
            checks: None,
            nodes: Some(vec![adc::UpstreamNode { host: "httpbin.org".to_string(), port: 80, weight: 100, priority: 0, metadata: None }]),
            scheme: adc::UpstreamScheme::Http,
            retries: None,
            retry_timeout: None,
            timeout: None,
            tls: None,
            keepalive_pool: None,
            pass_host: adc::UpstreamPassHost::default(),
            upstream_host: None,
            service_name: None,
            discovery_type: None,
            discovery_args: None,
        }),
        upstreams: None,
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: Some(adc::ServiceRoutes::Http {
            routes: vec![adc::Route {
                id: None,
                name: "validate-bad-plugin-route".to_string(),
                description: None,
                labels: None,
                hosts: None,
                uris: vec!["/bad-plugin".to_string()],
                priority: None,
                timeout: None,
                vars: None,
                methods: None,
                enable_websocket: None,
                remote_addrs: None,
                plugins: Some(plugins),
                filter_func: None,
            }],
        }),
    };
    let local = Configuration { services: Some(vec![service]), ..empty_configuration() };
    let events = diff(&local, &empty_configuration());

    let result = backend.validate(&events).await.unwrap();
    assert!(!result.success);
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].resource_type, "routes");
}
