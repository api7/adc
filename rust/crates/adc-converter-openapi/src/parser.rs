//! Builds service/upstream fields from OpenAPI `servers` entries, and
//! decides when a path/operation needs its own split-off service.

use adc_sdk::ConvertError;
use serde_json::{Map, Value, json};
use url::Url;

use crate::extension;
use crate::merge::shallow_merge;

/// Last-resort fallback when `Url::port_or_known_default()` doesn't
/// recognize the scheme (that method already covers http/https/ws/wss/ftp).
fn get_port(scheme: &str) -> u16 {
    if scheme == "http" { 80 } else { 443 }
}

/// Builds the `path_prefix`/`upstream` fields of a service from OpenAPI
/// `servers`. Returns a `{path_prefix?, upstream}` object, ready to be
/// shallow-merged into the service being built.
pub fn transform_upstream(oas_servers: &[Value], upstream_defaults: Option<&Map<String, Value>>) -> Result<Map<String, Value>, ConvertError> {
    let mut default_scheme = "https".to_string();
    let mut default_path_prefix: Option<String> = None;
    let mut nodes = Vec::with_capacity(oas_servers.len());
    for (idx, server) in oas_servers.iter().enumerate() {
        let mut url_str = server
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| ConvertError("server is missing a url".to_string()))?
            .to_string();
        if let Some(Value::Object(variables)) = server.get("variables") {
            for (name, variable) in variables {
                if let Some(default) = variable.get("default").and_then(Value::as_str) {
                    url_str = url_str.replace(&format!("{{{name}}}"), default);
                }
            }
        }
        let parsed = Url::parse(&url_str).map_err(|e| ConvertError(format!("invalid server url \"{url_str}\": {e}")))?;

        // Read after variable substitution: a scheme placeholder like
        // `{scheme}://host` isn't valid URL syntax until substituted, so
        // parsing the raw (pre-substitution) URL just to read its scheme
        // would fail before ever reaching the substitution above.
        if idx == 0 {
            default_scheme = parsed.scheme().to_string();
            if parsed.path() != "/" {
                default_path_prefix = Some(parsed.path().to_string());
            }
        }

        let host = parsed.host_str().ok_or_else(|| ConvertError(format!("server url \"{url_str}\" has no host")))?;
        let mut node = json!({
            "host": host,
            "port": parsed.port_or_known_default().unwrap_or_else(|| get_port(parsed.scheme())),
            "weight": 100,
        });
        if let Some(Value::Object(defaults)) = server.get(extension::UPSTREAM_NODE_DEFAULTS) {
            shallow_merge(node.as_object_mut().expect("just built as an object"), defaults);
        }
        nodes.push(node);
    }

    let mut upstream = json!({
        "scheme": default_scheme,
        "nodes": nodes,
        "timeout": {"connect": 60, "send": 60, "read": 60},
        "pass_host": "pass",
    });
    if let Some(defaults) = upstream_defaults {
        shallow_merge(upstream.as_object_mut().expect("just built as an object"), defaults);
    }

    let mut out = Map::new();
    if let Some(prefix) = default_path_prefix {
        out.insert("path_prefix".to_string(), Value::String(prefix));
    }
    out.insert("upstream".to_string(), upstream);
    Ok(out)
}

/// Builds a path/operation-scoped override of `main_service` when `context`
/// (the OpenAPI path item or operation) carries any of its own
/// `x-adc-service-defaults`/`x-adc-upstream-defaults`/`servers` — `None`
/// otherwise, meaning the caller should fold the route into `main_service`
/// instead of splitting it into its own service.
pub fn parse_separate_service(
    main_service: &Map<String, Value>,
    name: &str,
    context: Option<&Map<String, Value>>,
    path_service_defaults: Option<&Map<String, Value>>,
    path_upstream_defaults: Option<&Map<String, Value>>,
) -> Result<Option<Map<String, Value>>, ConvertError> {
    let Some(context) = context else { return Ok(None) };
    let context_upstream_defaults = match context.get(extension::UPSTREAM_DEFAULTS) {
        Some(Value::Object(m)) => Some(m),
        _ => None,
    };
    let context_service_defaults = match context.get(extension::SERVICE_DEFAULTS) {
        Some(Value::Object(m)) => Some(m),
        _ => None,
    };
    let context_servers = match context.get("servers") {
        Some(Value::Array(a)) => Some(a.as_slice()),
        _ => None,
    };
    if context_upstream_defaults.is_none() && context_service_defaults.is_none() && context_servers.is_none() {
        return Ok(None);
    }

    let mut service = main_service.clone();
    service.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(servers) = context_servers {
        let generated = transform_upstream(servers, context_upstream_defaults)?;
        shallow_merge(&mut service, &generated);
    }

    let mut upstream = match service.get("upstream") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    if let Some(defaults) = path_upstream_defaults {
        shallow_merge(&mut upstream, defaults);
    }
    if let Some(defaults) = context_upstream_defaults {
        shallow_merge(&mut upstream, defaults);
    }
    service.insert("upstream".to_string(), Value::Object(upstream));

    if let Some(defaults) = path_service_defaults {
        shallow_merge(&mut service, defaults);
    }
    if let Some(defaults) = context_service_defaults {
        shallow_merge(&mut service, defaults);
    }

    Ok(Some(service))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn servers(json_value: Value) -> Vec<Value> {
        json_value.as_array().unwrap().clone()
    }

    #[test]
    fn a_server_with_no_path_has_no_default_path_prefix() {
        let out = transform_upstream(&servers(json!([{"url": "https://example.com"}])), None).unwrap();
        assert!(!out.contains_key("path_prefix"));
        assert_eq!(out["upstream"]["nodes"], json!([{"host": "example.com", "port": 443, "weight": 100}]));
        assert_eq!(out["upstream"]["scheme"], json!("https"));
    }

    #[test]
    fn a_server_with_a_non_root_path_becomes_the_default_path_prefix() {
        let out = transform_upstream(&servers(json!([{"url": "http://example.com/api"}])), None).unwrap();
        assert_eq!(out["path_prefix"], json!("/api"));
        assert_eq!(out["upstream"]["nodes"][0]["port"], json!(80));
    }

    #[test]
    fn only_the_first_servers_path_becomes_the_default_prefix() {
        let out = transform_upstream(&servers(json!([{"url": "https://a.example.com"}, {"url": "https://b.example.com/other"}])), None).unwrap();
        assert!(!out.contains_key("path_prefix"));
    }

    #[test]
    fn server_variables_are_substituted_into_the_url() {
        let out = transform_upstream(
            &servers(json!([{"url": "https://{env}.example.com", "variables": {"env": {"default": "staging"}}}])),
            None,
        )
        .unwrap();
        assert_eq!(out["upstream"]["nodes"][0]["host"], json!("staging.example.com"));
    }

    #[test]
    fn an_explicit_port_overrides_the_scheme_default() {
        let out = transform_upstream(&servers(json!([{"url": "https://example.com:8443"}])), None).unwrap();
        assert_eq!(out["upstream"]["nodes"][0]["port"], json!(8443));
    }

    #[test]
    fn a_scheme_placeholder_is_substituted_before_the_scheme_is_read() {
        // "{scheme}://..." isn't valid URL syntax on its own — reading the
        // scheme before substitution would fail here even though the
        // substituted URL is perfectly valid.
        let out = transform_upstream(
            &servers(json!([{"url": "{scheme}://example.com", "variables": {"scheme": {"default": "https"}}}])),
            None,
        )
        .unwrap();
        assert_eq!(out["upstream"]["scheme"], json!("https"));
    }

    #[test]
    fn a_server_url_with_no_host_is_an_error() {
        let err = transform_upstream(&servers(json!([{"url": "data:text/plain,https://x"}])), None).unwrap_err();
        assert!(err.0.contains("no host"), "{}", err.0);
    }

    #[test]
    fn upstream_defaults_overlay_the_generated_upstream() {
        let defaults = json!({"scheme": "grpc"}).as_object().unwrap().clone();
        let out = transform_upstream(&servers(json!([{"url": "https://example.com"}])), Some(&defaults)).unwrap();
        assert_eq!(out["upstream"]["scheme"], json!("grpc"));
    }

    #[test]
    fn parse_separate_service_returns_none_without_any_override() {
        let main = json!({"name": "main"}).as_object().unwrap().clone();
        let context = json!({"operationId": "getFoo"}).as_object().unwrap().clone();
        assert_eq!(parse_separate_service(&main, "split", Some(&context), None, None).unwrap(), None);
    }

    #[test]
    fn context_service_defaults_win_over_everything_else() {
        let main = json!({"name": "main", "upstream": {"scheme": "https"}}).as_object().unwrap().clone();
        let context = json!({"x-adc-service-defaults": {"description": "from context"}}).as_object().unwrap().clone();
        let path_service_defaults = json!({"description": "from path"}).as_object().unwrap().clone();
        let result = parse_separate_service(&main, "split", Some(&context), Some(&path_service_defaults), None).unwrap().unwrap();
        assert_eq!(result["description"], json!("from context"));
        assert_eq!(result["name"], json!("split"));
    }

    #[test]
    fn context_servers_replace_the_upstream_and_defaults_layer_on_top() {
        let main = json!({"name": "main", "upstream": {"scheme": "https", "nodes": []}}).as_object().unwrap().clone();
        let context = json!({
            "servers": [{"url": "https://split.example.com"}],
            "x-adc-upstream-defaults": {"scheme": "grpc"},
        })
        .as_object()
        .unwrap()
        .clone();
        let result = parse_separate_service(&main, "split", Some(&context), None, None).unwrap().unwrap();
        assert_eq!(result["upstream"]["scheme"], json!("grpc"));
        assert_eq!(result["upstream"]["nodes"], json!([{"host": "split.example.com", "port": 443, "weight": 100}]));
    }
}
