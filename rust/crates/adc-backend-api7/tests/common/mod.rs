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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use serde_json::{Value, json};
use tokio::sync::OnceCell;

const BOOTSTRAP_PASSWORD: &str = "Admin12345!@#$%";

pub fn server() -> String {
    env::var("SERVER").unwrap_or_else(|_| "https://localhost:7443".to_string())
}

pub fn gateway_group() -> String {
    env::var("GATEWAY_GROUP").unwrap_or_else(|_| "default".to_string())
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
        assert!(
            response.status().is_success(),
            "PUT {path} failed: {}",
            response.status()
        );
    }

    async fn post(&self, path: &str, body: Value) -> Value {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .unwrap_or_else(|e| panic!("POST {path}: {e}"));
        assert!(
            response.status().is_success(),
            "POST {path} failed: {}",
            response.status()
        );
        response
            .json()
            .await
            .unwrap_or_else(|e| panic!("decoding response from POST {path}: {e}"))
    }
}

async fn activate_license(session: &DashboardSession, license: &Option<String>) {
    if let Some(license) = license {
        session
            .put("/api/license", json!({ "data": license }))
            .await;
    }
}

/// Ports `global-setup.ts`'s `initUser` + `generateToken`: log in as the
/// default admin, rotate the password (a fresh dashboard rejects most
/// endpoints until this happens), activate the license, then provision a
/// throwaway super-admin user and mint an API token from it — exactly the
/// TS suite's own bootstrap, so a fresh dashboard behaves identically
/// regardless of which suite talks to it first.
async fn bootstrap_token() -> String {
    let session = DashboardSession::new(server());
    session.wait_ready().await;

    let version = env::var("BACKEND_API7_VERSION")
        .ok()
        .and_then(|v| semver::Version::parse(&v).ok())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));
    let license = env::var("BACKEND_API7_LICENSE")
        .ok()
        .filter(|v| !v.is_empty());

    session.login("admin", "admin").await;
    // Mirrors the TS suite: on dashboards older than 3.2.15 the license
    // must be uploaded before the password can be changed at all.
    if version < semver::Version::new(3, 2, 15) {
        activate_license(&session, &license).await;
    }
    session
        .put(
            "/api/password",
            json!({ "old_password": "admin", "new_password": BOOTSTRAP_PASSWORD }),
        )
        .await;
    session.login("admin", BOOTSTRAP_PASSWORD).await;
    if version >= semver::Version::new(3, 2, 15) {
        activate_license(&session, &license).await;
    }

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

    session.login(&username, "test").await;
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
