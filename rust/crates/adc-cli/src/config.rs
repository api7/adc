//! `serde_json::Value`-level operations on a loaded configuration: merging
//! multiple files, stamping the `managed-by` label, and stripping `id`
//! fields before a `dump`. These stay at the `Value` level (rather than
//! walking `adc_sdk::resources::Configuration`'s typed fields) because the
//! shape they touch — "every top-level array, plus a resource-type-specific
//! set of nested arrays" — is generic across resource types; a typed walk
//! would just be this same table hand-duplicated per struct.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use adc_sdk::ResourceType;
use adc_sdk::resources::Configuration;
use regex::Regex;
use serde_json::Value;

use crate::error::CliError;

const ARRAY_KEYS: &[&str] = &["services", "ssls", "consumers", "consumer_groups"];
const MAP_KEYS: &[&str] = &["global_rules", "plugin_metadata"];

pub(crate) const MANAGED_BY_LABEL_KEY: &str = "managed-by";
const MANAGED_BY_LABEL_VALUE: &str = "adc";

/// Expands glob patterns (defaulting to `adc.yaml` when none are given) and
/// reads + parses each matched file (YAML, or JSON when the extension says
/// so) into a raw `Value`, paired with its source path for error messages.
pub async fn read_files(patterns: &[PathBuf]) -> Result<Vec<(PathBuf, Value)>, CliError> {
    let paths = resolve_files(patterns, Some("adc.yaml")).await?;
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let value = read_file(&path).await?;
        files.push((path, value));
    }
    Ok(files)
}

/// Expands `-f`/`--file` glob patterns into the concrete file paths they
/// match, falling back to `default` (if given) when `patterns` is empty.
pub async fn resolve_files(patterns: &[PathBuf], default: Option<&str>) -> Result<Vec<PathBuf>, CliError> {
    let patterns: Vec<PathBuf> = if patterns.is_empty() {
        match default {
            Some(default) => vec![PathBuf::from(default)],
            None => return Ok(Vec::new()),
        }
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
    Ok(paths)
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
                    let name = resource_key(array_key, &item)
                        .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?;
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

/// Expands `${VAR}` placeholders in every string value throughout `config`
/// (recursing into arrays and object values, but never object *keys* — a
/// key like `"${GLOBAL_PLUGIN}"` stays literal) using the process
/// environment. A variable that's unset or empty expands to `""`. `\${VAR}`
/// is a literal escape: the backslash is dropped and `${VAR}` is left in
/// place, never substituted.
pub fn replace_env_vars(config: &mut Value) {
    replace_env_vars_with(config, &|name| std::env::var(name).ok());
}

fn replace_env_vars_with(config: &mut Value, lookup: &dyn Fn(&str) -> Option<String>) {
    match config {
        Value::String(s) => *s = expand_env_vars(s, lookup),
        Value::Array(items) => items.iter_mut().for_each(|item| replace_env_vars_with(item, lookup)),
        Value::Object(map) => map.values_mut().for_each(|item| replace_env_vars_with(item, lookup)),
        _ => {}
    }
}

const ESCAPED_ENV_VAR_PLACEHOLDER: &str = "__ESCAPED_ENV_VAR_PLACEHOLDER__";

static ENV_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\{([0-9A-Za-z_]+)\}").expect("valid regex"));
static ESCAPED_ENV_VAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\\$\{([0-9A-Za-z_]+)\}").expect("valid regex"));
static RESTORE_ESCAPED_ENV_VAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("{ESCAPED_ENV_VAR_PLACEHOLDER}([0-9A-Za-z_]+){ESCAPED_ENV_VAR_PLACEHOLDER}")).expect("valid regex")
});

fn expand_env_vars(value: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    // `\${VAR}` occurrences are swapped out for a placeholder first so the
    // unescaped-substitution pass below can't touch them, then restored
    // (backslash dropped) at the end.
    let protected = ESCAPED_ENV_VAR
        .replace_all(value, |caps: &regex::Captures| format!("{ESCAPED_ENV_VAR_PLACEHOLDER}{}{ESCAPED_ENV_VAR_PLACEHOLDER}", &caps[1]));
    let substituted =
        ENV_VAR.replace_all(&protected, |caps: &regex::Captures| lookup(&caps[1]).unwrap_or_default());
    RESTORE_ESCAPED_ENV_VAR.replace_all(&substituted, |caps: &regex::Captures| format!("${{{}}}", &caps[1])).into_owned()
}

/// The field this resource is deduplicated by — missing, wrong-typed, or
/// empty is rejected outright rather than defaulting to `""`, which would
/// make two differently-broken resources look like a legitimate duplicate
/// of each other instead of surfacing the real problem (a missing field).
fn resource_key(array_key: &'static str, item: &Value) -> Result<String, String> {
    match array_key {
        "ssls" => {
            let snis = item.get("snis").and_then(Value::as_array).ok_or("ssl is missing a \"snis\" array")?;
            let mut snis: Vec<&str> = snis
                .iter()
                .map(|v| v.as_str().ok_or("ssl \"snis\" must be an array of strings"))
                .collect::<Result<_, _>>()?;
            if snis.is_empty() {
                return Err("ssl \"snis\" must not be empty".to_string());
            }
            snis.sort_unstable();
            Ok(snis.join(","))
        }
        "consumers" => required_field(item, "username", "consumer"),
        _ => required_field(item, "name", singular(array_key)),
    }
}

fn required_field(item: &Value, field: &str, resource: &str) -> Result<String, String> {
    match item.get(field).and_then(Value::as_str) {
        Some(value) if !value.is_empty() => Ok(value.to_string()),
        _ => Err(format!("{resource} is missing a non-empty \"{field}\" field")),
    }
}

fn singular(array_key: &'static str) -> &'static str {
    match array_key {
        "services" => "service",
        "ssls" => "ssl",
        "consumers" => "consumer",
        "consumer_groups" => "consumer_group",
        "global_rules" => "global_rule",
        _ => array_key,
    }
}

/// Stamps `managed-by: adc` onto every resource's `labels`. Thin wrapper
/// around [`fill_labels`] for this one fixed key/value pair.
pub fn inject_managed_by_label(config: &mut Value) {
    fill_labels(
        config,
        &HashMap::from([(
            MANAGED_BY_LABEL_KEY.to_string(),
            MANAGED_BY_LABEL_VALUE.to_string(),
        )]),
    );
}

/// Merges `labels` into every resource's own `labels` map (`labels` wins on
/// key conflict, so callers can force a value), including the nested spots
/// a resource can be authored under (`services[].routes`,
/// `services[].stream_routes`, `consumer_groups[].consumers`). Does not
/// reach `services[].upstreams` or `consumers[].credentials` — neither
/// carries its own independent identity worth labeling separately from its
/// parent.
pub fn fill_labels(config: &mut Value, labels: &HashMap<String, String>) {
    if labels.is_empty() {
        return;
    }
    let Value::Object(root) = config else { return };
    for key in ARRAY_KEYS {
        let Some(Value::Array(items)) = root.get_mut(*key) else {
            continue;
        };
        for item in items.iter_mut() {
            for (label_key, label_value) in labels {
                set_label(item, label_key, label_value);
            }
            let nested_keys: &[&str] = match *key {
                "services" => &["routes", "stream_routes"],
                "consumer_groups" => &["consumers"],
                _ => &[],
            };
            for nested_key in nested_keys {
                if let Some(Value::Array(nested)) = item.get_mut(*nested_key) {
                    for nested_item in nested.iter_mut() {
                        for (label_key, label_value) in labels {
                            set_label(nested_item, label_key, label_value);
                        }
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

/// Recursively sorts object keys alphabetically, at every depth, so the
/// written YAML has a stable, diff-friendly key order instead of whatever
/// order the fields happened to be constructed/read in.
pub fn sort_keys_recursively(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            for (_, v) in &mut entries {
                sort_keys_recursively(v);
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            *map = entries.into_iter().collect();
        }
        Value::Array(items) => {
            for item in items {
                sort_keys_recursively(item);
            }
        }
        _ => {}
    }
}

/// Strips `id` fields before writing a `dump` (unless `--with-id` was
/// given). Wider in scope than [`fill_labels`]: `id` is meaningful on
/// `services[].upstreams` and `consumers[].credentials` too, even though
/// neither gets its own labels.
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
/// `--include-resource-type`/`--exclude-resource-type`. Bucket-level rather
/// than per-item: a `Configuration` has no top-level `routes`/`upstreams`
/// key to filter on, since those only exist nested under `services`.
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

/// Drops resources whose `labels` don't carry every key/value pair in
/// `labels`. Delegates to `adc_backend_core` — the same function
/// `adc_backend_apisix::Fetcher::dump()` calls on its own output as a
/// client-side backstop for its unreliable server-side `labels[key]=value`
/// query filter. This call is what makes label filtering apply to `api7ee`
/// too, whose fetcher does no client-side re-check of its own.
pub fn filter_by_labels(config: &mut Configuration, labels: &HashMap<String, String>) {
    adc_backend_core::filter_configuration_by_labels(config, labels);
}

#[cfg(test)]
mod tests {
    use super::*;
    use adc_sdk::resources::{Consumer, ConsumerGroup, LabelValue, Labels, SSL, Service};
    use serde_json::json;

    #[test]
    fn sort_keys_recursively_sorts_every_object_at_every_depth() {
        let mut value = json!({"b": 1, "a": {"z": 1, "y": 2}, "c": [{"b": 1, "a": 2}]});
        sort_keys_recursively(&mut value);
        let Value::Object(root) = &value else { unreachable!() };
        assert_eq!(root.keys().collect::<Vec<_>>(), vec!["a", "b", "c"]);
        let Value::Object(nested) = &root["a"] else { unreachable!() };
        assert_eq!(nested.keys().collect::<Vec<_>>(), vec!["y", "z"]);
        let Value::Object(in_array) = &root["c"][0] else { unreachable!() };
        assert_eq!(in_array.keys().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    /// `sort_keys_recursively` only reorders the in-memory `Map`'s own
    /// iteration order — this proves that guarantee actually survives all
    /// the way through to the written-out string, for both serializers
    /// this CLI uses (`sort_keys_recursively`'s callers write YAML, but
    /// `cmd_dump`/`cmd_convert` both go through `serde_json::to_value`
    /// first, so a JSON regression here would matter too).
    #[test]
    fn the_sorted_order_survives_serialization_to_yaml_and_json() {
        let mut value = json!({"zebra": 1, "apple": {"z": 1, "a": 2}, "mango": [{"y": 1, "b": 2}]});
        sort_keys_recursively(&mut value);

        let yaml = serde_yaml_ng::to_string(&value).unwrap();
        assert_eq!(yaml, "apple:\n  a: 2\n  z: 1\nmango:\n- b: 2\n  y: 1\nzebra: 1\n");

        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"apple":{"a":2,"z":1},"mango":[{"b":2,"y":1}],"zebra":1}"#);
    }

    #[test]
    fn replace_env_vars_expands_placeholders_throughout_nested_arrays_and_objects_but_not_object_keys() {
        let lookup = |name: &str| match name {
            "NAME" => Some("name".to_string()),
            "SECRET" => Some("secret".to_string()),
            "CERT" | "KEY" => Some("-----".to_string()),
            "NOTE" => Some("note".to_string()),
            _ => None,
        };
        let mut config = json!({
            "services": [
                { "name": "Test ${NAME}", "routes": [{ "name": "Test ${NAME}", "uris": ["/test/${NAME}"] }] },
                { "name": "Test escape \\${NAME}" },
            ],
            "consumers": [{ "username": "TEST_${NAME}", "plugins": { "key-auth": { "key": "${SECRET}" } } }],
            "ssls": [{ "snis": ["test.com"], "certificates": [{ "certificate": "${CERT}", "key": "${KEY}" }] }],
            "global_rules": { "${GLOBAL_PLUGIN}": { "key": "${SECRET}" } },
            "plugin_metadata": { "file-logger": { "log_format": { "note": "${NOTE}" } } },
        });

        replace_env_vars_with(&mut config, &lookup);

        assert_eq!(
            config,
            json!({
                "services": [
                    { "name": "Test name", "routes": [{ "name": "Test name", "uris": ["/test/name"] }] },
                    { "name": "Test escape ${NAME}" },
                ],
                "consumers": [{ "username": "TEST_name", "plugins": { "key-auth": { "key": "secret" } } }],
                "ssls": [{ "snis": ["test.com"], "certificates": [{ "certificate": "-----", "key": "-----" }] }],
                // The key itself is never substituted, only values.
                "global_rules": { "${GLOBAL_PLUGIN}": { "key": "secret" } },
                "plugin_metadata": { "file-logger": { "log_format": { "note": "note" } } },
            })
        );
    }

    #[test]
    fn replace_env_vars_expands_an_unset_variable_to_an_empty_string() {
        let mut config = json!({ "name": "prefix-${MISSING}-suffix" });
        replace_env_vars_with(&mut config, &|_| None);
        assert_eq!(config, json!({ "name": "prefix--suffix" }));
    }

    fn labels(pairs: &[(&str, &str)]) -> Labels {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), LabelValue::Single(v.to_string())))
            .collect()
    }

    fn service(name: &str, labels: Option<Labels>) -> Service {
        Service {
            id: None,
            name: name.to_string(),
            description: None,
            labels,
            upstream: None,
            upstreams: None,
            plugins: None,
            path_prefix: None,
            strip_path_prefix: None,
            hosts: None,
            routes: None,
        }
    }

    fn ssl(sni: &str, labels: Option<Labels>) -> SSL {
        SSL {
            id: None,
            labels,
            r#type: Default::default(),
            snis: vec![sni.to_string()],
            certificates: vec![],
            client: None,
            ssl_protocols: None,
        }
    }

    fn consumer(username: &str, labels: Option<Labels>) -> Consumer {
        Consumer {
            username: username.to_string(),
            description: None,
            labels,
            plugins: None,
            credentials: None,
        }
    }

    fn consumer_group(name: &str, labels: Option<Labels>) -> ConsumerGroup {
        ConsumerGroup {
            id: None,
            name: name.to_string(),
            description: None,
            labels,
            plugins: None,
            consumers: None,
        }
    }

    mod filter_resource_types_tests {
        use super::*;

        fn sample_config() -> Configuration {
            Configuration {
                services: Some(vec![service("s1", None)]),
                ssls: Some(vec![ssl("example.com", None)]),
                consumers: Some(vec![consumer("c1", None)]),
                consumer_groups: Some(vec![consumer_group("g1", None)]),
                global_rules: None,
                plugin_metadata: None,
            }
        }

        #[test]
        fn no_include_and_no_exclude_is_a_no_op() {
            let mut config = sample_config();
            filter_resource_types(&mut config, &HashSet::new(), &HashSet::new());
            assert!(config.services.is_some());
            assert!(config.ssls.is_some());
            assert!(config.consumers.is_some());
            assert!(config.consumer_groups.is_some());
        }

        #[test]
        fn an_include_list_drops_every_bucket_not_on_it() {
            let mut config = sample_config();
            let include = HashSet::from([ResourceType::Service]);
            filter_resource_types(&mut config, &include, &HashSet::new());
            assert!(config.services.is_some());
            assert!(config.ssls.is_none());
            assert!(config.consumers.is_none());
            assert!(config.consumer_groups.is_none());
        }

        #[test]
        fn an_exclude_list_drops_only_the_buckets_on_it() {
            let mut config = sample_config();
            let exclude = HashSet::from([ResourceType::Ssl]);
            filter_resource_types(&mut config, &HashSet::new(), &exclude);
            assert!(config.services.is_some());
            assert!(config.ssls.is_none());
            assert!(config.consumers.is_some());
            assert!(config.consumer_groups.is_some());
        }
    }

    mod fill_labels_tests {
        use super::*;

        #[test]
        fn stamps_the_given_labels_onto_top_level_and_nested_resources() {
            let mut config = json!({
                "services": [{
                    "name": "s1",
                    "routes": [{"name": "r1", "uris": ["/foo"]}],
                    "stream_routes": [],
                }],
                "consumer_groups": [{
                    "name": "g1",
                    "consumers": [{"username": "c1"}],
                }],
            });
            fill_labels(
                &mut config,
                &HashMap::from([("env".to_string(), "prod".to_string())]),
            );

            assert_eq!(config["services"][0]["labels"]["env"], "prod");
            assert_eq!(config["services"][0]["routes"][0]["labels"]["env"], "prod");
            assert_eq!(config["consumer_groups"][0]["labels"]["env"], "prod");
            assert_eq!(
                config["consumer_groups"][0]["consumers"][0]["labels"]["env"],
                "prod"
            );
        }

        #[test]
        fn does_not_touch_service_upstreams_or_consumer_credentials() {
            let mut config = json!({
                "services": [{"name": "s1", "upstreams": [{"name": "u1"}]}],
                "consumers": [{"username": "c1", "credentials": [{"name": "cred1", "type": "key-auth", "config": {}}]}],
            });
            fill_labels(
                &mut config,
                &HashMap::from([("env".to_string(), "prod".to_string())]),
            );

            assert!(
                config["services"][0]["upstreams"][0]
                    .get("labels")
                    .is_none()
            );
            assert!(
                config["consumers"][0]["credentials"][0]
                    .get("labels")
                    .is_none()
            );
        }

        #[test]
        fn an_empty_label_map_is_a_no_op() {
            let mut config = json!({"services": [{"name": "s1"}]});
            let before = config.clone();
            fill_labels(&mut config, &HashMap::new());
            assert_eq!(config, before);
        }

        #[test]
        fn given_labels_overwrite_a_resources_existing_value_for_the_same_key() {
            let mut config = json!({
                "services": [{"name": "s1", "labels": {"env": "dev"}}],
            });
            fill_labels(
                &mut config,
                &HashMap::from([("env".to_string(), "prod".to_string())]),
            );
            assert_eq!(config["services"][0]["labels"]["env"], "prod");
        }
    }

    mod filter_by_labels_tests {
        use super::*;

        // The actual matching logic (include/exclude label combinations,
        // `LabelValue::Multiple`, unlabeled resources) is implemented and
        // tested in `adc_backend_core::filter_configuration_by_labels`,
        // which this function delegates to. This just confirms the
        // delegation itself is wired up correctly.
        #[test]
        fn delegates_to_the_shared_label_filter() {
            let mut config = Configuration {
                services: Some(vec![
                    service("matches", Some(labels(&[("env", "prod")]))),
                    service("does_not_match", Some(labels(&[("env", "dev")]))),
                ]),
                ssls: None,
                consumers: None,
                consumer_groups: None,
                global_rules: None,
                plugin_metadata: None,
            };
            filter_by_labels(&mut config, &HashMap::from([("env".to_string(), "prod".to_string())]));
            let names: Vec<&str> = config.services.as_ref().unwrap().iter().map(|s| s.name.as_str()).collect();
            assert_eq!(names, vec!["matches"]);
        }
    }

    mod merge_files_tests {
        use super::*;

        fn file(value: Value) -> Vec<(PathBuf, Value)> {
            vec![(PathBuf::from("a.yaml"), value)]
        }

        #[test]
        fn a_service_with_no_name_field_is_rejected_outright() {
            let err = merge_files(file(json!({"services": [{}]}))).unwrap_err();
            assert!(err.to_string().contains("name"), "{err}");
        }

        #[test]
        fn a_service_with_an_empty_name_is_rejected() {
            let err = merge_files(file(json!({"services": [{"name": ""}]}))).unwrap_err();
            assert!(err.to_string().contains("name"), "{err}");
        }

        #[test]
        fn a_consumer_with_no_username_field_is_rejected_outright() {
            let err = merge_files(file(json!({"consumers": [{}]}))).unwrap_err();
            assert!(err.to_string().contains("username"), "{err}");
        }

        #[test]
        fn an_ssl_with_no_snis_field_is_rejected_outright() {
            let err = merge_files(file(json!({"ssls": [{}]}))).unwrap_err();
            assert!(err.to_string().contains("snis"), "{err}");
        }

        #[test]
        fn an_ssl_with_an_empty_snis_array_is_rejected() {
            let err = merge_files(file(json!({"ssls": [{"snis": []}]}))).unwrap_err();
            assert!(err.to_string().contains("snis"), "{err}");
        }

        /// Two services both missing `name` used to collide into one silent
        /// `""` key and get reported as a duplicate, hiding the real
        /// problem — this pins down that the first one now fails outright
        /// with a message about the missing field instead.
        #[test]
        fn two_services_both_missing_a_name_report_the_missing_field_not_a_false_duplicate() {
            let err = merge_files(file(json!({"services": [{}, {}]}))).unwrap_err();
            let message = err.to_string();
            assert!(message.contains("name"), "{message}");
            assert!(!message.contains("duplicate"), "{message}: a missing field isn't a duplicate");
        }

        #[test]
        fn two_services_with_the_same_real_name_are_still_reported_as_a_duplicate() {
            let err = merge_files(file(json!({"services": [{"name": "svc"}, {"name": "svc"}]}))).unwrap_err();
            assert!(err.to_string().contains("duplicate"), "{err}");
        }

        #[test]
        fn a_duplicate_global_rule_is_reported_with_the_singular_word() {
            // Two files, not one object with a duplicate key (JSON objects
            // can't have duplicate keys) — needs a second file to collide.
            let files = vec![
                (PathBuf::from("a.yaml"), json!({"global_rules": {"gr1": {}}})),
                (PathBuf::from("b.yaml"), json!({"global_rules": {"gr1": {}}})),
            ];
            let err = merge_files(files).unwrap_err();
            assert!(err.to_string().contains("duplicate global_rule "), "{err}");
        }
    }
}
