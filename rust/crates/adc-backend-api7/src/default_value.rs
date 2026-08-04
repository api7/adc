//! Deriving per-resource-type default values from API7's `/api/schema/core`
//! JSON Schema: every field the schema declares a `default` for gets
//! extracted, then run through the same read-direction transform the
//! fetcher uses on a real fetched resource, so a value matching the
//! backend's own default doesn't show up as a spurious diff against local
//! config that simply omitted it.

use std::collections::HashMap;

use adc_backend_core::{HttpClient, Method};
use adc_sdk::resources::{self as adc};
use adc_sdk::{BackendError, DefaultValue, ResourceType};
use serde_json::{Map, Value, json};

use crate::typing;

pub async fn fetch(client: &HttpClient) -> Result<DefaultValue, BackendError> {
    let request = client.request(Method::GET, "/api/schema/core")?;
    let body: typing::ValueResponse<Map<String, Value>> = client.send_json(request).await?;
    let mut schema = body.value;
    patch_missing_upstream_schema(&mut schema);

    let mut core = HashMap::new();
    for (type_name, schema_entry) in schema {
        let Some(resource_type) = resource_type_from_str(&type_name) else {
            continue;
        };
        let merged = match schema_entry.get("allOf").and_then(Value::as_array) {
            Some(all_of) => merge_all_of(all_of.clone()),
            None => schema_entry,
        };
        let data = extract_object_default(&merged).unwrap_or_else(|| json!({}));
        if let Some(transformed) = transform_default(resource_type, data) {
            core.insert(resource_type, transformed);
        }
    }

    Ok(DefaultValue {
        core,
        plugins: HashMap::new(),
    })
}

/// Older API7 releases have no top-level `upstream` schema entry at all —
/// only a service's own nested `upstream` property. Synthesizes one from
/// that so upstream defaults still get extracted on those versions too.
fn patch_missing_upstream_schema(schema: &mut Map<String, Value>) {
    if schema.contains_key("upstream") {
        return;
    }
    let Some(mut upstream) = schema
        .get("service")
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.get("upstream"))
        .cloned()
    else {
        return;
    };
    if let Value::Object(map) = &mut upstream {
        map.insert("type".to_string(), Value::String("object".to_string()));
    }
    schema.insert("upstream".to_string(), upstream);
}

/// Merges an `allOf` schema composition's `properties` into one object —
/// only `properties` are merged, not other JSON Schema keywords. A
/// composition with no `object`-typed member at all merges to an empty
/// object rather than attempting to merge non-object schemas.
fn merge_all_of(mut items: Vec<Value>) -> Value {
    if items.len() < 2 {
        return items.pop().unwrap_or(Value::Null);
    }
    if !items
        .iter()
        .any(|item| item.get("type").and_then(Value::as_str) == Some("object"))
    {
        return json!({});
    }

    let mut iter = items.into_iter();
    let Some(Value::Object(mut first)) = iter.next() else {
        return json!({});
    };
    if !matches!(first.get("properties"), Some(Value::Object(_))) {
        first.insert("properties".to_string(), json!({}));
    }
    for item in iter {
        let Some(Value::Object(props)) = item.get("properties").cloned() else {
            continue;
        };
        let Some(Value::Object(merged_properties)) = first.get_mut("properties") else {
            unreachable!("just ensured `properties` is an object above");
        };
        merged_properties.extend(props);
    }
    Value::Object(first)
}

/// Recursively walks a JSON Schema object's `properties`, extracting each
/// field's declared default. Three cases, in order — an array field that
/// *isn't* an array-of-objects falls through to the plain-default case
/// rather than being dropped:
/// 1. An object-typed field recurses into its own nested defaults.
/// 2. An array field whose (non-tuple) item schema is itself object-typed
///    (e.g. `upstream.nodes`) becomes a one-element array of that item's
///    extracted defaults.
/// 3. Everything else (including a plain array with no such item schema)
///    takes the field's own declared `default` verbatim.
///
/// A field with nothing to contribute (no `default`, and neither case 1
/// nor 2 applies) is absent from the result, not `null`.
fn extract_object_default(schema: &Value) -> Option<Value> {
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return None;
    }
    let properties = schema.get("properties")?.as_object()?;

    let mut defaults = Map::new();
    for (key, field) in properties {
        let field_type = field.get("type").and_then(Value::as_str);
        let is_object_array_item = field_type == Some("array")
            && !matches!(field.get("items"), Some(Value::Array(_)))
            && field
                .get("items")
                .and_then(|items| items.get("type"))
                .and_then(Value::as_str)
                == Some("object");

        let value = if field_type == Some("object") {
            extract_object_default(field)
        } else if is_object_array_item {
            field
                .get("items")
                .and_then(extract_object_default)
                .map(|item_default| Value::Array(vec![item_default]))
        } else {
            field.get("default").cloned()
        };

        if let Some(value) = value {
            defaults.insert(key.clone(), value);
        }
    }
    Some(Value::Object(defaults))
}

fn resource_type_from_str(value: &str) -> Option<ResourceType> {
    Some(match value {
        "route" => ResourceType::Route,
        "service" => ResourceType::Service,
        "upstream" => ResourceType::Upstream,
        "ssl" => ResourceType::Ssl,
        "global_rule" => ResourceType::GlobalRule,
        "plugin_config" => ResourceType::PluginConfig,
        "plugin_metadata" => ResourceType::PluginMetadata,
        "consumer" => ResourceType::Consumer,
        "consumer_group" => ResourceType::ConsumerGroup,
        "consumer_credential" => ResourceType::ConsumerCredential,
        "stream_route" => ResourceType::StreamRoute,
        "stream_service" => ResourceType::InternalStreamService,
        _ => return None,
    })
}

/// Runs an extracted default object through the same read-direction
/// transform the fetcher applies to a real fetched resource, by treating
/// it as a (partial) API7 wire-shape object.
///
/// A default schema entry rarely populates every field a full resource
/// would (a certificate has no schema-level default, an id is never
/// defaulted, ...), so a conversion failure here contributes no default at
/// all for that resource type rather than failing the whole call — a
/// resource type without a usable default just doesn't show up in the
/// result, and every other resource type's default is unaffected.
fn transform_default(resource_type: ResourceType, data: Value) -> Option<Value> {
    match resource_type {
        ResourceType::Route => {
            let route: typing::Route = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Route::try_from(route).ok()?).ok()
        }
        ResourceType::Service | ResourceType::InternalStreamService => {
            let service: typing::Service = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Service::try_from(service).ok()?).ok()
        }
        ResourceType::Ssl => {
            let ssl: typing::Ssl = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::SSL::from(ssl)).ok()
        }
        ResourceType::Consumer => {
            let consumer: typing::Consumer = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Consumer::from(consumer)).ok()
        }
        ResourceType::Upstream => {
            let upstream: typing::Upstream = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Upstream::from(upstream)).ok()
        }
        _ => Some(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_plain_field_default() {
        let schema = json!({ "type": "object", "properties": { "retries": { "type": "integer", "default": 3 } } });
        assert_eq!(
            extract_object_default(&schema),
            Some(json!({ "retries": 3 }))
        );
    }

    #[test]
    fn omits_a_field_with_no_default_at_all() {
        let schema = json!({ "type": "object", "properties": { "name": { "type": "string" } } });
        assert_eq!(extract_object_default(&schema), Some(json!({})));
    }

    #[test]
    fn recurses_into_a_nested_object_field() {
        let schema = json!({
            "type": "object",
            "properties": {
                "checks": { "type": "object", "properties": { "active": { "type": "object", "properties": { "timeout": { "type": "integer", "default": 5 } } } } }
            }
        });
        assert_eq!(
            extract_object_default(&schema),
            Some(json!({ "checks": { "active": { "timeout": 5 } } }))
        );
    }

    #[test]
    fn wraps_an_array_of_objects_items_default_in_a_one_element_array() {
        let schema = json!({
            "type": "object",
            "properties": {
                "nodes": { "type": "array", "items": { "type": "object", "properties": { "weight": { "type": "integer", "default": 1 } } } }
            }
        });
        assert_eq!(
            extract_object_default(&schema),
            Some(json!({ "nodes": [{ "weight": 1 }] }))
        );
    }

    #[test]
    fn a_plain_array_field_falls_through_to_its_own_default() {
        // Not an array-of-objects (items has no `type: object`), so this
        // must fall through to the field's own `default` rather than
        // disappearing.
        let schema = json!({
            "type": "object",
            "properties": {
                "http_statuses": { "type": "array", "items": { "type": "integer" }, "default": [200, 302] }
            }
        });
        assert_eq!(
            extract_object_default(&schema),
            Some(json!({ "http_statuses": [200, 302] }))
        );
    }

    #[test]
    fn a_non_object_schema_has_no_extractable_default() {
        assert_eq!(extract_object_default(&json!({ "type": "string" })), None);
    }

    #[test]
    fn merges_all_of_properties_from_every_member_with_later_keys_winning() {
        let items = vec![
            json!({ "type": "object", "properties": { "a": { "default": 1 } } }),
            json!({ "type": "object", "properties": { "a": { "default": 2 }, "b": { "default": 3 } } }),
        ];
        let merged = merge_all_of(items);
        assert_eq!(merged["properties"]["a"], json!({ "default": 2 }));
        assert_eq!(merged["properties"]["b"], json!({ "default": 3 }));
    }

    #[test]
    fn merges_all_of_to_an_empty_object_when_no_member_is_object_typed() {
        let items = vec![json!({ "type": "string" }), json!({ "type": "integer" })];
        assert_eq!(merge_all_of(items), json!({}));
    }

    #[test]
    fn patches_a_missing_top_level_upstream_schema_from_the_services_own_property() {
        let mut schema = Map::new();
        schema.insert("service".to_string(), json!({ "properties": { "upstream": { "properties": { "retries": { "default": 3 } } } } }));
        patch_missing_upstream_schema(&mut schema);
        assert_eq!(
            schema["upstream"],
            json!({ "properties": { "retries": { "default": 3 } }, "type": "object" })
        );
    }

    #[test]
    fn leaves_an_existing_top_level_upstream_schema_alone() {
        let mut schema = Map::new();
        schema.insert("upstream".to_string(), json!({ "type": "object" }));
        schema.insert("service".to_string(), json!({ "properties": { "upstream": { "properties": { "retries": { "default": 99 } } } } }));
        patch_missing_upstream_schema(&mut schema);
        assert_eq!(schema["upstream"], json!({ "type": "object" }));
    }

    #[test]
    fn recognizes_every_wire_resource_type_name() {
        assert_eq!(resource_type_from_str("route"), Some(ResourceType::Route));
        assert_eq!(
            resource_type_from_str("stream_service"),
            Some(ResourceType::InternalStreamService)
        );
        assert_eq!(resource_type_from_str("not_a_real_type"), None);
    }
}
