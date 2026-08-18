//! Types shared across multiple resource definitions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A resource's labels: each value is a single string or a list of strings.
pub type Labels = HashMap<String, LabelValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Connect/send/read timeouts in seconds, shared by upstream and route configs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeout {
    pub connect: f64,
    pub send: f64,
    pub read: f64,
}
