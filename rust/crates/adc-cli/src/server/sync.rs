//! `PUT /sync`: lint (optional) + diff against the remote backend + apply.

use std::collections::{HashMap, HashSet};

use adc_sdk::resources::Configuration;
use adc_sdk::{BackendSyncOptions, BackendSyncResult, Event, ResourceType};
use axum::Json;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::schema::{self, SyncInput};
use super::{backend, bad_request, internal_error};
use crate::config;
use crate::error::CliError;
use crate::pipeline;

pub async fn sync_handler(body: Bytes) -> Response {
    let input: SyncInput = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => return bad_request(json!({"message": error.to_string(), "errors": []})),
    };
    let opts = input.task.opts;

    let tls_issues = schema::validate_tls_material(&opts);
    if !tls_issues.is_empty() {
        return bad_request(json!({"message": "invalid TLS material", "errors": tls_issues}));
    }

    let label_selector = opts.label_selector_or_default();
    let mut config_value = input.task.config;
    config::fill_labels(&mut config_value, &label_selector);

    let mut configuration: Configuration = match serde_json::from_value(config_value) {
        Ok(configuration) => configuration,
        Err(error) => {
            return bad_request(json!({"message": format!("invalid configuration: {error}"), "errors": []}));
        }
    };

    let (include, exclude) = opts.resource_type_sets();
    config::filter_resource_types(&mut configuration, &include, &exclude);

    if opts.lint {
        let issues = adc_sdk::lint::lint(&configuration);
        if !issues.is_empty() {
            return bad_request(json!({
                "message": "Lint configuration\nThe following errors were found in configuration:",
                "errors": issues.iter().map(lint_issue_json).collect::<Vec<_>>(),
            }));
        }
    }

    match run(&opts.backend, &opts, configuration, &include, &exclude, &label_selector).await {
        Ok(output) => (StatusCode::ACCEPTED, Json(output)).into_response(),
        Err(error) => internal_error(json!({"message": error.to_string()})),
    }
}

async fn run(
    backend_kind: &str,
    opts: &schema::Opts,
    local: Configuration,
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
    label_selector: &HashMap<String, String>,
) -> Result<Value, CliError> {
    let gateway = backend::build_backend(opts)?;
    let remote = pipeline::load_remote(gateway.as_ref(), include, exclude, label_selector).await?;
    let events = pipeline::diff(gateway.as_ref(), &local, &remote).await?;
    let sync_opts = BackendSyncOptions { concurrent: Some(opts.request_concurrent), exit_on_failure: Some(false) };
    let results = gateway.sync(events.clone(), sync_opts).await?;

    Ok(if backend_kind == "apisix-standalone" {
        output_for_apisix_standalone(&events, &results)
    } else {
        output(&results)
    })
}

fn status_of(total: usize, successes: usize, failures: usize) -> &'static str {
    if total == successes {
        "success"
    } else if total == failures {
        "all_failed"
    } else {
        "partial_failure"
    }
}

fn output(results: &[BackendSyncResult]) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    let (successes, failures): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.success);

    json!({
        "status": status_of(results.len(), successes.len(), failures.len()),
        "total_resources": results.len(),
        "success_count": successes.len(),
        "failed_count": failures.len(),
        "success": successes.iter().map(|r| json!({
            "server": r.server,
            "event": r.event.as_ref().map(simplify_event),
            "synced_at": now,
        })).collect::<Vec<_>>(),
        "failed": failures.iter().map(|r| json!({
            "server": r.server,
            "event": r.event.as_ref().map(simplify_event),
            "failed_at": now,
            "reason": r.error.as_ref().map(|e| e.to_string()).unwrap_or_default(),
        })).collect::<Vec<_>>(),
    })
}

/// One `BackendSyncResult` per *server* here, not per event — `success`/
/// `failed` describe `events` directly, `endpoint_status` carries the
/// per-server detail.
fn output_for_apisix_standalone(events: &[Event], results: &[BackendSyncResult]) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    let (successes, failures): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.success);

    json!({
        "status": status_of(results.len(), successes.len(), failures.len()),
        "total_resources": 0,
        "success_count": successes.len(),
        "failed_count": failures.len(),
        "success": events.iter().map(|event| json!({
            "event": simplify_event(event),
            "synced_at": now,
        })).collect::<Vec<_>>(),
        "failed": Vec::<Value>::new(),
        "endpoint_status": results.iter().map(|r| json!({
            "server": r.server,
            "success": r.success,
            "reason": r.error.as_ref().map(|e| e.to_string()),
            "requested_at": now,
        })).collect::<Vec<_>>(),
    })
}

fn simplify_event(event: &Event) -> Value {
    let mut value = serde_json::to_value(event).expect("Event always serializes");
    if let Value::Object(map) = &mut value {
        map.remove("old_value");
        map.remove("new_value");
        map.remove("diff");
    }
    value
}

fn lint_issue_json(issue: &adc_sdk::lint::LintIssue) -> Value {
    json!({
        "path": issue.path.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "message": issue.message,
    })
}
