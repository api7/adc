use std::collections::HashMap;

use serde_json::Value;

use crate::resource::ResourceType;

/// Per-resource-type and per-plugin default values, merged into local
/// configuration before diffing so that a value matching the backend's
/// default doesn't show up as a spurious change. Fetched from the backend
/// (`Backend::default_value`) and fed into the differ.
#[derive(Debug, Clone, Default)]
pub struct DefaultValue {
    pub core: HashMap<ResourceType, Value>,
    pub plugins: HashMap<String, Value>,
}
