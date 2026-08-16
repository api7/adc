//! The `x-adc-*` extension field names an OpenAPI document can use to
//! override generated names/labels/plugins/defaults.

use serde_json::{Map, Value};

pub const NAME: &str = "x-adc-name";
pub const LABELS: &str = "x-adc-labels";
pub const PLUGINS: &str = "x-adc-plugins";
pub const PLUGIN_PREFIX: &str = "x-adc-plugin-";
pub const SERVICE_DEFAULTS: &str = "x-adc-service-defaults";
pub const UPSTREAM_DEFAULTS: &str = "x-adc-upstream-defaults";
pub const UPSTREAM_NODE_DEFAULTS: &str = "x-adc-upstream-node-defaults";
pub const ROUTE_DEFAULTS: &str = "x-adc-route-defaults";

/// Merges `x-adc-plugins` with any individual `x-adc-plugin-<name>` keys
/// (the latter winning on a name collision), returning `None` if the result
/// would be empty.
pub fn parse_ext_plugins(context: &Map<String, Value>) -> Option<Map<String, Value>> {
    let mut plugins = match context.get(PLUGINS) {
        Some(Value::Object(map)) => map.clone(),
        _ => Map::new(),
    };
    for (key, value) in context {
        if let Some(plugin_name) = key.strip_prefix(PLUGIN_PREFIX) {
            plugins.insert(plugin_name.to_string(), value.clone());
        }
    }
    if plugins.is_empty() { None } else { Some(plugins) }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn merges_the_plugins_object_with_individual_plugin_keys() {
        let context = json!({
            "x-adc-plugins": {"cors": {}},
            "x-adc-plugin-acl": {"allow": ["a"]},
        })
        .as_object()
        .unwrap()
        .clone();
        assert_eq!(parse_ext_plugins(&context), Some(json!({"cors": {}, "acl": {"allow": ["a"]}}).as_object().unwrap().clone()));
    }

    #[test]
    fn an_individual_plugin_key_wins_over_the_same_name_in_the_plugins_object() {
        let context = json!({
            "x-adc-plugins": {"acl": {"allow": ["a"]}},
            "x-adc-plugin-acl": {"allow": ["b"]},
        })
        .as_object()
        .unwrap()
        .clone();
        assert_eq!(parse_ext_plugins(&context), Some(json!({"acl": {"allow": ["b"]}}).as_object().unwrap().clone()));
    }

    #[test]
    fn returns_none_when_there_are_no_plugins() {
        let context = json!({"operationId": "getFoo"}).as_object().unwrap().clone();
        assert_eq!(parse_ext_plugins(&context), None);
    }
}
