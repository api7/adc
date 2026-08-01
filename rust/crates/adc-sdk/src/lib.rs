//! ADC's core data model: resource type definitions, the typed resource
//! layer for parsing declarative configuration (`resources` module), differ
//! event types shared with backend/CLI consumers, and a generic JSON
//! value-diff utility.
//!
//! Not yet here: semantic validation (cross-field rules, regex, min/max) on
//! top of the `resources` types, a `Backend` trait, and JSON Schema export.
//! The differ's own field-metadata table lives in `adc-differ` instead of
//! here, since nothing outside the differ consumes it.

pub mod event;
pub mod resource;
pub mod resources;
pub mod utils;
pub mod value_diff;

pub use event::{Event, EventType};
pub use resource::{FieldListType, ResourceType};
pub use value_diff::{DiffPath, PathSegment, ValueDiff, diff_value};

use serde_json::{Map, Value};

/// The differ's working representation of a full configuration: a plain
/// `Map<String, Value>` keyed by config field name (`services`, `routes`,
/// `global_rules`, ...). The differ algorithm treats resource bodies as
/// opaque structural values rather than strongly-typed ones, since diffing
/// is a generic structural operation independent of any one resource's shape.
///
/// Distinct from `resources::InternalConfiguration`, the typed counterpart
/// used to parse and validate declarative configuration.
pub type InternalConfiguration = Map<String, Value>;

/// Per-resource-type and per-plugin default values, merged into local
/// configuration before diffing so that a value matching the backend's
/// default doesn't show up as a spurious change.
#[derive(Debug, Clone, Default)]
pub struct DefaultValue {
    pub core: std::collections::HashMap<ResourceType, Value>,
    pub plugins: std::collections::HashMap<String, Value>,
}
