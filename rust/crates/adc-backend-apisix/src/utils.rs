use adc_sdk::ResourceType;

/// Maps a resource type onto its admin API collection path segment (e.g.
/// `Service` -> `services`). Two exceptions: plugin metadata's collection
/// isn't pluralized, and consumer credentials live nested under a specific
/// consumer rather than as their own top-level collection.
pub fn resource_type_to_api_name(resource_type: ResourceType) -> String {
    match resource_type {
        ResourceType::PluginMetadata => resource_type.as_str().to_string(),
        ResourceType::ConsumerCredential => "consumers/%s/credentials".to_string(),
        _ => format!("{}s", resource_type.as_str()),
    }
}
