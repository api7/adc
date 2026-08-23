//! Applying a differ's `Event`s to a standalone cluster: `sync`.
//!
//! Unlike `adc-backend-apisix`/`adc-backend-api7` (one HTTP request per
//! event), standalone has no per-resource admin API at all — every event
//! is folded into one in-memory config document (cloned from whatever was
//! cached from the last dump/sync), then that *whole document* is written
//! to every server with a single `PUT /apisix/admin/configs`. So there's
//! one `BackendSyncResult` per *server*, not per event (`event` on each
//! result is always `None` — no single event owns that write), and
//! `BackendSyncOptions::concurrent` (which bounds how many *events* run at
//! once in the other two backends) has nothing to bound here; the only
//! fan-out is across servers, and that's always unbounded — small enough
//! server counts in practice that there's no need to cap it.

use std::collections::{HashMap, HashSet};

use adc_backend_core::{HttpClient, Method, concurrent_map, concurrent_map_until_err};
use adc_sdk::resources::{self as adc};
use adc_sdk::{BackendError, BackendSyncOptions, BackendSyncResult, Event, EventType, PathSegment, ResourceType, ValueDiff};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};

use crate::backend::StandaloneServer;
use crate::typing::{self, ApisixStandalone, ConsumerOrCredential};
use crate::utils::stable_timestamp;

const CONFIG_ENDPOINT: &str = "/apisix/admin/configs";
const HEADER_DIGEST: &str = "x-digest";

/// What a successful `Operator::sync` learned, for the caller (which owns
/// the cache lock this ran under) to write back. `new_state` is `None` when
/// no server accepted the write — nothing to cache in that case.
pub struct SyncOutcome {
    pub results: Vec<BackendSyncResult>,
    pub new_state: Option<(i64, ApisixStandalone)>,
}

pub struct Operator {
    servers: Vec<StandaloneServer>,
    old_raw_config: ApisixStandalone,
    latest_known_version: Option<i64>,
}

impl Operator {
    /// Doesn't touch `Cache` itself — the caller (`Backend::sync`) already
    /// holds the lock for this `cache_key`'s entry and passes in what it
    /// read; `sync` below hands back what to write, for the same held lock
    /// to apply. Two `Cache` accesses for the same key inside one already
    /// locked critical section would deadlock (`tokio::sync::Mutex` isn't
    /// reentrant), which is the real reason this doesn't just take a
    /// `cache_key` and read/write `Cache::global()` itself.
    pub fn new(servers: Vec<StandaloneServer>, old_raw_config: ApisixStandalone, latest_known_version: Option<i64>) -> Self {
        Self { servers, old_raw_config, latest_known_version }
    }

    pub async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<SyncOutcome, BackendError> {
        let mut new_config = self.old_raw_config.clone();
        let mut increase_version: HashSet<ResourceType> = HashSet::new();

        // An always-advancing wall-clock read would normally never regress
        // on its own, but this process isn't the only writer — an earlier
        // `sync` (in this process or another) may have already pushed the
        // cluster's config to a version at or ahead of what the wall clock
        // reads right now: either a real clock rollback, or simply two
        // syncs landing in the same millisecond (millisecond resolution is
        // coarse enough for this to happen under normal, fast-succession
        // load, not just as a clock-skew edge case). Either way, clamping
        // to one past the latest known version keeps every write both
        // acceptable to the data plane and strictly increasing.
        let timestamp = resolve_sync_timestamp(stable_timestamp(), self.latest_known_version);

        for event in &events {
            apply_event_for_service_inlined_upstream(&mut new_config, &mut increase_version, timestamp, event)?;
            apply_event(&mut new_config, &mut increase_version, timestamp, event)?;
        }

        filter_orphan_credentials(&mut new_config);
        bump_conf_versions(&mut new_config, &increase_version, timestamp);

        let body = serde_json::to_string(&new_config)
            .map_err(|e| BackendError::Serialization(format!("encoding sync config: {e}")))?;
        let digest = sha1_hex(body.as_bytes());

        let put = |server: StandaloneServer| {
            let body = body.clone();
            let digest = digest.clone();
            async move {
                match put_one(&server.client, body, digest).await {
                    Ok(()) => Ok(BackendSyncResult { success: true, event: None, error: None, server: Some(server.server) }),
                    Err(error) => Err((server.server, error)),
                }
            }
        };

        let exit_on_failure = opts.exit_on_failure.unwrap_or(true);
        let results = if exit_on_failure {
            match concurrent_map_until_err(self.servers.clone(), None, put).await {
                Ok(results) => results,
                Err((_, error)) => {
                    // A server earlier in the batch may have already
                    // accepted the new document before a later one failed
                    // and aborted the rest — the cache's idea of "current
                    // state" can't be trusted either way at that point, so
                    // the caller resets its held entry rather than leaving
                    // it pointing at data a live server may have already
                    // moved past. The next dump() re-fetches and re-runs
                    // `find_latest` to discover the cluster's real state
                    // instead of trusting stale cache.
                    return Err(error);
                }
            }
        } else {
            concurrent_map(self.servers.clone(), None, put)
                .await
                .into_iter()
                .map(|outcome| match outcome {
                    Ok(result) => result,
                    Err((server, error)) => BackendSyncResult { success: false, event: None, error: Some(error), server: Some(server) },
                })
                .collect()
        };

        // Keyed on "at least one server accepted the write", not on
        // per-server completion order — with concurrent writers, "cache
        // whatever the most recently completed request happened to see"
        // has no coherent meaning.
        let new_state = results.iter().any(|result| result.success).then_some((timestamp, new_config));

        Ok(SyncOutcome { results, new_state })
    }
}

async fn put_one(client: &HttpClient, body: String, digest: String) -> Result<(), BackendError> {
    let request = client.request(Method::PUT, CONFIG_ENDPOINT)?.header(HEADER_DIGEST, digest).body(body);
    client.send(request).await?;
    Ok(())
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Never below `latest_known` — clamps `now` up to one past it whenever
/// `now` isn't already strictly ahead, covering both a real clock rollback
/// and two syncs landing in the same wall-clock millisecond alike.
fn resolve_sync_timestamp(now: i64, latest_known: Option<i64>) -> i64 {
    match latest_known {
        Some(latest) if latest >= now => latest + 1,
        _ => now,
    }
}

fn missing_new_value(event: &Event) -> BackendError {
    BackendError::Other(format!("{:?} event for resource {:?} is missing new_value", event.event_type(), event.resource_id).into())
}

fn missing_parent(event: &Event) -> BackendError {
    BackendError::Other(format!("{:?} event for resource {:?} is missing a parent_id", event.resource_type, event.resource_id).into())
}

fn deserialize_event_value<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, BackendError> {
    serde_json::from_value(value.clone()).map_err(|e| BackendError::Serialization(format!("decoding event payload: {e}")))
}

/// `ConsumerCredential` events are keyed by `parentId/credentials/resourceId`
/// (their owning consumer's username plus their own id) since a bare
/// `resourceId` alone isn't unique across different consumers' credentials;
/// every other resource type's own `resourceId` is already unique on its own.
fn generate_id_from_event(event: &Event) -> Result<String, BackendError> {
    if event.resource_type == ResourceType::ConsumerCredential {
        let parent_id = event.parent_id.as_deref().ok_or_else(|| missing_parent(event))?;
        Ok(format!("{parent_id}/credentials/{}", event.resource_id))
    } else {
        Ok(event.resource_id.clone())
    }
}

fn diff_path_is_upstream(diff: &ValueDiff) -> bool {
    let path = match diff {
        ValueDiff::New { path, .. } | ValueDiff::Deleted { path, .. } | ValueDiff::Edit { path, .. } | ValueDiff::Array { path, .. } => path,
    };
    matches!(path.first(), Some(PathSegment::Key(key)) if key == "upstream")
}

fn from_adc_labels(labels: Option<adc::Labels>) -> Option<typing::StandaloneLabels> {
    labels.map(|labels| labels.into_iter().map(|(key, value)| (key, stringify_label_value(value))).collect())
}

fn stringify_label_value(value: adc::LabelValue) -> String {
    match value {
        adc::LabelValue::Single(s) => s,
        adc::LabelValue::Multiple(items) => serde_json::to_string(&items).unwrap_or_default(),
    }
}

/// Builds an upstream's wire body from its ADC shape, minus `id`/
/// `modifiedIndex`/`name` (every caller overwrites those with values that
/// come from the owning `Event`, not from the upstream resource itself —
/// see `from_adc_upstream` and `apply_event_for_service_inlined_upstream`).
/// `parent_id`, when set, stamps the service-association bookkeeping label
/// onto a *named* upstream; a service's own inline default upstream is
/// never passed one (see `typing::ADC_UPSTREAM_SERVICE_ID_LABEL`'s doc
/// comment).
fn from_adc_upstream_wire(res: &adc::Upstream, parent_id: Option<&str>) -> typing::Upstream {
    let mut labels = from_adc_labels(res.labels.clone());
    if let Some(parent_id) = parent_id {
        labels
            .get_or_insert_with(HashMap::new)
            .insert(typing::ADC_UPSTREAM_SERVICE_ID_LABEL.to_string(), parent_id.to_string());
    }

    typing::Upstream {
        modified_index: 0,
        id: String::new(),
        name: res.name.clone().unwrap_or_default(),
        desc: res.description.clone(),
        labels,

        nodes: res.nodes.clone(),
        scheme: Some(res.scheme),
        ty: Some(res.r#type),
        hash_on: res.hash_on.clone(),
        key: res.key.clone(),

        pass_host: Some(res.pass_host),
        upstream_host: res.upstream_host.clone(),
        retries: res.retries,
        retry_timeout: res.retry_timeout,
        timeout: res.timeout.clone(),
        tls: res.tls.clone(),
        keepalive_pool: res.keepalive_pool.clone(),

        checks: res.checks.clone(),
        discovery_type: res.discovery_type.clone(),
        service_name: res.service_name.clone(),
        discovery_args: res.discovery_args.clone(),
    }
}

fn from_adc_route(event: &Event, modified_index: i64) -> Result<typing::Route, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::Route = deserialize_event_value(new_value)?;
    let parent_id = event.parent_id.clone().ok_or_else(|| missing_parent(event))?;

    Ok(typing::Route {
        modified_index,
        id: generate_id_from_event(event)?,
        name: res.name,
        desc: res.description,
        labels: from_adc_labels(res.labels),

        uris: res.uris,
        hosts: res.hosts,
        methods: res.methods,
        remote_addrs: res.remote_addrs,
        vars: res.vars,
        filter_func: res.filter_func,

        plugins: res.plugins,
        service_id: parent_id,

        timeout: res.timeout,
        enable_websocket: res.enable_websocket,
        priority: res.priority,
        status: Some(1),
    })
}

fn from_adc_service(event: &Event, modified_index: i64) -> Result<typing::Service, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::Service = deserialize_event_value(new_value)?;
    let id = generate_id_from_event(event)?;

    Ok(typing::Service {
        modified_index,
        id: id.clone(),
        name: res.name,
        desc: res.description,
        labels: from_adc_labels(res.labels),

        hosts: res.hosts,
        // Always points at this service's own id, regardless of whether it
        // actually has a default upstream. A service with no default
        // upstream simply references an upstream document that was never
        // written; standalone tolerates the dangling reference.
        upstream_id: Some(id),
        plugins: res.plugins,
    })
}

fn from_adc_consumer(event: &Event, modified_index: i64) -> Result<typing::Consumer, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::Consumer = deserialize_event_value(new_value)?;

    Ok(typing::Consumer {
        modified_index,
        username: generate_id_from_event(event)?,
        desc: res.description,
        labels: from_adc_labels(res.labels),
        plugins: res.plugins,
    })
}

fn from_adc_credential(event: &Event, modified_index: i64) -> Result<typing::ConsumerCredential, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::ConsumerCredential = deserialize_event_value(new_value)?;

    let mut plugins = adc::Plugins::new();
    plugins.insert(res.r#type, Value::Object(res.config));

    Ok(typing::ConsumerCredential {
        modified_index,
        id: generate_id_from_event(event)?,
        name: res.name,
        desc: res.description,
        labels: from_adc_labels(res.labels),
        plugins: Some(plugins),
    })
}

fn from_adc_ssl(event: &Event, modified_index: i64) -> Result<typing::Ssl, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::SSL = deserialize_event_value(new_value)?;

    let mut certificates = res.certificates.into_iter();
    let first = certificates
        .next()
        .ok_or_else(|| BackendError::Other(format!("ssl {:?} has no certificates", event.resource_id).into()))?;
    let (certs, keys): (Vec<String>, Vec<String>) = certificates.map(|c| (c.certificate, c.key)).unzip();

    Ok(typing::Ssl {
        modified_index,
        id: generate_id_from_event(event)?,
        desc: None,
        labels: from_adc_labels(res.labels),

        ty: Some(res.r#type),
        snis: res.snis,
        cert: first.certificate,
        key: first.key,
        certs: (!certs.is_empty()).then_some(certs),
        keys: (!keys.is_empty()).then_some(keys),
        client: res.client,
        ssl_protocols: res.ssl_protocols,

        status: 1,
    })
}

fn from_adc_global_rule(event: &Event, modified_index: i64) -> Result<typing::GlobalRule, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let mut plugins = adc::Plugins::new();
    plugins.insert(event.resource_id.clone(), new_value.clone());

    Ok(typing::GlobalRule {
        modified_index,
        id: generate_id_from_event(event)?,
        plugins: Some(plugins),
    })
}

fn from_adc_plugin_metadata(event: &Event, modified_index: i64) -> Result<typing::PluginMetadata, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let extra = match new_value {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };

    Ok(typing::PluginMetadata {
        modified_index,
        id: generate_id_from_event(event)?,
        extra,
    })
}

fn from_adc_upstream(event: &Event, modified_index: i64) -> Result<typing::Upstream, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::Upstream = deserialize_event_value(new_value)?;

    let mut wire = from_adc_upstream_wire(&res, event.parent_id.as_deref());
    wire.modified_index = modified_index;
    wire.id = generate_id_from_event(event)?;
    Ok(wire)
}

fn from_adc_stream_route(event: &Event, modified_index: i64) -> Result<typing::StreamRoute, BackendError> {
    let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
    let res: adc::StreamRoute = deserialize_event_value(new_value)?;
    let parent_id = event.parent_id.clone().ok_or_else(|| missing_parent(event))?;

    Ok(typing::StreamRoute {
        modified_index,
        id: generate_id_from_event(event)?,
        name: res.name,
        desc: res.description,
        labels: from_adc_labels(res.labels),

        remote_addr: res.remote_addr,
        server_addr: res.server_addr,
        server_port: res.server_port,
        sni: res.sni,
        service_id: parent_id,

        plugins: res.plugins,
        protocol: None,
    })
}

/// Creates/updates/deletes one entry in `field`, matched by `identity`
/// against the id [`generate_id_from_event`] derives — the same lookup
/// logic every resource type needs, parameterized over its own collection
/// type and identity accessor.
///
/// `Create` and `Update` both upsert: whichever one fires, a matching
/// existing entry is replaced and a missing one is inserted — a `Create`
/// for an id that's already present (a duplicate differ event, or a retried
/// sync landing on a base that already has it) replaces it instead of
/// appending a second entry with the same id, and symmetrically an
/// `Update` for an id that isn't there yet still leaves the document with
/// it rather than silently dropping the write. `Delete` alone stays a
/// genuine no-op for a missing id — there's nothing sensible to insert for
/// a deletion. Returns whether `field` actually changed.
fn upsert_or_delete<T>(
    field: &mut Option<Vec<T>>,
    event: &Event,
    identity: impl Fn(&T) -> &str,
    build: impl FnOnce() -> Result<T, BackendError>,
) -> Result<bool, BackendError> {
    match event.event_type() {
        EventType::Create | EventType::Update => {
            let target_id = generate_id_from_event(event)?;
            let vec = field.get_or_insert_with(Vec::new);
            let built = build()?;
            match vec.iter_mut().find(|item| identity(item) == target_id) {
                Some(slot) => *slot = built,
                None => vec.push(built),
            }
            Ok(true)
        }
        EventType::Delete => {
            let target_id = generate_id_from_event(event)?;
            let Some(vec) = field.as_mut() else { return Ok(false) };
            match vec.iter().position(|item| identity(item) == target_id) {
                Some(pos) => {
                    vec.remove(pos);
                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }
}

fn apply_event(config: &mut ApisixStandalone, increase_version: &mut HashSet<ResourceType>, timestamp: i64, event: &Event) -> Result<(), BackendError> {
    // A CONSUMER_CREDENTIAL shares its owning consumer's collection and
    // conf_version counter — there's no separate "credentials" array on
    // the wire.
    let version_resource_type = match event.resource_type {
        ResourceType::ConsumerCredential => ResourceType::Consumer,
        other => other,
    };

    let changed = match event.resource_type {
        ResourceType::Route => upsert_or_delete(&mut config.routes, event, |r| r.id.as_str(), || from_adc_route(event, timestamp))?,
        ResourceType::Service => {
            // Only an UPDATE can be a no-op for this collection: when the
            // diff shows nothing but the inline default upstream changed,
            // the service body itself is untouched (that's already handled
            // separately by `apply_event_for_service_inlined_upstream`,
            // called before this for every SERVICE event regardless) — so
            // skip writing to `config.services` for that update to avoid
            // bumping `services_conf_version` for no real change. A CREATE
            // or DELETE always writes: `EventKind::diff()` only ever
            // returns `Some` for `Update`, so gating on it unconditionally
            // (as opposed to only within the `Update` arm) would silently
            // drop every service CREATE — `.unwrap_or(&[])` makes an empty
            // diff, and `.any()` over an empty slice is always `false`.
            if event.event_type() == EventType::Update {
                let diff = event.kind.diff().unwrap_or(&[]);
                if diff.iter().any(|d| !diff_path_is_upstream(d)) {
                    upsert_or_delete(&mut config.services, event, |s| s.id.as_str(), || from_adc_service(event, timestamp))?
                } else {
                    false
                }
            } else {
                upsert_or_delete(&mut config.services, event, |s| s.id.as_str(), || from_adc_service(event, timestamp))?
            }
        }
        ResourceType::Consumer => {
            upsert_or_delete(&mut config.consumers, event, ConsumerOrCredential::identity, || {
                Ok(ConsumerOrCredential::Consumer(from_adc_consumer(event, timestamp)?))
            })?
        }
        ResourceType::ConsumerCredential => {
            upsert_or_delete(&mut config.consumers, event, ConsumerOrCredential::identity, || {
                Ok(ConsumerOrCredential::Credential(from_adc_credential(event, timestamp)?))
            })?
        }
        ResourceType::Ssl => upsert_or_delete(&mut config.ssls, event, |s| s.id.as_str(), || from_adc_ssl(event, timestamp))?,
        ResourceType::GlobalRule => {
            upsert_or_delete(&mut config.global_rules, event, |g| g.id.as_str(), || from_adc_global_rule(event, timestamp))?
        }
        ResourceType::PluginMetadata => {
            upsert_or_delete(&mut config.plugin_metadata, event, |p| p.id.as_str(), || from_adc_plugin_metadata(event, timestamp))?
        }
        ResourceType::Upstream => {
            upsert_or_delete(&mut config.upstreams, event, |u| u.id.as_str(), || from_adc_upstream(event, timestamp))?
        }
        ResourceType::StreamRoute => {
            upsert_or_delete(&mut config.stream_routes, event, |r| r.id.as_str(), || from_adc_stream_route(event, timestamp))?
        }
        // Not part of standalone's config document.
        ResourceType::ConsumerGroup | ResourceType::InternalStreamService => false,
    };

    if changed {
        increase_version.insert(version_resource_type);
    }
    Ok(())
}

/// A service's default upstream is stored as its own entry in the
/// top-level `upstreams` array (id = the service's own id), not embedded
/// inline in the service body — this keeps that entry in sync with
/// whatever the differ's SERVICE event carries. A service with no default
/// upstream (`upstream: None`) has nothing to write here.
fn apply_event_for_service_inlined_upstream(
    config: &mut ApisixStandalone,
    increase_version: &mut HashSet<ResourceType>,
    timestamp: i64,
    event: &Event,
) -> Result<(), BackendError> {
    if event.resource_type != ResourceType::Service {
        return Ok(());
    }

    let build_wire = |event: &Event| -> Result<Option<typing::Upstream>, BackendError> {
        let new_value = event.kind.new_value().ok_or_else(|| missing_new_value(event))?;
        let service: adc::Service = deserialize_event_value(new_value)?;
        let Some(upstream) = service.upstream else { return Ok(None) };
        let mut wire = from_adc_upstream_wire(&upstream, None);
        wire.id = event.resource_id.clone();
        wire.modified_index = timestamp;
        wire.name = event.resource_name.clone();
        Ok(Some(wire))
    };

    match event.event_type() {
        EventType::Create => {
            if let Some(wire) = build_wire(event)? {
                let upstreams = config.upstreams.get_or_insert_with(Vec::new);
                // This is a plain `Vec`, not a map — nothing stops two
                // entries from sharing an id unless every write path checks
                // first. Replace an existing entry rather than appending,
                // matching `upsert_or_delete` and this function's own
                // Update/Delete branches below.
                match upstreams.iter_mut().find(|item| item.id == event.resource_id) {
                    Some(slot) => *slot = wire,
                    None => upstreams.push(wire),
                }
                increase_version.insert(ResourceType::Upstream);
            }
        }
        EventType::Update => {
            let diff = event.kind.diff().unwrap_or(&[]);
            if !diff.iter().any(diff_path_is_upstream) {
                return Ok(());
            }
            // `.as_mut()`, not `get_or_insert_with`: there's nothing to
            // update when no upstream has ever been written, and
            // materializing an empty `Vec` here would flip
            // `config.upstreams` from `None` to `Some(vec![])` — a real
            // (if harmless-looking) change to what gets cached and PUT to
            // the servers, for an event that changed nothing.
            if let Some(wire) = build_wire(event)?
                && let Some(upstreams) = config.upstreams.as_mut()
                && let Some(slot) = upstreams.iter_mut().find(|item| item.id == event.resource_id)
            {
                *slot = wire;
                increase_version.insert(ResourceType::Upstream);
            }
        }
        EventType::Delete => {
            if let Some(upstreams) = config.upstreams.as_mut()
                && let Some(pos) = upstreams.iter().position(|item| item.id == event.resource_id)
            {
                upstreams.remove(pos);
                increase_version.insert(ResourceType::Upstream);
            }
        }
    }
    Ok(())
}

/// A newly-created consumer credential with no matching consumer (or a
/// consumer deleted in the same batch as its credentials survive) has
/// nothing left to belong to — dropped rather than left dangling.
fn filter_orphan_credentials(config: &mut ApisixStandalone) {
    let Some(consumers) = &mut config.consumers else { return };
    let usernames: HashSet<String> = consumers
        .iter()
        .filter_map(ConsumerOrCredential::as_consumer)
        .map(|consumer| consumer.username.clone())
        .collect();

    consumers.retain(|item| match item {
        ConsumerOrCredential::Consumer(_) => true,
        ConsumerOrCredential::Credential(credential) => {
            let owner = credential.id.split('/').next().unwrap_or("");
            usernames.contains(owner)
        }
    });
}

fn bump_conf_versions(config: &mut ApisixStandalone, increase_version: &HashSet<ResourceType>, timestamp: i64) {
    for resource_type in increase_version {
        let field = match resource_type {
            ResourceType::Route => &mut config.routes_conf_version,
            ResourceType::Service => &mut config.services_conf_version,
            ResourceType::Consumer => &mut config.consumers_conf_version,
            ResourceType::Ssl => &mut config.ssls_conf_version,
            ResourceType::GlobalRule => &mut config.global_rules_conf_version,
            ResourceType::PluginMetadata => &mut config.plugin_metadata_conf_version,
            ResourceType::Upstream => &mut config.upstreams_conf_version,
            ResourceType::StreamRoute => &mut config.stream_routes_conf_version,
            ResourceType::ConsumerCredential | ResourceType::ConsumerGroup | ResourceType::InternalStreamService => continue,
        };
        *field = Some(timestamp);
    }
}

#[cfg(test)]
mod tests {
    use adc_sdk::EventKind;
    use serde_json::json;
    use tokio::task::JoinSet;

    use super::*;

    fn event(rt: ResourceType, kind: EventKind, id: &str) -> Event {
        Event::new(rt, kind, id, id)
    }

    fn empty_config() -> ApisixStandalone {
        ApisixStandalone::default()
    }

    #[test]
    fn no_known_latest_version_uses_the_wall_clock_time_as_is() {
        assert_eq!(resolve_sync_timestamp(100, None), 100);
    }

    #[test]
    fn a_wall_clock_time_already_ahead_of_the_latest_known_version_is_used_as_is() {
        assert_eq!(resolve_sync_timestamp(100, Some(50)), 100);
    }

    /// Regression test: two syncs landing in the same wall-clock millisecond
    /// (a real, non-exotic race under fast-succession load, not just a
    /// clock-rollback edge case) must still produce a strictly increasing
    /// timestamp, not the same one twice.
    #[test]
    fn a_wall_clock_time_equal_to_the_latest_known_version_is_bumped_past_it() {
        assert_eq!(resolve_sync_timestamp(100, Some(100)), 101);
    }

    #[test]
    fn a_wall_clock_time_behind_the_latest_known_version_is_bumped_past_it() {
        assert_eq!(resolve_sync_timestamp(50, Some(100)), 101);
    }

    #[test]
    fn create_route_pushes_it_with_the_parent_service_id() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let mut route_event = event(
            ResourceType::Route,
            EventKind::Create { new_value: json!({ "name": "r1", "uris": ["/x"] }) },
            "r1",
        );
        route_event.parent_id = Some("svc-1".to_string());

        apply_event(&mut config, &mut increase_version, 100, &route_event).unwrap();

        let routes = config.routes.unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].id, "r1");
        assert_eq!(routes[0].service_id, "svc-1");
        assert_eq!(routes[0].modified_index, 100);
        assert!(increase_version.contains(&ResourceType::Route));
    }

    /// Regression test: a SERVICE CREATE event must always be pushed into
    /// `config.services`. An earlier bug gated this on `event.kind.diff()`
    /// unconditionally — `diff()` only ever returns `Some` for an `Update`
    /// event, so a CREATE (or DELETE) silently fell through to an empty
    /// diff, and `.any()` over it was always `false`, meaning `apply_event`
    /// never actually added the service at all despite the event
    /// succeeding at the HTTP layer.
    #[test]
    fn create_service_pushes_it_into_the_services_collection() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let service_event = event(
            ResourceType::Service,
            EventKind::Create { new_value: json!({ "name": "svc-1" }) },
            "svc-1",
        );

        apply_event(&mut config, &mut increase_version, 100, &service_event).unwrap();

        let services = config.services.unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].id, "svc-1");
        assert!(increase_version.contains(&ResourceType::Service));
    }

    /// Regression test: a CREATE for an id that's already present (a
    /// duplicate differ event, or a retried sync landing on a base that
    /// already has it) must replace the existing entry, not append a
    /// second one sharing the same id.
    #[test]
    fn create_for_an_already_present_id_replaces_it_instead_of_duplicating() {
        let mut config = empty_config();
        config.services = Some(vec![typing::Service {
            modified_index: 1,
            id: "svc-1".to_string(),
            name: "svc-1".to_string(),
            desc: Some("original".to_string()),
            labels: None,
            hosts: None,
            upstream_id: None,
            plugins: None,
        }]);
        let mut increase_version = HashSet::new();
        let service_event = event(
            ResourceType::Service,
            EventKind::Create { new_value: json!({ "name": "svc-1", "description": "replaced" }) },
            "svc-1",
        );

        apply_event(&mut config, &mut increase_version, 200, &service_event).unwrap();

        let services = config.services.unwrap();
        assert_eq!(services.len(), 1, "must not end up with two entries sharing id \"svc-1\"");
        assert_eq!(services[0].desc.as_deref(), Some("replaced"));
        assert_eq!(services[0].modified_index, 200);
    }

    /// Regression test: an UPDATE for an id that isn't present yet must
    /// still insert it, rather than silently dropping the write.
    /// Uses `Route`, not `Service`: `ResourceType::Service`'s own branch in
    /// `apply_event` gates UPDATE on the diff touching more than just
    /// `upstream` before it even calls `upsert_or_delete` (see that
    /// branch's own doc comment), which would make this test exercise that
    /// gating instead of the upsert behavior it's actually meant to cover.
    #[test]
    fn update_for_a_missing_id_inserts_it_instead_of_dropping_the_write() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let mut route_event = event(
            ResourceType::Route,
            EventKind::Update {
                old_value: json!({ "name": "r1", "uris": ["/x"] }),
                new_value: json!({ "name": "r1", "uris": ["/x"] }),
                diff: None,
            },
            "r1",
        );
        route_event.parent_id = Some("svc-1".to_string());

        apply_event(&mut config, &mut increase_version, 300, &route_event).unwrap();

        let routes = config.routes.unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].id, "r1");
        assert!(increase_version.contains(&ResourceType::Route));
    }

    #[test]
    fn delete_service_removes_it_from_the_services_collection() {
        let mut config = empty_config();
        config.services = Some(vec![typing::Service {
            modified_index: 1,
            id: "svc-1".to_string(),
            name: "svc-1".to_string(),
            desc: None,
            labels: None,
            hosts: None,
            upstream_id: None,
            plugins: None,
        }]);
        let mut increase_version = HashSet::new();
        let delete_service_event = event(ResourceType::Service, EventKind::Delete { old_value: json!({}) }, "svc-1");

        apply_event(&mut config, &mut increase_version, 200, &delete_service_event).unwrap();

        assert_eq!(config.services.unwrap().len(), 0);
        assert!(increase_version.contains(&ResourceType::Service));
    }

    #[test]
    fn a_service_update_that_only_touches_upstream_leaves_the_services_collection_untouched_but_updates_the_inline_upstream() {
        let mut config = empty_config();
        config.services = Some(vec![typing::Service {
            modified_index: 1,
            id: "svc-1".to_string(),
            name: "svc-1".to_string(),
            desc: None,
            labels: None,
            hosts: None,
            upstream_id: Some("svc-1".to_string()),
            plugins: None,
        }]);
        config.upstreams = Some(vec![typing::Upstream {
            modified_index: 1,
            id: "svc-1".to_string(),
            name: "svc-1".to_string(),
            desc: None,
            labels: None,
            nodes: None,
            scheme: None,
            ty: None,
            hash_on: None,
            key: None,
            pass_host: None,
            upstream_host: None,
            retries: None,
            retry_timeout: None,
            timeout: None,
            tls: None,
            keepalive_pool: None,
            checks: None,
            discovery_type: None,
            service_name: None,
            discovery_args: None,
        }]);
        let mut increase_version = HashSet::new();
        let diff = vec![ValueDiff::Edit {
            path: vec![PathSegment::Key("upstream".to_string())],
            lhs: json!({}),
            rhs: json!({}),
        }];
        let service_event = event(
            ResourceType::Service,
            EventKind::Update {
                old_value: json!({ "name": "svc-1" }),
                new_value: json!({ "name": "svc-1", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }),
                diff: Some(diff),
            },
            "svc-1",
        );

        apply_event_for_service_inlined_upstream(&mut config, &mut increase_version, 200, &service_event).unwrap();
        apply_event(&mut config, &mut increase_version, 200, &service_event).unwrap();

        assert_eq!(config.services.as_ref().unwrap()[0].modified_index, 1, "service body itself must stay untouched");
        assert!(!increase_version.contains(&ResourceType::Service));

        let upstreams = config.upstreams.unwrap();
        assert_eq!(upstreams.len(), 1);
        assert_eq!(upstreams[0].id, "svc-1");
        assert!(increase_version.contains(&ResourceType::Upstream));
    }

    /// Regression test: a service CREATE whose inline upstream id already
    /// has an entry in `config.upstreams` must replace it, not append a
    /// second entry sharing the same id.
    #[test]
    fn service_create_for_an_already_present_inline_upstream_id_replaces_it_instead_of_duplicating() {
        let mut config = empty_config();
        config.upstreams = Some(vec![typing::Upstream {
            modified_index: 1,
            id: "svc-1".to_string(),
            name: "svc-1".to_string(),
            desc: None,
            labels: None,
            nodes: None,
            scheme: None,
            ty: None,
            hash_on: None,
            key: None,
            pass_host: None,
            upstream_host: None,
            retries: None,
            retry_timeout: None,
            timeout: None,
            tls: None,
            keepalive_pool: None,
            checks: None,
            discovery_type: None,
            service_name: None,
            discovery_args: None,
        }]);
        let mut increase_version = HashSet::new();
        let service_event = event(
            ResourceType::Service,
            EventKind::Create {
                new_value: json!({ "name": "svc-1", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }),
            },
            "svc-1",
        );

        apply_event_for_service_inlined_upstream(&mut config, &mut increase_version, 200, &service_event).unwrap();

        let upstreams = config.upstreams.unwrap();
        assert_eq!(upstreams.len(), 1, "must not end up with two entries sharing id \"svc-1\"");
        assert_eq!(upstreams[0].modified_index, 200);
    }

    #[test]
    fn service_create_with_no_default_upstream_creates_no_inline_upstream_entry() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let service_event = event(ResourceType::Service, EventKind::Create { new_value: json!({ "name": "svc-no-upstream" }) }, "svc-2");

        apply_event_for_service_inlined_upstream(&mut config, &mut increase_version, 300, &service_event).unwrap();

        assert!(config.upstreams.is_none());
        assert!(!increase_version.contains(&ResourceType::Upstream));
    }

    /// Regression test: deleting a service that never had a default
    /// upstream must leave `config.upstreams` at `None`, not flip it to
    /// `Some(vec![])` — the latter would serialize as a stray `"upstreams":
    /// []` key in the synced document even though nothing about upstreams
    /// actually changed.
    #[test]
    fn deleting_a_service_with_no_upstream_leaves_the_upstreams_field_absent() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let delete_service_event = event(ResourceType::Service, EventKind::Delete { old_value: json!({}) }, "svc-no-upstream");

        apply_event_for_service_inlined_upstream(&mut config, &mut increase_version, 300, &delete_service_event).unwrap();

        assert!(config.upstreams.is_none());
        assert!(!increase_version.contains(&ResourceType::Upstream));
    }

    /// Regression test: an update whose diff touches `upstream` but for
    /// which no upstream entry exists yet (e.g. `config.upstreams` was
    /// never populated) must also leave it `None`, for the same reason.
    #[test]
    fn updating_a_services_upstream_with_no_existing_entry_leaves_the_upstreams_field_absent() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        let diff = vec![ValueDiff::Edit {
            path: vec![PathSegment::Key("upstream".to_string())],
            lhs: json!({}),
            rhs: json!({}),
        }];
        let service_event = event(
            ResourceType::Service,
            EventKind::Update {
                old_value: json!({ "name": "svc-no-upstream" }),
                new_value: json!({ "name": "svc-no-upstream", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }),
                diff: Some(diff),
            },
            "svc-no-upstream",
        );

        apply_event_for_service_inlined_upstream(&mut config, &mut increase_version, 300, &service_event).unwrap();

        assert!(config.upstreams.is_none());
        assert!(!increase_version.contains(&ResourceType::Upstream));
    }

    #[test]
    fn delete_consumer_credential_matches_by_the_parent_prefixed_id() {
        let mut config = empty_config();
        config.consumers = Some(vec![
            ConsumerOrCredential::Consumer(typing::Consumer {
                modified_index: 1,
                username: "alice".to_string(),
                desc: None,
                labels: None,
                plugins: None,
            }),
            ConsumerOrCredential::Credential(typing::ConsumerCredential {
                modified_index: 1,
                id: "alice/credentials/key1".to_string(),
                name: "key1".to_string(),
                desc: None,
                labels: None,
                plugins: None,
            }),
        ]);
        let mut increase_version = HashSet::new();
        let mut delete_event = event(ResourceType::ConsumerCredential, EventKind::Delete { old_value: json!({}) }, "key1");
        delete_event.parent_id = Some("alice".to_string());

        apply_event(&mut config, &mut increase_version, 400, &delete_event).unwrap();

        let consumers = config.consumers.unwrap();
        assert_eq!(consumers.len(), 1);
        assert!(consumers[0].as_consumer().is_some());
        assert!(increase_version.contains(&ResourceType::Consumer));
    }

    #[test]
    fn filter_orphan_credentials_drops_credentials_whose_consumer_is_gone() {
        let mut config = empty_config();
        config.consumers = Some(vec![
            ConsumerOrCredential::Consumer(typing::Consumer {
                modified_index: 1,
                username: "alice".to_string(),
                desc: None,
                labels: None,
                plugins: None,
            }),
            ConsumerOrCredential::Credential(typing::ConsumerCredential {
                modified_index: 1,
                id: "alice/credentials/key1".to_string(),
                name: "key1".to_string(),
                desc: None,
                labels: None,
                plugins: None,
            }),
            ConsumerOrCredential::Credential(typing::ConsumerCredential {
                modified_index: 1,
                id: "bob/credentials/key2".to_string(),
                name: "key2".to_string(),
                desc: None,
                labels: None,
                plugins: None,
            }),
        ]);

        filter_orphan_credentials(&mut config);

        let remaining: Vec<&str> = config.consumers.as_ref().unwrap().iter().map(ConsumerOrCredential::identity).collect();
        assert_eq!(remaining, vec!["alice", "alice/credentials/key1"]);
    }

    #[test]
    fn bump_conf_versions_only_touches_resource_types_that_actually_changed() {
        let mut config = empty_config();
        let mut increase_version = HashSet::new();
        increase_version.insert(ResourceType::Route);

        bump_conf_versions(&mut config, &increase_version, 555);

        assert_eq!(config.routes_conf_version, Some(555));
        assert_eq!(config.services_conf_version, None);
    }

    /// Applying an (event batch, base config) pair is a pure computation —
    /// each call clones its own `new_config` from a shared base and never
    /// touches any state outside its own locals. Running many of these
    /// concurrently, each producing its own independently-verified result,
    /// is a smoke test that nothing here secretly relies on being called
    /// from a single thread (no hidden shared mutable state, no data races
    /// under Miri/TSan-style concurrent access) — a real multi-threaded
    /// runtime, not `current_thread`, so tasks genuinely run in parallel.
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn applying_independent_event_batches_concurrently_is_race_free() {
        let base = empty_config();

        let mut tasks = JoinSet::new();
        for i in 0..200i64 {
            let mut config = base.clone();
            tasks.spawn(async move {
                let mut increase_version = HashSet::new();
                let mut route_event = event(
                    ResourceType::Route,
                    EventKind::Create { new_value: json!({ "name": format!("r{i}"), "uris": ["/x"] }) },
                    &format!("r{i}"),
                );
                route_event.parent_id = Some(format!("svc-{i}"));
                apply_event(&mut config, &mut increase_version, i, &route_event).unwrap();

                let routes = config.routes.expect("route was just created");
                assert_eq!(routes.len(), 1);
                assert_eq!(routes[0].id, format!("r{i}"));
                assert_eq!(routes[0].modified_index, i);
            });
        }
        let results = tasks.join_all().await;
        assert_eq!(results.len(), 200);
    }
}
