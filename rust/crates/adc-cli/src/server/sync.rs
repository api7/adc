//! `PUT /sync`: lint (optional) + diff against the remote backend + apply.

use std::collections::{HashMap, HashSet};

use adc_sdk::resources::Configuration;
use adc_sdk::{Backend, BackendSyncOptions, BackendSyncResult, BackendValidateResult, Event, ResourceType};
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
        Ok((status, output)) => (status, Json(output)).into_response(),
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
) -> Result<(StatusCode, Value), CliError> {
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
    match events_for_output {
        Some(events) => Ok(output_for_apisix_standalone(gateway.as_ref(), &events, &results).await),
        None => Ok((StatusCode::ACCEPTED, output(&results))),
    }
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

/// `server` is only ever `Some` for the per-server backends (apisix-
/// standalone) — apisix/api7ee never set it (they have exactly one
/// target, not a cluster to distinguish between), and skip it entirely
/// rather than serializing an always-`null` field: matches the TS
/// implementation, where the equivalent property is simply never assigned
/// (`server?: string`) and so never appears in the JSON either — an
/// absent key there, not a `null` one.
#[derive(Serialize)]
struct SuccessEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    event: Option<Value>,
    synced_at: String,
}

#[derive(Serialize)]
struct FailedEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    event: Option<Value>,
    failed_at: String,
    reason: String,
}

fn output(results: &[BackendSyncResult]) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    let (successes, failures): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.success);

    json!({
        "status": status_of(results.len(), successes.len(), failures.len()),
        "total_resources": results.len(),
        "success_count": successes.len(),
        "failed_count": failures.len(),
        "success": successes.iter().map(|r| SuccessEntry {
            server: r.server.clone(),
            event: r.event.as_ref().map(simplify_event),
            synced_at: now.clone(),
        }).collect::<Vec<_>>(),
        "failed": failures.iter().map(|r| FailedEntry {
            server: r.server.clone(),
            event: r.event.as_ref().map(simplify_event),
            failed_at: now.clone(),
            reason: r.error.as_ref().map(|e| e.to_string()).unwrap_or_default(),
        }).collect::<Vec<_>>(),
    })
}

/// One `BackendSyncResult` per *server* here, not per event — `success`/
/// `failed` describe `events` directly (same shape as `output`'s, so the
/// caller doesn't need a standalone-specific parser for them), and
/// `endpoint_status` carries the per-server detail `output` has no
/// equivalent for.
///
/// `events` is never split between `success`/`failed` — apisix-standalone
/// writes one document in one request, so `events` as a whole landed or
/// it didn't: every server failing is always `400`, and puts every event
/// in `failed`; anything else (including a partial failure — some server
/// rejecting the exact same document some other server just accepted
/// isn't about the document being bad) puts every event in `success`.
///
/// On an all-failed `400`, re-validates `events` against the gateway (the
/// same call `/validate` itself makes) for a `reason` more specific than
/// the sync failure's own — a resource named in the re-validate's result
/// gets its own error; everything else falls back to the first server's
/// rejection reason (still useful — a conf_version conflict, say, rejects
/// the whole document for a reason that has nothing to do with any one
/// resource, and re-validating finds nothing to report there). Best-effort
/// — a re-validate that itself errors just means every event falls back
/// to that same shared reason.
async fn output_for_apisix_standalone(gateway: &dyn Backend, events: &[Event], results: &[BackendSyncResult]) -> (StatusCode, Value) {
    let now = chrono::Utc::now().to_rfc3339();
    let (successes, failures): (Vec<_>, Vec<_>) = results.iter().partition(|r| r.success);
    let status = status_of(results.len(), successes.len(), failures.len());

    let (http_status, success, failed) = match status {
        SyncStatus::AllFailed => {
            let errors = match gateway.validate(events).await {
                Ok(BackendValidateResult { errors, .. }) => errors,
                Err(_) => vec![],
            };
            let reason_by_resource: HashMap<&str, &str> =
                errors.iter().filter_map(|e| e.resource_id.as_deref().map(|id| (id, e.error.as_str()))).collect();
            let shared_reason = failures.first().and_then(|r| r.error.as_ref()).map(|e| e.to_string()).unwrap_or_default();

            let failed = events
                .iter()
                .map(|event| FailedEntry {
                    server: None,
                    event: Some(simplify_event(event)),
                    failed_at: now.clone(),
                    reason: reason_for(event, &reason_by_resource, &shared_reason),
                })
                .collect::<Vec<_>>();
            (StatusCode::BAD_REQUEST, Vec::new(), failed)
        }
        SyncStatus::Success | SyncStatus::PartialFailure => {
            let success = events
                .iter()
                .map(|event| SuccessEntry { server: None, event: Some(simplify_event(event)), synced_at: now.clone() })
                .collect::<Vec<_>>();
            (StatusCode::ACCEPTED, success, Vec::new())
        }
    };

    let body = json!({
        "status": status,
        "total_resources": 0,
        "success_count": successes.len(),
        "failed_count": failures.len(),
        "success": success,
        "failed": failed,
        "endpoint_status": results.iter().map(|r| EndpointStatusEntry {
            server: r.server.clone(),
            success: r.success,
            confirmation: confirmation_of(r),
            reason: r.error.as_ref().map(|e| e.to_string()),
            requested_at: now.clone(),
        }).collect::<Vec<_>>(),
    });
    (http_status, body)
}

/// The specific reason a re-validate found for `event` (matched by
/// `resource_id`), or `shared_reason` when it didn't — either because the
/// re-validate ran but didn't report anything for this particular event
/// (a resource swept up in the same failed batch without being
/// individually invalid), or because it found nothing at all (the whole
/// document was rejected for a reason that isn't about any one resource).
fn reason_for(event: &Event, reason_by_resource: &HashMap<&str, &str>, shared_reason: &str) -> String {
    reason_by_resource.get(event.resource_id.as_str()).map(|r| r.to_string()).unwrap_or_else(|| shared_reason.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Confirmation {
    Applied,
    Accepted,
}

#[derive(Serialize)]
struct EndpointStatusEntry {
    server: Option<String>,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    confirmation: Option<Confirmation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    requested_at: String,
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
    fn a_none_server_is_an_absent_key_not_a_null_value() {
        let entry = SuccessEntry { server: None, event: None, synced_at: "t".to_string() };
        let value = serde_json::to_value(entry).unwrap();
        assert!(!value.as_object().unwrap().contains_key("server"), "{value}");

        let entry = FailedEntry { server: None, event: None, failed_at: "t".to_string(), reason: "x".to_string() };
        let value = serde_json::to_value(entry).unwrap();
        assert!(!value.as_object().unwrap().contains_key("server"), "{value}");
    }

    #[test]
    fn a_some_server_still_serializes_normally() {
        let entry = SuccessEntry { server: Some("s1".to_string()), event: None, synced_at: "t".to_string() };
        let value = serde_json::to_value(entry).unwrap();
        assert_eq!(value["server"], json!("s1"));
    }

    #[test]
    fn endpoint_status_omits_confirmation_and_reason_when_absent_rather_than_nulling_them() {
        let entry = EndpointStatusEntry { server: Some("s1".to_string()), success: true, confirmation: None, reason: None, requested_at: "t".to_string() };
        let value = serde_json::to_value(entry).unwrap();
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("confirmation"), "{value}");
        assert!(!object.contains_key("reason"), "{value}");
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

    fn event_with_id(resource_id: &str) -> Event {
        Event::new(ResourceType::Service, adc_sdk::EventKind::Create { new_value: json!({"name": "svc"}) }, resource_id, "svc")
    }

    #[test]
    fn reason_for_prefers_the_re_validate_error_matched_by_resource_id() {
        let event = event_with_id("bad-one");
        let reason_by_resource = HashMap::from([("bad-one", "unknown plugin")]);
        assert_eq!(reason_for(&event, &reason_by_resource, "shared reason"), "unknown plugin");
    }

    #[test]
    fn reason_for_falls_back_to_the_shared_reason_when_this_event_has_no_specific_match() {
        let event = event_with_id("innocent-bystander");
        let reason_by_resource = HashMap::from([("bad-one", "unknown plugin")]);
        assert_eq!(reason_for(&event, &reason_by_resource, "shared reason"), "shared reason");
    }

    #[test]
    fn reason_for_falls_back_to_the_shared_reason_when_nothing_matched_at_all() {
        let event = event_with_id("any-id");
        assert_eq!(reason_for(&event, &HashMap::new(), "shared reason"), "shared reason");
    }
}
