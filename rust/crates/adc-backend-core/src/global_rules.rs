use adc_sdk::resources::Plugins;

/// Flattens a backend's global rules into the shape `Configuration::global_rules`
/// models: one entry per rule, keyed by plugin name.
///
/// A rule is compliant when it holds exactly one plugin and its id equals that
/// plugin's name. It becomes `plugin name -> plugin config`, which is what the
/// differ and the write path address. Any other rule becomes
/// `rule id -> {"plugins": {...}}`, so the differ sees an id the local config
/// never declares and emits a delete for it.
pub fn normalize_global_rules<I>(rules: I) -> Plugins
where
    I: IntoIterator<Item = (String, Plugins)>,
{
    let mut out = Plugins::new();
    for (id, mut plugins) in rules {
        if plugins.len() == 1
            && let Some(config) = plugins.remove(&id)
        {
            out.insert(id, config);
            continue;
        }
        let mut wrapper = serde_json::Map::new();
        wrapper.insert("plugins".to_string(), serde_json::Value::Object(plugins));
        out.insert(id, serde_json::Value::Object(wrapper));
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn plugins(value: Value) -> Plugins {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn compliant_rule_is_keyed_by_plugin_name_with_the_bare_config() {
        let out = normalize_global_rules([(
            "limit-count".to_string(),
            plugins(json!({ "limit-count": { "count": 1 } })),
        )]);
        assert_eq!(Value::Object(out), json!({ "limit-count": { "count": 1 } }));
    }

    #[test]
    fn multi_plugin_rule_keeps_its_id_and_nested_plugins() {
        let out = normalize_global_rules([(
            "legacy-rules".to_string(),
            plugins(json!({ "a": { "x": 1 }, "b": { "y": 2 }, "c": {} })),
        )]);
        assert_eq!(
            Value::Object(out),
            json!({ "legacy-rules": { "plugins": { "a": { "x": 1 }, "b": { "y": 2 }, "c": {} } } })
        );
    }

    #[test]
    fn single_plugin_rule_with_a_different_id_is_not_compliant() {
        let out = normalize_global_rules([(
            "rule-1".to_string(),
            plugins(json!({ "limit-count": { "count": 1 } })),
        )]);
        assert_eq!(
            Value::Object(out),
            json!({ "rule-1": { "plugins": { "limit-count": { "count": 1 } } } })
        );
    }

    #[test]
    fn id_equal_to_one_of_several_plugins_is_not_compliant() {
        let out = normalize_global_rules([(
            "a".to_string(),
            plugins(json!({ "a": {}, "b": {} })),
        )]);
        assert_eq!(Value::Object(out), json!({ "a": { "plugins": { "a": {}, "b": {} } } }));
    }

    #[test]
    fn empty_rule_is_kept_as_an_empty_placeholder() {
        let out = normalize_global_rules([("empty".to_string(), Plugins::new())]);
        assert_eq!(Value::Object(out), json!({ "empty": { "plugins": {} } }));
    }

    #[test]
    fn mixed_rules_are_each_normalized_independently() {
        let out = normalize_global_rules([
            ("ok".to_string(), plugins(json!({ "ok": { "k": 1 } }))),
            ("bad".to_string(), plugins(json!({ "p": {}, "q": {} }))),
        ]);
        assert_eq!(
            Value::Object(out),
            json!({ "ok": { "k": 1 }, "bad": { "plugins": { "p": {}, "q": {} } } })
        );
    }
}
