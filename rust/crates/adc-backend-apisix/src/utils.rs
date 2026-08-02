use adc_sdk::ResourceType;

/// Maps a resource type onto its admin API collection path segment (e.g.
/// `Service` -> `services`). Plugin metadata's collection isn't pluralized.
/// `ConsumerCredential` has no such collection at all — it lives nested
/// under a specific consumer's own path, which callers build themselves
/// (see `operator::main_path`'s dedicated branch) — so it's `None` here
/// rather than a made-up path fragment a new caller could accidentally use
/// as-is.
pub fn resource_type_to_api_name(resource_type: ResourceType) -> Option<String> {
    match resource_type {
        ResourceType::PluginMetadata => Some(resource_type.as_str().to_string()),
        ResourceType::ConsumerCredential => None,
        _ => Some(format!("{}s", resource_type.as_str())),
    }
}
