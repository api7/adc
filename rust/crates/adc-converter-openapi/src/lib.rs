//! Converts an OpenAPI (2.0/3.0/3.1) document into an ADC `Configuration`.
//! Deliberately narrower than a general OpenAPI toolkit: it doesn't do full
//! JSON Schema validation, `allOf`/`oneOf`/`anyOf` merging, external `$ref`
//! resolution, or Swagger 2.0's `definitions`/`parameters`/`consumes`
//! machinery — none of that is reachable once `prune` strips a document
//! down to the handful of fields this crate actually reads (`info`,
//! `servers`, and the `paths.*.{method}` fields plus `x-adc-*` extensions).
//! See each submodule's own doc comment for specifics.
//!
//! No strongly typed OpenAPI document model: the fields this crate cares
//! about are validated loosely (`crate::validate`'s module doc) and every
//! other field is opaque, so a `serde_json::Value`/`Map` tree is a better
//! fit than a struct hierarchy that would mostly just describe fields
//! nothing here reads.
//!
//! One deliberate simplification: `Configuration.services` isn't an
//! ordered collection as far as anything downstream cares, so this always
//! places the main service first followed by every split service in the
//! order its path/operation was encountered, rather than tracking the
//! exact position each kind of split would occupy.

mod dereference;
mod extension;
mod merge;
mod parser;
mod prune;
mod slugify;
mod upgrade;
mod validate;

use std::sync::LazyLock;

use adc_sdk::resources::Configuration;
use adc_sdk::{ConvertError, Converter};
use merge::shallow_merge;
use regex::Regex;
use serde_json::{Map, Value, json};
use slugify::slugify;

pub(crate) const HTTP_METHODS: &[&str] = &["get", "put", "post", "delete", "options", "head", "patch", "trace"];

static PATH_VARIABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{[^}]+\}").expect("valid regex"));

pub struct OpenApiConverter;

impl Converter for OpenApiConverter {
    fn to_adc(&self, input: &str) -> Result<Configuration, ConvertError> {
        let spec = parse_oas(input)?;
        let services = build_services(&spec)?;
        let value = json!({ "services": services });
        serde_json::from_value(value).map_err(|e| ConvertError(format!("failed to build ADC configuration: {e}")))
    }
}

/// Turns raw file content into a validated, ready-to-read document: parse
/// -> narrow Swagger 2.0 upgrade -> prune -> dereference -> validate, in
/// that order (see `crate::dereference` and `crate::prune`'s module docs
/// for why the prune step has to run before dereferencing).
fn parse_oas(content: &str) -> Result<Map<String, Value>, ConvertError> {
    let mut document = upgrade::parse_document(content)?;
    let object = document.as_object_mut().ok_or_else(|| ConvertError("OpenAPI document must be an object".to_string()))?;
    upgrade::upgrade_swagger_2_servers(object);
    prune::prune_conversion_document(object);

    let document = dereference::dereference(&document)?;
    let spec = document.as_object().ok_or_else(|| ConvertError("OpenAPI document must be an object".to_string()))?.clone();
    validate::validate_document(&spec)?;
    Ok(spec)
}

fn slug_join(parts: &[&str]) -> String {
    parts.iter().map(|p| slugify(p)).collect::<Vec<_>>().join("_")
}

fn convert_path_template(path: &str) -> String {
    PATH_VARIABLE.replace_all(path, |caps: &regex::Captures| format!(":{}", &caps[0][1..caps[0].len() - 1])).into_owned()
}

fn build_main_service(spec: &Map<String, Value>, title: &str) -> Result<Map<String, Value>, ConvertError> {
    let info = spec.get("info").and_then(Value::as_object);
    let name =
        spec.get(extension::NAME).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| title.to_string());

    let mut service = Map::new();
    service.insert("name".to_string(), Value::String(name));
    service.insert("description".to_string(), info.and_then(|i| i.get("description")).cloned().unwrap_or(Value::Null));
    service.insert("labels".to_string(), spec.get(extension::LABELS).cloned().unwrap_or(Value::Null));
    service.insert(
        "plugins".to_string(),
        extension::parse_ext_plugins(spec).map(Value::Object).unwrap_or(Value::Null),
    );
    service.insert("routes".to_string(), Value::Array(Vec::new()));

    let servers = spec.get("servers").and_then(Value::as_array).ok_or_else(|| ConvertError("servers is required".to_string()))?;
    let upstream_defaults = spec.get(extension::UPSTREAM_DEFAULTS).and_then(Value::as_object);
    let generated = parser::transform_upstream(servers, upstream_defaults)?;
    shallow_merge(&mut service, &generated);

    if let Some(defaults) = spec.get(extension::SERVICE_DEFAULTS).and_then(Value::as_object) {
        shallow_merge(&mut service, defaults);
    }
    Ok(service)
}

/// Builds one route: name (with its fallback chain), plugins (merged from
/// path and operation level), and the root->path->operation route-defaults
/// override chain.
#[allow(clippy::too_many_arguments)]
fn build_route(
    spec: &Map<String, Value>,
    path: &str,
    path_item: &Map<String, Value>,
    method: &str,
    operation: &Map<String, Value>,
    path_plugins: &Map<String, Value>,
    title: &str,
) -> Value {
    let mut plugins = path_plugins.clone();
    if let Some(op_plugins) = extension::parse_ext_plugins(operation) {
        shallow_merge(&mut plugins, &op_plugins);
    }

    let default_name = slug_join(&[title, path, method]);
    let name = operation
        .get(extension::NAME)
        .and_then(Value::as_str)
        .or_else(|| operation.get("operationId").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or(default_name);

    let mut route = Map::new();
    route.insert("name".to_string(), Value::String(name));
    route.insert(
        "description".to_string(),
        operation.get("summary").or_else(|| operation.get("description")).cloned().unwrap_or(Value::Null),
    );
    route.insert("labels".to_string(), operation.get(extension::LABELS).cloned().unwrap_or(Value::Null));
    route.insert("methods".to_string(), Value::Array(vec![Value::String(method.to_uppercase())]));
    route.insert("uris".to_string(), Value::Array(vec![Value::String(convert_path_template(path))]));
    route.insert("plugins".to_string(), if plugins.is_empty() { Value::Null } else { Value::Object(plugins) });

    // Route defaults, root -> path -> operation, each layer overwriting the last.
    for defaults_source in [spec.get(extension::ROUTE_DEFAULTS), path_item.get(extension::ROUTE_DEFAULTS), operation.get(extension::ROUTE_DEFAULTS)] {
        if let Some(Value::Object(defaults)) = defaults_source {
            shallow_merge(&mut route, defaults);
        }
    }

    Value::Object(route)
}

fn build_services(spec: &Map<String, Value>) -> Result<Vec<Value>, ConvertError> {
    let title = spec
        .get("info")
        .and_then(Value::as_object)
        .and_then(|i| i.get("title"))
        .and_then(Value::as_str)
        .expect("validated by validate_document");

    let mut main_service = build_main_service(spec, title)?;
    let mut split_services: Vec<Map<String, Value>> = Vec::new();

    if let Some(paths) = spec.get("paths").and_then(Value::as_object) {
        for (path, path_item_value) in paths {
            let Some(path_item) = path_item_value.as_object() else { continue };

            let main_service_name = main_service.get("name").and_then(Value::as_str).unwrap_or_default();
            let path_split_name = slug_join(&[main_service_name, path]);
            let path_split_service = parser::parse_separate_service(&main_service, &path_split_name, Some(path_item), None, None)?;

            let path_plugins = extension::parse_ext_plugins(path_item).unwrap_or_default();
            let path_upstream_defaults = path_item.get(extension::UPSTREAM_DEFAULTS).and_then(Value::as_object);
            let path_service_defaults = path_item.get(extension::SERVICE_DEFAULTS).and_then(Value::as_object);
            let root_upstream_defaults = spec.get(extension::UPSTREAM_DEFAULTS).and_then(Value::as_object);
            let mut op_upstream_defaults = Map::new();
            if let Some(d) = root_upstream_defaults {
                shallow_merge(&mut op_upstream_defaults, d);
            }
            if let Some(d) = path_upstream_defaults {
                shallow_merge(&mut op_upstream_defaults, d);
            }

            let mut routes = Vec::new();
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method).and_then(Value::as_object) else { continue };

                let op_split_name = slug_join(&[main_service_name, path, method]);
                let op_split_service = parser::parse_separate_service(
                    &main_service,
                    &op_split_name,
                    Some(operation),
                    path_service_defaults,
                    Some(&op_upstream_defaults),
                )?;

                let route = build_route(spec, path, path_item, method, operation, &path_plugins, title);

                if let Some(mut sep) = op_split_service {
                    sep.insert("routes".to_string(), Value::Array(vec![route]));
                    split_services.push(sep);
                    log::info!("{} \"{path}\" contains the service or upstream defaults, so it will be included to the separate service", method.to_uppercase());
                } else {
                    routes.push(route);
                }
            }

            if let Some(mut sep) = path_split_service {
                if routes.is_empty() {
                    continue;
                }
                sep.insert("routes".to_string(), Value::Array(routes));
                split_services.push(sep);
                log::info!("Path \"{path}\" contains the service or upstream defaults, so it will be included to the separate service");
            } else if let Some(Value::Array(main_routes)) = main_service.get_mut("routes") {
                main_routes.extend(routes);
            }
        }
    }

    let mut services: Vec<Map<String, Value>> = Vec::with_capacity(1 + split_services.len());
    services.push(main_service);
    services.extend(split_services);

    let services = services
        .into_iter()
        .map(|mut service| {
            inline_path_prefix(&mut service);
            service
        })
        .filter(|service| matches!(service.get("routes"), Some(Value::Array(r)) if !r.is_empty()))
        .map(Value::Object)
        .collect();
    Ok(services)
}

/// Folds a service's `path_prefix` directly into each of its routes' `uris`
/// and removes the field — APISIX has no first-class `path_prefix` concept
/// on a service, so this needs to be inlined for that backend to work.
fn inline_path_prefix(service: &mut Map<String, Value>) {
    let Some(Value::String(prefix)) = service.remove("path_prefix") else { return };
    // A trailing "/" (e.g. a server url like ".../v1/") would otherwise
    // double up against a route uri that already starts with "/".
    let prefix = prefix.trim_end_matches('/');
    if let Some(Value::Array(routes)) = service.get_mut("routes") {
        for route in routes {
            if let Some(Value::Array(uris)) = route.get_mut("uris") {
                for uri in uris {
                    if let Value::String(uri) = uri {
                        *uri = format!("{prefix}{uri}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_trailing_slash_on_the_prefix_does_not_double_up_against_the_uri() {
        let mut service =
            json!({"path_prefix": "/v1/", "routes": [{"uris": ["/foo"]}]}).as_object().unwrap().clone();
        inline_path_prefix(&mut service);
        assert_eq!(service["routes"][0]["uris"], json!(["/v1/foo"]));
    }

    #[test]
    fn a_prefix_without_a_trailing_slash_is_unaffected() {
        let mut service = json!({"path_prefix": "/v1", "routes": [{"uris": ["/foo"]}]}).as_object().unwrap().clone();
        inline_path_prefix(&mut service);
        assert_eq!(service["routes"][0]["uris"], json!(["/v1/foo"]));
    }
}
