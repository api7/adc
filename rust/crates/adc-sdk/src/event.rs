use serde::Serialize;
use serde_json::Value;

use crate::resource::ResourceType;
use crate::value_diff::ValueDiff;

/// The kind of change a differ event represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Create,
    Delete,
    Update,
    /// Internal use only, the backend does not need to handle such event type
    OnlySubEvents,
}

/// A single detected change between local and remote configuration for one resource.
///
/// `sub_events` is populated while the differ is building nested events and is
/// always cleared before the final flattened list is returned from
/// `DifferV4::diff`, so it is not part of the crate's public "result" contract
/// even though it stays on the struct.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub resource_type: ResourceType,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub resource_id: String,
    pub resource_name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Vec<ValueDiff>>,

    /// for nested events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip)]
    pub sub_events: Vec<Event>,
}

impl Event {
    pub fn new(resource_type: ResourceType, event_type: EventType, resource_id: impl Into<String>, resource_name: impl Into<String>) -> Self {
        Self {
            resource_type,
            event_type,
            resource_id: resource_id.into(),
            resource_name: resource_name.into(),
            old_value: None,
            new_value: None,
            diff: None,
            parent_id: None,
            sub_events: Vec::new(),
        }
    }
}
