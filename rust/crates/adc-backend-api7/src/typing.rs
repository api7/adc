//! API7 Enterprise Dashboard admin API wire shapes — what actually comes
//! back from (and gets sent to) `/apisix/admin/*` when scoped to a gateway
//! group, as opposed to `adc_sdk::resources::*` (ADC's own resource model).
//!
//! `Deserialize` stays permissive (no `deny_unknown_fields`) since it's
//! decoding a live, evolving third-party API, not validating user-authored
//! config. `Serialize` (for building sync/validate request bodies) omits
//! `None` fields via `skip_serializing_if` rather than sending explicit
//! `null`s, matching `adc-backend-apisix::typing`'s own convention.
//!
//! Nested shapes structurally identical to APISIX's own admin API (node
//! lists, timeouts, plugin maps, TLS) are reused directly from
//! `adc_sdk::resources` rather than duplicated — API7 Enterprise's
//! per-gateway-group admin API is APISIX-compatible for everything below
//! the resource envelope. `labels` is the one exception: every resource's
//! wire `labels` is a plain string map (unlike ADC's own string-or-array
//! `Labels`), with a multi-value label round-tripped as a JSON-array-shaped
//! string rather than a nested JSON array — see `transformer`'s label
//! conversion functions. Active health checks are also not reused (see
//! `UpstreamHealthCheckActive` below).
//!
//! `Route`/`Service`/`StreamRoute` read a resource's own id back as `id`,
//! but write it under a differently-named field instead
//! (`route_id`/`service_id`/`stream_route_id`) — the dashboard's admin API
//! quirk, not a modeling choice made here. `Route.service_id` is a second,
//! unrelated field with the same name: the *parent* service's id, present
//! in both directions.

use std::collections::HashMap;

use adc_sdk::resources::{
    Expr, HttpMethod, Plugins, SslClient, SslType, Timeout, UpstreamBalancer, UpstreamHealthCheckActiveHealthy,
    UpstreamHealthCheckActiveUnhealthy, UpstreamHealthCheckPassive, UpstreamHealthCheckType, UpstreamKeepalivePool,
    UpstreamNode, UpstreamPassHost, UpstreamScheme, UpstreamTls,
};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

/// API7's `UpstreamTimeout.{connect,send,read}` fields are `int` on
/// some old gateway versions; `serde_json` always emits a decimal point
/// for an `f64`, even a whole number like `111.0`, and Go's `int` unmarshal
/// rejects that outright.
/// A bare integer JSON literal is valid input either way, so whole-number
/// values are always written as one.
fn serialize_timeout<S: Serializer>(
    timeout: &Option<Timeout>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let Some(timeout) = timeout else {
        return serializer.serialize_none();
    };
    let mut map = serializer.serialize_map(Some(3))?;
    for (key, value) in [
        ("connect", timeout.connect),
        ("send", timeout.send),
        ("read", timeout.read),
    ] {
        if value.fract() == 0.0 {
            map.serialize_entry(key, &(value as i64))?;
        } else {
            map.serialize_entry(key, &value)?;
        }
    }
    map.end()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Route {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Write-only alias for this route's own id — see the module doc
    /// comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    /// The *parent* service's id — present on both read and write, unlike
    /// `route_id`/`id` above. See the module doc comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<Expr>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_timeout"
    )]
    pub timeout: Option<Timeout>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StreamRoute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Write-only alias for this stream route's own id — see the module
    /// doc comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Service {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Write-only alias for this service's own id — see the module doc
    /// comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    /// Whether this service's traffic is `http` or `stream` — decides
    /// whether the fetcher's cascading query fetches `routes` or
    /// `stream_routes` for it on read, and is derived from the upstream's
    /// scheme on write. Absent on read means `http`, matching APISIX.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip_path_prefix: Option<bool>,
    /// This service's own default upstream, embedded inline — unlike
    /// APISIX, API7 has no separate top-level admin-API resource for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,

    /// Populated by [`crate::fetcher::Fetcher`], not by the dashboard API
    /// itself, from a nested cascading query keyed on this service's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<Route>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_routes: Option<Vec<StreamRoute>>,
    /// Named (non-default) upstreams for canary release, fetched from (and
    /// written to) a separate `/services/{id}/upstreams` collection —
    /// distinct from this service's own inline `upstream` field above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstreams: Option<Vec<Upstream>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConsumerCredential {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Consumer {
    /// `#[serde(default)]` even though a real consumer always has one: a
    /// schema-derived default value object (see `crate::default_value`)
    /// never declares one, and without this, deserializing that object
    /// into `Consumer` would fail outright and drop consumers from the
    /// default-value set entirely instead of contributing an empty `{}`.
    #[serde(default)]
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,

    /// Populated by [`crate::fetcher::Fetcher`] from a separate
    /// `/consumers/{username}/credentials` collection; never written back
    /// as part of a consumer's own body — credentials sync as their own
    /// independent events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<ConsumerCredential>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Ssl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<SslType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<SslClient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snis: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
}

/// No id field: unlike every other resource, a global rule's identity is
/// its single plugin's name within `plugins`, not a separate field — the
/// URL path alone addresses it on write.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalRule {
    #[serde(default)]
    pub plugins: Plugins,
}

pub type PluginMetadata = Plugins;

/// API7's wire field is `req_headers`; ADC's is `http_req_headers`. Every
/// other field is named the same.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpstreamHealthCheckActive {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<UpstreamHealthCheckType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_method: Option<HttpMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req_headers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_req_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https_verify_certificate: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthy: Option<UpstreamHealthCheckActiveHealthy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unhealthy: Option<UpstreamHealthCheckActiveUnhealthy>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpstreamHealthCheck {
    pub active: UpstreamHealthCheckActive,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive: Option<UpstreamHealthCheckPassive>,
}

/// Shared between the top-level `/apisix/admin/upstreams` list entry and an
/// upstream inlined directly into a service/route body — mirrors
/// `adc_backend_apisix::typing::Upstream`'s own reasoning for treating `id`
/// as optional on one shared shape rather than splitting read/write structs.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Upstream {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<UpstreamNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheme: Option<UpstreamScheme>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<UpstreamBalancer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<UpstreamHealthCheck>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_args: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass_host: Option<UpstreamPassHost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_timeout: Option<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_timeout"
    )]
    pub timeout: Option<Timeout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<UpstreamTls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_pool: Option<UpstreamKeepalivePool>,
}

/// The list envelope every API7 admin collection endpoint returns —
/// `total` isn't modeled since nothing in this crate reads it.
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse<T> {
    pub list: Vec<T>,
}

/// The envelope a handful of *singular* admin endpoints return instead
/// (`/apisix/admin/plugin_metadata`, `/api/version`, `/api/schema/core`) —
/// one object, not a list.
#[derive(Debug, Clone, Deserialize)]
pub struct ValueResponse<T> {
    pub value: T,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn timeout(connect: f64, send: f64, read: f64) -> Timeout {
        Timeout {
            connect,
            send,
            read,
        }
    }

    #[test]
    fn a_route_s_whole_number_timeout_has_no_decimal_point() {
        let route = Route {
            timeout: Some(timeout(111.0, 222.0, 333.0)),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_value(&route).unwrap()["timeout"],
            json!({"connect": 111, "send": 222, "read": 333})
        );
    }

    #[test]
    fn a_fractional_timeout_value_keeps_its_decimal_point() {
        let route = Route {
            timeout: Some(timeout(1.5, 222.0, 333.0)),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_value(&route).unwrap()["timeout"],
            json!({"connect": 1.5, "send": 222, "read": 333})
        );
    }

    #[test]
    fn no_timeout_omits_the_field_entirely() {
        let route = Route::default();

        assert_eq!(serde_json::to_value(&route).unwrap().get("timeout"), None);
    }

    #[test]
    fn a_service_s_inline_default_upstream_timeout_is_also_normalized() {
        let service = Service {
            upstream: Some(Upstream {
                timeout: Some(timeout(60.0, 60.0, 60.0)),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_value(&service).unwrap()["upstream"]["timeout"],
            json!({"connect": 60, "send": 60, "read": 60})
        );
    }
}
