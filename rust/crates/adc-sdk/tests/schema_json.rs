//! Drift guard for `rust/schema.json`: fails if the committed export no
//! longer matches what `schemars` currently derives from
//! `resources::Configuration` — the same role as the TS SDK's own
//! `schema-json.spec.ts` ("should check schema.json is consistent with git
//! HEAD"). Doesn't regenerate the file; a red test here means someone
//! changed a `#[schemars(...)]`-relevant field and forgot to re-run
//! `cargo run -p adc-sdk --bin export-schema` and commit the result.

#[test]
fn schema_json_is_consistent_with_the_current_resource_model() {
    let current = schemars::schema_for!(adc_sdk::resources::Configuration);
    let current_json = serde_json::to_string_pretty(&current).expect("schema serializes to JSON") + "\n";

    let committed = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../schema.json"))
        .expect("rust/schema.json should exist — run `cargo run -p adc-sdk --bin export-schema`");

    assert_eq!(
        current_json, committed,
        "rust/schema.json is stale — re-run `cargo run -p adc-sdk --bin export-schema` from rust/ and commit the result"
    );
}
