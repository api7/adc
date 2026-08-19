//! Request bodies for `PUT /sync` and `PUT /validate`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct SyncInput {
    pub task: SyncTask,
}

#[derive(Debug, Deserialize)]
pub struct SyncTask {
    pub opts: Opts,
    pub config: Value,
}

#[derive(Debug, Deserialize)]
pub struct ValidateInput {
    pub task: ValidateTask,
}

#[derive(Debug, Deserialize)]
pub struct ValidateTask {
    pub opts: Opts,
    pub config: Value,
}

/// Shared by both endpoints — `/validate` just ignores `bypass_cache`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opts {
    pub backend: String,
    pub server: ServerAddr,
    pub token: String,
    #[serde(default = "default_true")]
    pub lint: bool,
    pub include_resource_type: Option<Vec<ServerResourceType>>,
    pub exclude_resource_type: Option<Vec<ServerResourceType>>,
    pub label_selector: Option<HashMap<String, String>>,
    pub cache_key: String,
    #[serde(default)]
    pub bypass_cache: bool,
    pub gateway_group: Option<String>,
    #[serde(default = "default_request_concurrent")]
    pub request_concurrent: usize,

    // TLS/mTLS to the backend gateway — raw PEM, not a file path.
    #[serde(default)]
    pub tls_skip_verify: bool,
    pub ca_cert: Option<String>,
    pub tls_client_cert: Option<String>,
    pub tls_client_key: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_request_concurrent() -> usize {
    10
}

impl Opts {
    pub fn resource_type_sets(
        &self,
    ) -> (
        std::collections::HashSet<adc_sdk::ResourceType>,
        std::collections::HashSet<adc_sdk::ResourceType>,
    ) {
        let include = self
            .include_resource_type
            .iter()
            .flatten()
            .map(|t| (*t).into())
            .collect();
        let exclude = self
            .exclude_resource_type
            .iter()
            .flatten()
            .map(|t| (*t).into())
            .collect();
        (include, exclude)
    }

    pub fn label_selector_or_default(&self) -> HashMap<String, String> {
        self.label_selector.clone().unwrap_or_default()
    }
}

/// `z.union([z.url(), z.array(z.url())])` — a single backend takes one
/// server URL, `apisix-standalone` addresses a cluster of them.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ServerAddr {
    Single(String),
    Multiple(Vec<String>),
}

impl ServerAddr {
    pub fn as_list(&self) -> Vec<String> {
        match self {
            ServerAddr::Single(server) => vec![server.clone()],
            ServerAddr::Multiple(servers) => servers.clone(),
        }
    }
}

/// Mirrors `cli::ResourceTypeArg`, kept separate so `adc_sdk::ResourceType` stays derive-free.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerResourceType {
    Route,
    Service,
    Upstream,
    Ssl,
    GlobalRule,
    PluginConfig,
    PluginMetadata,
    Consumer,
    ConsumerGroup,
    ConsumerCredential,
    StreamRoute,
}

impl From<ServerResourceType> for adc_sdk::ResourceType {
    fn from(value: ServerResourceType) -> Self {
        match value {
            ServerResourceType::Route => adc_sdk::ResourceType::Route,
            ServerResourceType::Service => adc_sdk::ResourceType::Service,
            ServerResourceType::Upstream => adc_sdk::ResourceType::Upstream,
            ServerResourceType::Ssl => adc_sdk::ResourceType::Ssl,
            ServerResourceType::GlobalRule => adc_sdk::ResourceType::GlobalRule,
            ServerResourceType::PluginConfig => adc_sdk::ResourceType::PluginConfig,
            ServerResourceType::PluginMetadata => adc_sdk::ResourceType::PluginMetadata,
            ServerResourceType::Consumer => adc_sdk::ResourceType::Consumer,
            ServerResourceType::ConsumerGroup => adc_sdk::ResourceType::ConsumerGroup,
            ServerResourceType::ConsumerCredential => adc_sdk::ResourceType::ConsumerCredential,
            ServerResourceType::StreamRoute => adc_sdk::ResourceType::StreamRoute,
        }
    }
}

/// One input-validation failure — `path` names the offending field.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: Vec<String>,
    pub message: String,
}

impl ValidationIssue {
    fn new(field: &str, message: impl Into<String>) -> Self {
        Self {
            path: vec![field.to_string()],
            message: message.into(),
        }
    }
}

/// cert/key must be provided together; any of the three, if given, must look like PEM.
pub fn validate_tls_material(opts: &Opts) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if opts.tls_client_cert.is_some() != opts.tls_client_key.is_some() {
        issues.push(ValidationIssue::new(
            "tlsClientKey",
            "tlsClientCert and tlsClientKey must be provided together",
        ));
    }
    for (field, value, noun) in [
        ("caCert", &opts.ca_cert, "certificate"),
        ("tlsClientCert", &opts.tls_client_cert, "certificate"),
        ("tlsClientKey", &opts.tls_client_key, "key"),
    ] {
        if let Some(value) = value
            && !is_pem_like(value)
        {
            issues.push(ValidationIssue::new(
                field,
                format!("{field} does not look like a PEM-encoded {noun}"),
            ));
        }
    }
    issues
}

fn is_pem_like(value: &str) -> bool {
    value.trim_start().starts_with("-----BEGIN")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(overrides: impl FnOnce(&mut Opts)) -> Opts {
        let mut opts = Opts {
            backend: "apisix".to_string(),
            server: ServerAddr::Single("http://127.0.0.1:9180".to_string()),
            token: "token".to_string(),
            lint: true,
            include_resource_type: None,
            exclude_resource_type: None,
            label_selector: None,
            cache_key: "default".to_string(),
            bypass_cache: false,
            gateway_group: None,
            request_concurrent: 10,
            tls_skip_verify: false,
            ca_cert: None,
            tls_client_cert: None,
            tls_client_key: None,
        };
        overrides(&mut opts);
        opts
    }

    #[test]
    fn deserializes_a_single_server_string() {
        let input: SyncInput = serde_json::from_value(serde_json::json!({
            "task": {
                "opts": {"backend": "apisix", "server": "http://a:9180", "token": "t", "cacheKey": "default"},
                "config": {}
            }
        }))
        .unwrap();
        assert_eq!(input.task.opts.server.as_list(), vec!["http://a:9180"]);
    }

    #[test]
    fn deserializes_an_array_of_servers() {
        let input: SyncInput = serde_json::from_value(serde_json::json!({
            "task": {
                "opts": {"backend": "apisix-standalone", "server": ["http://a:9180", "http://b:9180"], "token": "t", "cacheKey": "default"},
                "config": {}
            }
        }))
        .unwrap();
        assert_eq!(
            input.task.opts.server.as_list(),
            vec!["http://a:9180", "http://b:9180"]
        );
    }

    #[test]
    fn lint_defaults_to_true_and_bypass_cache_to_false() {
        let input: SyncInput = serde_json::from_value(serde_json::json!({
            "task": {
                "opts": {"backend": "apisix", "server": "http://a:9180", "token": "t", "cacheKey": "default"},
                "config": {}
            }
        }))
        .unwrap();
        assert!(input.task.opts.lint);
        assert!(!input.task.opts.bypass_cache);
    }

    #[test]
    fn resource_type_is_case_matched_snake_case() {
        let input: SyncInput = serde_json::from_value(serde_json::json!({
            "task": {
                "opts": {
                    "backend": "apisix", "server": "http://a:9180", "token": "t", "cacheKey": "default",
                    "includeResourceType": ["stream_route", "consumer_credential"]
                },
                "config": {}
            }
        }))
        .unwrap();
        assert_eq!(
            input.task.opts.include_resource_type.unwrap(),
            vec![
                ServerResourceType::StreamRoute,
                ServerResourceType::ConsumerCredential
            ]
        );
    }

    #[test]
    fn a_lone_tls_client_cert_without_a_key_is_rejected() {
        let opts =
            opts(|o| o.tls_client_cert = Some("-----BEGIN CERTIFICATE-----\n...".to_string()));
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["tlsClientKey"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_lone_tls_client_key_without_a_cert_is_rejected() {
        let opts =
            opts(|o| o.tls_client_key = Some("-----BEGIN PRIVATE KEY-----\n...".to_string()));
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["tlsClientKey"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_paired_cert_and_key_is_accepted_at_the_pairing_check() {
        let opts = opts(|o| {
            o.tls_client_cert = Some("-----BEGIN CERTIFICATE-----\n...".to_string());
            o.tls_client_key = Some("-----BEGIN PRIVATE KEY-----\n...".to_string());
        });
        let issues = validate_tls_material(&opts);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn a_non_pem_ca_cert_is_rejected() {
        let opts = opts(|o| o.ca_cert = Some("not-a-pem".to_string()));
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["caCert"]),
            "{issues:?}"
        );
    }

    #[test]
    fn no_tls_material_at_all_is_fine() {
        assert!(validate_tls_material(&opts(|_| {})).is_empty());
    }
}
