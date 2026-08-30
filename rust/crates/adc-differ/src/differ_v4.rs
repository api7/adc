//! The resource-diffing algorithm: compares local (desired) and remote
//! (actual) configuration and produces an ordered list of create/update/
//! delete events describing how to reconcile them.

use std::collections::{HashMap, HashSet};

use adc_sdk::{
    DefaultValue, Event, EventKind, EventType, ResourceType, resources::FlatConfiguration, diff_value,
    utils::generate_id,
};
use serde_json::{Map, Value, json};

use crate::differ_meta::{CollectionKind, ResourceDifferMeta, differ_meta};
use crate::field_meta::FieldMeta;

/// This crate's own internal working representation of a full
/// configuration: a plain `Map<String, Value>` keyed by config field name
/// (`services`, `routes`, `global_rules`, ...). The differ algorithm treats
/// resource bodies as opaque structural values rather than strongly-typed
/// ones, since diffing is a generic structural operation independent of any
/// one resource's shape.
///
/// Not part of this crate's public API — [`FlatConfiguration`] (the typed
/// counterpart used to parse and validate declarative configuration) is the
/// public boundary; this `Map` is what it converts to internally before
/// diffing, and never leaves the crate.
type InternalConfiguration = Map<String, Value>;

/// (name, id, item) extracted from one resource's raw JSON representation.
type ResourceTuple = (String, String, Value);

/// An event paired with the nested sub-events discovered while building it (from
/// this resource's `{listType: Map, nested: true}` fields). `sub_events` is
/// build-time-only bookkeeping consumed by `DifferV4::diff_map`, which lifts each
/// entry up one level into the final flattened list — it never appears on the
/// public `Event` type itself. `event` is `None` when this resource itself
/// didn't change but a nested one did — `handle_update`'s only producer of
/// that case — so there's nothing of its own to lift alongside `sub_events`.
struct BuiltEvent {
    event: Option<Event>,
    sub_events: Vec<Event>,
}

pub struct DifferV4 {
    default_value: DefaultValue,
}

impl DifferV4 {
    /// The public entry point: diffs two root-level configurations. Converts
    /// each into the flattened `InternalConfiguration` map `diff_map` (and
    /// every recursive sub-call below it) actually operates on — a root
    /// `FlatConfiguration` has nothing to flatten (a `Configuration` never
    /// has its own top-level `routes`/`upstreams`/...), so this conversion
    /// is a plain field copy, not real flattening work.
    pub fn diff(local: &FlatConfiguration, remote: &FlatConfiguration, default_value: Option<&DefaultValue>) -> Vec<Event> {
        let local = to_internal_configuration(local);
        let remote = to_internal_configuration(remote);
        DifferV4::diff_map(&local, &remote, default_value, None)
    }

    /// The recursive core: `parent_name` is `Some` only on a sub-call diffing
    /// one parent resource's own nested collections (see the `handle_*`
    /// methods below) — always `None` from the public `diff` entry point.
    fn diff_map(
        local: &InternalConfiguration,
        remote: &InternalConfiguration,
        default_value: Option<&DefaultValue>,
        parent_name: Option<&str>,
    ) -> Vec<Event> {
        let differ = DifferV4 { default_value: default_value.cloned().unwrap_or_default() };

        let mut result: Vec<BuiltEvent> = Vec::new();
        for &resource_type in ResourceType::ALL {
            let meta = differ_meta(resource_type);
            if meta.config_field.is_none() {
                continue;
            }
            let local_tuples = extract_tuples(local, &meta, parent_name);
            let remote_tuples = extract_tuples(remote, &meta, None);
            result.extend(differ.diff_resource(resource_type, &meta, local_tuples, remote_tuples));
        }

        // Unwrap one level of sub_events into the flattened list.
        let mut unwrapped: Vec<Event> = Vec::new();
        for BuiltEvent { event, sub_events } in result {
            unwrapped.extend(event);
            unwrapped.extend(sub_events);
        }

        unwrapped.sort_by_key(|e| order_priority(e.resource_type, e.event_type()));
        unwrapped
    }

    fn diff_resource(
        &self,
        resource_type: ResourceType,
        meta: &ResourceDifferMeta,
        local: Vec<ResourceTuple>,
        remote: Vec<ResourceTuple>,
    ) -> Vec<BuiltEvent> {
        let mut result = Vec::new();
        let local_id_map: HashMap<&str, &Value> =
            local.iter().map(|(_, id, item)| (id.as_str(), item)).collect();
        let mut seen_remote_ids: HashSet<String> = HashSet::new();

        for (remote_name, remote_id, raw_remote_item) in &remote {
            let remote_item = prepare_remote_item(raw_remote_item);

            match local_id_map.get(remote_id.as_str()) {
                None => {
                    result.push(self.handle_delete(meta, resource_type, remote_id, remote_name, remote_item));
                }
                Some(local_item_raw) => {
                    seen_remote_ids.insert(remote_id.clone());
                    let mut local_item = (*local_item_raw).clone();
                    strip_key(&mut local_item, "id");
                    if let Some(event) =
                        self.handle_update(meta, resource_type, remote_id, remote_name, local_item, remote_item)
                    {
                        result.push(event);
                    }
                }
            }
        }

        for (local_name, local_id, local_item_raw) in &local {
            if seen_remote_ids.contains(local_id) {
                continue;
            }
            let mut local_item = local_item_raw.clone();
            strip_key(&mut local_item, "id");
            result.push(self.handle_create(meta, resource_type, local_id, local_name, local_item));
        }

        result
    }

    fn handle_delete(
        &self,
        meta: &ResourceDifferMeta,
        resource_type: ResourceType,
        remote_id: &str,
        remote_name: &str,
        remote_item: Value,
    ) -> BuiltEvent {
        let sub_config = extract_sub_config(meta, &remote_item);
        let empty = InternalConfiguration::new();
        let parent_name = meta.propagates_parent_name.then_some(remote_name);
        let sub_events = DifferV4::diff_map(&empty, &sub_config, Some(&self.default_value), parent_name)
            .into_iter()
            .map(|e| postprocess_sub_event(remote_name, remote_id, e))
            .collect();

        let event = Event::new(resource_type, EventKind::Delete { old_value: remote_item }, remote_id, remote_name);
        BuiltEvent { event: Some(event), sub_events }
    }

    fn handle_create(
        &self,
        meta: &ResourceDifferMeta,
        resource_type: ResourceType,
        local_id: &str,
        local_name: &str,
        mut local_item: Value,
    ) -> BuiltEvent {
        let sub_config = extract_sub_config(meta, &local_item);
        let empty = InternalConfiguration::new();
        let parent_name = meta.propagates_parent_name.then_some(local_name);
        let sub_events = DifferV4::diff_map(&sub_config, &empty, Some(&self.default_value), parent_name)
            .into_iter()
            .map(|e| postprocess_sub_event(local_name, local_id, e))
            .collect();

        // Nested MAP-resource items need their own `id` stripped, and that
        // stripping has to be visible through `newValue` too, since it
        // describes the same item. `local_item` isn't cloned before this
        // point (unlike the remote/delete path's `prepare_remote_item`, or
        // update's early clone), so stripping it in place here keeps both
        // views consistent without a separate sync step.
        strip_nested_ids(meta, &mut local_item);

        let event = Event::new(resource_type, EventKind::Create { new_value: local_item }, local_id, local_name);
        BuiltEvent { event: Some(event), sub_events }
    }

    fn handle_update(
        &self,
        meta: &ResourceDifferMeta,
        resource_type: ResourceType,
        remote_id: &str,
        remote_name: &str,
        local_item: Value,
        remote_item: Value,
    ) -> Option<BuiltEvent> {
        // Resolved from the pristine `local_item`, before anything below
        // mutates it — safe to do this early since `apply_atomic_strips`
        // never touches `stream_routes` (the only field any resolver
        // inspects; SERVICE is the only type with one, and its own fields
        // are all `Map`, never `Array`, so the strip is a no-op for it).
        let default_type = meta.resolve_default_type.map(|f| f(&local_item)).unwrap_or(resource_type);
        let default_value = self.default_value.core.get(&default_type).cloned().unwrap_or_else(|| json!({}));

        // `local_item`/`remote_item` arrive with `id` already stripped by
        // the caller, so this is an apples-to-apples check: raw-equal here
        // means every deterministic step below (nested sub-resource
        // diffing, plugin diffing, the final `diff_value`) would run on
        // identical inputs on both sides and find nothing — skips all of
        // it instead of re-discovering that. Gated on `default_value`
        // being empty too: `merge_default` only ever touches `local_item`,
        // not `remote_item`, so when a default is actually configured, two
        // raw-identical items that both omit the defaulted field would
        // still produce a real event once merged (adding the default to
        // `local_item` alone) — this fast path must not skip that.
        // Doesn't help SSL much either way: `apply_atomic_strips` below
        // strips its private key from both sides first, so an unchanged
        // SSL resource's raw items are rarely equal before that strip even
        // happens.
        if local_item == remote_item && is_empty_value(&default_value) {
            return None;
        }
        let original_local_item = local_item.clone();
        let mut local_item = local_item;
        let mut remote_item = remote_item;

        apply_atomic_strips(meta, &mut local_item, &mut remote_item);

        // Compute sub-events and strip nested MAP-resource fields (routes/upstreams/...)
        // from local_item/remote_item *before* merge_default runs below. This is a pure
        // performance reordering, not a behavior change: merge_default only ever touches
        // keys present in `defaults`, so removing unrelated (nested-collection) keys from
        // `resource` first cannot change how the remaining keys get merged — and the
        // original code stripped these same keys from merged_local_item/remote_item right
        // after merge_default anyway, so the nested collections never survived into the
        // final comparison either way. Doing it first just avoids merge_default deep-cloning
        // (and immediately discarding) potentially large nested arrays on every update.
        let nested_fields: Vec<(&str, &FieldMeta)> = meta
            .fields
            .iter()
            .filter(|(_, fm)| matches!(fm, FieldMeta::Map { nested: true, .. }))
            .map(|(k, fm)| (*k, fm))
            .collect();

        let mut sub_events: Vec<Event> = Vec::new();
        if !nested_fields.is_empty() {
            let local_sub_config = extract_sub_config(meta, &local_item);
            let remote_sub_config = extract_sub_config(meta, &remote_item);
            let parent_name = meta.propagates_parent_name.then_some(remote_name);
            let nested_events =
                DifferV4::diff_map(&local_sub_config, &remote_sub_config, Some(&self.default_value), parent_name);
            sub_events.extend(
                nested_events.into_iter().map(|e| postprocess_sub_event(remote_name, remote_id, e)),
            );

            for (field_name, _) in &nested_fields {
                strip_key(&mut local_item, field_name);
                strip_key(&mut remote_item, field_name);
            }
        }
        let mut merged_local_item = merge_default(&local_item, &default_value);

        let output_remote_item = remote_item.clone();
        let mut plugin_changed = false;
        let mut output_local_item = original_local_item;

        let object_map_fields: Vec<&str> = meta
            .fields
            .iter()
            .filter(|(_, fm)| matches!(fm, FieldMeta::ObjectMap))
            .map(|(k, _)| *k)
            .collect();

        if !object_map_fields.is_empty() {
            for field_name in &object_map_fields {
                let local_plugins = merged_local_item.get(*field_name).cloned().unwrap_or_else(|| json!({}));
                let remote_plugins = remote_item.get(*field_name).cloned().unwrap_or_else(|| json!({}));

                let (field_changed, merged_local_plugins) = self.diff_plugins(&local_plugins, &remote_plugins);
                if field_changed {
                    plugin_changed = true;
                }

                if !is_empty_value(&merged_local_plugins) {
                    set_key(&mut merged_local_item, field_name, merged_local_plugins);
                }
            }

            output_local_item = merged_local_item.clone();
            if !plugin_changed {
                for field_name in &object_map_fields {
                    strip_key(&mut merged_local_item, field_name);
                    strip_key(&mut remote_item, field_name);
                }
            }
        }

        let diff = diff_value(&remote_item, &merged_local_item);

        if !plugin_changed && diff.is_none() && sub_events.is_empty() {
            return None;
        }

        let only_sub_events = !sub_events.is_empty() && !plugin_changed && diff.is_none();

        let event = if only_sub_events {
            None
        } else {
            let kind = EventKind::Update { old_value: output_remote_item, new_value: output_local_item, diff };
            Some(Event::new(resource_type, kind, remote_id, remote_name))
        };
        Some(BuiltEvent { event, sub_events })
    }

    fn diff_plugins(&self, local: &Value, remote: &Value) -> (bool, Value) {
        if is_empty_value(local) && is_empty_value(remote) {
            return (false, local.clone());
        }

        let local_obj = local.as_object().cloned().unwrap_or_default();
        let mut merged_local = Map::new();
        for (plugin_name, config) in &local_obj {
            let default = self.default_value.plugins.get(plugin_name).cloned().unwrap_or_else(|| json!({}));
            merged_local.insert(plugin_name.clone(), merge_default(config, &default));
        }
        let remote_obj = remote.as_object().cloned().unwrap_or_default();

        let checker = |left: &Map<String, Value>, right: &Map<String, Value>| -> bool {
            for (name, left_plugin) in left {
                match right.get(name) {
                    None => return true,
                    Some(right_plugin) => {
                        if diff_value(left_plugin, right_plugin).is_some() {
                            return true;
                        }
                    }
                }
            }
            false
        };

        let changed = checker(&merged_local, &remote_obj) || checker(&remote_obj, &merged_local);
        (changed, Value::Object(merged_local))
    }
}

/// `FlatConfiguration` serializes to a JSON object by construction (every
/// field is a named struct field), so the object cast can't fail.
pub(crate) fn to_internal_configuration(config: &FlatConfiguration) -> InternalConfiguration {
    match serde_json::to_value(config).expect("FlatConfiguration always serializes") {
        Value::Object(map) => map,
        other => unreachable!("FlatConfiguration must serialize to a JSON object, got {other:?}"),
    }
}

/// Build (name, id, item) tuples from an `InternalConfiguration` field.
fn extract_tuples(config: &InternalConfiguration, meta: &ResourceDifferMeta, parent_name: Option<&str>) -> Vec<ResourceTuple> {
    let Some(config_field) = meta.config_field else { return vec![] };
    let Some(field) = config.get(config_field) else { return vec![] };

    match meta.collection_kind {
        CollectionKind::Record => field
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), k.clone(), v.clone())).collect())
            .unwrap_or_default(),
        CollectionKind::Array => field
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|item| {
                        let name = (meta.get_name)(item);
                        let id = (meta.generate_id)(item, parent_name);
                        (name, id, item.clone())
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn prepare_remote_item(raw: &Value) -> Value {
    let mut item = raw.clone();
    strip_key(&mut item, "id");
    item
}

/// Recursively strip `id` from every item reachable through this resource's
/// `{listType: Map, nested: true}` fields — see the comment at the `handle_create`
/// call site for why this is needed on the create path only.
fn strip_nested_ids(meta: &ResourceDifferMeta, item: &mut Value) {
    for (field_name, field_meta) in meta.fields {
        let FieldMeta::Map { nested: true, config_key, .. } = field_meta else { continue };
        let Some(arr) = item.get_mut(*field_name).and_then(Value::as_array_mut) else { continue };
        let key = config_key.unwrap_or(field_name);
        let Some(&nested_type) = ResourceType::ALL.iter().find(|rt| differ_meta(**rt).config_field == Some(key)) else {
            continue;
        };
        let nested_meta = differ_meta(nested_type);
        for nested_item in arr.iter_mut() {
            strip_key(nested_item, "id");
            strip_nested_ids(&nested_meta, nested_item);
        }
    }
}

/// Build an `InternalConfiguration` containing only the nested sub-resource fields
/// declared as `{listType: Map, nested: true}` in the resource's field metadata.
fn extract_sub_config(meta: &ResourceDifferMeta, item: &Value) -> InternalConfiguration {
    let mut sub_config = InternalConfiguration::new();
    for (field_name, field_meta) in meta.fields {
        if let FieldMeta::Map { nested: true, config_key, .. } = field_meta
            && let Some(value) = item.get(*field_name) {
                let key = config_key.unwrap_or(field_name);
                sub_config.insert(key.to_string(), value.clone());
            }
    }
    sub_config
}

/// Strip atomic sub-fields from array items, as declared by `{listType: Array, stripItemFields}`.
/// Used for SSL certificates: the private key is removed before comparison because the remote
/// never returns it in plaintext.
fn apply_atomic_strips(meta: &ResourceDifferMeta, local_item: &mut Value, remote_item: &mut Value) {
    for (field_name, field_meta) in meta.fields {
        let FieldMeta::Array { strip_item_fields } = field_meta else { continue };
        for sub_field in *strip_item_fields {
            if let Some(arr) = local_item.get_mut(*field_name).and_then(Value::as_array_mut) {
                for item in arr.iter_mut() {
                    strip_key(item, sub_field);
                }
            }
            if let Some(arr) = remote_item.get_mut(*field_name).and_then(Value::as_array_mut) {
                for item in arr.iter_mut() {
                    strip_key(item, sub_field);
                }
            }
        }
    }
}

fn postprocess_sub_event(parent_name: &str, parent_id: &str, mut event: Event) -> Event {
    let regenerated = generate_id(&event.resource_name);
    let new_resource_id = if regenerated == event.resource_id {
        generate_id(&format!("{parent_name}.{}", event.resource_name))
    } else {
        event.resource_id
    };
    event.resource_id = new_resource_id;
    event.parent_id = Some(parent_id.to_string());
    event
}

fn merge_default(resource: &Value, defaults: &Value) -> Value {
    let mut result = resource.clone();
    let Value::Object(defaults_map) = defaults else { return result };
    let Value::Object(result_map) = &mut result else { return result };

    for (key, value) in defaults_map {
        let existing = result_map.get(key).cloned();
        match existing {
            None | Some(Value::Null) => {
                if !(value.is_object() || value.is_array()) {
                    result_map.insert(key.clone(), value.clone());
                }
            }
            Some(existing) => {
                if value.is_object() && existing.is_object() {
                    result_map.insert(key.clone(), merge_default(&existing, value));
                } else if let (Value::Array(value_arr), Value::Array(existing_arr)) = (value, &existing)
                    && let Some(first_default) = value_arr.first() {
                        let merged_arr: Vec<Value> =
                            existing_arr.iter().map(|item| merge_default(item, first_default)).collect();
                        result_map.insert(key.clone(), Value::Array(merged_arr));
                    }
            }
        }
    }

    result
}

fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Object(m) => m.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn strip_key(v: &mut Value, key: &str) {
    if let Value::Object(map) = v {
        map.remove(key);
    }
}

fn set_key(v: &mut Value, key: &str, value: Value) {
    if let Value::Object(map) = v {
        map.insert(key.to_string(), value);
    }
}

/// Event ordering table: deletions precede creates, SSL creates precede routes
/// (SSL may be referenced by upstream mTLS and must exist first). Combos not
/// listed here sort last.
fn order_priority(resource_type: ResourceType, event_type: EventType) -> u32 {
    use EventType::*;
    use ResourceType::*;
    match (resource_type, event_type) {
        (Route, Delete) => 0,
        (StreamRoute, Delete) => 1,
        (Service, Delete) => 2,
        (Upstream, Delete) => 3,
        (Consumer, Delete) => 5,
        (ConsumerGroup, Delete) => 6,
        (Ssl, Delete) => 7,

        (Ssl, Create) => 8,
        (Ssl, Update) => 9,
        (Route, Update) => 10,
        (StreamRoute, Update) => 11,
        (Service, Update) => 12,
        (Upstream, Update) => 13,
        (ConsumerGroup, Update) => 15,
        (Consumer, Update) => 16,

        (Service, Create) => 17,
        (Route, Create) => 19,
        (StreamRoute, Create) => 20,
        (ConsumerGroup, Create) => 21,
        (Consumer, Create) => 22,

        (Upstream, Create) => 23,
        (GlobalRule, Delete) => 24,
        (GlobalRule, Create) => 25,
        (GlobalRule, Update) => 26,
        (PluginMetadata, Delete) => 27,
        (PluginMetadata, Create) => 28,
        (PluginMetadata, Update) => 29,
        (ConsumerCredential, Delete) => 30,
        (ConsumerCredential, Create) => 31,
        (ConsumerCredential, Update) => 32,

        _ => u32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `merge_default` is a pure `Value` transform with no resource-specific
    // meaning of its own, so it's tested directly here rather than through
    // `DifferV4::diff` — going through a typed `FlatConfiguration` would
    // force this test's arbitrary field names into a real resource's
    // `deny_unknown_fields` schema, which isn't what's under test.
    #[test]
    fn selectively_merges_objects_in_default_values() {
        let resource = json!({ "name": "Test Service", "nested": {} });
        let defaults = json!({ "scalar": "default", "nested": { "present": "default", "missing": { "deep": "default" } } });

        // "scalar": missing from `resource`, not itself an object/array, so
        // it's filled in from `defaults`.
        // "nested": present as an (empty) object, so it's recursed into —
        // "present" (a missing scalar leaf) gets filled in, but "missing"
        // (a missing *object* leaf) is left absent rather than being
        // materialized wholesale from `defaults`.
        let expected = json!({ "name": "Test Service", "scalar": "default", "nested": { "present": "default" } });
        assert_eq!(merge_default(&resource, &defaults), expected);
    }
}
