//! The typed ADC resource model. This *is* the resource definition — the
//! shape the CLI parses local YAML/JSON declarative configuration into.
//!
//! Fields with a default value deserialize with that default already filled
//! in (see the doc comment on `upstream.rs` for the fields this applies to)
//! — parsing is active, not a passive shape description: a field that's
//! missing from input but has a default is still populated on the result.
//!
//! Not yet covered: semantic validation (cross-field rules, regex, min/max),
//! which is a separate, later pass on top of this same model.
//!
//! Naming note: distinct from `crate::resource` (the differ's `ResourceType`/
//! `FieldListType` metadata enums) and from `crate::InternalConfiguration`
//! (the `Map<String, Value>` alias `adc-differ` operates on) — different
//! modules, different concerns, no relation beyond sharing this crate.

pub mod common;
pub mod consumer;
pub mod route;
pub mod service;
pub mod ssl;
pub mod upstream;

pub use common::{
    Expr, Labels, LabelValue, Plugin, Plugins, Timeout, serialize_optional_whole_number_as_integer,
    serialize_whole_number_as_integer,
};
pub use consumer::{Consumer, ConsumerCredential, ConsumerGroup};
pub use route::{HttpMethod, Route, StreamRoute};
pub use service::{Service, ServiceRoutes};
pub use ssl::{SSL, SSLCertificate, SslClient, SslProtocol, SslType};
pub use upstream::{
    Upstream, UpstreamBalancer, UpstreamHealthCheck, UpstreamHealthCheckActive,
    UpstreamHealthCheckActiveHealthy, UpstreamHealthCheckActiveUnhealthy, UpstreamHealthCheckPassive,
    UpstreamHealthCheckPassiveHealthy, UpstreamHealthCheckPassiveUnhealthy, UpstreamHealthCheckType,
    UpstreamKeepalivePool, UpstreamNode, UpstreamPassHost, UpstreamScheme, UpstreamTls,
};

use serde::{Deserialize, Serialize};

/// A global rule is just a plugin config map applied gateway-wide.
pub type GlobalRule = Plugins;
/// Metadata (shared config) for a plugin, keyed by plugin name.
pub type PluginMetadata = Plugins;

/// The external, user-facing declarative config file shape: nested
/// sub-resources embedded under their parent, no top-level
/// routes/upstreams/consumer_credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssls: Option<Vec<SSL>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumers: Option<Vec<Consumer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_groups: Option<Vec<ConsumerGroup>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_rules: Option<GlobalRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_metadata: Option<PluginMetadata>,
}

/// The flattened internal representation: adds top-level
/// routes/stream_routes/consumer_credentials/upstreams alongside the nested
/// sub-resources, so every resource is also reachable directly by its own
/// collection field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InternalConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssls: Option<Vec<SSL>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumers: Option<Vec<Consumer>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_rules: Option<GlobalRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_metadata: Option<PluginMetadata>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<Route>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_routes: Option<Vec<StreamRoute>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_credentials: Option<Vec<ConsumerCredential>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstreams: Option<Vec<Upstream>>,
}
