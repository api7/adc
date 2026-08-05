//! Shared scaffolding for this crate's real-e2e test files: a live API7
//! Enterprise dashboard. See `e2e_gateway_group.rs`'s module doc for how to
//! bring one up. Unlike APISIX's static admin key, a fresh API7 dashboard
//! needs a session login + password rotation + license activation + token
//! generation dance before any admin API call works at all — this module
//! ports that dance from the TS e2e suite's `e2e/support/global-setup.ts`,
//! so a bare `cargo test --ignored` against a freshly `docker compose up`'d
//! dashboard works standalone, without going through the TS suite first.
#![allow(dead_code)]

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::resources::Configuration;
use adc_sdk::utils::generate_id;
use adc_sdk::{
    BackendError, BackendSyncOptions, BackendSyncResult, DefaultValue, Event, EventKind,
    ResourceType,
};
use serde_json::{Value, json};
use tokio::sync::OnceCell;

const BOOTSTRAP_PASSWORD: &str = "Admin12345!@#$%";

pub fn server() -> String {
    env::var("SERVER").unwrap_or_else(|_| "https://localhost:7443".to_string())
}

pub fn gateway_group() -> String {
    env::var("GATEWAY_GROUP").unwrap_or_else(|_| "default".to_string())
}

/// The dashboard version under test, from the same env var the CI matrix
/// sets — used to skip a test scenario that only applies above/below a
/// given release, the same role `semverCondition` plays in the TS suite.
/// Unset (a local run against whatever's in the compose file) is treated
/// as "newest", so every version-gated scenario runs by default.
pub fn server_version() -> semver::Version {
    match env::var("BACKEND_API7_VERSION") {
        Ok(v) => semver::Version::parse(&v)
            .unwrap_or_else(|e| panic!("BACKEND_API7_VERSION={v:?} is not a valid semver: {e}")),
        Err(_) => semver::Version::new(999, 999, 999),
    }
}

fn unique_name(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "{prefix}-{nanos}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// A session-cookie-authenticated client for the dashboard's own
/// (non-admin-API) endpoints used only during bootstrap (`/api/login`,
/// `/api/password`, `/api/license`, `/api/invites`, `/api/users/*`,
/// `/api/tokens`) — a separate concern from the `X-API-KEY`-authenticated
/// `HttpClient` the `GatewayGroupResolver` under test actually uses.
struct DashboardSession {
    client: reqwest::Client,
    base: String,
}

impl DashboardSession {
    fn new(base: String) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(true)
            .build()
            .expect("building the dashboard session client");
        Self { client, base }
    }

    /// Ports `global-setup.ts`'s `waitForDashboard`: polls until the
    /// dashboard answers at all (any HTTP response, not a connection
    /// error), rather than assuming it's ready right after `docker compose
    /// up -d` returns.
    async fn wait_ready(&self) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        loop {
            if self
                .client
                .get(format!("{}/api/status", self.base))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .is_ok()
            {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("dashboard at {} was not ready within 120s", self.base);
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn login(&self, username: &str, password: &str) {
        let response = self
            .client
            .post(format!("{}/api/login", self.base))
            .json(&json!({ "username": username, "password": password }))
            .send()
            .await
            .unwrap_or_else(|e| panic!("POST /api/login: {e}"));
        assert!(
            response.status().is_success(),
            "login as {username:?} failed: {}",
            response.status()
        );
    }

    async fn put(&self, path: &str, body: Value) {
        let response = self
            .client
            .put(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .unwrap_or_else(|e| panic!("PUT {path}: {e}"));
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        assert!(status.is_success(), "PUT {path} failed: {status}: {text}");
    }

    async fn post(&self, path: &str, body: Value) -> Value {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .unwrap_or_else(|e| panic!("POST {path}: {e}"));
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        assert!(status.is_success(), "POST {path} failed: {status}: {text}");
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("decoding response from POST {path}: {e}: {text}"))
    }
}

async fn activate_license(session: &DashboardSession, license: &Option<String>) {
    if let Some(license) = license {
        session
            .put("/api/license", json!({ "data": license }))
            .await;
    }
}

/// Ports `global-setup.ts`'s `initUser`: log in, then rotate the password —
/// a fresh account rejects everything else (including, it turns out,
/// minting an API token) until this happens, *even* for the throwaway
/// invited user `bootstrap_token` creates below, not just the built-in
/// `admin` account. `first_time` gates license activation only (TS's
/// `fisrtTime` param); the password rotation itself always runs.
async fn init_user(
    session: &DashboardSession,
    username: &str,
    password: &str,
    first_time: bool,
    version: &semver::Version,
    license: &Option<String>,
) {
    session.login(username, password).await;
    // Mirrors the TS suite: on dashboards older than 3.2.15 the license
    // must be uploaded before the password can be changed at all.
    if first_time && *version < semver::Version::new(3, 2, 15) {
        activate_license(session, license).await;
    }
    session
        .put(
            "/api/password",
            json!({ "old_password": password, "new_password": BOOTSTRAP_PASSWORD }),
        )
        .await;
    session.login(username, BOOTSTRAP_PASSWORD).await;
    if first_time && *version >= semver::Version::new(3, 2, 15) {
        activate_license(session, license).await;
    }
}

/// Ports `global-setup.ts`'s `initUser` + `generateToken`: log in as the
/// default admin, rotate the password (a fresh dashboard rejects most
/// endpoints until this happens), activate the license, then provision a
/// throwaway super-admin user (also password-rotated) and mint an API
/// token from it — exactly the TS suite's own bootstrap, so a fresh
/// dashboard behaves identically regardless of which suite talks to it
/// first.
async fn bootstrap_token() -> String {
    let session = DashboardSession::new(server());
    session.wait_ready().await;

    let version = server_version();
    let license = env::var("BACKEND_API7_LICENSE")
        .ok()
        .filter(|v| !v.is_empty());

    init_user(&session, "admin", "admin", true, &version, &license).await;

    let username = unique_name("adc-rust-e2e");
    let invite = session
        .post(
            "/api/invites",
            json!({ "username": username, "password": "test" }),
        )
        .await;
    let user_id = invite["value"]["id"]
        .as_str()
        .expect("invite response missing value.id")
        .to_string();
    session
        .put(
            &format!("/api/users/{user_id}/assigned_roles"),
            json!({ "roles": ["super_admin_id"] }),
        )
        .await;

    init_user(&session, &username, "test", false, &version, &license).await;

    let token = session
        .post(
            "/api/tokens",
            json!({ "expires_at": 0, "name": unique_name("adc-rust-e2e-token") }),
        )
        .await;
    token["value"]["token"]
        .as_str()
        .expect("token response missing value.token")
        .to_string()
}

static TOKEN: OnceCell<String> = OnceCell::const_new();

/// A pre-minted `TOKEN` env var (e.g. handed off by a TS e2e run against
/// the same dashboard) short-circuits the dance; otherwise it runs once
/// per test binary and every test shares the result.
pub async fn token() -> String {
    if let Ok(token) = env::var("TOKEN") {
        return token;
    }
    TOKEN.get_or_init(bootstrap_token).await.clone()
}

pub async fn client() -> HttpClient {
    HttpClient::new(HttpClientConfig {
        server: server(),
        token: token().await,
        timeout: None,
        tls: TlsConfig {
            skip_verify: true,
            ..Default::default()
        },
    })
    .unwrap()
}

pub async fn backend() -> adc_backend_api7::Backend {
    adc_backend_api7::Backend::new(
        client().await,
        gateway_group(),
        &token().await,
        adc_backend_core::ResourceFilter::default(),
    )
}

pub async fn sync_events(
    backend: &adc_backend_api7::Backend,
    events: Vec<Event>,
) -> Result<Vec<BackendSyncResult>, BackendError> {
    sync_events_with_opts(backend, events, BackendSyncOptions::default()).await
}

pub async fn sync_events_with_opts(
    backend: &adc_backend_api7::Backend,
    events: Vec<Event>,
    opts: BackendSyncOptions,
) -> Result<Vec<BackendSyncResult>, BackendError> {
    use adc_sdk::Backend as _;
    backend.sync(events, opts).await
}

pub async fn dump_configuration(
    backend: &adc_backend_api7::Backend,
) -> Result<Configuration, BackendError> {
    use adc_sdk::Backend as _;
    backend.dump().await
}

pub async fn get_default_value(
    backend: &adc_backend_api7::Backend,
) -> Result<DefaultValue, BackendError> {
    use adc_sdk::Backend as _;
    backend.default_value().await
}

/// Runs the real differ (not a stand-in) between a desired `local`
/// configuration and the `remote` one a dump just returned, the same way
/// `adc-cli`'s own `pipeline::diff` does — so a test can build events by
/// stating the shape it wants rather than hand-assembling each `Event`.
pub fn diff(
    local: &Configuration,
    remote: &Configuration,
    default_value: Option<&DefaultValue>,
) -> Vec<Event> {
    fn to_diff_map(configuration: &Configuration) -> adc_sdk::InternalConfiguration {
        match serde_json::to_value(configuration).expect("Configuration always serializes") {
            Value::Object(map) => map,
            _ => unreachable!("Configuration always serializes to a JSON object"),
        }
    }
    adc_differ::DifferV4::diff(
        &to_diff_map(local),
        &to_diff_map(remote),
        default_value,
        None,
    )
}

/// A resource's id is derived from its name (and parent, where nested) via
/// the same content hash the differ itself uses — an SSL's id is derived
/// from its SNIs instead, since `resource_name` for an SSL is, by this
/// whole suite's own convention, already the comma-joined SNI list (see
/// e.g. `sslName` in the test files that build one).
pub fn create_event(
    resource_type: ResourceType,
    resource_name: &str,
    resource: Value,
    parent_name: Option<&str>,
) -> Event {
    let resource_id = derive_resource_id(resource_type, resource_name, parent_name);
    let mut event = Event::new(
        resource_type,
        EventKind::Create {
            new_value: resource,
        },
        resource_id,
        resource_name,
    );
    event.parent_id = derive_parent_id(resource_type, parent_name);
    event
}

/// Same id derivation as [`create_event`], but carrying an `Update` — the
/// differ itself always attaches a real `old_value`/`diff`, but nothing
/// downstream of event construction in these tests reads either, so an
/// empty placeholder `old_value` stands in.
pub fn update_event(
    resource_type: ResourceType,
    resource_name: &str,
    resource: Value,
    parent_name: Option<&str>,
) -> Event {
    let created = create_event(resource_type, resource_name, resource.clone(), parent_name);
    Event {
        kind: EventKind::Update {
            old_value: json!({}),
            new_value: resource,
            diff: None,
        },
        ..created
    }
}

pub fn delete_event(
    resource_type: ResourceType,
    resource_name: &str,
    parent_name: Option<&str>,
) -> Event {
    let resource_id = derive_resource_id_for_delete(resource_type, resource_name, parent_name);
    let mut event = Event::new(
        resource_type,
        EventKind::Delete {
            old_value: json!({}),
        },
        resource_id,
        resource_name,
    );
    event.parent_id = derive_parent_id(resource_type, parent_name);
    event
}

pub fn override_event_resource_id(
    mut event: Event,
    resource_id: &str,
    parent_id: Option<&str>,
) -> Event {
    event.resource_id = resource_id.to_string();
    if let Some(parent_id) = parent_id {
        event.parent_id = Some(parent_id.to_string());
    }
    event
}

fn derive_resource_id(
    resource_type: ResourceType,
    resource_name: &str,
    parent_name: Option<&str>,
) -> String {
    match resource_type {
        ResourceType::Consumer | ResourceType::GlobalRule | ResourceType::PluginMetadata => {
            resource_name.to_string()
        }
        ResourceType::Ssl => generate_id(resource_name),
        _ => derive_resource_id_for_delete(resource_type, resource_name, parent_name),
    }
}

fn derive_resource_id_for_delete(
    resource_type: ResourceType,
    resource_name: &str,
    parent_name: Option<&str>,
) -> String {
    match resource_type {
        ResourceType::Consumer | ResourceType::GlobalRule | ResourceType::PluginMetadata => {
            resource_name.to_string()
        }
        _ => generate_id(&match parent_name {
            Some(parent) => format!("{parent}.{resource_name}"),
            None => resource_name.to_string(),
        }),
    }
}

fn derive_parent_id(resource_type: ResourceType, parent_name: Option<&str>) -> Option<String> {
    parent_name.map(|parent| {
        if resource_type == ResourceType::ConsumerCredential {
            parent.to_string()
        } else {
            generate_id(parent)
        }
    })
}

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-api7/e2e/assets")
}

pub fn read_asset(name: &str) -> String {
    let path = assets_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn resource_type_from_fixture_str(value: &str) -> ResourceType {
    match value {
        "route" => ResourceType::Route,
        "service" => ResourceType::Service,
        "upstream" => ResourceType::Upstream,
        "ssl" => ResourceType::Ssl,
        "global_rule" => ResourceType::GlobalRule,
        "plugin_config" => ResourceType::PluginConfig,
        "plugin_metadata" => ResourceType::PluginMetadata,
        "consumer" => ResourceType::Consumer,
        "consumer_group" => ResourceType::ConsumerGroup,
        "consumer_credential" => ResourceType::ConsumerCredential,
        "stream_route" => ResourceType::StreamRoute,
        other => panic!("unrecognized resourceType {other:?} in fixture"),
    }
}

/// Loads a `testdata/*.json` fixture — a plain JSON array of events in the
/// TS suite's own camelCase field names (`resourceType`/`resourceId`/...),
/// not `Event`'s own (deliberately snake_case, `Serialize`-only) wire
/// shape — so this reads the raw JSON structurally instead of deriving
/// `Deserialize` on `Event` just for this one fixture-loading path.
pub fn load_events_fixture(name: &str) -> Vec<Event> {
    let raw: Value = serde_json::from_str(&read_asset(&format!("testdata/{name}")))
        .unwrap_or_else(|e| panic!("parsing fixture {name}: {e}"));
    raw.as_array()
        .unwrap_or_else(|| panic!("fixture {name} is not a JSON array"))
        .iter()
        .map(|item| {
            let resource_type = resource_type_from_fixture_str(
                item["resourceType"]
                    .as_str()
                    .expect("event missing resourceType"),
            );
            let resource_id = item["resourceId"]
                .as_str()
                .expect("event missing resourceId")
                .to_string();
            let resource_name = item["resourceName"]
                .as_str()
                .expect("event missing resourceName")
                .to_string();
            let kind = match item["type"].as_str().expect("event missing type") {
                "create" => EventKind::Create {
                    new_value: item["newValue"].clone(),
                },
                "update" => EventKind::Update {
                    old_value: item.get("oldValue").cloned().unwrap_or(Value::Null),
                    new_value: item["newValue"].clone(),
                    diff: None,
                },
                "delete" => EventKind::Delete {
                    old_value: item.get("oldValue").cloned().unwrap_or(Value::Null),
                },
                other => panic!("unrecognized event type {other:?}"),
            };
            let mut event = Event::new(resource_type, kind, resource_id, resource_name);
            event.parent_id = item
                .get("parentId")
                .and_then(Value::as_str)
                .map(String::from);
            event
        })
        .collect()
}

/// A `serde_json::Value`-based stand-in for Jest's `toMatchObject`: every
/// key `expected` declares must be present in `actual` and itself match
/// (recursively, for nested objects); an array in `expected` must have the
/// same length as `actual`'s, with each element matching positionally by
/// the same rule; any other value must be exactly equal. Extra keys/object
/// fields in `actual` that `expected` doesn't mention are ignored.
pub fn assert_matches_object(actual: &Value, expected: &Value) {
    assert_matches_object_at(actual, expected, "$");
}

fn assert_matches_object_at(actual: &Value, expected: &Value, path: &str) {
    match expected {
        Value::Object(expected_map) => {
            let Value::Object(actual_map) = actual else {
                panic!("at {path}: expected an object matching {expected}, got {actual}");
            };
            for (key, expected_value) in expected_map {
                let actual_value = actual_map.get(key).unwrap_or_else(|| {
                    panic!("at {path}.{key}: key missing from actual value {actual}")
                });
                assert_matches_object_at(actual_value, expected_value, &format!("{path}.{key}"));
            }
        }
        Value::Array(expected_items) => {
            let Value::Array(actual_items) = actual else {
                panic!("at {path}: expected an array matching {expected}, got {actual}");
            };
            assert_eq!(
                actual_items.len(),
                expected_items.len(),
                "at {path}: array length mismatch (actual {actual_items:?} vs expected {expected_items:?})"
            );
            for (index, (actual_item, expected_item)) in
                actual_items.iter().zip(expected_items).enumerate()
            {
                assert_matches_object_at(actual_item, expected_item, &format!("{path}[{index}]"));
            }
        }
        _ => assert_eq!(actual, expected, "at {path}"),
    }
}
