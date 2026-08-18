//! Regenerates `rust/schema.json` from the current `resources::Configuration`
//! shape. Not part of the crate's public library API or the shipped `adc`
//! CLI — a dev-only tool, mirroring the TS SDK's own `nx run cli:export-schema`
//! (`apps/cli/src/linter/exporter.ts`), which is likewise a standalone
//! script rather than an `adc` subcommand.
//!
//! Usage: `cargo run -p adc-sdk --bin export-schema`, from the workspace
//! root — the output path is anchored to this crate's manifest dir (same
//! convention as `tests/schema_json.rs`'s drift check), not the cwd.

const SCHEMA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schema.json");

fn main() {
    let schema = schemars::schema_for!(adc_sdk::resources::Configuration);
    let json = serde_json::to_string_pretty(&schema).expect("schema serializes to JSON") + "\n";
    std::fs::write(SCHEMA_PATH, json).expect("writing schema.json");
}
