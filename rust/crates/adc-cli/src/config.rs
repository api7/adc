//! `serde_json::Value`-level operations on a loaded configuration: merging
//! multiple files, stamping the `managed-by` label, and stripping `id`
//! fields before a `dump`. These stay at the `Value` level (rather than
//! walking `adc_sdk::resources::Configuration`'s typed fields) because the
//! shape they touch — "every top-level array, plus a resource-type-specific
//! set of nested arrays" — is generic across resource types; a typed walk
//! would just be this same table hand-duplicated per struct.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use adc_sdk::ResourceType;
use adc_sdk::resources::Configuration;
use serde_json::Value;

use crate::error::CliError;

const ARRAY_KEYS: &[&str] = &["services", "ssls", "consumers", "consumer_groups"];
const MAP_KEYS: &[&str] = &["global_rules", "plugin_metadata"];

const MANAGED_BY_LABEL_KEY: &str = "managed-by";
const MANAGED_BY_LABEL_VALUE: &str = "adc";

/// Expands glob patterns (defaulting to `adc.yaml` when none are given) and
/// reads + parses each matched file (YAML, or JSON when the extension says
/// so) into a raw `Value`, paired with its source path for error messages.
pub async fn read_files(patterns: &[PathBuf]) -> Result<Vec<(PathBuf, Value)>, CliError> {
    let patterns: Vec<PathBuf> = if patterns.is_empty() {
        vec![PathBuf::from("adc.yaml")]
    } else {
        patterns.to_vec()
    };

    let mut paths = Vec::new();
    for pattern in &patterns {
        // glob's own directory walk is sync (no async equivalent in the
        // ecosystem worth pulling in for this); only the per-file read below
        // goes through tokio.
        let matched: Vec<PathBuf> = glob::glob(&pattern.to_string_lossy())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CliError::msg(format!("{}: {e}", pattern.display())))?
            .into_iter()
            .filter(|p| p.is_file())
            .collect();
        if matched.is_empty() {
            if pattern.is_file() {
                paths.push(pattern.clone());
            } else {
                return Err(CliError::msg(format!(
                    "file {} does not exist",
                    pattern.display()
                )));
            }
        } else {
            paths.extend(matched);
        }
    }

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let value = read_file(&path).await?;
        files.push((path, value));
    }
    Ok(files)
}

async fn read_file(path: &Path) -> Result<Value, CliError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?;
    let is_json = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if is_json {
        serde_json::from_str(&content)
            .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))
    } else {
        serde_yaml_ng::from_str(&content)
            .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))
    }
}

/// Merges the raw configurations parsed from each file into one, rejecting a
/// duplicate resource name/username/SNIs-set across files (rather than
/// silently letting the later file win) and an unrecognized top-level key.
pub fn merge_files(files: Vec<(PathBuf, Value)>) -> Result<Value, CliError> {
    let mut merged = serde_json::Map::new();
    let mut seen_keys: HashMap<&'static str, HashSet<String>> = HashMap::new();

    for (path, value) in files {
        let Value::Object(obj) = value else {
            return Err(CliError::msg(format!(
                "{}: configuration must be a YAML/JSON object",
                path.display()
            )));
        };

        for (key, val) in obj {
            if let Some(&array_key) = ARRAY_KEYS.iter().find(|k| **k == key) {
                let Some(items) = val.as_array().cloned() else {
                    return Err(CliError::msg(format!(
                        "{}: \"{key}\" must be an array",
                        path.display()
                    )));
                };
                let entry = merged
                    .entry(array_key)
                    .or_insert_with(|| Value::Array(vec![]));
                let Value::Array(entry) = entry else {
                    unreachable!()
                };
                let seen = seen_keys.entry(array_key).or_default();
                for item in items {
                    let name = resource_key(array_key, &item);
                    if !seen.insert(name.clone()) {
                        return Err(CliError::msg(format!(
                            "{}: duplicate {} \"{name}\"",
                            path.display(),
                            singular(array_key)
                        )));
                    }
                    entry.push(item);
                }
            } else if let Some(&map_key) = MAP_KEYS.iter().find(|k| **k == key) {
                let Some(items) = val.as_object().cloned() else {
                    return Err(CliError::msg(format!(
                        "{}: \"{key}\" must be an object",
                        path.display()
                    )));
                };
                let entry = merged
                    .entry(map_key)
                    .or_insert_with(|| Value::Object(Default::default()));
                let Value::Object(entry) = entry else {
                    unreachable!()
                };
                for (name, item) in items {
                    if entry.contains_key(&name) {
                        return Err(CliError::msg(format!(
                            "{}: duplicate {} \"{name}\"",
                            path.display(),
                            singular(map_key)
                        )));
                    }
                    entry.insert(name, item);
                }
            } else {
                return Err(CliError::msg(format!(
                    "{}: configuration contains an unknown key \"{key}\"",
                    path.display()
                )));
            }
        }
    }

    Ok(Value::Object(merged))
}

fn resource_key(array_key: &str, item: &Value) -> String {
    match array_key {
        "ssls" => item
            .get("snis")
            .and_then(Value::as_array)
            .map(|snis| {
                let mut snis: Vec<&str> = snis.iter().filter_map(Value::as_str).collect();
                snis.sort_unstable();
                snis.join(",")
            })
            .unwrap_or_default(),
        "consumers" => item
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn singular(array_key: &'static str) -> &'static str {
    match array_key {
        "services" => "service",
        "ssls" => "ssl",
        "consumers" => "consumer",
        "consumer_groups" => "consumer_group",
        _ => array_key,
    }
}

/// Stamps `managed-by: adc` onto every resource's `labels`, including the
/// nested spots a resource can be authored under (`services[].routes`,
/// `services[].stream_routes`, `consumer_groups[].consumers`) — mirrors the
/// TS CLI's `fillLabels` scope exactly, including its gaps (it does not
/// reach `services[].upstreams` or `consumers[].credentials`).
pub fn inject_managed_by_label(config: &mut Value) {
    let Value::Object(root) = config else { return };
    for key in ["services", "ssls", "consumers", "consumer_groups"] {
        let Some(Value::Array(items)) = root.get_mut(key) else {
            continue;
        };
        for item in items.iter_mut() {
            set_label(item, MANAGED_BY_LABEL_KEY, MANAGED_BY_LABEL_VALUE);
            let nested_keys: &[&str] = match key {
                "services" => &["routes", "stream_routes"],
                "consumer_groups" => &["consumers"],
                _ => &[],
            };
            for nested_key in nested_keys {
                if let Some(Value::Array(nested)) = item.get_mut(*nested_key) {
                    for nested_item in nested.iter_mut() {
                        set_label(nested_item, MANAGED_BY_LABEL_KEY, MANAGED_BY_LABEL_VALUE);
                    }
                }
            }
        }
    }
}

fn set_label(item: &mut Value, key: &str, value: &str) {
    let Value::Object(obj) = item else { return };
    let labels = obj
        .entry("labels")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Value::Object(labels) = labels {
        labels.insert(key.to_string(), Value::String(value.to_string()));
    }
}

/// Strips `id` fields before writing a `dump` (unless `--with-id` was
/// given) — mirrors the TS CLI's `recursiveRemoveIdField` scope, which is
/// wider than `inject_managed_by_label`'s: it also reaches
/// `services[].upstreams` and `consumers[].credentials`.
pub fn strip_ids(config: &mut Value) {
    let Value::Object(root) = config else { return };
    for key in ["services", "ssls", "consumers", "consumer_groups"] {
        let Some(Value::Array(items)) = root.get_mut(key) else {
            continue;
        };
        for item in items.iter_mut() {
            remove_id(item);
            let nested_keys: &[&str] = match key {
                "services" => &["routes", "stream_routes", "upstreams"],
                "consumer_groups" => &["consumers"],
                "consumers" => &["credentials"],
                _ => &[],
            };
            for nested_key in nested_keys {
                if let Some(Value::Array(nested)) = item.get_mut(*nested_key) {
                    for nested_item in nested.iter_mut() {
                        remove_id(nested_item);
                    }
                }
            }
        }
    }
}

fn remove_id(item: &mut Value) {
    if let Value::Object(obj) = item {
        obj.remove("id");
    }
}

/// Drops whole top-level resource-type buckets that don't match
/// `--include-resource-type`/`--exclude-resource-type` — mirrors the TS
/// CLI's `filterResourceType`, bucket-level rather than per-item (a
/// `Configuration` has no top-level `routes`/`upstreams` key to filter on;
/// those only exist nested under `services`).
pub fn filter_resource_types(
    config: &mut Configuration,
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
) {
    if include.is_empty() && exclude.is_empty() {
        return;
    }
    let keep = |rt: ResourceType| {
        if !include.is_empty() {
            include.contains(&rt)
        } else {
            !exclude.contains(&rt)
        }
    };

    if !keep(ResourceType::Service) {
        config.services = None;
    }
    if !keep(ResourceType::Ssl) {
        config.ssls = None;
    }
    if !keep(ResourceType::Consumer) {
        config.consumers = None;
    }
    if !keep(ResourceType::ConsumerGroup) {
        config.consumer_groups = None;
    }
    if !keep(ResourceType::GlobalRule) {
        config.global_rules = None;
    }
    if !keep(ResourceType::PluginMetadata) {
        config.plugin_metadata = None;
    }
}
