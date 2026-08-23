//! ADC's core data model: resource type definitions, the typed resource
//! layer for parsing declarative configuration (`resources` module), a
//! semantic-validation pass on top of that same model (`lint`), differ
//! event types shared with backend/CLI consumers, a generic JSON value-diff
//! utility, the `Backend` trait implemented by each gateway integration, and
//! the `Converter` trait implemented by each source-format converter.
//!
//! `lint` is a separate call (`lint::lint`), not baked into `Deserialize` —
//! deserializing a `resources::Configuration` only ever enforces shape
//! (types, required fields, unknown fields); semantic rules run only when a
//! caller explicitly asks. The differ's own field-metadata table lives in
//! `adc-differ` instead of here, since nothing outside the differ consumes it.

pub mod backend;
pub mod converter;
pub mod default_value;
pub mod event;
pub mod lint;
pub mod resource;
pub mod resources;
pub mod utils;
pub mod value_diff;

pub use backend::{
    Backend, BackendError, BackendMetadata, BackendSyncOptions, BackendSyncResult, BackendValidateResult,
    BackendValidationError, DEFAULT_EXIT_ON_FAILURE, SYNC_EVENT_SPAN_NAME,
};
pub use converter::{ConvertError, Converter};
pub use default_value::DefaultValue;
pub use event::{Event, EventKind, EventType};
pub use resource::{FieldListType, ResourceType};
pub use value_diff::{DiffPath, PathSegment, ValueDiff, diff_value, format_path};

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
