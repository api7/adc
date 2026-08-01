/// Mirrors the `FieldMeta` union in `libs/sdk/src/core/field-registry.ts`.
///
/// In TS this is attached to a Zod schema field via `.meta()`/`withDifferMeta()`
/// and later read back with `readFieldMeta()`. Since this Rust port skips the
/// full Zod-equivalent schema layer (see adc-sdk crate docs), the per-resource
/// field tables are hand-written directly in `differ_meta.rs` instead of being
/// derived from a schema at runtime — the source of truth (schema.ts) is still
/// the same, just read manually rather than reflected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldMeta {
    Map {
        #[allow(dead_code)] // kept for parity with TS; identity is handled by ResourceDifferMeta::get_name/generate_id instead
        list_map_key: &'static str,
        nested: bool,
        /// Key under which the nested collection appears in `InternalConfiguration`,
        /// if different from the field name (e.g. consumer.credentials -> consumer_credentials).
        config_key: Option<&'static str>,
    },
    ObjectMap,
    Atomic {
        #[allow(dead_code)]
        strip: bool,
    },
    Array {
        strip_item_fields: &'static [&'static str],
    },
}
