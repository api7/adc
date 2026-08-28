use adc_sdk::ResourceType;

/// The admin API collection path segment for a resource type (e.g.
/// `Service` -> `"services"`) — plain pluralization of its snake_case name,
/// except plugin metadata's collection, which isn't pluralized.
///
/// Purely mechanical: it says nothing about whether a given backend
/// actually exposes that collection at the top level. A resource type that
/// lives nested under its parent's own path instead (a consumer's
/// credentials, an API7 service's named upstreams, ...) still gets a
/// segment from this function — it's each backend's own path-building code
/// that decides which resource types to route elsewhere before ever
/// calling this.
pub fn resource_type_collection_name(resource_type: ResourceType) -> String {
    match resource_type {
        ResourceType::PluginMetadata => resource_type.to_string(),
        _ => format!("{resource_type}s"),
    }
}
