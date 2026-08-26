use adc_backend_core::resource_type_collection_name;
use adc_sdk::ResourceType;

/// `Upstream` and `ConsumerCredential` have no top-level admin API
/// collection at all — they live nested under their parent's own path,
/// which callers build themselves (see `operator::build_path`) — so
/// they're `None` here rather than a made-up path fragment a new caller
/// could accidentally use as-is.
pub fn resource_type_to_api_name(resource_type: ResourceType) -> Option<String> {
    match resource_type {
        ResourceType::Upstream | ResourceType::ConsumerCredential => None,
        other => Some(resource_type_collection_name(other)),
    }
}
