//! Black-box `/sync` coverage for the apisix-standalone backend through the
//! real ingress-server HTTP layer, against a live 3-instance standalone
//! APISIX cluster — same cluster `adc-backend-apisix-standalone`'s own e2e
//! suite uses (see that crate's `tests/common/mod.rs` for how to bring it
//! up). That suite already covers `Backend` itself thoroughly; this file
//! only proves the pieces it can't see: `output_for_apisix_standalone`'s
//! JSON shape, HTTP status mapping, and `endpoint_status`, exercised end to
//! end through a real HTTP request to a real spawned `adc` process — the
//! same black-box pattern `ingress_server_sigint.rs` uses.
//!
//! The real cluster here reports APISIX/3.17.0, below the 3.19.0
//! `?wait`-confirmation gate — every successful write is therefore always
//! `confirmed` regardless of the raw status APISIX itself returns for that
//! request. The `>= 3.19 and 202 → accepted, not applied` branch has no
//! real gateway to test against yet; `operator.rs`'s own unit tests are the
//! only coverage for it (see D10 in `impl/standalone/changes-by-project.md`).

use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::{Child, Command};
use tokio::time::timeout;

const SERVER1: &str = "http://localhost:19180";
const SERVER2: &str = "http://localhost:29180";
const SERVER3: &str = "http://localhost:39180";
const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";
const UNREACHABLE: &str = "http://127.0.0.1:1";

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// Same cluster, same wipe-and-recheck strategy as
/// `adc-backend-apisix-standalone`'s own `common::restart_apisix` — every
/// test here needs a clean document on all three real servers, since
/// standalone has no per-test namespacing of its own.
async fn restart_apisix() {
    let compose_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../libs/backend-apisix-standalone/e2e/assets");
    let status = Command::new("docker")
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

async fn wait_until_ready(server: &str) {
    let client = reqwest::Client::new();
    const MAX_ATTEMPTS: u32 = 60;
    const REQUIRED_CONSECUTIVE_SUCCESSES: u32 = 2;
    let mut consecutive_successes = 0;
    for attempt in 1..=MAX_ATTEMPTS {
        let got_200 = client
            .get(format!("{server}/apisix/admin/configs"))
            .header("X-API-KEY", TOKEN)
            .send()
            .await
            .map(|r| r.status().as_u16() == 200)
            .unwrap_or(false);
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

/// Spawns the real `adc` binary in `ingress-server` mode and waits for its
/// status listener to report ready.
struct Server {
    child: Child,
    base_url: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

async fn spawn_server() -> Server {
    let listen_port = free_port();
    let status_port = free_port();
    let child = Command::new(env!("CARGO_BIN_EXE_adc"))
        .args(["ingress-server", "--listen", &format!("http://127.0.0.1:{listen_port}"), "--listen-status", &format!("{status_port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to spawn the adc binary");

    let ready = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(response) = reqwest::get(format!("http://127.0.0.1:{status_port}/healthz/ready")).await
                && response.status().is_success()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(ready.is_ok(), "ingress-server never became ready");

    Server { child, base_url: format!("http://127.0.0.1:{listen_port}") }
}

fn sync_body(servers: &[&str], cache_key: &str, config: Value) -> Value {
    json!({
        "task": {
            "opts": {
                "backend": "apisix-standalone",
                "server": servers,
                "token": TOKEN,
                "cacheKey": cache_key,
            },
            "config": config,
        }
    })
}

async fn put_sync(server: &Server, body: &Value) -> (u16, Value) {
    let response = reqwest::Client::new().put(format!("{}/sync", server.base_url)).json(body).send().await.unwrap();
    let status = response.status().as_u16();
    let json = response.json().await.unwrap();
    (status, json)
}

/// `server`'s raw config document, bypassing this whole crate — used to
/// prove a rejected write actually left the real server untouched, not
/// just that the response said so. `error_for_status` first, so a failed
/// request surfaces as a panic instead of silently reading as "no
/// consumers". A genuinely successful response with no `consumers` field
/// at all is a real, different case — a fresh instance that has never had
/// anything written to it omits the key entirely rather than returning
/// `[]` (see `typing.rs`'s own tolerant deserialization for this) — and is
/// correctly treated as "no consumers".
async fn raw_consumers(server: &str) -> Vec<Value> {
    let response = reqwest::Client::new()
        .get(format!("{server}/apisix/admin/configs"))
        .header("X-API-KEY", TOKEN)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let body: Value = response.json().await.unwrap();
    body["consumers"].as_array().cloned().unwrap_or_default()
}

fn consumer(username: &str) -> Value {
    json!({"username": username, "plugins": {"limit-count": {"count": 10, "time_window": 60}}})
}

/// `limit-count` requires `count`/`time_window`; both are missing — the
/// same shape `e2e_validate.rs` uses, confirmed against a real standalone
/// admin API to 422 the whole config write, not just `/validate`.
fn consumer_with_bad_plugin(username: &str) -> Value {
    json!({"username": username, "plugins": {"limit-count": {}}})
}

#[tokio::test]
#[ignore]
async fn a_successful_sync_confirms_every_server() {
    restart_apisix().await;
    let server = spawn_server().await;
    let body = sync_body(&[SERVER1, SERVER2, SERVER3], "sync-e2e-success", json!({"consumers": [consumer("sync-ok")]}));

    let (status, json) = put_sync(&server, &body).await;
    // Every server confirmed the write (see the loop below) -- 200, not 202.
    assert_eq!(status, 200, "{json}");
    assert_eq!(json["status"], "success", "{json}");
    assert_eq!(json["total_resources"], 1, "{json}");
    assert_eq!(json["success_count"], 1, "{json}");
    assert_eq!(json["failed_count"], 0, "{json}");
    assert_eq!(json["success"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(json["failed"].as_array().unwrap().len(), 0, "{json}");

    let endpoint_status = json["endpoint_status"].as_array().unwrap();
    assert_eq!(endpoint_status.len(), 3, "{json}");
    for entry in endpoint_status {
        assert_eq!(entry["success"], true, "{json}");
        // The real cluster reports APISIX/3.17.0, below the 3.19.0 wait
        // gate — every successful write is confirmed regardless of the raw
        // status APISIX itself returned for this request.
        assert_eq!(entry["confirmation"], "applied", "{json}");
        assert!(entry["reason"].is_null(), "{json}");
    }
}

#[tokio::test]
#[ignore]
async fn an_all_server_rejection_reports_structured_per_resource_failures() {
    restart_apisix().await;
    let server = spawn_server().await;
    let config = json!({"consumers": [consumer("innocent-bystander"), consumer_with_bad_plugin("the-bad-one")]});
    let body = sync_body(&[SERVER1, SERVER2, SERVER3], "sync-e2e-all-failed", config);

    let (status, json) = put_sync(&server, &body).await;
    assert_eq!(status, 422, "{json}");
    assert_eq!(json["status"], "all_failed", "{json}");
    assert_eq!(json["total_resources"], 2, "{json}");
    assert_eq!(json["success_count"], 0, "{json}");
    assert_eq!(json["failed_count"], 2, "{json}");
    assert_eq!(json["success"].as_array().unwrap().len(), 0, "{json}");

    let failed = json["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 2, "{json}");
    let bad = failed.iter().find(|e| e["event"]["resource_name"] == "the-bad-one").expect("the bad resource must be in `failed`");
    let good = failed
        .iter()
        .find(|e| e["event"]["resource_name"] == "innocent-bystander")
        .expect("the whole document was rejected — the innocent resource must be in `failed` too");

    let bad_reason = bad["reason"].as_str().unwrap();
    assert!(bad_reason.to_lowercase().contains("limit-count"), "{bad_reason}");
    let good_reason = good["reason"].as_str().unwrap();
    assert!(!good_reason.is_empty(), "{json}");

    let endpoint_status = json["endpoint_status"].as_array().unwrap();
    assert_eq!(endpoint_status.len(), 3, "{json}");
    for entry in endpoint_status {
        assert_eq!(entry["success"], false, "{json}");
        assert!(entry["confirmation"].is_null(), "{json}");
        assert!(entry["reason"].as_str().unwrap().to_lowercase().contains("limit-count"), "{json}");
    }

    // Rejected outright by every server — neither consumer, not even the
    // innocent one, actually landed anywhere on any of the three.
    for target in [SERVER1, SERVER2, SERVER3] {
        assert!(raw_consumers(target).await.is_empty(), "an all-failed write must leave {target} untouched");
    }

    // The cluster isn't left poisoned by the rejected write — a follow-up
    // sync with just the valid resource still succeeds normally.
    let recovery_body = sync_body(&[SERVER1, SERVER2, SERVER3], "sync-e2e-all-failed", json!({"consumers": [consumer("innocent-bystander")]}));
    let (status, json) = put_sync(&server, &recovery_body).await;
    assert_eq!(status, 200, "{json}");
    assert_eq!(json["status"], "success", "{json}");
    assert_eq!(raw_consumers(SERVER1).await.len(), 1, "{json}");
}

#[tokio::test]
#[ignore]
async fn a_partial_server_failure_still_reports_the_event_as_synced() {
    restart_apisix().await;
    let server = spawn_server().await;
    let body = sync_body(&[SERVER1, SERVER2, UNREACHABLE], "sync-e2e-partial-failure", json!({"consumers": [consumer("sync-partial")]}));

    let (status, json) = put_sync(&server, &body).await;
    assert_eq!(status, 202, "{json}");
    assert_eq!(json["status"], "partial_failure", "{json}");
    assert_eq!(json["total_resources"], 1, "{json}");
    assert_eq!(json["success_count"], 1, "{json}");
    assert_eq!(json["failed_count"], 0, "{json}");
    assert_eq!(json["success"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(json["failed"].as_array().unwrap().len(), 0, "{json}");

    let endpoint_status = json["endpoint_status"].as_array().unwrap();
    assert_eq!(endpoint_status.len(), 3, "{json}");
    let successes = endpoint_status.iter().filter(|e| e["success"] == true).count();
    let failures = endpoint_status.iter().filter(|e| e["success"] == false).count();
    assert_eq!(successes, 2, "{json}");
    assert_eq!(failures, 1, "{json}");
    let failed_entry = endpoint_status.iter().find(|e| e["success"] == false).unwrap();
    assert!(failed_entry["confirmation"].is_null(), "{json}");
    assert!(failed_entry["reason"].as_str().is_some_and(|r| !r.is_empty()), "{json}");
}
