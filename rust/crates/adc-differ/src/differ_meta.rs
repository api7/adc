//! Per-resource-type metadata the differ needs: which `InternalConfiguration`
//! field holds the resource's collection, how to derive an id/name for a
//! local (not-yet-synced) item, and each field's merge/diff strategy.
//!
//! The per-field entries (`fields`) are hand-transcribed here rather than
//! derived by reflecting over `adc_sdk::resources`' struct definitions at
//! runtime — if a resource's shape changes there, this table must be updated
//! to match by hand.

use adc_sdk::ResourceType;
use adc_sdk::utils::generate_id;
use serde_json::Value;

use crate::field_meta::FieldMeta;

fn str_field<'a>(item: &'a Value, key: &str) -> &'a str {
    item.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub enum CollectionKind {
    /// Most resource types: an array of items in `InternalConfiguration`.
    Array,
    /// `global_rules` / `plugin_metadata`: a `Record<string, Plugin>` whose
    /// keys are themselves used directly as the resource name/id.
    Record,
}

pub struct ResourceDifferMeta {
    /// Key on `InternalConfiguration` that holds this resource's collection.
    /// `None` means the type is never present at the top level (only reachable,
    /// if at all, as a sub-resource).
    pub config_field: Option<&'static str>,
    pub collection_kind: CollectionKind,
    /// Derive the display name used as `resourceName` in events.
    /// Not called for `CollectionKind::Record` (see `extract_tuples`).
    pub get_name: fn(&Value) -> String,
    /// Compute or extract the ID for a local item (no server-assigned id yet).
    /// Not called for `CollectionKind::Record`.
    pub generate_id: fn(&Value, Option<&str>) -> String,
    /// Whether to pass the parent resource name when generating IDs for child resources.
    pub propagates_parent_name: bool,
    /// Resolve which `ResourceType` to use for default-value lookup.
    /// Only needed for `Service`, which may be a stream service.
    pub resolve_default_type: Option<fn(&Value) -> ResourceType>,
    /// Per-field merge strategies, hand-transcribed independently of
    /// `adc_sdk::resources`' struct definitions (see module docs).
    pub fields: &'static [(&'static str, FieldMeta)],
}

pub fn differ_meta(resource_type: ResourceType) -> ResourceDifferMeta {
    match resource_type {
        ResourceType::Service => ResourceDifferMeta {
            config_field: Some("services"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, _parent| {
                r.get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| generate_id(str_field(r, "name")))
            },
            propagates_parent_name: true,
            resolve_default_type: Some(|r| {
                if r.get("stream_routes").is_some() {
                    ResourceType::InternalStreamService
                } else {
                    ResourceType::Service
                }
            }),
            fields: &[
                ("upstreams", FieldMeta::Map { list_map_key: "name", nested: true, config_key: None }),
                ("plugins", FieldMeta::ObjectMap),
                ("routes", FieldMeta::Map { list_map_key: "name", nested: true, config_key: None }),
                ("stream_routes", FieldMeta::Map { list_map_key: "name", nested: true, config_key: None }),
            ],
        },

        ResourceType::Ssl => ResourceDifferMeta {
            config_field: Some("ssls"),
            collection_kind: CollectionKind::Array,
            get_name: |r| join_snis(r),
            generate_id: |r, _parent| {
                r.get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| generate_id(&join_snis(r)))
            },
            propagates_parent_name: false,
            resolve_default_type: None,
            fields: &[("certificates", FieldMeta::Array { strip_item_fields: &["key"] })],
        },

        ResourceType::Consumer => ResourceDifferMeta {
            config_field: Some("consumers"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "username").to_string(),
            generate_id: |r, _parent| str_field(r, "username").to_string(),
            propagates_parent_name: true,
            resolve_default_type: None,
            fields: &[
                ("plugins", FieldMeta::ObjectMap),
                (
                    "credentials",
                    FieldMeta::Map { list_map_key: "name", nested: true, config_key: Some("consumer_credentials") },
                ),
            ],
        },

        ResourceType::GlobalRule => ResourceDifferMeta {
            config_field: Some("global_rules"),
            collection_kind: CollectionKind::Record,
            get_name: |_| String::new(), // unused for Record kind, see extract_tuples
            generate_id: |_, _| String::new(),
            propagates_parent_name: false,
            resolve_default_type: None,
            fields: &[],
        },

        ResourceType::PluginMetadata => ResourceDifferMeta {
            config_field: Some("plugin_metadata"),
            collection_kind: CollectionKind::Record,
            get_name: |_| String::new(),
            generate_id: |_, _| String::new(),
            propagates_parent_name: false,
            resolve_default_type: None,
            fields: &[],
        },

        ResourceType::Route => ResourceDifferMeta {
            config_field: Some("routes"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, parent| generate_id_with_parent(r, parent),
            propagates_parent_name: true,
            resolve_default_type: None,
            fields: &[("plugins", FieldMeta::ObjectMap)],
        },

        ResourceType::StreamRoute => ResourceDifferMeta {
            config_field: Some("stream_routes"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, parent| generate_id_with_parent(r, parent),
            propagates_parent_name: false,
            resolve_default_type: None,
            fields: &[("plugins", FieldMeta::ObjectMap)],
        },

        ResourceType::ConsumerCredential => ResourceDifferMeta {
            config_field: Some("consumer_credentials"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, parent| generate_id_with_parent(r, parent),
            propagates_parent_name: true,
            resolve_default_type: None,
            fields: &[],
        },

        ResourceType::Upstream => ResourceDifferMeta {
            config_field: Some("upstreams"),
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, parent| generate_id_with_parent(r, parent),
            propagates_parent_name: true,
            resolve_default_type: None,
            fields: &[],
        },

        // CONSUMER_GROUP is not yet reachable via InternalConfiguration (no
        // top-level configField, and nothing currently declares it as a
        // nested field); kept here so the metadata table stays complete over
        // all ResourceType variants, ready for whenever a nested field
        // declares one.
        ResourceType::ConsumerGroup => ResourceDifferMeta {
            config_field: None,
            collection_kind: CollectionKind::Array,
            get_name: |r| str_field(r, "name").to_string(),
            generate_id: |r, _parent| {
                r.get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| generate_id(str_field(r, "name")))
            },
            propagates_parent_name: false,
            resolve_default_type: None,
            fields: &[
                ("plugins", FieldMeta::ObjectMap),
                ("consumers", FieldMeta::Map { list_map_key: "username", nested: true, config_key: None }),
            ],
        },

        ResourceType::InternalStreamService => unreachable!("internal use only, never diffed directly"),
    }
}

fn join_snis(r: &Value) -> String {
    r.get("snis")
        .and_then(Value::as_array)
        .map(|snis| {
            snis.iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}

fn generate_id_with_parent(r: &Value, parent: Option<&str>) -> String {
    if let Some(id) = r.get("id").and_then(Value::as_str) {
        return id.to_string();
    }
    let name = str_field(r, "name");
    let seed = match parent {
        Some(p) => format!("{p}.{name}"),
        None => name.to_string(),
    };
    generate_id(&seed)
}
