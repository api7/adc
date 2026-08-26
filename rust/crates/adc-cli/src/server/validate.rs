//! `PUT /validate`: lint (optional) + backend-side validation of the
//! events that would be produced against an *empty* remote config.

use adc_sdk::resources::Configuration;
use adc_sdk::BackendValidateResult;
use axum::Json;
use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::schema::{self, ValidateInput};
use super::{backend, bad_request, internal_error};
use crate::config;
use crate::pipeline;

fn empty_configuration() -> Configuration {
    Configuration {
        services: None,
        ssls: None,
        consumers: None,
        consumer_groups: None,
        global_rules: None,
        plugin_metadata: None,
    }
}

pub async fn validate_handler(body: Bytes) -> Response {
    let input: ValidateInput = match serde_json::from_slice(&body) {
        Ok(input) => input,
        Err(error) => {
            return bad_request(json!({"success": false, "source": "input", "message": error.to_string(), "errors": []}));
        }
    };
    let opts = input.task.opts;

    let mut issues = schema::validate_server_addr(&opts);
    issues.extend(schema::validate_tls_material(&opts));
    if !issues.is_empty() {
        return bad_request(json!({
            "success": false, "source": "input",
            "message": "invalid request", "errors": issues,
        }));
    }

    let label_selector = opts.label_selector_or_default();
    let mut config_value = input.task.config;
    config::fill_labels(&mut config_value, &label_selector);

    let mut configuration: Configuration = match serde_json::from_value(config_value) {
        Ok(configuration) => configuration,
        Err(error) => {
            return bad_request(json!({
                "success": false, "source": "input",
                "message": format!("invalid configuration: {error}"), "errors": [],
            }));
        }
    };

    let (include, exclude) = opts.resource_type_sets();
    config::filter_resource_types(&mut configuration, &include, &exclude);

    if opts.lint {
        let issues = adc_sdk::lint::lint(&configuration);
        if !issues.is_empty() {
            return bad_request(json!({
                "success": false, "source": "lint",
                "message": "Lint configuration\nThe following errors were found in configuration:",
                "errors": issues.iter().map(|issue| json!({
                    "path": issue.path.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "message": issue.message,
                })).collect::<Vec<_>>(),
            }));
        }
    }

    let gateway = match backend::build_backend(&opts) {
        Ok(gateway) => gateway,
        Err(error) => return internal_error(json!({"success": false, "message": error.to_string(), "errors": []})),
    };

    let events = match pipeline::diff(gateway.as_ref(), &configuration, &empty_configuration()).await {
        Ok(events) => events,
        Err(error) => return internal_error(json!({"success": false, "message": error.to_string(), "errors": []})),
    };

    match gateway.validate(&events).await {
        Ok(BackendValidateResult { success, error_message, errors }) => {
            let mut body = json!({"success": success, "source": "validate", "errors": errors_json(&errors)});
            if let Some(message) = error_message {
                body["message"] = json!(message);
            }
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(adc_sdk::BackendError::Unsupported(_)) => bad_request(json!({
            "success": false, "source": "validate",
            "message": "Validate is not supported by the current backend.", "errors": [],
        })),
        Err(error) => internal_error(json!({"success": false, "message": error.to_string(), "errors": []})),
    }
}

fn errors_json(errors: &[adc_sdk::BackendValidationError]) -> serde_json::Value {
    json!(
        errors
            .iter()
            .map(|e| json!({
                "resource_type": e.resource_type,
                "resource_id": e.resource_id,
                "resource_name": e.resource_name,
                "index": e.index,
                "error": e.error,
            }))
            .collect::<Vec<_>>()
    )
}
