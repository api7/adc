use std::collections::{HashMap, HashSet};

use adc_sdk::ResourceType;
use adc_sdk::resources::{Configuration, LabelValue, Labels};
use reqwest::RequestBuilder;

/// What a fetcher should skip and how it should narrow a request, decided
/// once at `Backend` construction time and consulted before every
/// top-level collection request `dump()` makes — a service's nested
/// routes/upstreams or a consumer's nested credentials aren't filtered
/// individually, only whichever top-level collection they came from ever
/// gets fetched at all.
#[derive(Debug, Clone, Default)]
pub struct ResourceFilter {
    pub include: HashSet<ResourceType>,
    pub exclude: HashSet<ResourceType>,
    pub label_selector: HashMap<String, String>,
}

impl ResourceFilter {
    /// An empty (include, exclude) pair skips nothing — the common case,
    /// when neither `--include-resource-type` nor `--exclude-resource-type`
    /// was given.
    pub fn is_skip(&self, resource_type: ResourceType) -> bool {
        if !self.include.is_empty() && !self.include.contains(&resource_type) {
            return true;
        }
        if !self.exclude.is_empty() && self.exclude.contains(&resource_type) {
            return true;
        }
        false
    }

    /// Adds one `labels[key]=value` query parameter per `--label-selector`
    /// entry. A no-op when there's no selector, so callers can chain this
    /// unconditionally.
    pub fn attach_label_selector(&self, builder: RequestBuilder) -> RequestBuilder {
        if self.label_selector.is_empty() {
            return builder;
        }
        let params: Vec<(String, &str)> = self
            .label_selector
            .iter()
            .map(|(key, value)| (format!("labels[{key}]"), value.as_str()))
            .collect();
        builder.query(&params)
    }

    /// Drops resources whose `labels` don't carry every key/value pair in
    /// `label_selector`. A client-side backstop for [`Self::attach_label_selector`]:
    /// nothing here guarantees the server actually understood that query
    /// parameter and narrowed its response, so a fetcher can't treat
    /// "asked the server to filter" as "the result is filtered" — this
    /// re-checks every resource it got back, regardless of whether the
    /// server already did the same filtering itself.
    pub fn filter_configuration(&self, config: &mut Configuration) {
        filter_configuration_by_labels(config, &self.label_selector);
    }
}

/// Top-level only (`services`/`ssls`/`consumers`/`consumer_groups`) —
/// `global_rules`/`plugin_metadata` aren't per-resource collections a label
/// selector could narrow down. A resource with no `labels` at all never
/// matches a non-empty selector.
pub fn filter_configuration_by_labels(config: &mut Configuration, label_selector: &HashMap<String, String>) {
    if label_selector.is_empty() {
        return;
    }
    if let Some(services) = &mut config.services {
        services.retain(|s| matches_labels(&s.labels, label_selector));
    }
    if let Some(ssls) = &mut config.ssls {
        ssls.retain(|s| matches_labels(&s.labels, label_selector));
    }
    if let Some(consumers) = &mut config.consumers {
        consumers.retain(|c| matches_labels(&c.labels, label_selector));
    }
    if let Some(consumer_groups) = &mut config.consumer_groups {
        consumer_groups.retain(|g| matches_labels(&g.labels, label_selector));
    }
}

fn matches_labels(resource_labels: &Option<Labels>, required: &HashMap<String, String>) -> bool {
    let Some(resource_labels) = resource_labels else {
        return false;
    };
    required.iter().all(|(key, value)| match resource_labels.get(key) {
        Some(LabelValue::Single(v)) => v == value,
        Some(LabelValue::Multiple(values)) => values.iter().any(|v| v == value),
        None => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_nothing_when_neither_include_nor_exclude_is_set() {
        let filter = ResourceFilter::default();
        assert!(!filter.is_skip(ResourceType::Service));
        assert!(!filter.is_skip(ResourceType::Consumer));
    }

    #[test]
    fn an_include_list_skips_everything_not_on_it() {
        let filter = ResourceFilter {
            include: [ResourceType::Service].into_iter().collect(),
            ..Default::default()
        };
        assert!(!filter.is_skip(ResourceType::Service));
        assert!(filter.is_skip(ResourceType::Consumer));
    }

    #[test]
    fn an_exclude_list_skips_only_whats_on_it() {
        let filter = ResourceFilter {
            exclude: [ResourceType::Service].into_iter().collect(),
            ..Default::default()
        };
        assert!(filter.is_skip(ResourceType::Service));
        assert!(!filter.is_skip(ResourceType::Consumer));
    }

    #[test]
    fn an_empty_include_list_is_the_same_as_no_include_list() {
        let filter = ResourceFilter {
            include: HashSet::new(),
            ..Default::default()
        };
        assert!(!filter.is_skip(ResourceType::Service));
    }

    fn consumer(username: &str, labels: Option<Labels>) -> adc_sdk::resources::Consumer {
        adc_sdk::resources::Consumer {
            username: username.to_string(),
            description: None,
            labels,
            plugins: None,
            credentials: None,
        }
    }

    fn consumer_group(name: &str, labels: Option<Labels>) -> adc_sdk::resources::ConsumerGroup {
        adc_sdk::resources::ConsumerGroup {
            id: None,
            name: name.to_string(),
            description: None,
            labels,
            plugins: None,
            consumers: None,
        }
    }

    fn empty_configuration() -> Configuration {
        Configuration {
            services: None,
            ssls: None,
            consumers: None,
            consumer_groups: None,
            global_rules: None,
            plugin_metadata: None,
        }
    }

    #[test]
    fn an_empty_selector_is_a_no_op() {
        let mut config = empty_configuration();
        config.consumers = Some(vec![consumer("c1", None)]);
        filter_configuration_by_labels(&mut config, &HashMap::new());
        assert_eq!(config.consumers.unwrap().len(), 1);
    }

    #[test]
    fn keeps_only_resources_matching_every_required_label() {
        let mut config = empty_configuration();
        config.consumers = Some(vec![
            consumer(
                "matches",
                Some(Labels::from([
                    ("env".to_string(), LabelValue::Single("prod".to_string())),
                    ("team".to_string(), LabelValue::Single("core".to_string())),
                ])),
            ),
            consumer(
                "missing_one_key",
                Some(Labels::from([("env".to_string(), LabelValue::Single("prod".to_string()))])),
            ),
            consumer(
                "wrong_value",
                Some(Labels::from([
                    ("env".to_string(), LabelValue::Single("dev".to_string())),
                    ("team".to_string(), LabelValue::Single("core".to_string())),
                ])),
            ),
        ]);
        let required = HashMap::from([
            ("env".to_string(), "prod".to_string()),
            ("team".to_string(), "core".to_string()),
        ]);
        filter_configuration_by_labels(&mut config, &required);
        let usernames: Vec<&str> = config.consumers.as_ref().unwrap().iter().map(|c| c.username.as_str()).collect();
        assert_eq!(usernames, vec!["matches"]);
    }

    #[test]
    fn a_resource_with_no_labels_never_matches_a_non_empty_selector() {
        let mut config = empty_configuration();
        config.consumers = Some(vec![consumer("c1", None)]);
        filter_configuration_by_labels(&mut config, &HashMap::from([("env".to_string(), "prod".to_string())]));
        assert!(config.consumers.unwrap().is_empty());
    }

    #[test]
    fn a_multiple_value_label_matches_if_any_entry_equals_the_required_value() {
        let labels = Labels::from([(
            "env".to_string(),
            LabelValue::Multiple(vec!["dev".to_string(), "prod".to_string()]),
        )]);
        let mut config = empty_configuration();
        config.consumer_groups = Some(vec![consumer_group("g1", Some(labels))]);
        filter_configuration_by_labels(&mut config, &HashMap::from([("env".to_string(), "prod".to_string())]));
        assert_eq!(config.consumer_groups.unwrap().len(), 1);
    }
}
