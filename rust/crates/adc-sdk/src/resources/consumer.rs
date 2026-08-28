//! The `Consumer`, `ConsumerCredential` and `ConsumerGroup` resources.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{Labels, Plugin, Plugins};

/// A credential attached to a consumer (e.g. an API key or JWT secret).
/// `type` is kept as a plain string rather than a closed enum: the 4-value
/// restriction ("key-auth"/"basic-auth"/"jwt-auth"/"hmac-auth") is a semantic
/// rule for the validation layer, not encoded here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerCredential {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[a-zA-Z0-9-_.]+$"))]
    pub id: Option<String>,
    #[schemars(length(min = 1, max = 65536))]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65536))]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(rename = "type")]
    pub r#type: String,
    pub config: Plugin,
}

/// A consumer, identified by `username` rather than `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Consumer {
    #[schemars(length(min = 1, max = 65536))]
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65536))]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<ConsumerCredential>>,
}

/// A named group of consumers sharing plugin configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[a-zA-Z0-9-_.]+$"))]
    pub id: Option<String>,
    #[schemars(length(min = 1, max = 65536))]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 65536))]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumers: Option<Vec<Consumer>>,
}
