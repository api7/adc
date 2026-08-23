/// The kinds of resources ADC manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResourceType {
    Route,
    Service,
    Upstream,
    Ssl,
    GlobalRule,
    PluginMetadata,
    Consumer,
    ConsumerGroup,
    ConsumerCredential,
    StreamRoute,
    /// internal use only
    InternalStreamService,
}

impl ResourceType {
    pub const ALL: &'static [ResourceType] = &[
        ResourceType::Service,
        ResourceType::Ssl,
        ResourceType::Consumer,
        ResourceType::GlobalRule,
        ResourceType::PluginMetadata,
        ResourceType::Route,
        ResourceType::StreamRoute,
        ResourceType::ConsumerCredential,
        ResourceType::Upstream,
        ResourceType::ConsumerGroup,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Route => "route",
            ResourceType::Service => "service",
            ResourceType::Upstream => "upstream",
            ResourceType::Ssl => "ssl",
            ResourceType::GlobalRule => "global_rule",
            ResourceType::PluginMetadata => "plugin_metadata",
            ResourceType::Consumer => "consumer",
            ResourceType::ConsumerGroup => "consumer_group",
            ResourceType::ConsumerCredential => "consumer_credential",
            ResourceType::StreamRoute => "stream_route",
            ResourceType::InternalStreamService => "stream_service",
        }
    }
}

impl serde::Serialize for ResourceType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Merge strategies for resource fields, mirroring structured-merge-diff listType semantics.
///
/// These four variants intentionally mirror `field_meta::FieldMeta`'s four variants
/// one-to-one (`Map`/`ObjectMap`/`Atomic`/`Array`) — this enum is the public
/// vocabulary consumers see, while `FieldMeta` is the internal representation
/// carrying each variant's actual per-field data (e.g. `list_map_key`,
/// `strip_item_fields`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldListType {
    /// Array of objects, identity by a declared key field. nested=true triggers sub-event diffing.
    Map,
    /// Record<string, V> — identity by property key (e.g. plugins).
    ObjectMap,
    /// Treat the field as an opaque value; strip=true removes it before comparison.
    Atomic,
    /// Plain array whose items need individual sub-field stripping before comparison.
    Array,
}
