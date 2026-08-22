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
    #[serde(default = "default_timeout_ms")]
    pub timeout: u64,

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

fn default_timeout_ms() -> u64 {
    30_000
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

/// Every entry must be an http(s) URL with a host — `ServerAddr`'s
/// `#[serde(untagged)]` only checks the string-vs-array shape, not `z.url()`'s
/// format contract, and a bare `url::Url::parse` still accepts things this
/// is never going to dispatch a request to (`file:///etc/passwd`, `mailto:`).
pub fn validate_server_addr(opts: &Opts) -> Vec<ValidationIssue> {
    opts.server
        .as_list()
        .iter()
        .filter(|server| !is_usable_server_url(server))
        .map(|server| ValidationIssue::new("server", format!("{server:?} is not a valid URL")))
        .collect()
}

fn is_usable_server_url(server: &str) -> bool {
    let Ok(url) = url::Url::parse(server) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

/// cert/key must be provided together; any of the three, if given, must be PEM.
pub fn validate_tls_material(opts: &Opts) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if opts.tls_client_cert.is_some() != opts.tls_client_key.is_some() {
        issues.push(ValidationIssue::new(
            "tlsClientKey",
            "tlsClientCert and tlsClientKey must be provided together",
        ));
    }
    for (field, value, noun, is_valid) in [
        (
            "caCert",
            &opts.ca_cert,
            "certificate",
            is_valid_pem_certificate as fn(&str) -> bool,
        ),
        (
            "tlsClientCert",
            &opts.tls_client_cert,
            "certificate",
            is_valid_pem_certificate,
        ),
        (
            "tlsClientKey",
            &opts.tls_client_key,
            "key",
            is_valid_pem_private_key,
        ),
    ] {
        if let Some(value) = value
            && !is_valid(value)
        {
            issues.push(ValidationIssue::new(
                field,
                format!("{field} does not look like a PEM-encoded {noun}"),
            ));
        }
    }
    issues
}

/// Parses the decoded DER via `RootCertStore::add`, not just the PEM
/// framing — garbage base64 between valid `-----BEGIN/END CERTIFICATE-----`
/// markers would otherwise pass and only fail later, deep inside backend
/// construction.
fn is_valid_pem_certificate(value: &str) -> bool {
    let mut reader = std::io::BufReader::new(value.as_bytes());
    let Ok(certs) = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>() else {
        return false;
    };
    if certs.is_empty() {
        return false;
    }
    let mut store = rustls::RootCertStore::empty();
    certs.into_iter().all(|cert| store.add(cert).is_ok())
}

/// Goes through the installed `CryptoProvider`, not a specific backend
/// module, so this stays agnostic to which one `server::run` installs.
fn is_valid_pem_private_key(value: &str) -> bool {
    let mut reader = std::io::BufReader::new(value.as_bytes());
    let Ok(Some(key)) = rustls_pemfile::private_key(&mut reader) else {
        return false;
    };
    let Some(provider) = rustls::crypto::CryptoProvider::get_default() else {
        return false;
    };
    provider.key_provider.load_private_key(key).is_ok()
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
            timeout: 30_000,
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

    // A real self-signed EC cert/key pair (generated once via `openssl req
    // -x509 -newkey ec ...`, not fetched at test time) — validation parses
    // this for real, not just checking for a `-----BEGIN` prefix.
    const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIBcjCCARmgAwIBAgIUWp+abBNKuUPdUIeouYDaDgHPIO4wCgYIKoZIzj0EAwIw\n\
DzENMAsGA1UEAwwEdGVzdDAeFw0yNjA4MTgwMzIwMDJaFw0yNjA4MTkwMzIwMDJa\n\
MA8xDTALBgNVBAMMBHRlc3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARVo3/X\n\
uhOYfghuoLbag2VJvGofvgPYtXcdh4oFCmXB1MOupxSI3DqCFvMJc/QeH92Nz/qW\n\
vLW7TEWRCo2/Bay1o1MwUTAdBgNVHQ4EFgQUy80qZFI7+wryg4UyeI+YsHfSqgow\n\
HwYDVR0jBBgwFoAUy80qZFI7+wryg4UyeI+YsHfSqgowDwYDVR0TAQH/BAUwAwEB\n\
/zAKBggqhkjOPQQDAgNHADBEAiB+ddl9S2GSo8/NF37M47JI1HtxOzQQTizSoAQd\n\
tx5+SQIgKX3ASSnC8rrNGSFda+y79MOudxia/iQouBhv8Fb/hnE=\n\
-----END CERTIFICATE-----\n";
    const KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgO3caXwd/kpykMTTw\n\
+IoxA9NXadu2yhvQXw/rxkgjhZChRANCAARVo3/XuhOYfghuoLbag2VJvGofvgPY\n\
tXcdh4oFCmXB1MOupxSI3DqCFvMJc/QeH92Nz/qWvLW7TEWRCo2/Bay1\n\
-----END PRIVATE KEY-----\n";

    #[test]
    fn a_paired_cert_and_key_is_accepted_at_the_pairing_check() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let opts = opts(|o| {
            o.tls_client_cert = Some(CERT_PEM.to_string());
            o.tls_client_key = Some(KEY_PEM.to_string());
        });
        let issues = validate_tls_material(&opts);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn a_lone_tls_client_key_missing_its_cert_is_flagged_on_tls_client_key_only() {
        // A lone tlsClientKey should be reported once, on the pairing
        // check — not duplicated by the per-field PEM-validity check,
        // since the key itself is valid PEM.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let opts = opts(|o| o.tls_client_key = Some(KEY_PEM.to_string()));
        let issues = validate_tls_material(&opts);
        assert_eq!(
            issues,
            vec![ValidationIssue::new(
                "tlsClientKey",
                "tlsClientCert and tlsClientKey must be provided together",
            )],
            "{issues:?}"
        );
    }

    #[test]
    fn an_incomplete_pem_certificate_is_rejected() {
        let opts = opts(|o| o.ca_cert = Some("-----BEGIN CERTIFICATE-----\n...".to_string()));
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["caCert"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_well_formed_pem_wrapping_garbage_der_is_rejected() {
        // Valid PEM framing, but the base64 payload isn't a real X.509
        // certificate — must fail at the DER-parsing check, not just the
        // PEM-extraction one.
        let garbage_cert = "-----BEGIN CERTIFICATE-----\n\
dGhpcyBpcyBub3QgYSB2YWxpZCB4NTA5IGNlcnRpZmljYXRlLCBqdXN0IHNvbWUgcGFkZGluZyBieXRlcyB0byBtYWtlIGl0IGxvbmcgZW5vdWdo\n\
-----END CERTIFICATE-----\n";
        let opts = opts(|o| o.ca_cert = Some(garbage_cert.to_string()));
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["caCert"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_well_formed_pem_wrapping_garbage_der_key_is_rejected() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let garbage_key = "-----BEGIN PRIVATE KEY-----\n\
dGhpcyBpcyBub3QgYSB2YWxpZCBwcml2YXRlIGtleSBkZXIsIGp1c3QgcGFkZGluZyBieXRlcyB0byBtYWtlIGl0IGxvbmcgZW5vdWdoIHRvIGxvb2sgcmVhbA==\n\
-----END PRIVATE KEY-----\n";
        let opts = opts(|o| {
            o.tls_client_cert = Some(CERT_PEM.to_string());
            o.tls_client_key = Some(garbage_key.to_string());
        });
        let issues = validate_tls_material(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["tlsClientKey"]),
            "{issues:?}"
        );
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

    #[test]
    fn a_valid_server_url_passes() {
        assert!(validate_server_addr(&opts(|_| {})).is_empty());
    }

    #[test]
    fn a_malformed_single_server_is_rejected() {
        let opts = opts(|o| o.server = ServerAddr::Single("not a url".to_string()));
        let issues = validate_server_addr(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["server"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_non_http_scheme_that_still_parses_as_a_url_is_rejected() {
        // A real host, unlike `a_url_without_a_host_is_rejected`'s
        // `mailto:` — isolates the scheme check from the host check.
        let opts = opts(|o| o.server = ServerAddr::Single("ftp://example.com/resource".to_string()));
        let issues = validate_server_addr(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["server"]),
            "{issues:?}"
        );
    }

    #[test]
    fn a_url_without_a_host_is_rejected() {
        let opts = opts(|o| o.server = ServerAddr::Single("mailto:a@b.com".to_string()));
        let issues = validate_server_addr(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["server"]),
            "{issues:?}"
        );
    }

    #[test]
    fn one_malformed_entry_among_valid_ones_is_still_rejected() {
        let opts = opts(|o| {
            o.server =
                ServerAddr::Multiple(vec!["http://a:9180".to_string(), "not a url".to_string()])
        });
        let issues = validate_server_addr(&opts);
        assert!(
            issues.iter().any(|i| i.path == vec!["server"]),
            "{issues:?}"
        );
    }

    #[test]
    fn multiple_valid_servers_pass() {
        let opts = opts(|o| {
            o.server = ServerAddr::Multiple(vec![
                "http://a:9180".to_string(),
                "http://b:9180".to_string(),
            ])
        });
        assert!(validate_server_addr(&opts).is_empty());
    }
}
