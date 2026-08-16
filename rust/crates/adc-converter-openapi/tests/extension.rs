//! `x-adc-name`/`x-adc-labels`/`x-adc-plugins`/`x-adc-*-defaults` extension
//! field handling. The `x-adc-*-defaults` fixtures (cases 7-12) use real
//! `Route`/`Service`/`Upstream`/`UpstreamNode` field names rather than
//! placeholder ones: those types are `#[serde(deny_unknown_fields)]`
//! structs (by design — see `adc-sdk/src/resources/mod.rs`'s module doc),
//! so an arbitrary unknown key would fail the final
//! `serde_json::from_value::<Configuration>` rather than pass through.
//! Each fixture exercises the same root->path->operation
//! override-precedence chain through fields that are actually part of the
//! resource shape (`id`/`filter_func`/`priority` for routes,
//! `hosts`/`strip_path_prefix`/`description` for services,
//! `hash_on`/`key`/`retries` for upstreams, `priority`/`metadata` for
//! upstream nodes). Cases 10-11 combine several of those defaults kinds at
//! once.

use adc_sdk::Converter;
use adc_sdk::resources::{Configuration, LabelValue, Service};
use adc_converter_openapi::OpenApiConverter;

fn read_fixture(fixture: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/assets/{fixture}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn convert(fixture: &str) -> Result<Configuration, adc_sdk::ConvertError> {
    OpenApiConverter.to_adc(&read_fixture(fixture))
}

#[test]
fn case_1_override_resource_name() {
    let config = convert("extension-1.yaml").unwrap();
    let services = config.services.unwrap();
    assert_eq!(services.len(), 1);
    let svc = &services[0];
    assert_eq!(svc.name, "override service name");
    let routes = svc.routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes[0].name, "override route name");
}

#[test]
fn case_2_empty_override_name_is_rejected() {
    assert!(convert("extension-2.yaml").is_err());
}

#[test]
fn case_3_labels_are_attached_to_the_service_and_route() {
    let config = convert("extension-3.yaml").unwrap();
    let services = config.services.unwrap();
    let svc = &services[0];
    let labels = svc.labels.as_ref().unwrap();
    assert_eq!(labels.get("test1"), Some(&LabelValue::Single("test1".to_string())));

    let route = &svc.routes.as_ref().unwrap().http().unwrap()[0];
    let route_labels = route.labels.as_ref().unwrap();
    assert_eq!(route_labels.get("test2"), Some(&LabelValue::Single("test2".to_string())));
    assert_eq!(route_labels.get("test3"), Some(&LabelValue::Multiple(vec!["test3".to_string(), "test4".to_string()])));
}

#[test]
fn case_4_a_nested_object_label_value_is_rejected() {
    // x-adc-labels.test1 is `{test2: test}` — an object, not the
    // string/string[] `Labels` allows; caught by the final
    // `serde_json::from_value::<Configuration>` deserialize, same as any
    // other shape error this crate leaves to serde rather than
    // hand-checking (see `crate::validate`'s module doc).
    assert!(convert("extension-4.yaml").is_err());
}

fn service<'a>(config: &'a Configuration, name: &str) -> &'a Service {
    config.services.as_ref().unwrap().iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no service named {name:?}"))
}

#[test]
fn case_5_root_and_operation_plugins_merge_with_individual_keys_winning() {
    let config = convert("extension-5.yaml").unwrap();
    let svc = &config.services.unwrap()[0];
    let plugins = svc.plugins.as_ref().unwrap();
    assert_eq!(plugins.get("test1"), Some(&serde_json::json!({"test1-key": "test1-value"})));
    // x-adc-plugin-test2 overrides the "test2" entry from x-adc-plugins.
    assert_eq!(plugins.get("test2"), Some(&serde_json::json!({"test2-key": "test3-value-override"})));
    assert_eq!(plugins.get("test3"), Some(&serde_json::json!({"test3-key": "test3-value"})));

    let route = &svc.routes.as_ref().unwrap().http().unwrap()[0];
    let route_plugins = route.plugins.as_ref().unwrap();
    assert_eq!(route_plugins.get("test1"), Some(&serde_json::json!({"test1-key": "test1-value"})));
    assert_eq!(route_plugins.get("test2"), Some(&serde_json::json!({"test2-key": "test3-value-override"})));
    assert_eq!(route_plugins.get("test3"), Some(&serde_json::json!({"test3-key": "test3-value"})));
}

#[test]
fn case_6_plugins_accumulate_from_root_through_path_to_operation() {
    let config = convert("extension-6.yaml").unwrap();
    let svc = &config.services.unwrap()[0];
    let plugins = svc.plugins.as_ref().unwrap();
    assert_eq!(plugins.get("root1"), Some(&serde_json::json!({"root1-key": "value"})));
    assert_eq!(plugins.get("root2"), Some(&serde_json::json!({"root2-key": "value"})));

    let routes = svc.routes.as_ref().unwrap().http().unwrap();
    let get = routes.iter().find(|r| r.name == "httpbin.org_anything_get").unwrap();
    let get_plugins = get.plugins.as_ref().unwrap();
    assert_eq!(get_plugins.get("path1"), Some(&serde_json::json!({"path1-key": "value"})));
    // The operation's own x-adc-plugins.path2 overrides the path level one.
    assert_eq!(get_plugins.get("path2"), Some(&serde_json::json!({"path2-key": "value-override"})));
    assert_eq!(get_plugins.get("method1"), Some(&serde_json::json!({"method1-key": "value"})));
    assert_eq!(get_plugins.get("method2"), Some(&serde_json::json!({"method2-key": "value"})));

    let put = routes.iter().find(|r| r.name == "httpbin.org_anything_put").unwrap();
    let put_plugins = put.plugins.as_ref().unwrap();
    assert_eq!(put_plugins.get("path1"), Some(&serde_json::json!({"path1-key": "value"})));
    assert_eq!(put_plugins.get("path2"), Some(&serde_json::json!({"path2-key": "value"})));
    assert!(put_plugins.get("method1").is_none());
}

#[test]
fn case_7_route_defaults_layer_root_path_and_operation() {
    let config = convert("extension-7.yaml").unwrap();

    let main = service(&config, "httpbin.org");
    let put = &main.routes.as_ref().unwrap().http().unwrap()[0];
    assert_eq!(put.id.as_deref(), Some("root-id"));
    assert_eq!(put.filter_func.as_deref(), Some("return true -- path"));
    assert_eq!(put.priority, Some(3));

    let split = service(&config, "httpbin.org_anything_get");
    let get = &split.routes.as_ref().unwrap().http().unwrap()[0];
    assert_eq!(get.id.as_deref(), Some("root-id"));
    assert_eq!(get.filter_func.as_deref(), Some("return true -- path"));
    assert_eq!(get.priority, Some(30));
}

#[test]
fn case_8_service_defaults_layer_root_path_and_operation() {
    let config = convert("extension-8.yaml").unwrap();
    // The path-level x-adc-service-defaults alone is enough to split
    // "/anything" out of the main service, which then has no routes left
    // and gets filtered out entirely.
    assert!(config.services.as_ref().unwrap().iter().all(|s| s.name != "httpbin.org"));

    let path_split = service(&config, "httpbin.org_anything");
    assert_eq!(path_split.hosts, Some(vec!["root-host".to_string()]));
    assert_eq!(path_split.strip_path_prefix, Some(true));
    assert_eq!(path_split.description.as_deref(), Some("root-description"));

    let op_split = service(&config, "httpbin.org_anything_get");
    assert_eq!(op_split.hosts, Some(vec!["root-host".to_string()]));
    assert_eq!(op_split.strip_path_prefix, Some(true));
    assert_eq!(op_split.description.as_deref(), Some("get-description"));
}

#[test]
fn case_9_upstream_defaults_layer_root_path_and_operation() {
    let config = convert("extension-9.yaml").unwrap();
    assert!(config.services.as_ref().unwrap().iter().all(|s| s.name != "httpbin.org"));

    let path_split = service(&config, "httpbin.org_anything");
    let upstream = path_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(3));

    let op_split = service(&config, "httpbin.org_anything_get");
    let upstream = op_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(30));
}

#[test]
fn case_10_service_and_upstream_defaults_layer_together() {
    let config = convert("extension-10.yaml").unwrap();
    assert!(config.services.as_ref().unwrap().iter().all(|s| s.name != "httpbin.org"));

    let path_split = service(&config, "httpbin.org_anything");
    assert_eq!(path_split.hosts, Some(vec!["root-host".to_string()]));
    assert_eq!(path_split.strip_path_prefix, Some(true));
    assert_eq!(path_split.description.as_deref(), Some("root-description"));
    let upstream = path_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(3));

    let op_split = service(&config, "httpbin.org_anything_get");
    assert_eq!(op_split.hosts, Some(vec!["root-host".to_string()]));
    assert_eq!(op_split.strip_path_prefix, Some(true));
    assert_eq!(op_split.description.as_deref(), Some("get-description"));
    let upstream = op_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(30));
}

#[test]
fn case_11_route_service_and_upstream_defaults_layer_together_across_six_methods() {
    let config = convert("extension-11.yaml").unwrap();
    let services = config.services.as_ref().unwrap();
    assert!(services.iter().all(|s| s.name != "httpbin.org"));
    assert_eq!(services.len(), 5);

    // "/anything" only splits off its own operation-level service when the
    // operation carries its own service/upstream defaults or `servers` —
    // x-adc-route-defaults alone (PUT, OPTIONS) never triggers a split, so
    // both land in the path-level split alongside each other.
    let path_split = service(&config, "httpbin.org_anything");
    assert_eq!(path_split.hosts, Some(vec!["root-host".to_string()]));
    assert_eq!(path_split.strip_path_prefix, Some(true));
    assert_eq!(path_split.description.as_deref(), Some("root-description"));
    let upstream = path_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(3));
    let routes = path_split.routes.as_ref().unwrap().http().unwrap();
    assert_eq!(routes.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["httpbin.org_anything_put", "httpbin.org_anything_options"]);
    let put = routes.iter().find(|r| r.name == "httpbin.org_anything_put").unwrap();
    assert_eq!(put.id.as_deref(), Some("root-id"));
    assert_eq!(put.filter_func.as_deref(), Some("return true -- path"));
    assert_eq!(put.priority, Some(300));
    let options = routes.iter().find(|r| r.name == "httpbin.org_anything_options").unwrap();
    assert_eq!(options.priority, Some(3));

    let get_split = service(&config, "httpbin.org_anything_get");
    assert_eq!(get_split.description.as_deref(), Some("get-description"));
    assert_eq!(get_split.strip_path_prefix, Some(true));
    let upstream = get_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.hash_on.as_deref(), Some("vars"));
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(30));
    let get = &get_split.routes.as_ref().unwrap().http().unwrap()[0];
    assert_eq!(get.priority, Some(30));

    let post_split = service(&config, "httpbin.org_anything_post");
    assert_eq!(post_split.description.as_deref(), Some("post-description"));
    assert_eq!(post_split.strip_path_prefix, Some(true));
    let upstream = post_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(3));
    let post = &post_split.routes.as_ref().unwrap().http().unwrap()[0];
    assert_eq!(post.priority, Some(3));

    let delete_split = service(&config, "httpbin.org_anything_delete");
    assert_eq!(delete_split.description.as_deref(), Some("root-description"));
    let upstream = delete_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(3000));

    let patch_split = service(&config, "httpbin.org_anything_patch");
    assert_eq!(patch_split.description.as_deref(), Some("patch-description"));
    let upstream = patch_split.upstream.as_ref().unwrap();
    assert_eq!(upstream.key.as_deref(), Some("path-key"));
    assert_eq!(upstream.retries, Some(4000));
}

#[test]
fn case_12_upstream_node_defaults_are_independent_per_service() {
    let config = convert("extension-12.yaml").unwrap();

    let main = service(&config, "httpbin.org");
    let nodes = main.upstream.as_ref().unwrap().nodes.as_ref().unwrap();
    assert_eq!(nodes.len(), 2);
    let org = nodes.iter().find(|n| n.host == "httpbin.org").unwrap();
    assert_eq!(org.priority, 1);
    assert_eq!(org.metadata.as_ref().unwrap().get("tag"), Some(&serde_json::json!("root")));
    let com = nodes.iter().find(|n| n.host == "httpbin.com").unwrap();
    assert_eq!(com.priority, 2);
    assert!(com.metadata.is_none());

    let split = service(&config, "httpbin.org_anything_get");
    let nodes = split.upstream.as_ref().unwrap().nodes.as_ref().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].host, "httpbin.org");
    assert_eq!(nodes[0].priority, 10);
    assert_eq!(nodes[0].metadata.as_ref().unwrap().get("tag"), Some(&serde_json::json!("override")));
}
