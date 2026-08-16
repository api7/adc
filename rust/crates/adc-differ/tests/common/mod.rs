//! Shared test helpers for the integration tests in this directory.

use adc_sdk::{Event, EventKind, InternalConfiguration, ResourceType};
use serde_json::Value;

/// Converts a `json!({...})` literal into an `InternalConfiguration`. Panics
/// on a non-object `Value` rather than silently falling back to an empty
/// map, so a malformed test fixture fails loudly instead of quietly diffing
/// against `{}`.
#[allow(dead_code)]
pub fn config(v: Value) -> InternalConfiguration {
    v.as_object().cloned().unwrap_or_else(|| panic!("expected a JSON object, got: {v}"))
}

#[allow(dead_code)]
pub fn ev(rt: ResourceType, kind: EventKind, id: &str, name: &str) -> Event {
    Event::new(rt, kind, id, name)
}
