//! Core conversion scenarios (single path, multiple paths, multiple
//! servers, server variables, per-path/operation service splitting, route
//! naming, and pruning a document with a circular `$ref` before
//! dereferencing), driven by the `basic-*.yaml` fixtures in `tests/assets`.
//!
//! Assertions look services/routes up by name rather than asserting on
//! `services`/`routes` array position: this crate's `services` array order
//! is a deliberate simplification (see the crate's own module doc), so
//! it isn't something a parity test should pin down.

use adc_sdk::Converter;
use adc_sdk::resources::{Configuration, Service};
use adc_converter_openapi::OpenApiConverter;

fn convert(fixture: &str) -> Configuration {
    let content = std::fs::read_to_string(format!("{}/tests/assets/{fixture}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    OpenApiConverter.to_adc(&content).unwrap()
}

fn service<'a>(config: &'a Configuration, name: &str) -> &'a Service {
    config.services.as_ref().unwrap().iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no service named {name:?}"))
}

fn route_names(service: &Service) -> Vec<&str> {
    service.routes.as_ref().unwrap().http().unwrap().iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn case_1_single_path() {
    let config = convert("basic-1.yaml");
    let services = config.services.as_ref().unwrap();
    assert_eq!(services.len(), 1);
    let svc = &services[0];
    assert_eq!(svc.name, "httpbin.org");
    assert_eq!(svc.description.as_deref(), Some("httpbin.org description"));
    assert_eq!(
        route_names(svc),
        vec![
            "httpbin.org_anything_get",
            "httpbin.org_anything_put",
            "httpbin.org_anything_post",
            "httpbin.org_anything_delete",
            "httpbin.org_anything_patch",
        ]
    );
    let upstream = svc.upstream.as_ref().unwrap();
    assert_eq!(upstream.nodes.as_ref().unwrap().len(), 1);
    assert_eq!(upstream.nodes.as_ref().unwrap()[0].host, "httpbin.org");
    assert_eq!(upstream.nodes.as_ref().unwrap()[0].port, 443u16);
}

#[test]
fn case_2_multiple_paths() {
    let config = convert("basic-2.yaml");
    let svc = service(&config, "httpbin.org");
    let routes = svc.routes.as_ref().unwrap().http().unwrap();
    let redirect = routes.iter().find(|r| r.name == "httpbin.org_absolute-redirectn_get").unwrap();
    assert_eq!(redirect.uris, vec!["/absolute-redirect/:n"]);
}

#[test]
fn case_3_multiple_servers() {
    let config = convert("basic-3.yaml");
    let svc = service(&config, "httpbin.org");
    let nodes = svc.upstream.as_ref().unwrap().nodes.as_ref().unwrap();
    let hosts_ports: Vec<(&str, u16)> = nodes.iter().map(|n| (n.host.as_str(), n.port)).collect();
    // The 5th server's path (httpbin.us/{test}) is dropped: only the
    // *first* server's path can become the service's `path_prefix`.
    assert_eq!(
        hosts_ports,
        vec![("httpbin.org", 443), ("httpbin.net", 443), ("httpbin.com", 80), ("httpbin.com", 8080), ("httpbin.us", 80)]
    );
}

#[test]
fn case_4_server_variables() {
    let config = convert("basic-4.yaml");
    let svc = service(&config, "httpbin.org");
    let nodes = svc.upstream.as_ref().unwrap().nodes.as_ref().unwrap();
    assert_eq!(nodes[0].host, "httpbin.us");
    assert_eq!(nodes[1].host, "httpbin.org");
    let routes = svc.routes.as_ref().unwrap().http().unwrap();
    assert!(routes.iter().all(|r| r.uris == vec!["/test1Value/test2Value/anything"]));
}

#[test]
fn case_5_servers_in_path_and_operation() {
    let config = convert("basic-5.yaml");

    let main = service(&config, "httpbin.org");
    assert_eq!(route_names(main), vec!["httpbin.org_absolute-redirectn_get"]);
    assert_eq!(main.upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].host, "httpbin.org");

    let path_split = service(&config, "httpbin.org_anything");
    assert_eq!(path_split.upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].host, "httpbin.net");
    assert_eq!(
        route_names(path_split),
        vec!["httpbin.org_anything_put", "httpbin.org_anything_post", "httpbin.org_anything_delete", "httpbin.org_anything_patch"]
    );

    let op_split = service(&config, "httpbin.org_anything_get");
    assert_eq!(op_split.upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].host, "httpbin.com");
    assert_eq!(route_names(op_split), vec!["httpbin.org_anything_get"]);

    assert_eq!(config.services.as_ref().unwrap().len(), 3);
}

#[test]
fn case_6_route_less_main_service_is_dropped() {
    let config = convert("basic-6.yaml");
    let services = config.services.as_ref().unwrap();
    // The main service ends up with zero routes (every "/anything" route
    // was siphoned into a split service) and is filtered out entirely.
    assert!(services.iter().all(|s| s.name != "httpbin.org"));
    assert_eq!(services.len(), 2);
}

#[test]
fn case_7_route_name_fallback_chain() {
    let config = convert("basic-7.yaml");
    let svc = service(&config, "httpbin.org");
    assert_eq!(route_names(svc), vec!["httpbin.org_anything_get", "Anything_PUT"]);
}

#[test]
fn case_8_circular_component_schema_is_pruned_away_before_dereferencing() {
    // basic-8.yaml's components.schemas has a genuine cycle
    // (SectorNode.children -> SectorNode); prune_conversion_document
    // deletes components.schemas before dereference ever runs, so this
    // must convert cleanly rather than erroring on a circular $ref.
    let config = convert("basic-8.yaml");
    let svc = service(&config, "SectorAPI");
    assert_eq!(route_names(svc), vec!["getSectors"]);
    assert_eq!(svc.upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].host, "localhost");
    assert_eq!(svc.upstream.as_ref().unwrap().nodes.as_ref().unwrap()[0].port, 8080);
}

#[test]
fn case_9_swagger_2_0_document_upgrades_host_baseuri_and_schemes_into_servers() {
    let config = convert("swagger-2.yaml");
    let svc = service(&config, "httpbin.org");
    let upstream = svc.upstream.as_ref().unwrap();
    assert_eq!(upstream.scheme, adc_sdk::resources::UpstreamScheme::Https);
    let nodes = upstream.nodes.as_ref().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].host, "httpbin.org");
    assert_eq!(nodes[0].port, 443);
    let routes = svc.routes.as_ref().unwrap().http().unwrap();
    // basePath "/v1" is inlined into the route's own uri.
    assert_eq!(routes[0].uris, vec!["/v1/anything"]);
}

#[test]
fn case_10_path_and_operation_level_splits_both_honor_the_root_x_adc_name() {
    let config = convert("basic-5-named.yaml");
    let services = config.services.as_ref().unwrap();
    assert!(services.iter().all(|s| s.name != "custom-name"), "the main service has no routes of its own left");
    assert_eq!(services.len(), 2);

    let path_split = service(&config, "custom-name_anything");
    assert_eq!(route_names(path_split), vec!["httpbin.org_anything_put"]);

    let op_split = service(&config, "custom-name_anything_get");
    assert_eq!(route_names(op_split), vec!["httpbin.org_anything_get"]);
}
