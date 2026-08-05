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
//! Nested shapes structurally identical to APISIX's own admin API (health
//! checks, node lists, timeouts, plugin maps, TLS) are reused directly from
//! `adc_sdk::resources` rather than duplicated — API7 Enterprise's
//! per-gateway-group admin API is APISIX-compatible for everything below
//! the resource envelope. `labels` is the one exception: every resource's
//! wire `labels` is a plain string map (unlike ADC's own string-or-array
//! `Labels`), with a multi-value label round-tripped as a JSON-array-shaped
//! string rather than a nested JSON array — see `transformer`'s label
//! conversion functions.
//!
//! `Route`/`Service`/`StreamRoute` read a resource's own id back as `id`,
//! but write it under a differently-named field instead
//! (`route_id`/`service_id`/`stream_route_id`) — the dashboard's admin API
//! quirk, not a modeling choice made here. `Route.service_id` is a second,
//! unrelated field with the same name: the *parent* service's id, present
//! in both directions.

use std::collections::HashMap;

use adc_sdk::resources::{
    Expr, Plugins, SslClient, SslType, Timeout, UpstreamBalancer, UpstreamHealthCheck,
    UpstreamKeepalivePool, UpstreamNode, UpstreamPassHost, UpstreamScheme, UpstreamTls,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    pub server_port: Option<i64>,
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "adc_sdk::resources::serialize_optional_whole_number_as_integer"
    )]
    pub retry_timeout: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
