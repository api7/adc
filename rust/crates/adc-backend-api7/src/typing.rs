//! API7 Enterprise Dashboard admin API wire shapes — what actually comes
//! back from `/apisix/admin/*` when scoped to a gateway group, as opposed to
//! `adc_sdk::resources::*` (ADC's own resource model). `Deserialize` stays
//! permissive (no `deny_unknown_fields`) since it's decoding a live,
//! evolving third-party API, not validating user-authored config.
//!
//! Nested shapes structurally identical to APISIX's own admin API (health
//! checks, node lists, timeouts, plugin maps, labels, TLS) are reused
//! directly from `adc_sdk::resources` rather than duplicated — API7
//! Enterprise's per-gateway-group admin API is APISIX-compatible for
//! everything below the resource envelope.
//!
//! Only fields the fetcher actually reads are modeled here. `Route`/
//! `Service`/`StreamRoute` also carry a write-only id field
//! (`route_id`/`service_id`/`stream_route_id`) the dashboard expects in a
//! sync request body instead of `id` — deferred to the transformer/operator
//! work that builds those bodies.

use std::collections::HashMap;

use adc_sdk::resources::{
    Expr, Labels, Plugins, SslClient, SslType, Timeout, UpstreamBalancer, UpstreamHealthCheck,
    UpstreamKeepalivePool, UpstreamNode, UpstreamPassHost, UpstreamScheme, UpstreamTls,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct Route {
    pub id: Option<String>,
    pub name: String,
    pub desc: Option<String>,
    /// Narrower than every other resource's `labels`: API7's route schema
    /// only accepts plain string label values, same as APISIX's own.
    pub labels: Option<HashMap<String, String>>,
    pub service_id: Option<String>,

    pub plugins: Option<Plugins>,

    pub paths: Vec<String>,
    pub methods: Option<Vec<String>>,
    pub vars: Option<Expr>,

    pub enable_websocket: Option<bool>,
    pub priority: Option<i64>,
    pub timeout: Option<Timeout>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamRoute {
    pub id: Option<String>,
    pub name: Option<String>,
    pub desc: Option<String>,
    pub labels: Option<HashMap<String, String>>,
    pub service_id: Option<String>,

    pub plugins: Option<Plugins>,

    pub server_addr: Option<String>,
    pub server_port: Option<i64>,
    pub remote_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    pub id: Option<String>,
    pub name: Option<String>,
    pub desc: Option<String>,
    pub labels: Option<Labels>,
    /// Whether this service's traffic is `http` or `stream` — decides
    /// whether the fetcher's cascading query below fetches `routes` or
    /// `stream_routes` for it. Absent means `http`, matching APISIX.
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub hosts: Option<Vec<String>>,
    pub path_prefix: Option<String>,
    pub strip_path_prefix: Option<bool>,
    pub upstream: Option<Upstream>,
    pub plugins: Option<Plugins>,

    /// Populated by [`crate::fetcher::Fetcher`], not by the dashboard API
    /// itself, from a nested cascading query keyed on this service's id.
    pub routes: Option<Vec<Route>>,
    pub stream_routes: Option<Vec<StreamRoute>>,
    /// Named (non-default) upstreams for canary release, fetched from a
    /// separate `/services/{id}/upstreams` collection — distinct from this
    /// service's own inline `upstream` field above.
    pub upstreams: Option<Vec<Upstream>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConsumerCredential {
    pub id: Option<String>,
    pub name: String,
    pub desc: Option<String>,
    pub labels: Option<Labels>,
    pub plugins: Plugins,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Consumer {
    pub username: String,
    pub desc: Option<String>,
    pub labels: Option<Labels>,
    pub plugins: Option<Plugins>,

    /// Populated by [`crate::fetcher::Fetcher`] from a separate
    /// `/consumers/{username}/credentials` collection.
    pub credentials: Option<Vec<ConsumerCredential>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ssl {
    pub id: Option<String>,
    pub labels: Option<Labels>,
    #[serde(rename = "type")]
    pub ty: Option<SslType>,
    pub cert: Option<String>,
    pub certs: Option<Vec<String>>,
    pub key: Option<String>,
    pub keys: Option<Vec<String>>,
    pub client: Option<SslClient>,
    pub snis: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlobalRule {
    pub plugins: Plugins,
}

pub type PluginMetadata = Plugins;

/// Shared between the top-level `/apisix/admin/upstreams` list entry and an
/// upstream inlined directly into a service/route body — mirrors
/// `adc_backend_apisix::typing::Upstream`'s own reasoning for treating `id`
/// as optional on one shared shape rather than splitting read/write structs.
#[derive(Debug, Clone, Deserialize)]
pub struct Upstream {
    pub id: Option<String>,
    pub name: Option<String>,
    pub desc: Option<String>,
    pub labels: Option<Labels>,

    pub nodes: Option<Vec<UpstreamNode>>,
    pub scheme: Option<UpstreamScheme>,
    #[serde(rename = "type")]
    pub ty: Option<UpstreamBalancer>,
    pub hash_on: Option<String>,
    pub key: Option<String>,
    pub checks: Option<UpstreamHealthCheck>,

    pub discovery_type: Option<String>,
    pub service_name: Option<String>,
    pub discovery_args: Option<Value>,

    pub pass_host: Option<UpstreamPassHost>,
    pub upstream_host: Option<String>,
    pub retries: Option<u32>,
    pub retry_timeout: Option<f64>,
    pub timeout: Option<Timeout>,
    pub tls: Option<UpstreamTls>,
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
