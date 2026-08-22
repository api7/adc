//! APISIX standalone's `/apisix/admin/configs` wire shape — the whole
//! declarative config document standalone mode reads/writes atomically, as
//! opposed to `adc_sdk::resources::*` (ADC's own resource model). Unlike
//! `adc-backend-apisix`'s per-collection admin API, every resource type here
//! lives as an array inside one document, each entry stamped with a
//! `modifiedIndex` version number instead of being independently versioned.
//!
//! `labels` is a plain `Record<string, string>` on every resource here
//! (never the string-or-array `Labels` union `adc_sdk::resources` and
//! `adc-backend-apisix`'s wire types use) — standalone's admin API schema
//! only ever accepts flat string values.
//!
//! `Deserialize` stays permissive (no `deny_unknown_fields`): a live
//! standalone config document carries fields this crate doesn't need to
//! model (e.g. `X-Last-Modified`/`X-Digest` metadata APISIX embeds in the
//! body itself), and an unrecognized field from a newer APISIX release
//! should be ignored, not rejected. `Serialize` omits `None` fields via
//! `skip_serializing_if` rather than sending explicit `null`s.

use std::collections::HashMap;

use adc_sdk::resources::{
    Expr, Plugins, SslClient, SslProtocol, SslType, Timeout, UpstreamBalancer, UpstreamHealthCheck,
    UpstreamKeepalivePool, UpstreamNode, UpstreamPassHost, UpstreamScheme, UpstreamTls,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Bookkeeping label a service-inlined default upstream is stamped with, so
/// [`crate::transformer::to_adc`] can find "which upstreams belong to this
/// service" among the flat top-level `upstreams` array — mirrors
/// `adc-backend-apisix`'s identically-named constant (this crate does
/// depend on that one, for its `Validator`, but not for this: the two
/// crates' upstream wire shapes differ too much to share the constant's
/// usage, so it's redefined here rather than imported).
pub const ADC_UPSTREAM_SERVICE_ID_LABEL: &str = "__ADC_UPSTREAM_SERVICE_ID";

pub type StandaloneLabels = HashMap<String, String>;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Route {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,

    pub uris: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<adc_sdk::resources::HttpMethod>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addrs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_func: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    pub service_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_websocket: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i64>,
}

/// APISIX's cjson encodes an empty Lua table as a JSON object (`{}`)
/// instead of an array — a standalone config document read back after being
/// stored with an empty `nodes: []` can come back as `nodes: {}`. A plain
/// `Vec<UpstreamNode>` would reject that outright, so this accepts either
/// shape and normalizes `{}` to an empty vec.
fn deserialize_upstream_nodes<'de, D>(deserializer: D) -> Result<Option<Vec<UpstreamNode>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) if map.is_empty() => Ok(Some(Vec::new())),
        Some(other) => serde_json::from_value(other).map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Upstream {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_upstream_nodes"
    )]
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<UpstreamHealthCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_args: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Service {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Consumer {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConsumerCredential {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

/// `consumers[]` holds both consumers and their credentials in one flat
/// array, discriminated by which required field is present: a `Consumer`
/// always has `username`, a `ConsumerCredential` never does (it has `id` +
/// `name` instead) — matches the TS union's own runtime discrimination
/// (`'username' in item`). `#[serde(untagged)]` tries `Consumer` first;
/// that's only safe because the two shapes' required fields never overlap.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ConsumerOrCredential {
    Consumer(Consumer),
    Credential(ConsumerCredential),
}

impl ConsumerOrCredential {
    /// The key `crate::operator` matches against to find "the entry this
    /// event refers to" — a consumer's `username`, or a credential's `id`
    /// (already `parentId/credentials/resourceId`-shaped, see
    /// `crate::operator::generate_id_from_event`).
    pub fn identity(&self) -> &str {
        match self {
            ConsumerOrCredential::Consumer(consumer) => &consumer.username,
            ConsumerOrCredential::Credential(credential) => &credential.id,
        }
    }

    pub fn as_consumer(&self) -> Option<&Consumer> {
        match self {
            ConsumerOrCredential::Consumer(consumer) => Some(consumer),
            ConsumerOrCredential::Credential(_) => None,
        }
    }

    pub fn as_credential(&self) -> Option<&ConsumerCredential> {
        match self {
            ConsumerOrCredential::Consumer(_) => None,
            ConsumerOrCredential::Credential(credential) => Some(credential),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Ssl {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,

    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<SslType>,
    pub snis: Vec<String>,
    pub cert: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<SslClient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssl_protocols: Option<Vec<SslProtocol>>,

    pub status: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GlobalRule {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

/// A plugin's shared config: `id` + `modifiedIndex` are the only fields
/// this crate cares about; the rest of the plugin's own config keys pass
/// through untouched via `extra`, matching the TS schema's `looseObject`
/// (arbitrary additional keys, shape depends on which plugin it configures).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PluginMetadata {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamRouteProtocolLogger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Vec<Value>>,
    pub conf: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamRouteProtocol {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superior_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logger: Option<Vec<StreamRouteProtocolLogger>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StreamRoute {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<StandaloneLabels>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    pub service_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<StreamRouteProtocol>,
}

/// The whole `/apisix/admin/configs` document: every resource type's array,
/// plus a per-collection `${collection}_conf_version` version number bumped
/// whenever that collection changes (see `crate::operator`). All fields are
/// optional since a fresh standalone instance's config starts out empty.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ApisixStandalone {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<Route>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<Service>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumers: Option<Vec<ConsumerOrCredential>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssls: Option<Vec<Ssl>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_rules: Option<Vec<GlobalRule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_metadata: Option<Vec<PluginMetadata>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstreams: Option<Vec<Upstream>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_routes: Option<Vec<StreamRoute>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumers_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssls_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_rules_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_metadata_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstreams_conf_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_routes_conf_version: Option<i64>,
}
