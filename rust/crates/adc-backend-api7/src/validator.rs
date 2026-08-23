//! Pre-flight validation against API7's `/apisix/admin/configs/validate`
//! endpoint: batches every create/update event's wire body by resource
//! type and asks the dashboard to check it without actually applying
//! anything, then maps any reported errors back onto the `Event`s that
//! produced them.
//!
//! Unlike `adc_backend_apisix::Validator`, a version too old to support
//! this endpoint at all is rejected by a client-side check before any
//! request is made, rather than by interpreting a 404 response.

use std::collections::HashMap;

use adc_backend_core::{HttpClient, Method};
use adc_sdk::resources::{self as adc};
use adc_sdk::{
    BackendError, BackendValidateResult, BackendValidationError, Event, EventType, ResourceType,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::transformer;
use crate::typing;

const MINIMUM_VALIDATE_VERSION: Version = Version::new(3, 9, 10);

pub struct Validator {
    client: HttpClient,
    version: Version,
    gateway_group_id: Option<String>,
}

/// One entry per group API7's validate endpoint recognizes — deliberately
/// not every `ResourceType`: consumer credentials, consumer groups, plugin
/// configs and standalone upstream events never appear in this payload
/// (unlike `adc_backend_apisix`'s validator, there's no `upstreams` group
/// at all here — a service's default upstream travels embedded in its own
/// body).
#[derive(Debug, Default, Serialize)]
struct ValidateRequestBody {
    routes: Vec<typing::Route>,
    services: Vec<typing::Service>,
    consumers: Vec<typing::Consumer>,
    ssls: Vec<typing::Ssl>,
    global_rules: Vec<typing::GlobalRule>,
    stream_routes: Vec<typing::StreamRoute>,
    plugin_metadata: Vec<Value>,
}

/// Per group, the `(resource_name, Event)` that produced each entry, in the
/// same order they were pushed — the validate response reports errors by
/// `(resource_type, index)`, and this is what turns that back into a name
/// and an `Event` for `BackendValidationError`.
type ValidateIndex = HashMap<&'static str, Vec<(String, Event)>>;

#[derive(Debug, Deserialize)]
struct ValidateErrorResponse {
    error_msg: Option<String>,
    #[serde(default)]
    errors: Vec<RawValidationError>,
}

#[derive(Debug, Deserialize)]
struct RawValidationError {
    resource_type: String,
    resource_id: Option<String>,
    index: usize,
    error: String,
}

impl Validator {
    pub fn new(client: HttpClient, version: Version, gateway_group_id: Option<String>) -> Self {
        Self {
            client,
            version,
            gateway_group_id,
        }
    }

    pub async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        if self.version < MINIMUM_VALIDATE_VERSION {
            return Err(BackendError::Unsupported(format!(
                "validate is not supported by the current backend version ({}). Please upgrade to a newer version.",
                self.version
            )));
        }

        let (body, index) = build_request(events)?;
        let mut request = self
            .client
            .request(Method::POST, "/apisix/admin/configs/validate")?
            .json(&body);
        if let Some(id) = &self.gateway_group_id {
            request = request.query(&[("gateway_group_id", id)]);
        }
        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200..=299 => Ok(BackendValidateResult {
                success: true,
                error_message: None,
                errors: vec![],
            }),
            400 => {
                let payload: ValidateErrorResponse = response.json().await.map_err(|e| {
                    BackendError::Serialization(format!("decoding validate error response: {e}"))
                })?;
                let errors = payload
                    .errors
                    .into_iter()
                    .map(|raw| enrich(raw, &index))
                    .collect();
                Ok(BackendValidateResult {
                    success: false,
                    error_message: payload.error_msg,
                    errors,
                })
            }
            status => match HttpClient::require_success(response).await {
                Err(error) => Err(error),
                // `require_success` only accepts a 2xx status, and this arm
                // is only reached for one that's neither 2xx nor 400 — so
                // this is unreachable in practice, but a descriptive error
                // beats a panic if that ever stops being true.
                Ok(_) => Err(BackendError::Other(
                    format!("unexpected successful response with status {status} from validate").into(),
                )),
            },
        }
    }
}

fn enrich(raw: RawValidationError, index: &ValidateIndex) -> BackendValidationError {
    let matched = index
        .get(raw.resource_type.as_str())
        .and_then(|group| group.get(raw.index));
    BackendValidationError {
        resource_type: raw.resource_type,
        resource_id: raw.resource_id,
        resource_name: matched.map(|(name, _)| name.clone()),
        index: raw.index,
        error: raw.error,
        event: matched.map(|(_, event)| event.clone()),
    }
}

fn missing_parent(event: &Event) -> BackendError {
    BackendError::Other(
        format!(
            "{:?} event for resource {:?} is missing a parent_id",
            event.resource_type, event.resource_id
        )
        .into(),
    )
}

fn deserialize_event_value<T: serde::de::DeserializeOwned>(
    value: &Value,
) -> Result<T, BackendError> {
    serde_json::from_value(value.clone())
        .map_err(|e| BackendError::Serialization(format!("decoding event payload: {e}")))
}

fn build_request(events: &[Event]) -> Result<(ValidateRequestBody, ValidateIndex), BackendError> {
    let mut body = ValidateRequestBody::default();
    let mut index: ValidateIndex = HashMap::new();

    for event in events {
        if !matches!(event.event_type(), EventType::Create | EventType::Update) {
            continue;
        }
        let new_value = event
            .kind
            .new_value()
            .ok_or_else(|| BackendError::Other("create/update event missing new_value".into()))?;
        let track = |index: &mut ValidateIndex, group: &'static str| {
            index
                .entry(group)
                .or_default()
                .push((event.resource_name.clone(), event.clone()));
        };

        match event.resource_type {
            ResourceType::Service => {
                let mut service: adc::Service = deserialize_event_value(new_value)?;
                service.id = Some(event.resource_id.clone());
                body.services.push(typing::Service::from(service));
                track(&mut index, "services");
            }
            ResourceType::Route => {
                let mut route: adc::Route = deserialize_event_value(new_value)?;
                route.id = Some(event.resource_id.clone());
                let parent_id = event
                    .parent_id
                    .clone()
                    .ok_or_else(|| missing_parent(event))?;
                body.routes
                    .push(transformer::transform_route(route, parent_id));
                track(&mut index, "routes");
            }
            ResourceType::StreamRoute => {
                let mut route: adc::StreamRoute = deserialize_event_value(new_value)?;
                route.id = Some(event.resource_id.clone());
                let parent_id = event
                    .parent_id
                    .clone()
                    .ok_or_else(|| missing_parent(event))?;
                body.stream_routes
                    .push(transformer::transform_stream_route(route, parent_id));
                track(&mut index, "stream_routes");
            }
            ResourceType::Consumer => {
                let consumer: adc::Consumer = deserialize_event_value(new_value)?;
                body.consumers.push(typing::Consumer::from(consumer));
                track(&mut index, "consumers");
            }
            ResourceType::Ssl => {
                let mut ssl: adc::SSL = deserialize_event_value(new_value)?;
                ssl.id = Some(event.resource_id.clone());
                body.ssls
                    .push(typing::Ssl::try_from(ssl).map_err(BackendError::Serialization)?);
                track(&mut index, "ssls");
            }
            ResourceType::GlobalRule => {
                let mut plugins = adc::Plugins::new();
                plugins.insert(event.resource_id.clone(), new_value.clone());
                body.global_rules.push(typing::GlobalRule { plugins });
                track(&mut index, "global_rules");
            }
            ResourceType::PluginMetadata => {
                let mut value = new_value.clone();
                if let Value::Object(map) = &mut value {
                    map.insert("id".to_string(), Value::String(event.resource_id.clone()));
                }
                body.plugin_metadata.push(value);
                track(&mut index, "plugin_metadata");
            }
            ResourceType::ConsumerCredential
            | ResourceType::ConsumerGroup
            | ResourceType::Upstream
            | ResourceType::InternalStreamService => {
                // Not part of API7's validate payload — see
                // `ValidateRequestBody`'s doc comment.
            }
        }
    }

    Ok((body, index))
}

#[cfg(test)]
mod tests {
    use adc_sdk::EventKind;
    use serde_json::json;

    use super::*;

    #[test]
    fn enrich_matches_a_known_resource_type_and_index() {
        let event = Event::new(
            ResourceType::Route,
            EventKind::Create {
                new_value: json!({}),
            },
            "route-1",
            "route-1",
        );
        let mut index: ValidateIndex = HashMap::new();
        index.insert("routes", vec![("get-anything".to_string(), event.clone())]);

        let raw = RawValidationError {
            resource_type: "routes".to_string(),
            resource_id: None,
            index: 0,
            error: "bad route".to_string(),
        };
        let result = enrich(raw, &index);

        assert_eq!(result.resource_type, "routes");
        assert_eq!(result.resource_name.as_deref(), Some("get-anything"));
        assert_eq!(result.event, Some(event));
    }

    #[test]
    fn enrich_handles_an_unrecognized_resource_type_without_panicking() {
        let index: ValidateIndex = HashMap::new();
        let raw = RawValidationError {
            resource_type: "unknown_type".to_string(),
            resource_id: None,
            index: 0,
            error: "some error".to_string(),
        };

        let result = enrich(raw, &index);

        assert_eq!(result.resource_type, "unknown_type");
        assert!(result.resource_name.is_none());
        assert!(result.event.is_none());
    }

    #[test]
    fn build_request_skips_delete_events() {
        let delete_event = Event::new(
            ResourceType::Route,
            EventKind::Delete {
                old_value: json!({}),
            },
            "route-1",
            "route-1",
        );
        let (body, index) = build_request(&[delete_event]).unwrap();
        assert!(body.routes.is_empty());
        assert!(index.is_empty());
    }

    #[test]
    fn build_request_omits_resource_types_with_no_validate_group() {
        let mut credential_event = Event::new(
            ResourceType::ConsumerCredential,
            EventKind::Create {
                new_value: json!({}),
            },
            "cred-1",
            "cred-1",
        );
        credential_event.parent_id = Some("user1".to_string());
        let (body, index) = build_request(&[credential_event]).unwrap();
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            json!({"routes": [], "services": [], "consumers": [], "ssls": [], "global_rules": [], "stream_routes": [], "plugin_metadata": []})
        );
        assert!(index.is_empty());
    }

    #[test]
    fn build_request_stamps_a_plugin_metadata_events_resource_id_as_its_id() {
        let event = Event::new(
            ResourceType::PluginMetadata,
            EventKind::Create {
                new_value: json!({ "log_format": {} }),
            },
            "http-logger",
            "http-logger",
        );
        let (body, _index) = build_request(&[event]).unwrap();
        assert_eq!(body.plugin_metadata[0]["id"], "http-logger");
        assert_eq!(body.plugin_metadata[0]["log_format"], json!({}));
    }
}
