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

use std::collections::BTreeMap;

use adc_sdk::resources::{
    Expr, HttpMethod, Plugins, SslClient, SslProtocol, SslType, Timeout, UpstreamBalancer,
    UpstreamHealthCheckActiveHealthy, UpstreamHealthCheckActiveUnhealthy,
    UpstreamHealthCheckPassive, UpstreamHealthCheckType, UpstreamKeepalivePool, UpstreamNode,
    UpstreamPassHost, UpstreamScheme, UpstreamTls,
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

pub type StandaloneLabels = BTreeMap<String, String>;

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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
/// stored with an empty array can come back as `{}` where a `Vec` was
/// expected. Normalizes that (and an explicit `null`, which `#[serde(default)]`
/// alone doesn't cover — that only fills in a *missing* key, not a present
/// one holding `null`) to an empty vec instead of rejecting it.
fn array_tolerating_empty_object<T: serde::de::DeserializeOwned>(value: Value) -> Result<Vec<T>, serde_json::Error> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Object(map) if map.is_empty() => Ok(Vec::new()),
        other => serde_json::from_value(other),
    }
}

fn deserialize_array_tolerating_empty_object<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    array_tolerating_empty_object(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

/// `#[serde(default)]` alone only fills in a *missing* `conf_version` key —
/// a present key holding `null` still fails to deserialize into a bare
/// `i64`. Falls back to `0` for that case too.
fn deserialize_conf_version<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<i64>::deserialize(deserializer)?.unwrap_or(0))
}

fn deserialize_upstream_nodes<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<UpstreamNode>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(other) => array_tolerating_empty_object(other).map(Some).map_err(serde::de::Error::custom),
    }
}

/// APISIX config file's wire field is `req_headers`; ADC's is
/// `http_req_headers`. Every other field is named the same.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct UpstreamHealthCheck {
    pub active: UpstreamHealthCheckActive,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive: Option<UpstreamHealthCheckPassive>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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
/// `name` instead). `#[serde(untagged)]` tries `Consumer` first; that's
/// only safe because the two shapes' required fields never overlap.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ConsumerOrCredential {
    Consumer(Consumer),
    Credential(ConsumerCredential),
}

impl ConsumerOrCredential {
    /// The key `crate::operator::stamp_versions` matches entries by — a
    /// consumer's `username`, or a credential's `id` (already
    /// `parentId/credentials/resourceId`-shaped, see
    /// `crate::transformer::credential_to_wire`).
    pub fn identity(&self) -> &str {
        match self {
            ConsumerOrCredential::Consumer(consumer) => &consumer.username,
            ConsumerOrCredential::Credential(credential) => &credential.id,
        }
    }

    /// The `modifiedIndex` slot for whichever variant this is — lets
    /// `crate::operator::stamp_versions` treat `Vec<ConsumerOrCredential>`
    /// the same generic way it treats every other collection, instead of a
    /// bespoke case just for this one mixed-variant type.
    pub fn modified_index(&self) -> i64 {
        match self {
            ConsumerOrCredential::Consumer(consumer) => consumer.modified_index,
            ConsumerOrCredential::Credential(credential) => credential.modified_index,
        }
    }

    pub fn modified_index_mut(&mut self) -> &mut i64 {
        match self {
            ConsumerOrCredential::Consumer(consumer) => &mut consumer.modified_index,
            ConsumerOrCredential::Credential(credential) => &mut credential.modified_index,
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct GlobalRule {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
}

/// A plugin's shared config: `id` + `modifiedIndex` are the only fields
/// this crate cares about; the rest of the plugin's own config keys —
/// arbitrary, shape depends on which plugin it configures — pass through
/// untouched via `extra`'s `#[serde(flatten)]`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct PluginMetadata {
    #[serde(rename = "modifiedIndex")]
    pub modified_index: i64,
    pub id: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StreamRouteProtocolLogger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Vec<Value>>,
    pub conf: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StreamRouteProtocol {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superior_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logger: Option<Vec<StreamRouteProtocolLogger>>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
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
/// whenever that collection changes (see `crate::operator`). Every array and
/// every `conf_version` is always present — `#[serde(default)]` tolerates an
/// older/fresh document that omits one on read, and this crate always writes
/// all 16 explicitly (`[]`/`0`, not an omitted field) rather than leaving it
/// to the reader to tell "empty" apart from "not sent".
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct ApisixStandalone {
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub routes: Vec<Route>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub services: Vec<Service>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub consumers: Vec<ConsumerOrCredential>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub ssls: Vec<Ssl>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub global_rules: Vec<GlobalRule>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub plugin_metadata: Vec<PluginMetadata>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub upstreams: Vec<Upstream>,
    #[serde(default, deserialize_with = "deserialize_array_tolerating_empty_object")]
    pub stream_routes: Vec<StreamRoute>,

    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub routes_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub services_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub consumers_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub ssls_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub global_rules_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub plugin_metadata_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub upstreams_conf_version: i64,
    #[serde(default, deserialize_with = "deserialize_conf_version")]
    pub stream_routes_conf_version: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_conf_version_defaults_to_zero() {
        let wire: ApisixStandalone = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(wire.routes_conf_version, 0);
    }

    #[test]
    fn an_explicit_null_conf_version_falls_back_to_zero_instead_of_failing_to_deserialize() {
        let wire: ApisixStandalone = serde_json::from_value(serde_json::json!({ "routes_conf_version": null })).unwrap();
        assert_eq!(wire.routes_conf_version, 0);
    }

    #[test]
    fn a_missing_array_field_defaults_to_empty() {
        let wire: ApisixStandalone = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(wire.routes, vec![]);
    }

    #[test]
    fn an_explicit_null_array_field_falls_back_to_empty_instead_of_failing_to_deserialize() {
        let wire: ApisixStandalone = serde_json::from_value(serde_json::json!({ "routes": null })).unwrap();
        assert_eq!(wire.routes, vec![]);
    }

    #[test]
    fn a_cjson_empty_object_in_place_of_an_array_deserializes_as_empty() {
        let wire: ApisixStandalone = serde_json::from_value(serde_json::json!({ "routes": {} })).unwrap();
        assert_eq!(wire.routes, vec![]);
    }
}
