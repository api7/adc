//! The `Route` (HTTP) and `StreamRoute` (TCP/UDP) resources.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use super::common::{Expr, Labels, Plugins, Timeout};

/// `remote_addrs` items: IPv4, IPv6, IPv4 CIDR, or IPv6 CIDR. Patterns copied
/// verbatim from the TS SDK's exported `schema.json` (Zod's own
/// `z.ipv4()`/`z.ipv6()`/`z.cidrv4()`/`z.cidrv6()`), not hand-rolled here.
/// The outer `anyOf`+`null` (rather than just the array schema) is what
/// keeps this field correctly excluded from `required` — `schema_with`
/// replaces schemars' usual `Option<T>` handling, so it has to be redone
/// here explicitly (see `common.rs`'s note on `schema_with`).
fn ip_or_cidr_schema(_gen: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [
            {
                "type": "array",
                "items": {
                    "anyOf": [
                        {"type": "string", "format": "ipv4", "pattern": r"^(?:(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(?:25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])$"},
                        {"type": "string", "format": "ipv6", "pattern": r"^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:))$"},
                        {"type": "string", "format": "cidrv4", "pattern": r"^((25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]|[0-9])\/([0-9]|[1-2][0-9]|3[0-2])$"},
                        {"type": "string", "format": "cidrv6", "pattern": r"^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|::|([0-9a-fA-F]{1,4})?::([0-9a-fA-F]{1,4}:?){0,6})\/(12[0-8]|1[01][0-9]|[1-9]?[0-9])$"}
                    ]
                }
            },
            {"type": "null"}
        ]
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
    #[serde(rename = "CONNECT")]
    Connect,
    #[serde(rename = "TRACE")]
    Trace,
    #[serde(rename = "PURGE")]
    Purge,
}

/// An HTTP route: matches requests by URI/host/method and applies plugins to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Route {
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
    #[schemars(inner(length(min = 1)))]
    pub hosts: Option<Vec<String>>,
    #[schemars(length(min = 1))]
    pub uris: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<Expr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub methods: Option<Vec<HttpMethod>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "ip_or_cidr_schema")]
    pub remote_addrs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_func: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A JSON `null` and an absent key must deserialize to the identical
    /// `Option<T>::None` for every optional field here — callers that build
    /// this document as a `serde_json::Value` in code (rather than through
    /// `Route`'s own `Serialize` impl) rely on this to fill an unset field
    /// with `Value::Null` rather than tracking which keys to omit.
    #[test]
    fn an_explicit_null_and_an_absent_key_deserialize_identically() {
        let with_null = json!({
            "name": "r", "uris": ["/x"],
            "description": null, "labels": null, "plugins": null,
        });
        let absent = json!({"name": "r", "uris": ["/x"]});
        let route_with_null: Route = serde_json::from_value(with_null).unwrap();
        let route_absent: Route = serde_json::from_value(absent).unwrap();
        assert_eq!(route_with_null, route_absent);
        assert!(route_with_null.description.is_none());
        assert!(route_with_null.labels.is_none());
        assert!(route_with_null.plugins.is_none());
        assert!(route_absent.description.is_none());
        assert!(route_absent.labels.is_none());
        assert!(route_absent.plugins.is_none());
    }
}

/// A stream (TCP/UDP/TLS) route: matches connections by address/SNI rather than URI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamRoute {
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
    pub remote_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub server_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub sni: Option<String>,
}
