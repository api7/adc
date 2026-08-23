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
use adc_sdk::resources::FlatConfiguration;
use adc_sdk::{DefaultValue, Event, ResourceType};
use serde_json::Value;

fn parse_default_value(v: &Value, context: &str) -> DefaultValue {
    let mut default_value = DefaultValue::default();
    if let Some(core) = v.get("core").and_then(Value::as_object) {
        for (k, val) in core {
            let rt: ResourceType = k
                .parse()
                .unwrap_or_else(|_| panic!("{context}: unknown resource type {k:?} in defaultValue.core"));
            default_value.core.insert(rt, val.clone());
        }
    }
    if let Some(plugins) = v.get("plugins").and_then(Value::as_object) {
        for (k, val) in plugins {
            default_value.plugins.insert(k.clone(), val.clone());
        }
    }
    default_value
}

/// `None` (the key is absent) is a legitimate empty config; `Some` holding
/// anything that doesn't deserialize as a `FlatConfiguration` (wrong shape,
/// unknown field, non-object) is very likely a malformed fixture and panics
/// instead of silently running the differ against an empty config as if
/// nothing were wrong.
fn load_config(v: Option<&Value>, context: &str) -> FlatConfiguration {
    match v {
        None => FlatConfiguration::default(),
        Some(value) => {
            serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{context}: {e}"))
        }
    }
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

        let local = load_config(fixture.get("local"), &format!("{}: \"local\"", path.display()));
        let remote = load_config(fixture.get("remote"), &format!("{}: \"remote\"", path.display()));
        let default_value = fixture
            .get("defaultValue")
            .map(|v| parse_default_value(v, &format!("{}: \"defaultValue\"", path.display())));

        let events = DifferV4::diff(&local, &remote, default_value.as_ref());
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

    #[test]
    fn load_config_treats_a_missing_key_as_an_empty_config() {
        assert_eq!(load_config(None, "ctx"), FlatConfiguration::default());
    }

    #[test]
    fn load_config_accepts_an_object() {
        let value = serde_json::json!({"services": []});
        assert_eq!(load_config(Some(&value), "ctx"), FlatConfiguration { services: Some(vec![]), ..Default::default() });
    }

    #[test]
    #[should_panic(expected = "invalid type")]
    fn load_config_rejects_a_non_object_instead_of_silently_defaulting() {
        let value = serde_json::json!("not an object");
        load_config(Some(&value), "ctx");
    }

    #[test]
    fn parse_default_value_reads_known_core_and_plugin_entries() {
        let value = serde_json::json!({
            "core": {"route": {"a": 1}},
            "plugins": {"cors": {"b": 2}},
        });
        let default_value = parse_default_value(&value, "ctx");
        assert_eq!(default_value.core.get(&ResourceType::Route), Some(&serde_json::json!({"a": 1})));
        assert_eq!(default_value.plugins.get("cors"), Some(&serde_json::json!({"b": 2})));
    }

    #[test]
    #[should_panic(expected = "unknown resource type")]
    fn parse_default_value_rejects_an_unknown_core_key_instead_of_silently_dropping_it() {
        let value = serde_json::json!({"core": {"raute": {}}});
        parse_default_value(&value, "ctx");
    }
}
