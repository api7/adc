//! The `Upstream` resource and its nested health-check/tls/keepalive-pool shapes.
//!
//! Fields with a declared default are deserialized as their bare type (not
//! `Option<T>`), with the default filled in via `#[serde(default = ...)]` when
//! the key is missing — parsing actively applies defaults rather than just
//! describing an optional shape. Fields that are merely optional with no
//! default stay `Option<T>` and are genuinely absent when not provided.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::common::{Labels, Timeout};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpstreamBalancer {
    #[default]
    #[serde(rename = "roundrobin")]
    RoundRobin,
    #[serde(rename = "chash")]
    Chash,
    #[serde(rename = "least_conn")]
    LeastConn,
    #[serde(rename = "ewma")]
    Ewma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpstreamScheme {
    #[serde(rename = "grpc")]
    Grpc,
    #[serde(rename = "grpcs")]
    Grpcs,
    #[default]
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "https")]
    Https,
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "tls")]
    Tls,
    #[serde(rename = "udp")]
    Udp,
    #[serde(rename = "kafka")]
    Kafka,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpstreamPassHost {
    #[default]
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "node")]
    Node,
    #[serde(rename = "rewrite")]
    Rewrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UpstreamHealthCheckType {
    #[default]
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "https")]
    Https,
    #[serde(rename = "tcp")]
    Tcp,
}

fn default_healthy_http_statuses() -> Vec<u32> {
    vec![200, 302]
}
fn default_successes() -> u32 {
    2
}
fn default_unhealthy_http_statuses() -> Vec<u32> {
    vec![429, 404, 500, 501, 502, 503, 504, 505]
}
fn default_http_failures() -> u32 {
    5
}
fn default_tcp_failures() -> u32 {
    2
}
fn default_timeouts() -> u32 {
    3
}
fn default_interval() -> u32 {
    1
}
fn default_active_timeout() -> f64 {
    1.0
}
fn default_concurrency() -> f64 {
    10.0
}
fn default_http_path() -> String {
    "/".to_string()
}
fn default_keepalive_pool_size() -> u32 {
    320
}
fn default_keepalive_idle_timeout() -> f64 {
    60.0
}
fn default_keepalive_requests() -> u32 {
    1000
}

/// A single upstream target: host, port, and load-balancing weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamNode {
    pub host: String,
    pub port: u32,
    pub weight: i64,
    #[serde(default)]
    pub priority: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Map<String, Value>>,
}

/// Passive-health-check thresholds for marking a target healthy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckPassiveHealthy {
    #[serde(default = "default_healthy_http_statuses")]
    pub http_statuses: Vec<u32>,
    #[serde(default = "default_successes")]
    pub successes: u32,
}

/// Passive-health-check thresholds for marking a target unhealthy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckPassiveUnhealthy {
    #[serde(default = "default_unhealthy_http_statuses")]
    pub http_statuses: Vec<u32>,
    #[serde(default = "default_http_failures")]
    pub http_failures: u32,
    #[serde(default = "default_tcp_failures")]
    pub tcp_failures: u32,
    #[serde(default = "default_timeouts")]
    pub timeouts: u32,
}

/// Active-health-check thresholds for marking a target healthy, plus the
/// polling interval (seconds, default 1) active checks need that passive
/// checks don't.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckActiveHealthy {
    #[serde(default = "default_healthy_http_statuses")]
    pub http_statuses: Vec<u32>,
    #[serde(default = "default_successes")]
    pub successes: u32,
    #[serde(default = "default_interval")]
    pub interval: u32,
}

/// Active-health-check thresholds for marking a target unhealthy, plus the
/// polling interval (seconds, default 1) active checks need that passive
/// checks don't.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckActiveUnhealthy {
    #[serde(default = "default_unhealthy_http_statuses")]
    pub http_statuses: Vec<u32>,
    #[serde(default = "default_http_failures")]
    pub http_failures: u32,
    #[serde(default = "default_tcp_failures")]
    pub tcp_failures: u32,
    #[serde(default = "default_timeouts")]
    pub timeouts: u32,
    #[serde(default = "default_interval")]
    pub interval: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckActive {
    #[serde(rename = "type", default)]
    pub r#type: UpstreamHealthCheckType,
    #[serde(default = "default_active_timeout")]
    pub timeout: f64,
    #[serde(default = "default_concurrency")]
    pub concurrency: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    #[serde(default = "default_http_path")]
    pub http_path: String,
    #[serde(default = "default_true")]
    pub https_verify_certificate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_request_headers: Option<Vec<String>>,
    // The wrapping object itself is only `.optional()` (no `.default()`), so
    // absence stays `None` — only fields *inside* it (once present) default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthy: Option<UpstreamHealthCheckActiveHealthy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unhealthy: Option<UpstreamHealthCheckActiveUnhealthy>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheckPassive {
    #[serde(rename = "type", default)]
    pub r#type: UpstreamHealthCheckType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthy: Option<UpstreamHealthCheckPassiveHealthy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unhealthy: Option<UpstreamHealthCheckPassiveUnhealthy>,
}

/// Health-check configuration: `active` (the gateway polls targets) is
/// required whenever `checks` is set at all; `passive` (inferred from live
/// traffic) is optional on top of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamHealthCheck {
    pub active: UpstreamHealthCheckActive,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passive: Option<UpstreamHealthCheckPassive>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamKeepalivePool {
    #[serde(default = "default_keepalive_pool_size")]
    pub size: u32,
    #[serde(default = "default_keepalive_idle_timeout")]
    pub idle_timeout: f64,
    #[serde(default = "default_keepalive_requests")]
    pub requests: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamTls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cert: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cert_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify: Option<bool>,
}

/// An upstream target group. `id` is always present on the struct as an
/// `Option`, but whether it's actually required depends on where the upstream
/// appears: a service's own default `upstream` doesn't need one, while a
/// named entry in `upstreams[]` does. That's a semantic rule for the
/// validation layer, not a structural one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(rename = "type", default)]
    pub r#type: UpstreamBalancer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<UpstreamHealthCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<UpstreamNode>>,
    #[serde(default)]
    pub scheme: UpstreamScheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_timeout: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<UpstreamTls>,
    // The wrapping object is only `.optional()` (no `.default()` on it as a
    // whole), so absence stays `None`; once present, its fields default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive_pool: Option<UpstreamKeepalivePool>,
    #[serde(default)]
    pub pass_host: UpstreamPassHost,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_host: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_args: Option<Map<String, Value>>,
}
