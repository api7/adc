//! Shared test helpers for the integration tests in this directory.

use adc_sdk::resources::FlatConfiguration;
use adc_sdk::{Event, EventKind, ResourceType};
use serde_json::Value;

/// Converts a `json!({...})` literal into a `FlatConfiguration`. Panics on a
/// shape that doesn't deserialize (wrong field name/type, non-object) rather
/// than silently dropping or defaulting it, so a malformed test fixture
/// fails loudly instead of quietly diffing against the wrong input.
#[allow(dead_code)]
pub fn config(v: Value) -> FlatConfiguration {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("expected a valid FlatConfiguration, got {v}: {e}"))
}

#[allow(dead_code)]
pub fn ev(rt: ResourceType, kind: EventKind, id: &str, name: &str) -> Event {
    Event::new(rt, kind, id, name)
}
