//! `apply`: the structural counterpart to [`DifferV4::diff`][crate::DifferV4::diff].
//!
//! `diff(local, remote)` describes how to turn `remote` into `local`;
//! `apply(events, remote)` performs that turn, reconstructing `local` from
//! `remote` plus the events `diff` computed between them —
//! `apply(diff(local, remote), remote) == local` (mod array order and the
//! `Some(vec![])`/`None` equivalence `diff` already treats as one thing; see
//! this module's property test). A caller that only has `remote` (a cached
//! baseline) and `events` (a differ's output) can use this to recover the
//! `local` they were computed from, without having kept `local` itself
//! around.
//!
//! Operates on the same untyped `InternalConfiguration` representation
//! `DifferV4` diffs on — matching identity derivation (`ResourceDifferMeta::
//! generate_id`) and nesting structure (`FieldMeta::Map { nested: true, .. }`)
//! is what makes the round-trip exact; reimplementing either independently
//! from the typed `adc_sdk::resources` side would drift from `diff` the
//! moment one of the two is updated without the other.

use std::collections::{HashMap, HashSet};

use adc_sdk::resources::FlatConfiguration;
use adc_sdk::{Event, EventKind, ResourceType};
use serde_json::{Map, Value};

use crate::differ_meta::{CollectionKind, ResourceDifferMeta, differ_meta};
use crate::differ_v4::to_internal_configuration;
use crate::field_meta::FieldMeta;

type InternalConfiguration = Map<String, Value>;

/// See the module doc comment.
pub fn apply(events: &[Event], base: &FlatConfiguration) -> FlatConfiguration {
    let mut doc = to_internal_configuration(base);
    // Grouped once, up front, rather than `apply_events` re-scanning all of
    // `events` by `parent_id` on every recursive call — with N resources
    // each nesting their own sub-events, that scan-per-parent made the
    // whole walk O(events * parents) instead of O(events + parents).
    let mut by_parent: HashMap<Option<&str>, Vec<&Event>> = HashMap::new();
    for event in events {
        by_parent.entry(event.parent_id.as_deref()).or_default().push(event);
    }
    apply_events(&mut doc, &by_parent, None);
    serde_json::from_value(Value::Object(doc)).unwrap_or_else(|e| {
        panic!("apply produced a document that failed to deserialize back into FlatConfiguration: {e}")
    })
}

/// Applies every event whose `parent_id` matches `parent_id` — the
/// resources living directly in `container` — then recurses into each of
/// those resources' own nested collections (if any) for the events that
/// belong one level deeper, keyed by that resource's own id as the new
/// `parent_id`. `None` at the root; `Some(id)` when `container` is standing
/// in for one specific parent resource's nested fields (see the loop below).
fn apply_events(container: &mut InternalConfiguration, events_by_parent: &HashMap<Option<&str>, Vec<&Event>>, parent_id: Option<&str>) {
    let mut by_type: HashMap<ResourceType, Vec<&Event>> = HashMap::new();
    for &event in events_by_parent.get(&parent_id).into_iter().flatten() {
        by_type.entry(event.resource_type).or_default().push(event);
    }
    for (resource_type, type_events) in by_type {
        apply_to_collection(container, resource_type, &type_events);
    }

    // Only resource types with `Map { nested: true, .. }` fields (Service,
    // Consumer) have children of their own to recurse into; every other
    // type's `fields` table has none, so this is a no-op for them.
    for &resource_type in ResourceType::ALL {
        let meta = differ_meta(resource_type);
        let Some(config_field) = meta.config_field else { continue };
        let nested_fields: Vec<(&'static str, Option<&'static str>)> = meta
            .fields
            .iter()
            .filter_map(|(field_name, field_meta)| match field_meta {
                FieldMeta::Map { nested: true, config_key, .. } => Some((*field_name, *config_key)),
                _ => None,
            })
            .collect();
        if nested_fields.is_empty() {
            continue;
        }
        let Some(items) = container.get_mut(config_field).and_then(Value::as_array_mut) else { continue };
        for item in items {
            let item_id = (meta.generate_id)(item, None);
            let Value::Object(item_map) = item else { continue };
            for (field_name, config_key) in &nested_fields {
                // Keyed the same way `DifferV4`'s `extract_sub_config` keys
                // it (`config_key`, falling back to the field's own name),
                // so `apply_to_collection`'s `differ_meta(..).config_field`
                // lookup for the nested resource type lines up.
                let nested_key = config_key.unwrap_or(field_name);
                let mut nested_doc = InternalConfiguration::new();
                if let Some(existing) = item_map.remove(*field_name) {
                    nested_doc.insert(nested_key.to_string(), existing);
                }
                apply_events(&mut nested_doc, events_by_parent, Some(item_id.as_str()));
                if let Some(value) = nested_doc.remove(nested_key) {
                    item_map.insert((*field_name).to_string(), value);
                }
            }
        }
    }
}

/// Applies `events` (all for the same `resource_type`, all already filtered
/// to this nesting level) onto `container`'s collection field for that type.
/// An empty result collapses back to an absent field rather than an empty
/// array/object — the differ itself never distinguishes the two (an absent
/// field and an empty one extract to the same zero tuples, see
/// `extract_tuples`), so this is the collection's canonical written form,
/// not a lossy simplification.
fn apply_to_collection(container: &mut InternalConfiguration, resource_type: ResourceType, events: &[&Event]) {
    let meta = differ_meta(resource_type);
    let Some(config_field) = meta.config_field else { return };

    match meta.collection_kind {
        CollectionKind::Record => {
            let mut map = match container.remove(config_field) {
                Some(Value::Object(map)) => map,
                _ => Map::new(),
            };
            for event in events {
                match &event.kind {
                    EventKind::Delete { .. } => {
                        map.remove(&event.resource_id);
                    }
                    EventKind::Create { new_value } | EventKind::Update { new_value, .. } => {
                        map.insert(event.resource_id.clone(), new_value.clone());
                    }
                }
            }
            if !map.is_empty() {
                container.insert(config_field.to_string(), Value::Object(map));
            }
        }
        CollectionKind::Array => {
            let mut items = match container.remove(config_field) {
                Some(Value::Array(items)) => items,
                _ => Vec::new(),
            };
            // Indexed once up front rather than scanning `items` per event
            // (the previous `.iter_mut().find()`/`.retain()` per event made
            // a sync touching many resources in a large collection
            // quadratic — O(events * items) instead of O(events + items)).
            let mut index: HashMap<String, usize> =
                items.iter().enumerate().map(|(i, item)| ((meta.generate_id)(item, None), i)).collect();
            let mut deleted: HashSet<&str> = HashSet::new();

            for event in events {
                match &event.kind {
                    EventKind::Delete { .. } => {
                        deleted.insert(event.resource_id.as_str());
                    }
                    EventKind::Create { new_value } | EventKind::Update { new_value, .. } => {
                        let mut item = new_value.clone();
                        stamp_identity(&mut item, resource_type, &event.resource_id);
                        // Whatever this event's own `new_value` happens to
                        // carry under a nested field name is not reliable
                        // (see `strip_own_nested_fields`'s doc comment) —
                        // dropped unconditionally, in favor of the old
                        // item's own nested content (an Update) or nothing
                        // (a Create, where the nested sub-events emitted
                        // alongside this one — see `DifferV4::handle_create`
                        // — fully populate it during the recursive pass in
                        // `apply_events`).
                        strip_own_nested_fields(&mut item, &meta);
                        match index.get(&event.resource_id) {
                            Some(&i) => {
                                carry_over_nested_fields(&items[i], &mut item, &meta);
                                items[i] = item;
                            }
                            None => {
                                index.insert(event.resource_id.clone(), items.len());
                                items.push(item);
                            }
                        }
                        // A resource id can't legitimately get both a
                        // Delete and a Create/Update within one diff's
                        // events, but this guards against a stale entry
                        // rather than relying on that.
                        deleted.remove(event.resource_id.as_str());
                    }
                }
            }

            if !deleted.is_empty() {
                items.retain(|item| !deleted.contains((meta.generate_id)(item, None).as_str()));
            }
            if !items.is_empty() {
                container.insert(config_field.to_string(), Value::Array(items));
            }
        }
    }
}

/// The differ always strips a resource's own identity out of `new_value`
/// before building an event — `id` via `strip_key(.., "id")` on both the
/// create and update paths in `DifferV4`, since `event.resource_id` already
/// carries it. `Consumer` is the one type with no `id` field at all:
/// `username` is its identity, a required field that's already present in
/// `new_value` as ordinary resource data, never stripped.
fn stamp_identity(item: &mut Value, resource_type: ResourceType, resource_id: &str) {
    if resource_type == ResourceType::Consumer {
        return;
    }
    if let Value::Object(map) = item {
        map.insert("id".to_string(), Value::String(resource_id.to_string()));
    }
}

/// Whether a Create/Update event's own `new_value` carries its nested
/// fields (`routes`/`stream_routes`/`upstreams` on a `Service`, `credentials`
/// on a `Consumer`) at all — and if so, whether their own ids survive — is
/// inconsistent across `DifferV4`'s code paths: present with ids stripped
/// on a Create (`handle_create` doesn't strip the nested fields themselves,
/// only recursively strips `id` within them via `strip_nested_ids`); present
/// with ids intact on an Update whose only change was to a non-plugin field
/// (`output_local_item` is cloned *before* nested fields are stripped for
/// diffing); absent entirely on an Update that also changed a `plugins`/
/// `global_rules`-style field (`output_local_item` becomes the *stripped*
/// `merged_local_item` in that branch). None of that content should ever be
/// trusted regardless: the differ always *also* emits a complete, separate
/// set of nested events (see `DifferV4::handle_create`/`handle_update`'s own
/// `sub_events`), computed by diffing the nested collection directly against
/// the parent's *previous* one — trusting the embedded copy on top of that
/// would at best duplicate it, having already been reconstructed from the
/// nested events. So this is called unconditionally, and the recursive pass
/// in `apply_events` is the only place nested content ever gets written.
fn strip_own_nested_fields(item: &mut Value, meta: &ResourceDifferMeta) {
    let Value::Object(map) = item else { return };
    for (field_name, field_meta) in meta.fields {
        if matches!(field_meta, FieldMeta::Map { nested: true, .. }) {
            map.remove(*field_name);
        }
    }
}

/// The counterpart to [`strip_own_nested_fields`]: an Update replaces
/// `old`'s entry in the collection outright, so whatever nested content
/// `old` was holding has to be copied onto `new_item` first, or it's lost
/// before the recursive pass in `apply_events` even gets a chance to look
/// for it (that pass finds an item's nested fields by reading them off the
/// item itself, not off whatever used to be there). Nothing to do for a
/// Create — there is no `old` — its nested content is built up from
/// scratch by the recursive pass instead, see `strip_own_nested_fields`.
fn carry_over_nested_fields(old: &Value, new_item: &mut Value, meta: &ResourceDifferMeta) {
    let (Value::Object(old_map), Value::Object(new_map)) = (old, new_item) else { return };
    for (field_name, field_meta) in meta.fields {
        if matches!(field_meta, FieldMeta::Map { nested: true, .. })
            && let Some(value) = old_map.get(*field_name)
        {
            new_map.insert((*field_name).to_string(), value.clone());
        }
    }
}
