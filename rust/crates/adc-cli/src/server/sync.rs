//! `PUT /sync`: lint (optional) + diff against the remote backend + apply.

use std::collections::{HashMap, HashSet};

use adc_sdk::resources::Configuration;
use adc_sdk::{BackendSyncOptions, BackendSyncResult, Event, ResourceType};
use axum::Json;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};

use super::schema::{self, SyncInput};
use super::{backend, bad_request, internal_error, sync_lock};
use crate::config;
use crate::error::CliError;
use crate::pipeline;

pub async fn sync_handler(body: Bytes) -> Response {
    let input: SyncInput = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => return bad_request(json!({"message": error.to_string(), "errors": []})),
    };
    let opts = input.task.opts;

    let mut issues = schema::validate_server_addr(&opts);
    issues.extend(schema::validate_tls_material(&opts));
    if !issues.is_empty() {
        return bad_request(json!({"message": "invalid request", "errors": issues}));
    }

    let label_selector = opts.label_selector_or_default();
    let mut config_value = input.task.config;
    config::fill_labels(&mut config_value, &label_selector);

    let mut configuration: Configuration = match serde_json::from_value(config_value) {
        Ok(configuration) => configuration,
        Err(error) => {
            return bad_request(
                json!({"message": format!("invalid configuration: {error}"), "errors": []}),
            );
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

    match run(
        &opts.backend,
        &opts,
        configuration,
        &include,
        &exclude,
        &label_selector,
    )
    .await
    {
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
    let is_apisix_standalone = backend_kind == "apisix-standalone";

    let _standalone_guard = if is_apisix_standalone {
        Some(sync_lock::lock(&opts.cache_key).await)
    } else {
        None
    };

    let gateway = backend::build_backend(opts)?;
    let remote = pipeline::load_remote(gateway.as_ref(), include, exclude, label_selector).await?;
    let events = pipeline::diff(gateway.as_ref(), &local, &remote).await?;
    let sync_opts = BackendSyncOptions {
        concurrent: Some(opts.request_concurrent),
        exit_on_failure: Some(false),
    };

    let events_for_output = is_apisix_standalone.then(|| events.clone());
    let results = gateway.sync(events, sync_opts).await?;
    Ok(match events_for_output {
        Some(events) => output_for_apisix_standalone(&events, &results),
        None => output(&results),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum SyncStatus {
    Success,
    AllFailed,
    PartialFailure,
}

fn status_of(total: usize, successes: usize, failures: usize) -> SyncStatus {
    if total == successes {
        SyncStatus::Success
    } else if total == failures {
        SyncStatus::AllFailed
    } else {
        SyncStatus::PartialFailure
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
            "confirmation": confirmation_of(r),
            "reason": r.error.as_ref().map(|e| e.to_string()),
            "requested_at": now,
        })).collect::<Vec<_>>(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Confirmation {
    Applied,
    Accepted,
}

/// `null` for a failed write (nothing was confirmed *or* accepted) or a
/// backend/cluster that never reports the distinction (`confirmed: None`)
/// — `Applied` when the write was confirmed picked up by the data plane,
/// `Accepted` when it was only accepted for later processing.
fn confirmation_of(result: &BackendSyncResult) -> Option<Confirmation> {
    if !result.success {
        return None;
    }
    match result.confirmed {
        Some(true) => Some(Confirmation::Applied),
        Some(false) => Some(Confirmation::Accepted),
        None => None,
    }
}

fn simplify_event(event: &Event) -> Value {
    let mut value = match serde_json::to_value(event) {
        Ok(value) => value,
        // The sync itself already happened by the time this runs — a
        // response with a placeholder event beats panicking and losing the
        // result entirely.
        Err(error) => return json!({"error": format!("failed to serialize event: {error}")}),
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn result(success: bool, confirmed: Option<bool>) -> BackendSyncResult {
        BackendSyncResult { success, event: None, error: None, server: Some("s1".to_string()), confirmed }
    }

    #[test]
    fn a_failed_write_has_no_confirmation_regardless_of_what_confirmed_says() {
        assert!(confirmation_of(&result(false, Some(true))).is_none());
        assert!(confirmation_of(&result(false, None)).is_none());
    }

    #[test]
    fn a_backend_that_never_reports_the_distinction_has_no_confirmation() {
        assert!(confirmation_of(&result(true, None)).is_none());
    }

    #[test]
    fn a_confirmed_write_serializes_to_applied() {
        let confirmation = confirmation_of(&result(true, Some(true))).unwrap();
        assert_eq!(serde_json::to_value(confirmation).unwrap(), json!("applied"));
    }

    #[test]
    fn a_merely_accepted_write_serializes_to_accepted() {
        let confirmation = confirmation_of(&result(true, Some(false))).unwrap();
        assert_eq!(serde_json::to_value(confirmation).unwrap(), json!("accepted"));
    }

    #[test]
    fn status_of_serializes_to_the_three_snake_case_values() {
        assert_eq!(serde_json::to_value(status_of(2, 2, 0)).unwrap(), json!("success"));
        assert_eq!(serde_json::to_value(status_of(2, 0, 2)).unwrap(), json!("all_failed"));
        assert_eq!(serde_json::to_value(status_of(2, 1, 1)).unwrap(), json!("partial_failure"));
    }
}
