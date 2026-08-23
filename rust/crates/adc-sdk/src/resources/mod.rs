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
//! `FieldListType` metadata enums) and from `adc_differ`'s own internal
//! `InternalConfiguration` (a `Map<String, Value>` alias, private to that
//! crate) — different crates, different concerns, no relation beyond
//! `FlatConfiguration` being what `adc-differ` converts into that internal
//! shape at its public API boundary.

pub mod common;
pub mod consumer;
pub mod route;
pub mod service;
pub mod ssl;
pub mod upstream;

pub use common::{Expr, Labels, LabelValue, Plugin, Plugins, Timeout};
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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A global rule is just a plugin config map applied gateway-wide.
pub type GlobalRule = Plugins;
/// Metadata (shared config) for a plugin, keyed by plugin name.
pub type PluginMetadata = Plugins;

/// The external, user-facing declarative config file shape: nested
/// sub-resources embedded under their parent, no top-level
/// routes/upstreams/consumer_credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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

/// The flattened representation: adds top-level
/// routes/stream_routes/consumer_credentials/upstreams alongside the nested
/// sub-resources, so every resource is also reachable directly by its own
/// collection field. This is the shape `adc-differ` diffs at every recursion
/// level — the root call converts a `Configuration` into this via `From`,
/// and each recursive sub-call builds one directly to represent just one
/// parent resource's own nested collections.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlatConfiguration {
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

/// The root-level starting point for a diff: carries over every field
/// `FlatConfiguration` also has at the root (nothing to flatten there — a
/// `Configuration` never has its own top-level `routes`/`upstreams`/
/// `stream_routes`/`consumer_credentials`; those only appear once the differ
/// recurses into a service's nested collections). `consumer_groups` has no
/// counterpart on `FlatConfiguration` and is dropped: it isn't wired into
/// `adc-differ`'s resource-type table yet, so it's never read from either
/// shape today.
impl From<Configuration> for FlatConfiguration {
    fn from(config: Configuration) -> Self {
        FlatConfiguration {
            services: config.services,
            ssls: config.ssls,
            consumers: config.consumers,
            global_rules: config.global_rules,
            plugin_metadata: config.plugin_metadata,
            routes: None,
            stream_routes: None,
            consumer_credentials: None,
            upstreams: None,
        }
    }
}
