//! Batch runner for `fixtures/differ/*.json`, used by the parity comparator
//! (see `scripts/compare-differ-fixtures.mjs`) to cross-check this crate's
//! output against the reference differ implementation.
//! Not part of the crate's public library API — this is dev tooling only.
//!
//! Usage: run_fixtures <fixtures-dir> [--out <file>]
//!
//! Reads every `*.json` file in the given directory (each shaped as
//! `{ "local": ..., "remote": ..., "defaultValue"?: ... }`), runs
//! `DifferV4::diff` on each, and writes a `{ "<fixture-name>": [<events>] }`
//! JSON map to stdout (or the given output file) keyed by filename without
//! its `.json` extension.

use std::collections::BTreeMap;
use std::env;
use std::fs;

use adc_differ::DifferV4;
use adc_sdk::{DefaultValue, Event, InternalConfiguration, ResourceType};
use serde_json::Value;

/// Inverse of `ResourceType::as_str()`. Not exposed by adc-sdk itself since
/// nothing in the library needs to parse a resource type back from a string.
fn resource_type_from_str(s: &str) -> Option<ResourceType> {
    Some(match s {
        "route" => ResourceType::Route,
        "service" => ResourceType::Service,
        "upstream" => ResourceType::Upstream,
        "ssl" => ResourceType::Ssl,
        "global_rule" => ResourceType::GlobalRule,
        "plugin_config" => ResourceType::PluginConfig,
        "plugin_metadata" => ResourceType::PluginMetadata,
        "consumer" => ResourceType::Consumer,
        "consumer_group" => ResourceType::ConsumerGroup,
        "consumer_credential" => ResourceType::ConsumerCredential,
        "stream_route" => ResourceType::StreamRoute,
        "stream_service" => ResourceType::InternalStreamService,
        _ => return None,
    })
}

fn parse_default_value(v: &Value) -> DefaultValue {
    let mut default_value = DefaultValue::default();
    if let Some(core) = v.get("core").and_then(Value::as_object) {
        for (k, val) in core {
            if let Some(rt) = resource_type_from_str(k) {
                default_value.core.insert(rt, val.clone());
            }
        }
    }
    if let Some(plugins) = v.get("plugins").and_then(Value::as_object) {
        for (k, val) in plugins {
            default_value.plugins.insert(k.clone(), val.clone());
        }
    }
    default_value
}

fn load_config(v: Option<&Value>) -> InternalConfiguration {
    v.and_then(Value::as_object).cloned().unwrap_or_default()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let fixtures_dir = args.get(1).expect("usage: run_fixtures <fixtures-dir> [--out <file>]");
    let out_path = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1));

    let mut entries: Vec<_> = fs::read_dir(fixtures_dir)
        .unwrap_or_else(|e| panic!("read dir {fixtures_dir}: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    entries.sort();

    let mut results: BTreeMap<String, Vec<Event>> = BTreeMap::new();
    for path in entries {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap().to_string();
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let fixture: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));

        let local = load_config(fixture.get("local"));
        let remote = load_config(fixture.get("remote"));
        let default_value = fixture.get("defaultValue").map(parse_default_value);

        let events = DifferV4::diff(&local, &remote, default_value.as_ref(), None);
        results.insert(name, events);
    }

    let json = serde_json::to_string_pretty(&results).expect("serialize results");
    match out_path {
        Some(path) => fs::write(path, json).unwrap_or_else(|e| panic!("write {path}: {e}")),
        None => println!("{json}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every ResourceType variant listed explicitly (not via ResourceType::ALL,
    // which excludes some of these) so adding a variant without a matching
    // resource_type_from_str arm fails this test instead of silently falling
    // through to None at fixture-parsing time.
    #[test]
    fn resource_type_from_str_round_trips_every_variant() {
        for rt in [
            ResourceType::Route,
            ResourceType::Service,
            ResourceType::Upstream,
            ResourceType::Ssl,
            ResourceType::GlobalRule,
            ResourceType::PluginConfig,
            ResourceType::PluginMetadata,
            ResourceType::Consumer,
            ResourceType::ConsumerGroup,
            ResourceType::ConsumerCredential,
            ResourceType::StreamRoute,
            ResourceType::InternalStreamService,
        ] {
            assert_eq!(resource_type_from_str(rt.as_str()), Some(rt));
        }
    }
}
