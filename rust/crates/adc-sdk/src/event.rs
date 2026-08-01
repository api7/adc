use serde::Serialize;
use serde_json::Value;

use crate::resource::ResourceType;
use crate::value_diff::ValueDiff;

/// Bare discriminant for [`EventKind`], carrying no payload. Useful where code only
/// needs to know which kind of event this is (sorting, filtering) without matching
/// out each variant's fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Create,
    Delete,
    Update,
    /// Internal use only, the backend does not need to handle such event type
    OnlySubEvents,
}

/// The kind of change a differ event represents, together with the payload that
/// only makes sense for that kind. Encoding this as an enum (rather than a
/// `Create`/`Delete`/`Update` tag plus a handful of `Option` fields on `Event`)
/// makes the correlation between event type and populated fields a property the
/// compiler checks: an `Update` always carries both `old_value` and `new_value`,
/// a `Delete` can never accidentally carry a `diff`, and so on.
///
/// `#[serde(tag = "type")]` keeps the wire format identical to a flat struct with
/// a `type` discriminant field: `{"type": "create", "newValue": ...}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Create {
        new_value: Value,
    },
    Delete {
        old_value: Value,
    },
    Update {
        old_value: Value,
        new_value: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        diff: Option<Vec<ValueDiff>>,
    },
    /// Internal use only: exists to carry `sub_events` up to the caller during
    /// tree construction. Never appears in the differ's final flattened event
    /// list, so it carries no payload of its own.
    OnlySubEvents,
}

impl EventKind {
    pub fn event_type(&self) -> EventType {
        match self {
            EventKind::Create { .. } => EventType::Create,
            EventKind::Delete { .. } => EventType::Delete,
            EventKind::Update { .. } => EventType::Update,
            EventKind::OnlySubEvents => EventType::OnlySubEvents,
        }
    }

    pub fn old_value(&self) -> Option<&Value> {
        match self {
            EventKind::Delete { old_value } | EventKind::Update { old_value, .. } => Some(old_value),
            EventKind::Create { .. } | EventKind::OnlySubEvents => None,
        }
    }

    pub fn new_value(&self) -> Option<&Value> {
        match self {
            EventKind::Create { new_value } | EventKind::Update { new_value, .. } => Some(new_value),
            EventKind::Delete { .. } | EventKind::OnlySubEvents => None,
        }
    }

    pub fn diff(&self) -> Option<&[ValueDiff]> {
        match self {
            EventKind::Update { diff, .. } => diff.as_deref(),
            EventKind::Create { .. } | EventKind::Delete { .. } | EventKind::OnlySubEvents => None,
        }
    }
}

/// A single detected change between local and remote configuration for one resource.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    pub resource_type: ResourceType,
    #[serde(flatten)]
    pub kind: EventKind,
    pub resource_id: String,
    pub resource_name: String,

    /// for nested events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl Event {
    pub fn new(
        resource_type: ResourceType,
        kind: EventKind,
        resource_id: impl Into<String>,
        resource_name: impl Into<String>,
    ) -> Self {
        Self {
            resource_type,
            kind,
            resource_id: resource_id.into(),
            resource_name: resource_name.into(),
            parent_id: None,
        }
    }

    pub fn event_type(&self) -> EventType {
        self.kind.event_type()
    }
}
