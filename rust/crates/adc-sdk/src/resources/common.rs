//! Types shared across multiple resource definitions.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A resource's labels: each value is a single string or a list of strings.
pub type Labels = HashMap<String, LabelValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LabelValue {
    Single(String),
    Multiple(Vec<String>),
}

/// An arbitrary, unvalidated plugin configuration object — its shape depends
/// entirely on which plugin it configures, so it's kept structurally open
/// (no `deny_unknown_fields`) rather than typed field-by-field.
pub type Plugin = serde_json::Map<String, Value>;

/// A plugin name to configuration map.
pub type Plugins = serde_json::Map<String, Value>;

/// An APISIX condition expression: an arbitrary nested array structure
/// evaluated by the gateway at request time.
pub type Expr = Vec<Value>;

/// `#[schemars(range(...))]` only sets inclusive `minimum`/`maximum` — a
/// timeout of exactly 0 makes no sense, so this hand-writes the
/// `exclusiveMinimum` keyword `range` can't express.
fn positive_number_schema(_gen: &mut SchemaGenerator) -> Schema {
    json_schema!({"type": "number", "exclusiveMinimum": 0})
}

// id/name/description/port bounds are repeated directly on each field via
// stacked `#[schemars(length(...), regex(...))]`/`range(...)` attributes
// rather than factored into a shared `schema_with` fn: on an `Option<T>`
// field, `schema_with` needs an explicit `#[serde(default)]` alongside it
// (to stay out of `required`) and its returned schema needs its own
// `"anyOf": [..., {"type": "null"}]` (Option<T> doesn't get that for free
// the way it does for plain field generation) — verified empirically, easy
// to get wrong silently. Plain stacked attributes don't have either problem.
// `schema_with` is still used below and elsewhere for schemas no stackable
// attribute can express (`anyOf` unions, `exclusiveMinimum`, `allOf`).

/// Connect/send/read timeouts in seconds, shared by upstream and route configs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Timeout {
    #[schemars(schema_with = "positive_number_schema")]
    pub connect: f64,
    #[schemars(schema_with = "positive_number_schema")]
    pub send: f64,
    #[schemars(schema_with = "positive_number_schema")]
    pub read: f64,
}
