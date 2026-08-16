//! Narrow port of `@scalar/openapi-parser`'s `upgrade()` /
//! `@scalar/openapi-upgrader`'s `upgradeFromTwoToThree` — just the
//! `host`/`basePath`/`schemes` -> `servers` conversion, the only part of
//! the real Swagger 2.0 -> 3.0 -> 3.1 upgrade chain `crate::parser`'s own
//! read surface (`info`/`servers`/`paths.*.{method}` + `x-adc-*`) actually
//! depends on. The rest of that ~800-line upgrader (`definitions` ->
//! `components.schemas`, `consumes`/`produces`, `parameters: in: body`,
//! ...) touches fields `crate::prune::prune_conversion_document` deletes
//! before this converter ever reads them, so it's skipped.
//!
//! Runs unconditionally, same as the real upgrader: a 3.x document simply
//! has no `host` key, so the `if let Some(host) = ...` branch is a no-op —
//! there's no version check gating it either upstream or here.

use adc_sdk::ConvertError;
use serde_json::{Map, Value};

pub fn parse_document(content: &str) -> Result<Value, ConvertError> {
    if content.trim().is_empty() {
        return Err(ConvertError("no OpenAPI document found".to_string()));
    }
    if let Ok(value) = serde_json::from_str(content) {
        return Ok(value);
    }
    serde_yaml_ng::from_str(content).map_err(|e| ConvertError(format!("failed to parse OpenAPI document: {e}")))
}

pub fn upgrade_swagger_2_servers(document: &mut Map<String, Value>) {
    if let Some(host) = document.get("host").and_then(Value::as_str).map(str::to_string) {
        let schemes: Vec<String> = match document.get("schemes") {
            Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_string).collect(),
            _ => Vec::new(),
        };
        // Also covers a non-empty `schemes` array that filtered down to
        // nothing (every entry was a non-string) — falling through to an
        // empty `servers` array instead would leave `paths`/`info` intact
        // but no server to derive an upstream from at all.
        let schemes = if schemes.is_empty() { vec!["http".to_string()] } else { schemes };
        let base_path = document.get("basePath").and_then(Value::as_str).unwrap_or("").to_string();
        let servers: Vec<Value> =
            schemes.into_iter().map(|scheme| serde_json::json!({"url": format!("{scheme}://{host}{base_path}")})).collect();
        document.insert("servers".to_string(), Value::Array(servers));
        document.remove("basePath");
        document.remove("schemes");
        document.remove("host");
    } else if let Some(base_path) = document.get("basePath").and_then(Value::as_str).map(str::to_string) {
        document.insert("servers".to_string(), Value::Array(vec![serde_json::json!({"url": base_path})]));
        document.remove("basePath");
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_json_content() {
        let value = parse_document(r#"{"info":{"title":"t"}}"#).unwrap();
        assert_eq!(value["info"]["title"], json!("t"));
    }

    #[test]
    fn parses_yaml_content() {
        let value = parse_document("info:\n  title: t\n").unwrap();
        assert_eq!(value["info"]["title"], json!("t"));
    }

    #[test]
    fn host_basepath_and_schemes_become_servers() {
        let mut doc = json!({"host": "example.com", "basePath": "/v1", "schemes": ["https"]}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "https://example.com/v1"}]));
        assert!(!doc.contains_key("host") && !doc.contains_key("basePath") && !doc.contains_key("schemes"));
    }

    #[test]
    fn missing_schemes_defaults_to_http() {
        let mut doc = json!({"host": "example.com"}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "http://example.com"}]));
    }

    #[test]
    fn multiple_schemes_produce_multiple_servers() {
        let mut doc = json!({"host": "example.com", "schemes": ["http", "https"]}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "http://example.com"}, {"url": "https://example.com"}]));
    }

    #[test]
    fn a_bare_base_path_with_no_host_becomes_a_relative_server_url() {
        let mut doc = json!({"basePath": "/v1"}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "/v1"}]));
    }

    #[test]
    fn a_schemes_array_with_no_string_entries_still_defaults_to_http() {
        let mut doc = json!({"host": "example.com", "schemes": [1, true]}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "http://example.com"}]));
    }

    #[test]
    fn a_document_without_host_or_basepath_is_untouched() {
        let mut doc = json!({"servers": [{"url": "https://example.com"}]}).as_object().unwrap().clone();
        upgrade_swagger_2_servers(&mut doc);
        assert_eq!(doc["servers"], json!([{"url": "https://example.com"}]));
    }
}
