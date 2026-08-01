//! Minimal Rust port of `@api7/adc-sdk`, scoped to exactly what `adc-differ`
//! needs: resource/event type definitions, the differ field-metadata table,
//! and a generic JSON value-diff utility (replacing the TS side's
//! `datum-diff` dependency).
//!
//! What's deliberately not here: input validation (a `validator`-crate
//! equivalent of `libs/sdk/src/core/schema.ts`'s Zod refinements), a
//! `Backend` trait, and `schemars` JSON Schema export. Resource bodies are
//! plain `serde_json::Value` rather than per-resource typed structs — see
//! the note on `InternalConfiguration` below for why.

pub mod differ_meta;
pub mod event;
pub mod field_meta;
pub mod resource;
pub mod utils;
pub mod value_diff;

pub use differ_meta::{CollectionKind, ResourceDifferMeta, differ_meta};
pub use event::{Event, EventType};
pub use field_meta::FieldMeta;
pub use resource::{FieldListType, ResourceType};
pub use value_diff::{DiffPath, PathSegment, ValueDiff, diff_value};

use serde_json::{Map, Value};

/// Mirrors `InternalConfiguration` in `libs/sdk/src/core/schema.ts`.
///
/// Unlike the TS side, this is a plain `Map<String, Value>` keyed by config
/// field name (`services`, `routes`, `global_rules`, ...) rather than a
/// strongly-typed struct — see the crate-level docs for why: the differ
/// algorithm itself treats resource bodies as opaque structural values, so a
/// generic `Value`-based representation is both the lowest-risk and most
/// faithful translation of `differv4.ts`'s actual (dynamically-typed) behavior.
pub type InternalConfiguration = Map<String, Value>;

/// Mirrors `DefaultValue` in `libs/sdk/src/core/differ.ts`.
#[derive(Debug, Clone, Default)]
pub struct DefaultValue {
    pub core: std::collections::HashMap<ResourceType, Value>,
    pub plugins: std::collections::HashMap<String, Value>,
}
