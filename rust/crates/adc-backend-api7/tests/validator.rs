//! Not a real-dashboard e2e test: a local HTTP server (same pattern as
//! `adc-backend-core/tests/http_client.rs`) standing in for one specific,
//! canned `/apisix/admin/configs/validate` response — this exercises the
//! `Validator`'s own request-building and error-to-`Event` mapping logic,
//! not real gateway validation behavior, so it doesn't need `docker
//! compose up` or `#[ignore]`.

use adc_backend_api7::tests::Validator;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_sdk::utils::generate_id;
use adc_sdk::{Event, EventKind, ResourceType};
use axum::Json;
use axum::extract::State;
use axum::routing::post;
use semver::Version;
use serde_json::{Value, json};
use tokio::net::TcpListener;

async fn spawn_validate_server(status: u16, body: Value) -> String {
    let status = axum::http::StatusCode::from_u16(status).unwrap();
    let router = axum::Router::new()
        .route(
            "/apisix/admin/configs/validate",
            post(move |State(body): State<Value>| async move { (status, Json(body)) }),
        )
        .with_state(body);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

fn client(server: String) -> HttpClient {
    HttpClient::new(HttpClientConfig {
        server,
        token: "test-token".to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap()
}

/// A minimal but *structurally valid* `adc_sdk::resources::X` payload for
/// each resource type this file exercises — `Validator::build_request`
/// deserializes into the strongly-typed ADC shape before transforming, so a
/// genuinely incomplete object fails before ever reaching the mocked
/// server.
fn create_event(
    resource_type: ResourceType,
    resource_name: &str,
    parent_id: Option<&str>,
) -> Event {
    let new_value = match resource_type {
        ResourceType::Consumer => json!({ "username": resource_name }),
        ResourceType::Route => json!({ "name": resource_name, "uris": [] }),
        _ => json!({ "name": resource_name }),
    };
    let mut event = Event::new(
        resource_type,
        EventKind::Create { new_value },
        generate_id(resource_name),
        resource_name,
    );
    event.parent_id = parent_id.map(String::from);
    event
}

#[tokio::test]
async fn embeds_the_event_in_validation_errors_for_routes() {
    let server = spawn_validate_server(
        400,
        json!({
            "error_msg": "Configuration validation failed",
            "errors": [{
                "resource_type": "routes",
                "index": 0,
                "error": "does not match schema due to: Error at \"/methods/0\": value is not one of the allowed values",
            }],
        }),
    )
    .await;
    let parent_id = generate_id("httpbin.org");
    let events = vec![
        create_event(ResourceType::Service, "httpbin.org", None),
        create_event(ResourceType::Route, "get-anything", Some(&parent_id)),
    ];

    let validator = Validator::new(
        client(server),
        Version::new(3, 10, 0),
        Some("default".to_string()),
    );
    let result = validator.validate(&events).await.unwrap();

    assert!(!result.success);
    assert_eq!(
        result.error_message.as_deref(),
        Some("Configuration validation failed")
    );
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].resource_type, "routes");
    assert_eq!(
        result.errors[0].resource_name.as_deref(),
        Some("get-anything")
    );
    let event = result.errors[0]
        .event
        .as_ref()
        .expect("event should be embedded");
    assert_eq!(event.resource_type, ResourceType::Route);
    assert_eq!(event.resource_name, "get-anything");
    assert_eq!(event.parent_id.as_deref(), Some(parent_id.as_str()));
    assert_eq!(event.event_type(), adc_sdk::EventType::Create);
    assert_eq!(
        event.kind.new_value(),
        Some(&json!({ "name": "get-anything", "uris": [] }))
    );
}

#[tokio::test]
async fn embeds_the_event_in_validation_errors_for_services() {
    let server = spawn_validate_server(
        400,
        json!({
            "error_msg": "Configuration validation failed",
            "errors": [{ "resource_type": "services", "index": 0, "error": "does not match schema due to: plugins validation failed" }],
        }),
    )
    .await;
    let events = vec![
        create_event(ResourceType::Service, "httpbin.org", None),
        create_event(ResourceType::Route, "get-anything", Some("some-parent")),
    ];

    let validator = Validator::new(
        client(server),
        Version::new(3, 10, 0),
        Some("default".to_string()),
    );
    let result = validator.validate(&events).await.unwrap();

    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].resource_type, "services");
    assert_eq!(
        result.errors[0].resource_name.as_deref(),
        Some("httpbin.org")
    );
    let event = result.errors[0]
        .event
        .as_ref()
        .expect("event should be embedded");
    assert_eq!(event.resource_type, ResourceType::Service);
    assert_eq!(event.resource_name, "httpbin.org");
}

#[tokio::test]
async fn succeeds_when_there_are_no_validation_errors() {
    let server = spawn_validate_server(200, json!({})).await;
    let events = vec![create_event(ResourceType::Service, "httpbin.org", None)];

    let validator = Validator::new(
        client(server),
        Version::new(3, 10, 0),
        Some("default".to_string()),
    );
    let result = validator.validate(&events).await.unwrap();

    assert!(result.success);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn handles_multiple_errors_with_correct_event_mapping() {
    let server = spawn_validate_server(
        400,
        json!({
            "error_msg": "Configuration validation failed",
            "errors": [
                { "resource_type": "routes", "index": 0, "error": "error on route-a" },
                { "resource_type": "routes", "index": 1, "error": "error on route-b" },
                { "resource_type": "consumers", "index": 0, "error": "error on user1" },
            ],
        }),
    )
    .await;
    let parent_id = generate_id("my-service");
    let events = vec![
        create_event(ResourceType::Service, "my-service", None),
        create_event(ResourceType::Route, "route-a", Some(&parent_id)),
        create_event(ResourceType::Route, "route-b", Some(&parent_id)),
        create_event(ResourceType::Consumer, "user1", None),
    ];

    let validator = Validator::new(
        client(server),
        Version::new(3, 10, 0),
        Some("default".to_string()),
    );
    let result = validator.validate(&events).await.unwrap();

    assert_eq!(result.errors.len(), 3);

    assert_eq!(result.errors[0].resource_name.as_deref(), Some("route-a"));
    let event = result.errors[0].event.as_ref().unwrap();
    assert_eq!(event.resource_name, "route-a");
    assert_eq!(event.parent_id.as_deref(), Some(parent_id.as_str()));

    assert_eq!(result.errors[1].resource_name.as_deref(), Some("route-b"));
    let event = result.errors[1].event.as_ref().unwrap();
    assert_eq!(event.resource_name, "route-b");
    assert_eq!(event.parent_id.as_deref(), Some(parent_id.as_str()));

    assert_eq!(result.errors[2].resource_name.as_deref(), Some("user1"));
    let event = result.errors[2].event.as_ref().unwrap();
    assert_eq!(event.resource_name, "user1");
    assert_eq!(event.parent_id, None);
}

#[tokio::test]
async fn handles_an_error_with_no_matching_event_index_gracefully() {
    let server = spawn_validate_server(400, json!({ "errors": [{ "resource_type": "unknown_type", "index": 0, "error": "some error" }] })).await;
    let events = vec![create_event(ResourceType::Service, "my-service", None)];

    let validator = Validator::new(
        client(server),
        Version::new(3, 10, 0),
        Some("default".to_string()),
    );
    let result = validator.validate(&events).await.unwrap();

    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].event.is_none());
}

#[tokio::test]
async fn rejects_up_front_when_the_version_is_below_the_minimum() {
    // No server needed: the version check runs before any request is sent.
    let client = client("http://127.0.0.1:1".to_string());
    let events = vec![create_event(ResourceType::Service, "my-service", None)];

    let validator = Validator::new(client, Version::new(3, 9, 9), Some("default".to_string()));
    let error = validator.validate(&events).await.unwrap_err();

    assert!(error.to_string().contains("not supported"), "{error}");
}
