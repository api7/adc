//! The `Route` (HTTP) and `StreamRoute` (TCP/UDP) resources.

use serde::{Deserialize, Serialize};

use super::common::{Expr, Labels, Plugins, Timeout};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    pub uris: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<Expr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<HttpMethod>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_addrs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_func: Option<String>,
}

/// A stream (TCP/UDP/TLS) route: matches connections by address/SNI rather than URI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamRoute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub server_port: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
}
