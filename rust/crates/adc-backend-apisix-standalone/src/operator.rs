//! Applying a differ's `Event`s to a standalone cluster: `sync`.
//!
//! Unlike `adc-backend-apisix`/`adc-backend-api7` (one HTTP request per
//! event), standalone has no per-resource admin API at all. Every sync:
//!
//! 1. Reconstructs the whole desired `Configuration` (`adc_differ::apply`,
//!    folding `events` onto the cached baseline this instance's last `dump`
//!    returned — see `crate::backend::Backend::dump`'s doc comment).
//! 2. Builds the wire document directly off it
//!    (`crate::transformer::transform_to_wire`) — no folding onto a prior
//!    wire document the way this crate used to.
//! 3. Stamps every resource's `modifiedIndex` and every collection's
//!    `*_conf_version` off what `events` says changed (`ChangeSet`,
//!    `stamp_versions`) — carrying over the value the collection's matching
//!    entry had last time otherwise.
//! 4. Writes the result to every server with a single
//!    `PUT /apisix/admin/configs`.
//!
//! So there's one `BackendSyncResult` per *server*, not per event (`event`
//! on each result is always `None` — no single event owns that write), and
//! `BackendSyncOptions::concurrent` (which bounds how many *events* run at
//! once in the other two backends) has nothing to bound here; the only
//! fan-out is across servers, and that's always unbounded — small enough
//! server counts in practice that there's no need to cap it.

use std::collections::{HashMap, HashSet};

use adc_backend_core::{HttpClient, Method, concurrent_map, concurrent_map_until_err, missing_parent};
use adc_sdk::resources::{Configuration, FlatConfiguration};
use adc_sdk::{
    BackendError, BackendSyncOptions, BackendSyncResult, DEFAULT_EXIT_ON_FAILURE, Event, EventType, PathSegment,
    ResourceType, ValueDiff,
};
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::backend::StandaloneServer;
use crate::transformer::transform_to_wire;
use crate::typing::ApisixStandalone;
use crate::utils::stable_timestamp;

const CONFIG_ENDPOINT: &str = "/apisix/admin/configs";
const HEADER_DIGEST: &str = "x-digest";

/// What a successful `Operator::sync` learned, for the caller (which owns
/// the cache lock this ran under) to write back: `desired` as the next
/// sync's reconstruction baseline (`crate::cache::CachedEntry::config`),
/// `wire` as the source for the next sync's version-stamping baseline
/// (`crate::cache::CachedEntry::versions`, via `WireVersions::from_wire`).
pub struct SyncedState {
    pub timestamp: i64,
    pub desired: Configuration,
    pub wire: ApisixStandalone,
}

/// `new_state` is `None` when no server accepted the write — nothing to
/// cache in that case.
pub struct SyncOutcome {
    pub results: Vec<BackendSyncResult>,
    pub new_state: Option<SyncedState>,
}

/// The per-resource `modifiedIndex` and per-collection `*_conf_version`
/// values a wire document last carried — everything `stamp_versions` needs
/// to carry a value over for a resource `events` doesn't mention, without
/// holding onto the wire document itself.
#[derive(Debug, Clone, Default)]
pub struct WireVersions {
    resources: HashMap<(ResourceType, String), i64>,
    types: HashMap<ResourceType, i64>,
}

impl WireVersions {
    pub fn from_wire(wire: &ApisixStandalone) -> Self {
        let mut versions = Self::default();
        versions.collect(&wire.routes, ResourceType::Route, |r| r.id.as_str(), |r| r.modified_index);
        versions.collect(&wire.stream_routes, ResourceType::StreamRoute, |r| r.id.as_str(), |r| r.modified_index);
        versions.collect(&wire.services, ResourceType::Service, |s| s.id.as_str(), |s| s.modified_index);
        versions.collect(&wire.upstreams, ResourceType::Upstream, |u| u.id.as_str(), |u| u.modified_index);
        versions.collect(&wire.consumers, ResourceType::Consumer, |c| c.identity(), |c| c.modified_index());
        versions.collect(&wire.ssls, ResourceType::Ssl, |s| s.id.as_str(), |s| s.modified_index);
        versions.collect(&wire.global_rules, ResourceType::GlobalRule, |g| g.id.as_str(), |g| g.modified_index);
        versions.collect(&wire.plugin_metadata, ResourceType::PluginMetadata, |p| p.id.as_str(), |p| p.modified_index);

        for (resource_type, conf_version) in [
            (ResourceType::Route, wire.routes_conf_version),
            (ResourceType::StreamRoute, wire.stream_routes_conf_version),
            (ResourceType::Service, wire.services_conf_version),
            (ResourceType::Upstream, wire.upstreams_conf_version),
            (ResourceType::Consumer, wire.consumers_conf_version),
            (ResourceType::Ssl, wire.ssls_conf_version),
            (ResourceType::GlobalRule, wire.global_rules_conf_version),
            (ResourceType::PluginMetadata, wire.plugin_metadata_conf_version),
        ] {
            versions.types.insert(resource_type, conf_version);
        }

        versions
    }

    fn collect<T>(&mut self, items: &[T], resource_type: ResourceType, identity: impl Fn(&T) -> &str, modified_index: impl Fn(&T) -> i64) {
        for item in items {
            self.resources.insert((resource_type, identity(item).to_string()), modified_index(item));
        }
    }

    /// There's no single document-wide version, just one counter per
    /// resource collection — the highest one is what a subsequent sync
    /// must not regress below (see `resolve_sync_timestamp`'s clock-rollback
    /// guard).
    pub fn highest_conf_version(&self) -> i64 {
        self.types.values().copied().max().unwrap_or(0)
    }
}

pub struct Operator {
    servers: Vec<StandaloneServer>,
    prior_desired: Configuration,
    old_versions: WireVersions,
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
    pub fn new(
        servers: Vec<StandaloneServer>,
        prior_desired: Configuration,
        old_versions: WireVersions,
        latest_known_version: Option<i64>,
    ) -> Self {
        Self { servers, prior_desired, old_versions, latest_known_version }
    }

    pub async fn sync(&self, events: Vec<Event>, opts: BackendSyncOptions) -> Result<SyncOutcome, BackendError> {
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

        // `adc_differ::apply` operates on `FlatConfiguration` (the shape
        // `DifferV4::diff` itself diffs on) — converted back to
        // `Configuration` immediately after; see `From<FlatConfiguration>
        // for Configuration`'s doc comment for why that round-trip is safe
        // here specifically.
        let flat_prior = FlatConfiguration::from(self.prior_desired.clone());
        let desired = Configuration::from(adc_differ::apply(&events, &flat_prior));
        let mut wire = transform_to_wire(&desired);
        let changes = ChangeSet::from_events(&events)?;
        stamp_versions(&mut wire, &self.old_versions, &changes, timestamp);

        let body = serde_json::to_string(&wire)
            .map_err(|e| BackendError::Serialization(format!("encoding sync config: {e}")))?;
        let digest = sha1_hex(body.as_bytes());

        let put = |server: StandaloneServer| {
            let body = body.clone();
            let digest = digest.clone();
            async move {
                match put_one(&server.client, body, digest).await {
                    Ok(()) => Ok(BackendSyncResult { success: true, event: None, error: None, server: Some(server.server) }),
                    Err(error) => {
                        if let Some(message) = conf_version_rejection_message(&error) {
                            tracing::error!(
                                "conf_version rejected by {}: (\"{message}\") — another writer is likely active on this cluster.",
                                server.server
                            );
                        }
                        Err((server.server, error))
                    }
                }
            }
        };

        let exit_on_failure = opts.exit_on_failure.unwrap_or(DEFAULT_EXIT_ON_FAILURE);
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
                    // the probe to discover the cluster's real state
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
        let new_state = results.iter().any(|result| result.success).then(|| SyncedState { timestamp, desired, wire });

        Ok(SyncOutcome { results, new_state })
    }
}

async fn put_one(client: &HttpClient, body: String, digest: String) -> Result<(), BackendError> {
    let request = client.request(Method::PUT, CONFIG_ENDPOINT)?.header(HEADER_DIGEST, digest).body(body);
    client.send(request).await?;
    Ok(())
}

/// Whether `error` is APISIX rejecting a PUT for carrying a stale
/// `*_conf_version` — observed verbatim from a real 3.17.0 instance:
/// `{"error_msg":"services_conf_version must be greater than or equal to
/// (<N>)"}`, i.e. a `400` whose message names one of the eight
/// `*_conf_version` fields. Means someone else's write landed on this
/// cluster after the baseline this sync computed its own versions from —
/// this crate's own cache regressing (a bug) would look identical from
/// here, since the two aren't distinguishable from the response alone.
/// Returns the message, for the caller to log — not a bool, so the ERROR
/// log below doesn't need its own second, separate extraction.
fn conf_version_rejection_message(error: &BackendError) -> Option<&str> {
    match error {
        BackendError::Api { status: 400, message } if message.contains("conf_version") => Some(message),
        _ => None,
    }
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

/// Which resources/collections `events` says changed this sync — the sole
/// input `stamp_versions` stamps off; it never inspects `wire`/`old`'s own
/// content to decide what changed.
struct ChangeSet {
    /// `(collection, wire id)` pairs `events` touched — a real event's own
    /// id (`generate_id_from_event`), plus, for `Upstream`, a service's own
    /// id whenever its inline default upstream changed (that synthesized
    /// wire entry's id — see `crate::transformer::transform_to_wire`).
    /// Keyed by collection, not just id: a service and its own synthesized
    /// inline-upstream entry share the *same* id string, so a bare
    /// `HashSet<String>` would make a Service event look like it also
    /// changed the (unrelated) Upstream entry with a matching id, or vice
    /// versa.
    ids: HashSet<(ResourceType, String)>,
    /// Resource types with at least one changed member this sync.
    /// `ConsumerCredential` folds into `Consumer` (they share one wire
    /// collection and `conf_version`). A `Service` `Update` whose diff is
    /// entirely about `upstream` does *not* count as a `Service` change —
    /// the service's own wire fields (`upstream_id` always points at its
    /// own id, whether or not it actually has a default upstream) are
    /// untouched by an upstream-only edit.
    types: HashSet<ResourceType>,
}

impl ChangeSet {
    fn mark(&mut self, resource_type: ResourceType, id: String) {
        self.ids.insert((resource_type, id));
        self.types.insert(resource_type);
    }

    fn from_events(events: &[Event]) -> Result<Self, BackendError> {
        let mut changes = ChangeSet { ids: HashSet::new(), types: HashSet::new() };

        for event in events {
            match event.resource_type {
                ResourceType::Service => {
                    let upstream_only_update = event.event_type() == EventType::Update
                        && event.kind.diff().is_some_and(|diff| diff.iter().all(diff_path_is_upstream));
                    if !upstream_only_update {
                        changes.mark(ResourceType::Service, generate_id_from_event(event)?);
                    }

                    // Independent of the service's own body: does this
                    // event touch its inline default upstream at all?
                    if service_upstream_changed(event) {
                        changes.mark(ResourceType::Upstream, event.resource_id.clone());
                    }
                }
                ResourceType::ConsumerCredential => {
                    changes.mark(ResourceType::Consumer, generate_id_from_event(event)?);
                }
                ResourceType::ConsumerGroup | ResourceType::InternalStreamService => {}
                other => {
                    changes.mark(other, generate_id_from_event(event)?);
                }
            }
        }

        Ok(changes)
    }
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

/// Whether a `Service` event touches its inline default upstream: for a
/// `Create`/`Delete`, whether the service actually has (had) one at all;
/// for an `Update`, whether the diff touches it (alone or alongside other
/// fields — unlike `upstream_only_update` above, this doesn't care which).
fn service_upstream_changed(event: &Event) -> bool {
    match event.event_type() {
        EventType::Create => has_upstream(event.kind.new_value()),
        EventType::Delete => has_upstream(event.kind.old_value()),
        EventType::Update => event.kind.diff().is_some_and(|diff| diff.iter().any(diff_path_is_upstream)),
    }
}

fn has_upstream(value: Option<&Value>) -> bool {
    value.and_then(|v| v.get("upstream")).is_some_and(|upstream| !upstream.is_null())
}

/// Fills in every resource's `modifiedIndex` and every collection's
/// `*_conf_version` on `wire` — freshly built by `transform_to_wire`, with
/// every one of those fields still at its zero placeholder — off `changes`.
/// A resource whose `(collection, id)` is in `changes.ids` gets `timestamp`;
/// every other resource carries over whatever `modifiedIndex` `old` last
/// recorded for it, or `timestamp` if `old` has nothing for it either
/// (shouldn't happen in steady state, but safer than leaving a `0`). A
/// collection's own `*_conf_version` is `timestamp` whenever `changes.types`
/// contains its resource type, otherwise `old`'s value carries over
/// unchanged — which is also what makes a genuinely no-op sync (no events)
/// produce a wire document with every field identical to last time's, so
/// its `X-Digest` matches and APISIX 204s instead of reprocessing.
fn stamp_versions(wire: &mut ApisixStandalone, old: &WireVersions, changes: &ChangeSet, timestamp: i64) {
    stamp_collection(&mut wire.routes, old, changes, ResourceType::Route, timestamp, |r| r.id.as_str(), |r| &mut r.modified_index);
    wire.routes_conf_version = conf_version(changes, old, ResourceType::Route, timestamp);

    stamp_collection(
        &mut wire.stream_routes,
        old,
        changes,
        ResourceType::StreamRoute,
        timestamp,
        |r| r.id.as_str(),
        |r| &mut r.modified_index,
    );
    wire.stream_routes_conf_version = conf_version(changes, old, ResourceType::StreamRoute, timestamp);

    stamp_collection(&mut wire.services, old, changes, ResourceType::Service, timestamp, |s| s.id.as_str(), |s| &mut s.modified_index);
    wire.services_conf_version = conf_version(changes, old, ResourceType::Service, timestamp);

    stamp_collection(&mut wire.upstreams, old, changes, ResourceType::Upstream, timestamp, |u| u.id.as_str(), |u| &mut u.modified_index);
    wire.upstreams_conf_version = conf_version(changes, old, ResourceType::Upstream, timestamp);

    stamp_collection(&mut wire.consumers, old, changes, ResourceType::Consumer, timestamp, |c| c.identity(), |c| c.modified_index_mut());
    wire.consumers_conf_version = conf_version(changes, old, ResourceType::Consumer, timestamp);

    stamp_collection(&mut wire.ssls, old, changes, ResourceType::Ssl, timestamp, |s| s.id.as_str(), |s| &mut s.modified_index);
    wire.ssls_conf_version = conf_version(changes, old, ResourceType::Ssl, timestamp);

    stamp_collection(
        &mut wire.global_rules,
        old,
        changes,
        ResourceType::GlobalRule,
        timestamp,
        |g| g.id.as_str(),
        |g| &mut g.modified_index,
    );
    wire.global_rules_conf_version = conf_version(changes, old, ResourceType::GlobalRule, timestamp);

    stamp_collection(
        &mut wire.plugin_metadata,
        old,
        changes,
        ResourceType::PluginMetadata,
        timestamp,
        |p| p.id.as_str(),
        |p| &mut p.modified_index,
    );
    wire.plugin_metadata_conf_version = conf_version(changes, old, ResourceType::PluginMetadata, timestamp);
}

fn conf_version(changes: &ChangeSet, old: &WireVersions, resource_type: ResourceType, timestamp: i64) -> i64 {
    if changes.types.contains(&resource_type) {
        timestamp
    } else {
        old.types.get(&resource_type).copied().unwrap_or(timestamp)
    }
}

/// One collection's worth of `stamp_versions` — see that function's doc
/// comment for the semantics. `resource_type` scopes both the
/// `changes.ids` and `old.resources` lookups — see `ChangeSet::ids`'s doc
/// comment for why a bare id isn't enough (a service and its synthesized
/// inline-upstream entry share one).
fn stamp_collection<T>(
    new_items: &mut [T],
    old: &WireVersions,
    changes: &ChangeSet,
    resource_type: ResourceType,
    timestamp: i64,
    identity: impl Fn(&T) -> &str,
    modified_index_mut: impl Fn(&mut T) -> &mut i64,
) {
    for item in new_items.iter_mut() {
        let id = identity(item).to_string();
        let stamped = if changes.ids.contains(&(resource_type, id.clone())) {
            timestamp
        } else {
            old.resources.get(&(resource_type, id)).copied().unwrap_or(timestamp)
        };
        *modified_index_mut(item) = stamped;
    }
}

#[cfg(test)]
mod tests {
    use adc_sdk::EventKind;
    use serde_json::json;

    use super::*;
    use crate::typing::{self, ConsumerOrCredential};

    #[test]
    fn a_400_naming_a_conf_version_field_is_recognized() {
        // Verbatim from a real 3.17.0 instance (seeded with an inflated
        // `services_conf_version` via raw PUT, then PUT a normal one back).
        let error = BackendError::Api {
            status: 400,
            message: "services_conf_version must be greater than or equal to (99999999999999)".to_string(),
        };
        assert_eq!(
            conf_version_rejection_message(&error),
            Some("services_conf_version must be greater than or equal to (99999999999999)")
        );
    }

    #[test]
    fn a_400_not_naming_a_conf_version_field_is_not_a_conf_version_rejection() {
        let error = BackendError::Api { status: 400, message: "missing digest header".to_string() };
        assert_eq!(conf_version_rejection_message(&error), None);
    }

    #[test]
    fn a_non_400_status_is_never_a_conf_version_rejection_even_if_it_mentions_one() {
        let error = BackendError::Api { status: 500, message: "services_conf_version must be greater than or equal to (1)".to_string() };
        assert_eq!(conf_version_rejection_message(&error), None);
    }

    #[test]
    fn a_non_api_error_is_never_a_conf_version_rejection() {
        assert_eq!(conf_version_rejection_message(&BackendError::Transport("connection refused".to_string())), None);
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

    fn event(rt: ResourceType, kind: EventKind, id: &str) -> Event {
        Event::new(rt, kind, id, id)
    }

    fn changes(ids: &[(ResourceType, &str)], types: &[ResourceType]) -> ChangeSet {
        ChangeSet {
            ids: ids.iter().map(|(rt, id)| (*rt, id.to_string())).collect(),
            types: types.iter().copied().collect(),
        }
    }

    fn route(id: &str, service_id: &str) -> typing::Route {
        typing::Route {
            modified_index: 0,
            id: id.to_string(),
            name: id.to_string(),
            desc: None,
            labels: None,
            uris: vec!["/x".to_string()],
            hosts: None,
            methods: None,
            remote_addrs: None,
            vars: None,
            filter_func: None,
            plugins: None,
            service_id: service_id.to_string(),
            timeout: None,
            enable_websocket: None,
            priority: None,
            status: Some(1),
        }
    }

    fn service(id: &str) -> typing::Service {
        typing::Service {
            modified_index: 0,
            id: id.to_string(),
            name: id.to_string(),
            desc: None,
            labels: None,
            hosts: None,
            upstream_id: Some(id.to_string()),
            plugins: None,
        }
    }

    fn upstream(id: &str) -> typing::Upstream {
        typing::Upstream { modified_index: 0, id: id.to_string(), name: id.to_string(), ..Default::default() }
    }

    // --- stamp_versions ---

    #[test]
    fn a_changed_route_is_stamped_fresh_and_bumps_its_conf_version() {
        let mut wire = ApisixStandalone { routes: vec![route("r1", "svc-1")], ..Default::default() };
        let old = ApisixStandalone::default();

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[(ResourceType::Route, "r1")], &[ResourceType::Route]), 100);

        assert_eq!(wire.routes[0].modified_index, 100);
        assert_eq!(wire.routes_conf_version, 100);
    }

    #[test]
    fn an_unlisted_route_carries_over_its_old_modified_index_and_conf_version_stays_put() {
        let mut old_route = route("r1", "svc-1");
        old_route.modified_index = 42;
        let old = ApisixStandalone { routes: vec![old_route], routes_conf_version: 42, ..Default::default() };
        let mut wire = ApisixStandalone { routes: vec![route("r1", "svc-1")], ..Default::default() };

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[], &[]), 100);

        assert_eq!(wire.routes[0].modified_index, 42, "an id not in the ChangeSet must carry over the old modifiedIndex");
        assert_eq!(wire.routes_conf_version, 42, "conf_version must not bump when its type isn't in the ChangeSet");
    }

    /// A resource `changes` names but `old` has no matching entry for —
    /// shouldn't happen in steady state, but falls back to a fresh
    /// timestamp rather than leaving a `0`.
    #[test]
    fn a_changed_route_with_no_matching_old_entry_falls_back_to_a_fresh_timestamp() {
        let old = ApisixStandalone::default();
        let mut wire = ApisixStandalone { routes: vec![route("r1", "svc-1")], ..Default::default() };

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[(ResourceType::Route, "r1")], &[ResourceType::Route]), 100);

        assert_eq!(wire.routes[0].modified_index, 100);
    }

    /// A service update that only touches its inline default upstream must
    /// bump `upstreams_conf_version` but leave `services_conf_version`
    /// alone — this is `ChangeSet::from_events`'s job to decide (see below);
    /// `stamp_versions` itself just stamps off whatever `ChangeSet` it's given.
    #[test]
    fn only_the_types_named_in_the_changeset_get_a_fresh_conf_version() {
        let old = ApisixStandalone {
            services: vec![typing::Service { modified_index: 1, ..service("svc-1") }],
            services_conf_version: 1,
            upstreams: vec![upstream("svc-1")],
            upstreams_conf_version: 1,
            ..Default::default()
        };
        let mut wire = ApisixStandalone { services: vec![service("svc-1")], upstreams: vec![upstream("svc-1")], ..Default::default() };

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[(ResourceType::Upstream, "svc-1")], &[ResourceType::Upstream]), 100);

        assert_eq!(wire.services[0].modified_index, 1, "Service wasn't in the ChangeSet's types");
        assert_eq!(wire.services_conf_version, 1);
        assert_eq!(wire.upstreams[0].modified_index, 100);
        assert_eq!(wire.upstreams_conf_version, 100);
    }

    #[test]
    fn resyncing_with_an_empty_changeset_produces_a_byte_identical_document() {
        let old_route = {
            let mut r = route("r1", "svc-1");
            r.modified_index = 7;
            r
        };
        let old = ApisixStandalone { routes: vec![old_route], routes_conf_version: 7, ..Default::default() };
        let mut wire = ApisixStandalone { routes: vec![route("r1", "svc-1")], ..Default::default() };

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[], &[]), 999);

        assert_eq!(wire, old, "an empty ChangeSet must reproduce the exact same document APISIX already has");
    }

    /// Mixed-variant collection (`Vec<ConsumerOrCredential>`): a credential
    /// changing must not disturb its sibling consumer's `modifiedIndex`.
    #[test]
    fn an_unlisted_consumer_keeps_its_modified_index_when_only_its_credential_is_in_the_changeset() {
        let consumer = ConsumerOrCredential::Consumer(typing::Consumer {
            modified_index: 5,
            username: "alice".to_string(),
            desc: None,
            labels: None,
            plugins: None,
        });
        let old_credential = ConsumerOrCredential::Credential(typing::ConsumerCredential {
            modified_index: 5,
            id: "alice/credentials/key1".to_string(),
            name: "key1".to_string(),
            desc: None,
            labels: None,
            plugins: None,
        });
        let old = ApisixStandalone { consumers: vec![consumer.clone(), old_credential], consumers_conf_version: 5, ..Default::default() };
        let new_credential = ConsumerOrCredential::Credential(typing::ConsumerCredential {
            modified_index: 0,
            id: "alice/credentials/key1".to_string(),
            name: "key1".to_string(),
            desc: None,
            labels: None,
            plugins: None,
        });
        let mut wire = ApisixStandalone {
            consumers: vec![
                ConsumerOrCredential::Consumer(typing::Consumer { modified_index: 0, ..consumer.as_consumer().unwrap().clone() }),
                new_credential,
            ],
            ..Default::default()
        };

        stamp_versions(&mut wire, &WireVersions::from_wire(&old), &changes(&[(ResourceType::Consumer, "alice/credentials/key1")], &[ResourceType::Consumer]), 100);

        let consumers = wire.consumers;
        assert_eq!(consumers[0].as_consumer().unwrap().modified_index, 5, "alice's own id isn't in the ChangeSet");
        assert_eq!(consumers[1].as_credential().unwrap().modified_index, 100);
        assert_eq!(wire.consumers_conf_version, 100);
    }

    // --- ChangeSet::from_events ---

    #[test]
    fn a_route_create_event_lands_in_the_changeset() {
        let mut route_event = event(ResourceType::Route, EventKind::Create { new_value: json!({ "name": "r1", "uris": ["/x"] }) }, "r1");
        route_event.parent_id = Some("svc-1".to_string());

        let changes = ChangeSet::from_events(&[route_event]).unwrap();

        assert!(changes.ids.contains(&(ResourceType::Route, "r1".to_string())));
        assert!(changes.types.contains(&ResourceType::Route));
    }

    #[test]
    fn a_service_create_event_lands_in_the_changeset() {
        let service_event = event(ResourceType::Service, EventKind::Create { new_value: json!({ "name": "svc-1" }) }, "svc-1");

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(changes.ids.contains(&(ResourceType::Service, "svc-1".to_string())));
        assert!(changes.types.contains(&ResourceType::Service));
    }

    /// A `Service` `Update` whose diff is entirely about `upstream` must not
    /// register a `Service` change — only an `Upstream` one, via the
    /// service's own id.
    #[test]
    fn a_service_update_that_only_touches_upstream_registers_only_as_an_upstream_change() {
        let diff = vec![ValueDiff::Edit { path: vec![PathSegment::Key("upstream".to_string())], lhs: json!({}), rhs: json!({}) }];
        let service_event = event(
            ResourceType::Service,
            EventKind::Update {
                old_value: json!({ "name": "svc-1" }),
                new_value: json!({ "name": "svc-1", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }),
                diff: Some(diff),
            },
            "svc-1",
        );

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(!changes.types.contains(&ResourceType::Service), "service body itself must not register as changed");
        assert!(changes.types.contains(&ResourceType::Upstream));
        assert!(
            changes.ids.contains(&(ResourceType::Upstream, "svc-1".to_string())),
            "the synthesized inline-upstream id is the service's own id"
        );
    }

    /// A `Service` `Update` that touches *both* `upstream` and another
    /// field must register as a real `Service` change too.
    #[test]
    fn a_service_update_that_touches_upstream_and_another_field_registers_as_both() {
        let diff = vec![
            ValueDiff::Edit { path: vec![PathSegment::Key("upstream".to_string())], lhs: json!({}), rhs: json!({}) },
            ValueDiff::Edit { path: vec![PathSegment::Key("description".to_string())], lhs: json!("a"), rhs: json!("b") },
        ];
        let service_event = event(
            ResourceType::Service,
            EventKind::Update { old_value: json!({ "name": "svc-1" }), new_value: json!({ "name": "svc-1" }), diff: Some(diff) },
            "svc-1",
        );

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(changes.types.contains(&ResourceType::Service));
        assert!(changes.types.contains(&ResourceType::Upstream));
    }

    #[test]
    fn a_service_create_with_no_default_upstream_does_not_register_an_upstream_change() {
        let service_event = event(ResourceType::Service, EventKind::Create { new_value: json!({ "name": "svc-2" }) }, "svc-2");

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(!changes.types.contains(&ResourceType::Upstream));
    }

    #[test]
    fn a_service_create_with_a_default_upstream_registers_an_upstream_change() {
        let service_event = event(
            ResourceType::Service,
            EventKind::Create { new_value: json!({ "name": "svc-1", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }) },
            "svc-1",
        );

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(changes.types.contains(&ResourceType::Upstream));
        assert!(changes.ids.contains(&(ResourceType::Upstream, "svc-1".to_string())));
    }

    #[test]
    fn a_service_delete_of_a_service_that_had_a_default_upstream_registers_an_upstream_change() {
        let service_event = event(
            ResourceType::Service,
            EventKind::Delete { old_value: json!({ "name": "svc-1", "upstream": { "nodes": [{"host":"1.1.1.1","port":80,"weight":1}] } }) },
            "svc-1",
        );

        let changes = ChangeSet::from_events(&[service_event]).unwrap();

        assert!(changes.types.contains(&ResourceType::Upstream));
    }

    #[test]
    fn a_consumer_credential_event_maps_to_the_composite_id_and_the_consumer_type() {
        let mut credential_event =
            event(ResourceType::ConsumerCredential, EventKind::Delete { old_value: json!({}) }, "key1");
        credential_event.parent_id = Some("alice".to_string());

        let changes = ChangeSet::from_events(&[credential_event]).unwrap();

        assert!(changes.ids.contains(&(ResourceType::Consumer, "alice/credentials/key1".to_string())));
        assert!(changes.types.contains(&ResourceType::Consumer));
        assert!(!changes.types.contains(&ResourceType::ConsumerCredential), "credentials share Consumer's conf_version, not their own");
    }

    #[test]
    fn a_consumer_credential_event_with_no_parent_id_is_an_error() {
        let credential_event = event(ResourceType::ConsumerCredential, EventKind::Delete { old_value: json!({}) }, "key1");

        assert!(ChangeSet::from_events(&[credential_event]).is_err());
    }

    /// Property tests: instead of a handful of hand-picked scenarios, these
    /// generate hundreds of random combinations and check the same
    /// invariants hold for all of them — specifically the "changing X never
    /// moves Y" guarantee `stamp_versions`/`ChangeSet` exist to provide,
    /// which the hand-picked unit tests above can only sample a few points
    /// of.
    mod proptests {
        use std::collections::HashMap;

        use adc_sdk::PathSegment;
        use proptest::prelude::*;

        use super::*;

        fn id_strategy() -> impl Strategy<Value = String> {
            "[a-z]{1,4}"
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1024))]

            /// For any set of resources, any prior per-resource version
            /// table, any subset of them marked changed, and any timestamp:
            /// a marked resource's `modifiedIndex` always becomes exactly
            /// `timestamp`, and an unmarked one always carries over exactly
            /// what `old` recorded for it (or `timestamp` if `old` had
            /// nothing) — never anything else, never a value bleeding over
            /// from a different resource.
            #[test]
            fn stamp_collection_only_bumps_ids_the_changeset_names(
                ids in prop::collection::hash_set(id_strategy(), 1..8),
                changed_ids in prop::collection::hash_set(id_strategy(), 0..8),
                old_indices in prop::collection::hash_map(id_strategy(), 1i64..1_000_000, 0..8),
                timestamp in 1_000_000i64..2_000_000,
            ) {
                let resource_type = ResourceType::Route;
                let old = WireVersions {
                    resources: old_indices.iter().map(|(id, index)| ((resource_type, id.clone()), *index)).collect(),
                    types: HashMap::new(),
                };
                let changes = ChangeSet {
                    ids: changed_ids.iter().map(|id| (resource_type, id.clone())).collect(),
                    types: HashSet::new(),
                };
                let mut items: Vec<typing::Route> = ids.iter().map(|id| route(id, "svc")).collect();

                stamp_collection(&mut items, &old, &changes, resource_type, timestamp, |r| r.id.as_str(), |r| &mut r.modified_index);

                for item in &items {
                    if changed_ids.contains(&item.id) {
                        prop_assert_eq!(item.modified_index, timestamp, "a changed id must get the fresh timestamp");
                    } else {
                        let expected = old_indices.get(&item.id).copied().unwrap_or(timestamp);
                        prop_assert_eq!(item.modified_index, expected, "an unlisted id must carry over its old value exactly");
                    }
                }
            }

            /// Same guarantee one level up: a collection's `conf_version`
            /// moves to `timestamp` exactly when its type is in the
            /// changeset, and otherwise carries over `old`'s value exactly
            /// (or `timestamp` if `old` had none).
            #[test]
            fn conf_version_only_bumps_a_type_the_changeset_names(
                old_version in prop::option::of(1i64..1_000_000),
                is_changed in any::<bool>(),
                timestamp in 1_000_000i64..2_000_000,
            ) {
                let resource_type = ResourceType::Route;
                let mut old = WireVersions::default();
                if let Some(v) = old_version {
                    old.types.insert(resource_type, v);
                }
                let mut changes = ChangeSet { ids: HashSet::new(), types: HashSet::new() };
                if is_changed {
                    changes.types.insert(resource_type);
                }

                let result = conf_version(&changes, &old, resource_type, timestamp);

                if is_changed {
                    prop_assert_eq!(result, timestamp);
                } else {
                    prop_assert_eq!(result, old_version.unwrap_or(timestamp));
                }
            }

            /// The exact spec `ChangeSet::from_events` implements for a
            /// `Service` `Update`: `Service` registers as changed unless the
            /// diff is *entirely* about `upstream` (an empty diff counts as
            /// "entirely upstream" vacuously — matches `Iterator::all` on an
            /// empty diff, though a real differ never emits an event with
            /// one); `Upstream` registers independently, whenever *any*
            /// diff entry touches it. Generated over random diffs (random
            /// length, random mix of upstream/non-upstream paths) instead
            /// of the handful of hand-built ones above.
            #[test]
            fn service_update_marks_service_and_upstream_independently(
                path_is_upstream in prop::collection::vec(any::<bool>(), 0..6),
            ) {
                let diff: Vec<ValueDiff> = path_is_upstream
                    .iter()
                    .enumerate()
                    .map(|(i, &is_upstream)| {
                        let key = if is_upstream { "upstream".to_string() } else { format!("field{i}") };
                        ValueDiff::Edit { path: vec![PathSegment::Key(key)], lhs: json!(null), rhs: json!(null) }
                    })
                    .collect();
                let expect_service = !diff.is_empty() && path_is_upstream.iter().any(|&is_upstream| !is_upstream);
                let expect_upstream = path_is_upstream.iter().any(|&is_upstream| is_upstream);

                let service_event = Event {
                    resource_type: ResourceType::Service,
                    kind: EventKind::Update { old_value: json!({ "name": "svc-1" }), new_value: json!({ "name": "svc-1" }), diff: Some(diff) },
                    resource_id: "svc-1".to_string(),
                    resource_name: "svc-1".to_string(),
                    parent_id: None,
                };

                let changes = ChangeSet::from_events(&[service_event]).unwrap();

                prop_assert_eq!(changes.types.contains(&ResourceType::Service), expect_service);
                prop_assert_eq!(changes.types.contains(&ResourceType::Upstream), expect_upstream);
                prop_assert_eq!(changes.ids.contains(&(ResourceType::Upstream, "svc-1".to_string())), expect_upstream);
            }

            /// Same independence for `Create`/`Delete`: `Service` always
            /// registers (there's no "only upstream changed" concept for a
            /// resource that didn't exist a moment ago either way);
            /// `Upstream` registers iff the service actually has (had) a
            /// non-null `upstream` key.
            #[test]
            fn service_create_and_delete_mark_upstream_only_when_present(
                has_upstream_key in any::<bool>(),
                upstream_is_null in any::<bool>(),
                is_create in any::<bool>(),
            ) {
                let value = if has_upstream_key {
                    json!({ "name": "svc-1", "upstream": if upstream_is_null { Value::Null } else { json!({ "nodes": [] }) } })
                } else {
                    json!({ "name": "svc-1" })
                };
                let expect_upstream = has_upstream_key && !upstream_is_null;

                let kind = if is_create {
                    EventKind::Create { new_value: value }
                } else {
                    EventKind::Delete { old_value: value }
                };
                let service_event = Event {
                    resource_type: ResourceType::Service,
                    kind,
                    resource_id: "svc-1".to_string(),
                    resource_name: "svc-1".to_string(),
                    parent_id: None,
                };

                let changes = ChangeSet::from_events(&[service_event]).unwrap();

                prop_assert!(changes.types.contains(&ResourceType::Service), "create/delete always registers as a Service change");
                prop_assert_eq!(changes.types.contains(&ResourceType::Upstream), expect_upstream);
            }
        }
    }
}
