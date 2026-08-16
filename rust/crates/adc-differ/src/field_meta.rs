/// Per-field merge/diff strategy: how the differ should compare and combine
/// a given resource field when computing an update event.
///
/// These strategies are hand-written into each resource's metadata table in
/// `differ_meta.rs`, independently of the field definitions in `adc_sdk::resources`
/// — the two must be kept in sync by hand when a resource's shape changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldMeta {
    Map {
        // Documents which key uniquely identifies each item in this nested
        // list, even though nothing here reads it — actual identity
        // resolution goes through ResourceDifferMeta::get_name/generate_id.
        #[allow(dead_code)]
        list_map_key: &'static str,
        nested: bool,
        /// Key under which the nested collection appears in `InternalConfiguration`,
        /// if different from the field name (e.g. consumer.credentials -> consumer_credentials).
        config_key: Option<&'static str>,
    },
    ObjectMap,
    // No resource currently declares an Atomic field — kept for completeness
    // of the merge-strategy vocabulary, not because anything constructs it.
    #[allow(dead_code)]
    Atomic {
        strip: bool,
    },
    Array {
        strip_item_fields: &'static [&'static str],
    },
}
