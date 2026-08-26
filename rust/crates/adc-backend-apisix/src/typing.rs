//! APISIX admin API wire shapes — what actually comes back from (and gets
//! sent to) `/apisix/admin/*`, as opposed to `adc_sdk::resources::*` (ADC's
//! own resource model). The two are close but not identical: this side
//! carries APISIX-only linkage fields (`upstream_id`, `service_id`, ...).
//!
//! `Deserialize` stays permissive (no `deny_unknown_fields`) since it's
//! decoding a live, evolving third-party API rather than validating
//! user-authored config — an unrecognized field from a newer APISIX release
//! should be ignored, not rejected. `Serialize` (for building sync request
//! bodies) omits `None` fields via `skip_serializing_if` rather than sending
//! explicit `null`s, matching APISIX's own admin API examples and avoiding
//! any risk of an explicit `null` being interpreted differently than an
//! absent key.
//!
//! Nested shapes structurally identical to `adc_sdk::resources` (node
//! lists, timeouts, plugin maps, labels) are reused directly; active health
//! checks are not (see `UpstreamHealthCheckActive` below).

use std::collections::HashMap;

use adc_sdk::resources::{
    Expr, HttpMethod, Labels, Plugins, SslClient, SslProtocol, SslType, Timeout, UpstreamBalancer,
    UpstreamHealthCheckActiveHealthy, UpstreamHealthCheckActiveUnhealthy,
    UpstreamHealthCheckPassive, UpstreamHealthCheckType, UpstreamKeepalivePool, UpstreamNode,
    UpstreamPassHost, UpstreamScheme, UpstreamTls,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ADC_UPSTREAM_SERVICE_ID_LABEL: &str = "__ADC_UPSTREAM_SERVICE_ID";

/// Label a stream route's ADC name is smuggled through, since APISIX
/// stream routes have no native `name` field — see `transformer::
/// transform_stream_route`'s doc comment.
pub const ADC_NAME_LABEL: &str = "__ADC_NAME";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Route {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Narrower than every other resource's `labels` field (`Labels`,
    /// string-or-array): APISIX's admin API schema for routes only accepts
    /// plain string label values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uris: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addrs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_func: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Service {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,

    /// Populated by [`crate::Fetcher`], not by APISIX itself: named
    /// upstreams associated with this service via
    /// [`ADC_UPSTREAM_SERVICE_ID_LABEL`], indexed back onto the owning
    /// service once all upstreams are fetched.
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
    pub labels: Option<Labels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Consumer {
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<ConsumerCredential>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ssl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<SslType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snis: Option<Vec<String>>,
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
    pub ssl_protocols: Option<Vec<SslProtocol>>,

    pub status: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,
    pub plugins: Plugins,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConsumerGroup {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,
    pub plugins: Plugins,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalRule {
    pub id: String,
    #[serde(default)]
    pub plugins: Plugins,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamRouteProtocolLogger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamRouteProtocol {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superior_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logger: Option<Vec<StreamRouteProtocolLogger>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StreamRoute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<StreamRouteProtocol>,
}

/// APISIX accepts (and returns) upstream targets either as a list of node
/// objects, or as a legacy `"host:port": weight` map — both shapes are live
/// on real instances, so both need to parse. Sync always writes the list
/// form.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UpstreamNodes {
    List(Vec<UpstreamNode>),
    Map(HashMap<String, i64>),
}

/// APISIX's wire field is `req_headers`; ADC's is `http_req_headers`. Every
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
/// upstream inlined directly into a route/service/stream_route body (APISIX
/// calls the latter shape `InlineUpstream`, i.e. `Omit<Upstream, 'id'>`) —
/// modeled here as one type with an optional `id`, matching how
/// `adc_sdk::resources::Upstream` already treats `id` as position-dependent
/// rather than duplicating near-identical fields across two structs. On
/// write, `id` is intentionally left unset for a standalone upstream PUT —
/// the URL path already carries it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Upstream {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nodes: Option<UpstreamNodes>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<UpstreamTls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_pool: Option<UpstreamKeepalivePool>,
}

/// The etcd-derived list envelope every APISIX admin API list endpoint
/// returns. Only `key` and `value` are modeled — the response also carries
/// `createdIndex`/`modifiedIndex`/`total`, but nothing in this crate reads
/// them.
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse<T> {
    pub list: Vec<ListItem<T>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListItem<T> {
    pub key: String,
    pub value: T,
}
