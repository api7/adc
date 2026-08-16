//! Validates the fields this crate actually reads off the OpenAPI document.
//! There's no strongly typed OpenAPI document model in this crate (see the
//! module doc on `crate::lib` for why) — everything is read off a plain
//! `serde_json::Value` tree, so structural/type checking isn't a side
//! effect of deserializing into a struct here and has to be done by hand.
//! Every object checked here is loose: unknown keys pass through untouched,
//! only the specific fields below are checked.
//!
//! `servers[].url` is checked for containing `http://`/`https://` anywhere
//! in the string, not anchored to the start.

use adc_sdk::ConvertError;
use serde_json::{Map, Value};

use crate::HTTP_METHODS;
use crate::extension;

pub fn validate_document(spec: &Map<String, Value>) -> Result<(), ConvertError> {
    let info = spec.get("info").and_then(Value::as_object).ok_or_else(|| ConvertError("info is required".to_string()))?;
    if !matches!(info.get("title"), Some(Value::String(_))) {
        return Err(ConvertError("info.title is required and must be a string".to_string()));
    }

    validate_name(spec, "x-adc-name at the document root")?;

    let servers = spec.get("servers").and_then(Value::as_array).ok_or_else(|| ConvertError("servers is required".to_string()))?;
    if servers.is_empty() {
        return Err(ConvertError("servers must contain at least one entry".to_string()));
    }
    for server in servers {
        let url = server.get("url").and_then(Value::as_str).ok_or_else(|| ConvertError("servers[].url is required".to_string()))?;
        if !(url.contains("http://") || url.contains("https://")) {
            return Err(ConvertError(format!("servers[].url must start with \"https://\" or \"http://\": {url}")));
        }
    }

    if let Some(paths) = spec.get("paths").and_then(Value::as_object) {
        for path_item in paths.values() {
            let Some(path_item) = path_item.as_object() else { continue };
            for method in HTTP_METHODS {
                let Some(operation) = path_item.get(*method).and_then(Value::as_object) else { continue };
                validate_name(operation, &format!("x-adc-name on the \"{method}\" operation"))?;
            }
        }
    }

    Ok(())
}

fn validate_name(context: &Map<String, Value>, label: &str) -> Result<(), ConvertError> {
    match context.get(extension::NAME) {
        Some(Value::String(s)) if s.is_empty() => Err(ConvertError(format!("{label} must not be empty"))),
        Some(Value::String(_)) | None => Ok(()),
        Some(_) => Err(ConvertError(format!("{label} must be a string"))),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn doc(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn a_minimal_valid_document_passes() {
        let spec = doc(json!({"info": {"title": "t"}, "servers": [{"url": "https://example.com"}], "paths": {}}));
        assert!(validate_document(&spec).is_ok());
    }

    #[test]
    fn missing_info_title_fails() {
        let spec = doc(json!({"info": {}, "servers": [{"url": "https://example.com"}]}));
        assert!(validate_document(&spec).is_err());
    }

    #[test]
    fn empty_servers_fails() {
        let spec = doc(json!({"info": {"title": "t"}, "servers": []}));
        assert!(validate_document(&spec).is_err());
    }

    #[test]
    fn a_non_http_server_url_fails() {
        let spec = doc(json!({"info": {"title": "t"}, "servers": [{"url": "ftp://example.com"}]}));
        assert!(validate_document(&spec).is_err());
    }

    #[test]
    fn an_empty_root_x_adc_name_fails() {
        let spec = doc(json!({"info": {"title": "t"}, "servers": [{"url": "https://example.com"}], "x-adc-name": ""}));
        assert!(validate_document(&spec).is_err());
    }

    #[test]
    fn an_empty_operation_x_adc_name_fails() {
        let spec = doc(json!({
            "info": {"title": "t"},
            "servers": [{"url": "https://example.com"}],
            "paths": {"/foo": {"get": {"x-adc-name": ""}}},
        }));
        assert!(validate_document(&spec).is_err());
    }
}
