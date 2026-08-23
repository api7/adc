//! Shared between every backend's operator/validator: converting between an
//! `Event`'s untyped JSON payload and a resource's own typed body, and the
//! "this event needed a parent it doesn't have" error every nested resource
//! type (routes, upstreams, credentials, ...) can hit the same way.

use adc_sdk::{BackendError, Event};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn deserialize_event_value<T: DeserializeOwned>(value: &Value) -> Result<T, BackendError> {
    serde_json::from_value(value.clone()).map_err(|e| BackendError::Serialization(format!("decoding event payload: {e}")))
}

pub fn to_request_body<T: Serialize>(value: T) -> Result<Value, BackendError> {
    serde_json::to_value(value).map_err(|e| BackendError::Serialization(format!("encoding request body: {e}")))
}

pub fn missing_parent(event: &Event) -> BackendError {
    BackendError::Other(format!("{:?} event for resource {:?} is missing a parent_id", event.resource_type, event.resource_id).into())
}
