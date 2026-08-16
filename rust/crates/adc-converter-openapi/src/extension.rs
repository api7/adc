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
    let mut plugins = context.get(PLUGINS).and_then(Value::as_object).cloned().unwrap_or_default();
    for (key, value) in context {
        if let Some(plugin_name) = key.strip_prefix(PLUGIN_PREFIX) {
            plugins.insert(plugin_name.to_string(), value.clone());
        }
    }
    if plugins.is_empty() { None } else { Some(plugins) }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[rstest]
    #[case::merges_the_plugins_object_with_individual_plugin_keys(
        json!({"x-adc-plugins": {"cors": {}}, "x-adc-plugin-acl": {"allow": ["a"]}}),
        Some(json!({"cors": {}, "acl": {"allow": ["a"]}})),
    )]
    #[case::an_individual_plugin_key_wins_over_the_same_name_in_the_plugins_object(
        json!({"x-adc-plugins": {"acl": {"allow": ["a"]}}, "x-adc-plugin-acl": {"allow": ["b"]}}),
        Some(json!({"acl": {"allow": ["b"]}})),
    )]
    #[case::returns_none_when_there_are_no_plugins(json!({"operationId": "getFoo"}), None)]
    fn parse_ext_plugins_cases(#[case] context: Value, #[case] expected: Option<Value>) {
        let context = context.as_object().unwrap().clone();
        let expected = expected.map(|v| v.as_object().unwrap().clone());
        assert_eq!(parse_ext_plugins(&context), expected);
    }
}
