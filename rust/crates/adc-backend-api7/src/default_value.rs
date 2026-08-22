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
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::typing;

/// JSON Schema keyword names this module reads/writes by key, kept as
/// constants so a typo shows up at compile time (an unrecognized string
/// literal would otherwise just silently never match).
const KEY_TYPE: &str = "type";
const KEY_PROPERTIES: &str = "properties";
const KEY_ITEMS: &str = "items";
const KEY_DEFAULT: &str = "default";
const KEY_ALL_OF: &str = "allOf";
/// The `type` keyword's value for an object schema, not a key itself —
/// grouped with the `KEY_*` constants above since it's compared against
/// `KEY_TYPE`'s value everywhere this module cares about it.
const TYPE_OBJECT: &str = "object";

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
        let merged = match schema_entry.get(KEY_ALL_OF).and_then(Value::as_array) {
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
        map.insert(KEY_TYPE.to_string(), Value::String(TYPE_OBJECT.to_string()));
    }
    schema.insert("upstream".to_string(), upstream);
}

/// Merges an `allOf` schema composition's `properties` into one object —
/// only `properties` are merged, not other JSON Schema keywords. A
/// composition with no `object`-typed member at all merges to an empty
/// object rather than attempting to merge non-object schemas.
///
/// Builds the result from a fresh object rather than mutating whichever
/// member happens to come first: nothing downstream reads any keyword but
/// `type`/`properties`, so there's no reason to carry a member's other
/// fields (`required`, `additionalProperties`, ...) into the result, and no
/// need to hunt for "the" object-typed member to use as a base — every
/// member's `properties` gets merged in regardless of position, and `type`
/// is set once we already know at least one member declared it as `object`.
fn merge_all_of(items: Vec<Value>) -> Value {
    if items.len() < 2 {
        return items.into_iter().next().unwrap_or(Value::Null);
    }
    if !items
        .iter()
        .any(|item| item.get(KEY_TYPE).and_then(Value::as_str) == Some(TYPE_OBJECT))
    {
        return json!({});
    }

    let mut properties = Map::new();
    for item in &items {
        if let Some(Value::Object(props)) = item.get(KEY_PROPERTIES) {
            properties.extend(props.clone());
        }
    }
    let mut merged = Map::new();
    merged.insert(KEY_TYPE.to_string(), Value::String(TYPE_OBJECT.to_string()));
    merged.insert(KEY_PROPERTIES.to_string(), Value::Object(properties));
    Value::Object(merged)
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
    if schema.get(KEY_TYPE).and_then(Value::as_str) != Some(TYPE_OBJECT) {
        return None;
    }
    let properties = schema.get(KEY_PROPERTIES)?.as_object()?;

    let mut defaults = Map::new();
    for (key, field) in properties {
        let field_type = field.get(KEY_TYPE).and_then(Value::as_str);
        let is_object_array_item = field_type == Some("array")
            && !matches!(field.get(KEY_ITEMS), Some(Value::Array(_)))
            && field
                .get(KEY_ITEMS)
                .and_then(|items| items.get(KEY_TYPE))
                .and_then(Value::as_str)
                == Some(TYPE_OBJECT);

        let value = if field_type == Some(TYPE_OBJECT) {
            extract_object_default(field)
        } else if is_object_array_item {
            field
                .get(KEY_ITEMS)
                .and_then(extract_object_default)
                .map(|item_default| Value::Array(vec![item_default]))
        } else {
            field.get(KEY_DEFAULT).cloned()
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

/// A schema-derived default `nodes` entry commonly declares only
/// `priority` (`{"priority": 0}`) — there's no sensible universal default
/// for `host`/`port`/`weight`, so the schema never populates them.
/// `adc_sdk::resources::UpstreamNode` requires all three (deliberately, for
/// real fetched/authored data), so deserializing straight into it fails on
/// exactly this partial shape. This lenient stand-in exists only to absorb
/// that one gap: real fetched upstream data always has complete nodes and
/// never needs it.
#[derive(Deserialize)]
struct LenientUpstreamNode {
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    weight: i64,
    #[serde(default)]
    priority: i64,
    #[serde(default)]
    metadata: Option<Map<String, Value>>,
}

impl From<LenientUpstreamNode> for adc::UpstreamNode {
    fn from(node: LenientUpstreamNode) -> Self {
        adc::UpstreamNode {
            host: node.host,
            port: node.port,
            weight: node.weight,
            priority: node.priority,
            metadata: node.metadata,
        }
    }
}

/// Rewrites `upstream["nodes"]` in place through [`LenientUpstreamNode`], so
/// the strict typed deserialization downstream in [`transform_default`]
/// sees a structurally complete (if zero-valued) node instead of a bare
/// `{"priority": 0}`. A no-op if `upstream` has no `nodes` array, or if a
/// specific entry doesn't even parse as the lenient shape (left as-is,
/// letting the caller's own strict deserialization fail and drop that
/// resource type's default the way it already does for any other
/// unrecoverable shape).
fn repair_upstream_nodes(upstream: &mut Value) {
    let Some(nodes) = upstream.get_mut("nodes").and_then(Value::as_array_mut) else {
        return;
    };
    for node in nodes {
        let Ok(lenient) = serde_json::from_value::<LenientUpstreamNode>(node.clone()) else {
            continue;
        };
        if let Ok(repaired) = serde_json::to_value(adc::UpstreamNode::from(lenient)) {
            *node = repaired;
        }
    }
}

/// A schema-derived default `client` entry commonly declares only `depth`
/// (`{"depth": 1}`) — there's no sensible universal default for `ca` (a CA
/// certificate). `adc_sdk::resources::SslClient` requires it (deliberately,
/// for real fetched/authored data), so deserializing straight into it fails
/// on exactly this partial shape. This lenient stand-in exists only to
/// absorb that one gap: real fetched SSL data always has a complete
/// `client` block and never needs it.
#[derive(Deserialize)]
struct LenientSslClient {
    #[serde(default)]
    ca: String,
    #[serde(default = "default_client_depth")]
    depth: u32,
    #[serde(default)]
    skip_mtls_uri_regex: Option<Vec<String>>,
}

/// Matches `adc_sdk::resources::SslClient`'s own default for this field.
fn default_client_depth() -> u32 {
    1
}

impl From<LenientSslClient> for adc::SslClient {
    fn from(client: LenientSslClient) -> Self {
        adc::SslClient {
            ca: client.ca,
            depth: client.depth,
            skip_mtls_uri_regex: client.skip_mtls_uri_regex,
        }
    }
}

/// Rewrites `ssl["client"]` in place through [`LenientSslClient`], the same
/// way [`repair_upstream_nodes`] does for a partial upstream node.
fn repair_ssl_client(ssl: &mut Value) {
    let Some(client) = ssl.get("client") else {
        return;
    };
    let Ok(lenient) = serde_json::from_value::<LenientSslClient>(client.clone()) else {
        return;
    };
    if let Ok(repaired) = serde_json::to_value(adc::SslClient::from(lenient)) {
        ssl["client"] = repaired;
    }
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
fn transform_default(resource_type: ResourceType, mut data: Value) -> Option<Value> {
    match resource_type {
        ResourceType::Route => {
            let route: typing::Route = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Route::try_from(route).ok()?).ok()
        }
        ResourceType::Service | ResourceType::InternalStreamService => {
            if let Some(upstream) = data.get_mut("upstream") {
                repair_upstream_nodes(upstream);
            }
            let service: typing::Service = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Service::try_from(service).ok()?).ok()
        }
        ResourceType::Ssl => {
            repair_ssl_client(&mut data);
            let ssl: typing::Ssl = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::SSL::from(ssl)).ok()
        }
        ResourceType::Consumer => {
            let consumer: typing::Consumer = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Consumer::from(consumer)).ok()
        }
        ResourceType::Upstream => {
            repair_upstream_nodes(&mut data);
            let upstream: typing::Upstream = serde_json::from_value(data).ok()?;
            serde_json::to_value(adc::Upstream::from(upstream)).ok()
        }
        // ConsumerCredential/StreamRoute among others: no schema-level
        // default in practice, so the untransformed fallback never actually
        // needs to bridge their wire/ADC field-name differences.
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
    fn merges_all_of_when_the_object_typed_member_is_not_first() {
        // The `type: object` member is second, not first — merging must not
        // depend on which member happens to come first in the array.
        let items = vec![
            json!({ "properties": { "a": { "default": 1 } } }),
            json!({ "type": "object", "properties": { "b": { "default": 2 } } }),
        ];
        let merged = merge_all_of(items);
        assert_eq!(merged["type"], json!("object"));
        assert_eq!(merged["properties"]["a"], json!({ "default": 1 }));
        assert_eq!(merged["properties"]["b"], json!({ "default": 2 }));
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

    #[test]
    fn repair_upstream_nodes_fills_in_a_partial_node_with_zero_values() {
        let mut upstream = json!({ "nodes": [{ "priority": 0 }] });
        repair_upstream_nodes(&mut upstream);
        assert_eq!(
            upstream["nodes"],
            json!([{ "host": "", "port": 0, "weight": 0, "priority": 0 }])
        );
    }

    #[test]
    fn repair_upstream_nodes_is_a_no_op_with_no_nodes_field() {
        let mut upstream = json!({ "scheme": "http" });
        repair_upstream_nodes(&mut upstream);
        assert_eq!(upstream, json!({ "scheme": "http" }));
    }

    #[test]
    fn a_service_with_only_a_partial_default_upstream_node_still_produces_a_default() {
        let data = json!({
            "strip_path_prefix": true,
            "upstream": { "nodes": [{ "priority": 0 }], "scheme": "http" },
        });
        let transformed = transform_default(ResourceType::Service, data).unwrap();
        assert_eq!(transformed["upstream"]["nodes"][0]["host"], "");
        assert_eq!(transformed["strip_path_prefix"], true);
    }

    #[test]
    fn repair_ssl_client_fills_in_a_partial_client_with_zero_values() {
        let mut ssl = json!({ "client": { "depth": 1 } });
        repair_ssl_client(&mut ssl);
        assert_eq!(ssl["client"], json!({ "ca": "", "depth": 1 }));
    }

    #[test]
    fn repair_ssl_client_defaults_a_missing_depth_to_one_not_zero() {
        let mut ssl = json!({ "client": {} });
        repair_ssl_client(&mut ssl);
        assert_eq!(ssl["client"]["depth"], 1);
    }

    #[test]
    fn repair_ssl_client_is_a_no_op_with_no_client_field() {
        let mut ssl = json!({ "type": "server" });
        repair_ssl_client(&mut ssl);
        assert_eq!(ssl, json!({ "type": "server" }));
    }

    #[test]
    fn an_ssl_with_only_a_partial_default_client_still_produces_a_default() {
        let data = json!({ "client": { "depth": 1 } });
        let transformed = transform_default(ResourceType::Ssl, data).unwrap();
        assert_eq!(transformed["client"]["depth"], 1);
    }
}
