//! Pre-flight validation against APISIX's `/apisix/admin/configs/validate`
//! endpoint: batches every create/update event's wire body by resource type
//! and asks APISIX to check it without actually applying anything, then maps
//! any reported errors back onto the `Event`s that produced them.

use std::collections::HashMap;

use adc_backend_core::{HttpClient, Method};
use adc_sdk::resources::{self as adc};
use adc_sdk::{
    BackendError, BackendValidateResult, BackendValidationError, Event, EventType, ResourceType,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::transformer;
use crate::typing;

pub struct Validator {
    client: HttpClient,
}

/// One entry per group APISIX's validate endpoint recognizes
/// (`routes`/`services`/`consumers`/`ssls`/`global_rules`/`stream_routes`/
/// `plugin_metadata`/`upstreams`) — deliberately not every `ResourceType`:
/// consumer credentials, consumer groups, plugin configs and standalone
/// upstream events never appear in this payload, matching the TS
/// validator's own `switch` (no case for them, nothing pushed).
#[derive(Debug, Default, Serialize)]
struct ValidateRequestBody {
    routes: Vec<typing::Route>,
    services: Vec<typing::Service>,
    consumers: Vec<typing::Consumer>,
    ssls: Vec<typing::Ssl>,
    global_rules: Vec<typing::GlobalRule>,
    stream_routes: Vec<typing::StreamRoute>,
    plugin_metadata: Vec<Value>,
    upstreams: Vec<typing::Upstream>,
}

/// Per group, the `(resource_name, Event)` that produced each entry, in the
/// same order they were pushed — APISIX's validate response reports errors
/// by `(resource_type, index)`, and this is what turns that back into a
/// name and an `Event` for `BackendValidationError`.
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
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn validate(&self, events: &[Event]) -> Result<BackendValidateResult, BackendError> {
        let (body, index) = build_request(events)?;

        let request = self
            .client
            .request(Method::POST, "/apisix/admin/configs/validate")?
            .json(&body);
        let response = self.client.execute(request).await?;

        match response.status().as_u16() {
            200..=299 => Ok(BackendValidateResult { success: true, error_message: None, errors: vec![] }),
            404 => Err(BackendError::Unsupported(
                "validate is not supported by this APISIX version; please upgrade to a newer version".to_string(),
            )),
            400 => {
                let payload: ValidateErrorResponse = response
                    .json()
                    .await
                    .map_err(|e| BackendError::Serialization(format!("decoding validate error response: {e}")))?;
                let errors = payload.errors.into_iter().map(|raw| enrich(raw, &index)).collect();
                Ok(BackendValidateResult { success: false, error_message: payload.error_msg, errors })
            }
            _ => Err(HttpClient::require_success(response).await.unwrap_err()),
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
                let (wire_service, wire_upstream) = transformer::transform_service(service);
                body.services.push(wire_service);
                track(&mut index, "services");
                if let Some(upstream) = wire_upstream {
                    body.upstreams.push(upstream);
                    track(&mut index, "upstreams");
                }
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
                    .push(transformer::transform_stream_route(route, parent_id, true));
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
                body.ssls.push(typing::Ssl::from(ssl));
                track(&mut index, "ssls");
            }
            ResourceType::GlobalRule => {
                let mut plugins = adc::Plugins::new();
                plugins.insert(event.resource_id.clone(), new_value.clone());
                body.global_rules.push(typing::GlobalRule {
                    id: event.resource_id.clone(),
                    plugins,
                });
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
            | ResourceType::PluginConfig
            | ResourceType::Upstream
            | ResourceType::InternalStreamService => {
                // Not part of APISIX's validate payload — matches the TS
                // validator's `switch`, which has no case for these either.
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
    fn enrich_handles_an_out_of_range_index_without_panicking() {
        let event = Event::new(
            ResourceType::Route,
            EventKind::Create {
                new_value: json!({}),
            },
            "route-1",
            "route-1",
        );
        let mut index: ValidateIndex = HashMap::new();
        index.insert("routes", vec![("get-anything".to_string(), event)]);

        let raw = RawValidationError {
            resource_type: "routes".to_string(),
            resource_id: None,
            index: 5,
            error: "bad route".to_string(),
        };
        let result = enrich(raw, &index);

        assert!(result.resource_name.is_none());
        assert!(result.event.is_none());
    }
}
