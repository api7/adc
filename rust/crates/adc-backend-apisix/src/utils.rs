use adc_backend_core::resource_type_collection_name;
use adc_sdk::ResourceType;

/// `ConsumerCredential` has no top-level admin API collection at all — it
/// lives nested under a specific consumer's own path, which callers build
/// themselves (see `operator::main_path`'s dedicated branch) — so it's
/// `None` here rather than a made-up path fragment a new caller could
/// accidentally use as-is.
pub fn resource_type_to_api_name(resource_type: ResourceType) -> Option<String> {
    match resource_type {
        ResourceType::ConsumerCredential => None,
        other => Some(resource_type_collection_name(other)),
    }
}
