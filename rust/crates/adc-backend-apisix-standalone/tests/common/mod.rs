//! Shared scaffolding for this crate's real-e2e test files: three live
//! standalone APISIX admin APIs, all sharing the same static admin key —
//! see `libs/backend-apisix-standalone/e2e/assets/docker-compose.yaml` (the
//! same fixture the TS suite uses) for how to bring them up, or
//! `.github/workflows/e2e.yaml`'s `apisix-standalone-rust` job for how CI
//! does it. Not every test file uses every item here, so dead-code warnings
//! are suppressed at the module level rather than per item.
#![allow(dead_code)]

use std::time::Duration;

use adc_backend_apisix_standalone::tests::Cache;
use adc_backend_apisix_standalone::Backend;
use adc_backend_core::{HttpClient, HttpClientConfig, Method, TlsConfig};
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::utils::generate_id;
use adc_sdk::{DefaultValue, Event, EventKind, ResourceType};
use serde_json::Value;

pub const SERVER1: &str = "http://localhost:19180";
pub const SERVER2: &str = "http://localhost:29180";
pub const SERVER3: &str = "http://localhost:39180";
pub const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

fn tls() -> TlsConfig {
    TlsConfig { skip_verify: true, ..Default::default() }
}

pub fn backend_options(servers: Vec<String>, cache_key: &str) -> adc_backend_apisix_standalone::BackendOptions {
    adc_backend_apisix_standalone::BackendOptions {
        servers,
        tokens: vec![TOKEN.to_string()],
        cache_key: cache_key.to_string(),
        bypass_cache: false,
        timeout: Some(Duration::from_secs(10)),
        tls: tls(),
    }
}

/// A backend against just `SERVER1` — matches most of the TS suite's
/// `describe` blocks, which only ever exercise a single instance.
pub fn backend(cache_key: &str) -> Backend {
    Backend::new(backend_options(vec![SERVER1.to_string()], cache_key)).unwrap()
}

/// A backend against one specific server, for scenarios that write to each
/// standalone instance independently before a later multi-server backend
/// reads them back.
pub fn backend_for(server: &str, cache_key: &str) -> Backend {
    Backend::new(backend_options(vec![server.to_string()], cache_key)).unwrap()
}

/// A backend against all three servers, mirroring the TS suite's `servers`/
/// `tokens` comma-joined constants.
pub fn backend_multi(cache_key: &str) -> Backend {
    Backend::new(backend_options(vec![SERVER1.to_string(), SERVER2.to_string(), SERVER3.to_string()], cache_key)).unwrap()
}

/// The CI matrix runs this suite against every supported APISIX release
/// (`BACKEND_APISIX_VERSION`, same env var the TS e2e suite reads) — falls
/// back to a version high enough to exercise every version-gated code path
/// when unset, for local runs against whatever's in the compose file.
pub fn apisix_version() -> semver::Version {
    match std::env::var("BACKEND_APISIX_VERSION") {
        Ok(v) => semver::Version::parse(&v).unwrap_or_else(|e| panic!("BACKEND_APISIX_VERSION={v:?} is not a valid semver: {e}")),
        Err(_) => semver::Version::new(999, 999, 999),
    }
}

/// Mirrors `support/utils.ts`'s `createEvent`'s id-generation rule: most
/// resource types hash `parent_name.resource_name` (or just `resource_name`
/// with no parent); consumers/global rules/plugin metadata use their name
/// as-is (they're addressed by it directly, not a derived hash); SSLs hash
/// their SNI list instead of a name, so they don't go through this helper
/// at all (see `create_ssl_event`).
fn resource_id_for(rt: ResourceType, resource_name: &str, parent_name: Option<&str>) -> String {
    match rt {
        ResourceType::Consumer | ResourceType::GlobalRule | ResourceType::PluginMetadata => resource_name.to_string(),
        _ => match parent_name {
            Some(parent) => generate_id(&format!("{parent}.{resource_name}")),
            None => generate_id(resource_name),
        },
    }
}

fn parent_id_for(rt: ResourceType, parent_name: Option<&str>) -> Option<String> {
    parent_name.map(|parent| if rt == ResourceType::ConsumerCredential { parent.to_string() } else { generate_id(parent) })
}

pub fn create_event(rt: ResourceType, resource_name: &str, new_value: Value, parent_name: Option<&str>) -> Event {
    let mut event = Event::new(rt, EventKind::Create { new_value }, resource_id_for(rt, resource_name, parent_name), resource_name);
    event.parent_id = parent_id_for(rt, parent_name);
    event
}

pub fn update_event(rt: ResourceType, resource_name: &str, new_value: Value, old_value: Value, parent_name: Option<&str>) -> Event {
    let mut event = Event::new(
        rt,
        EventKind::Update { old_value, new_value, diff: None },
        resource_id_for(rt, resource_name, parent_name),
        resource_name,
    );
    event.parent_id = parent_id_for(rt, parent_name);
    event
}

pub fn delete_event(rt: ResourceType, resource_name: &str, parent_name: Option<&str>) -> Event {
    let mut event = Event::new(
        rt,
        EventKind::Delete { old_value: Value::Null },
        resource_id_for(rt, resource_name, parent_name),
        resource_name,
    );
    event.parent_id = parent_id_for(rt, parent_name);
    event
}

pub fn cache() -> &'static Cache {
    Cache::global()
}

/// Wipes every standalone instance back to an empty config, the same way
/// the TS suite's own `restartAPISIX()` (a `docker compose restart` in
/// `libs/backend-apisix-standalone/e2e/assets`) does before each scenario —
/// standalone's declarative document lives entirely on the live servers,
/// with no per-test namespacing, so without this, resources left behind by
/// one test function would collide with (or be silently reused by) the
/// next one to run against the same three containers.
pub async fn restart_apisix() {
    let compose_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-apisix-standalone/e2e/assets");
    let status = tokio::process::Command::new("docker")
        .args(["compose", "restart"])
        .current_dir(&compose_dir)
        .status()
        .await
        .unwrap_or_else(|e| panic!("failed to run `docker compose restart` in {compose_dir:?}: {e}"));
    assert!(status.success(), "`docker compose restart` in {compose_dir:?} failed");

    for server in [SERVER1, SERVER2, SERVER3] {
        wait_until_ready(server).await;
    }
}

/// Polls `server`'s admin API until it answers `/apisix/admin/configs` with
/// a genuine `200` — not merely "anything but 404". A fixed post-restart
/// sleep isn't reliable across APISIX versions: real container logs from a
/// 3.13.0 CI failure show `docker compose restart` returning (Docker's own
/// "container started" signal) 1-2 full seconds *before* APISIX's own boot
/// sequence inside it — `init_etcd`, then per-worker `init_worker_by_lua`
/// loading ~80 plugins — actually finishes registering the admin routes.
/// A request landing in that window can get more than just a plain 404:
/// nginx's master process can already be accepting connections before
/// content routing is live, so a stray non-404, non-200 status (a 5xx from
/// Lua init not being ready, or similar) is possible too — checking only
/// "not 404" treated one of those as "ready" once and let a real request
/// moments later land back in the same still-initializing window. Requiring
/// a 200 specifically, twice in a row, is a tighter bar that a single lucky
/// sample during a churning startup can't satisfy by accident.
async fn wait_until_ready(server: &str) {
    let client = HttpClient::new(HttpClientConfig {
        server: server.to_string(),
        token: TOKEN.to_string(),
        timeout: Some(Duration::from_secs(2)),
        tls: tls(),
    })
    .unwrap();

    const MAX_ATTEMPTS: u32 = 60;
    const REQUIRED_CONSECUTIVE_SUCCESSES: u32 = 2;
    let mut consecutive_successes = 0;
    for attempt in 1..=MAX_ATTEMPTS {
        let got_200 = match client.request(Method::GET, "/apisix/admin/configs") {
            Ok(request) => match client.execute(request).await {
                Ok(response) => response.status().as_u16() == 200,
                Err(_) => false,
            },
            Err(_) => false,
        };
        consecutive_successes = if got_200 { consecutive_successes + 1 } else { 0 };
        if consecutive_successes >= REQUIRED_CONSECUTIVE_SUCCESSES {
            return;
        }
        if attempt == MAX_ATTEMPTS {
            panic!("{server} never became ready after `docker compose restart` ({MAX_ATTEMPTS} attempts)");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Runs the real differ (not a stand-in) between a desired `local`
/// configuration and the `remote` one a dump just returned — the same way
/// the TS suite's own `Differ.diff(config, await dumpConfiguration(backend))`
/// calls work, and `adc-backend-api7`'s e2e suite's own `common::diff`. Used
/// wherever the TS spec builds events via `Differ.diff` rather than its
/// hand-rolled `createEvent`/`updateEvent`/`deleteEvent` — an `Update`
/// event's `diff` field content matters for standalone specifically (a
/// SERVICE update only touches the `services` collection when the diff
/// shows more than just its `upstream` changed), so those scenarios need a
/// real diff, not a hand-built one with `diff: None`.
pub fn diff(local: &Configuration, remote: &Configuration) -> Vec<Event> {
    let local = adc_sdk::resources::FlatConfiguration::from(local.clone());
    let remote = adc_sdk::resources::FlatConfiguration::from(remote.clone());
    adc_differ::DifferV4::diff(&local, &remote, None::<&DefaultValue>)
}

/// Reads one `*_conf_version` field straight off `SERVER1`'s admin API —
/// bypasses this crate's own cache entirely, so it reflects what the server
/// actually has, not what we think we last wrote. `field` is the raw JSON
/// key, e.g. `"consumers_conf_version"`.
pub async fn raw_conf_version(field: &str) -> Option<i64> {
    let client = HttpClient::new(HttpClientConfig {
        server: SERVER1.to_string(),
        token: TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let request = client.request(Method::GET, "/apisix/admin/configs").unwrap();
    let body: Value = client.send_json(request).await.unwrap();
    body.get(field).and_then(Value::as_i64)
}

/// The whole standalone config document, straight off `SERVER1`'s admin
/// API — same rationale as [`raw_conf_version`]: this crate's own cache
/// only remembers per-resource version numbers (`Cache::versions`), not
/// resource content, so a test asserting on actual wire content (a node's
/// host, a label, `nodes == []`, ...) has to read it from the server
/// directly.
pub async fn raw_config() -> adc_backend_apisix_standalone::tests::typing::ApisixStandalone {
    let client = HttpClient::new(HttpClientConfig {
        server: SERVER1.to_string(),
        token: TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap();
    let request = client.request(Method::GET, "/apisix/admin/configs").unwrap();
    client.send_json(request).await.unwrap()
}

/// An `adc::Upstream` with every field at its zero value — shared starting
/// point for tests that only care about a couple of fields, via struct
/// update syntax (`..common::base_upstream()`).
pub fn base_upstream() -> adc::Upstream {
    adc::Upstream {
        id: None,
        name: None,
        description: None,
        labels: None,
        r#type: adc::UpstreamBalancer::default(),
        hash_on: None,
        key: None,
        checks: None,
        nodes: None,
        scheme: adc::UpstreamScheme::default(),
        retries: None,
        retry_timeout: None,
        timeout: None,
        tls: None,
        keepalive_pool: None,
        pass_host: adc::UpstreamPassHost::default(),
        upstream_host: None,
        service_name: None,
        discovery_type: None,
        discovery_args: None,
    }
}

/// An `adc::Service` with every field at its zero value — see
/// [`base_upstream`].
pub fn base_service() -> adc::Service {
    adc::Service {
        id: None,
        name: String::new(),
        description: None,
        labels: None,
        upstream: None,
        upstreams: None,
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: None,
    }
}

pub fn empty_configuration() -> Configuration {
    Configuration {
        services: None,
        ssls: None,
        consumers: None,
        consumer_groups: None,
        global_rules: None,
        plugin_metadata: None,
    }
}
