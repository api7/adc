//! Semantic ("lint") validation on top of `resources::Configuration`.
//!
//! Structural validity (types, required fields, unknown fields) is already
//! guaranteed by the time a `Configuration` value exists — `serde`
//! deserialization is the gate for that (see `resources`' own doc comment).
//! This module is a separate, explicit pass on top of that same,
//! already-valid value, checking what `serde` can't express: string
//! length/format, numeric ranges, and a handful of genuine cross-field
//! rules.
//!
//! Most rules are declared once, as `#[schemars(...)]` attributes right on
//! the `resources` structs, and enforced here by compiling that same
//! derived JSON Schema with `jsonschema` and running it against the
//! serialized configuration — one declaration, used both to export
//! `schema.json` (see the `export-schema` binary) and to validate at
//! runtime, rather than maintaining two separate attribute sets that could
//! drift apart. Only the rules not represented in that derived schema at
//! all get hand-written functions below — some genuinely cross-field
//! (comparing more than one field), others single-field but not expressed
//! as a schema constraint for other reasons (a custom message, an
//! allow-list that isn't meant to live in the wire schema, ...).

use std::sync::LazyLock;

use crate::resources::{Configuration, Consumer, Service, Upstream};
use crate::value_diff::{DiffPath, PathSegment, format_path};

static SCHEMA_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema = schemars::schema_for!(Configuration);
    let schema_value = serde_json::to_value(&schema).expect("derived schema serializes to JSON");
    jsonschema::validator_for(&schema_value).expect("derived schema is a valid JSON Schema")
});

/// One semantic-validation failure: where in the configuration it occurred,
/// and what's wrong. Structural failures never reach here — see this
/// module's doc comment.
#[derive(Debug, Clone, PartialEq)]
pub struct LintIssue {
    pub path: DiffPath,
    pub message: String,
}

impl std::fmt::Display for LintIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", format_path(&self.path), self.message)
        }
    }
}

/// Runs every semantic rule against an already-structurally-valid
/// `Configuration`, collecting every violation rather than stopping at the
/// first one, so a caller sees every problem in one pass instead of
/// fixing and re-running repeatedly.
pub fn lint(config: &Configuration) -> Vec<LintIssue> {
    let instance = serde_json::to_value(config).expect("Configuration always serializes");
    let mut issues: Vec<LintIssue> = SCHEMA_VALIDATOR
        .iter_errors(&instance)
        .map(|e| {
            let path = json_pointer_to_diff_path(&e.instance_path().to_string());
            let message = masked_message(&e, &path);
            LintIssue { path, message }
        })
        .collect();
    check_cross_field_rules(config, &mut issues);
    issues
}

/// Field names that might carry secret material (inline PEM content, or the
/// plaintext of a `$secret://`/`$env://` reference) — `jsonschema`'s own
/// error `Display` embeds the failing instance value verbatim, which is fine
/// for debugging but not for a CLI's stdout/logs, so these get a generic
/// message instead of the real one.
const SENSITIVE_FIELD_NAMES: &[&str] = &["certificate", "key", "ca"];

fn masked_message(error: &jsonschema::ValidationError, path: &[PathSegment]) -> String {
    let sensitive = matches!(path.last(), Some(PathSegment::Key(key)) if SENSITIVE_FIELD_NAMES.contains(&key.as_str()));
    if sensitive {
        "does not match the expected format (value redacted)".to_string()
    } else {
        error.to_string()
    }
}

/// Converts a JSON Pointer (e.g. `/services/0/upstream/nodes`) into a
/// `DiffPath`. Numeric segments become `Index`, everything else `Key`.
fn json_pointer_to_diff_path(pointer: &str) -> DiffPath {
    pointer
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| match segment.parse::<usize>() {
            Ok(index) => PathSegment::Index(index),
            Err(_) => PathSegment::Key(segment.to_string()),
        })
        .collect()
}

fn push_key(base: &[PathSegment], key: &str) -> DiffPath {
    let mut path = base.to_vec();
    path.push(PathSegment::Key(key.to_string()));
    path
}

fn push_index(base: &[PathSegment], index: usize) -> DiffPath {
    let mut path = base.to_vec();
    path.push(PathSegment::Index(index));
    path
}

fn check_cross_field_rules(config: &Configuration, issues: &mut Vec<LintIssue>) {
    for (i, service) in config.services.iter().flatten().enumerate() {
        check_service(service, &push_index(&push_key(&[], "services"), i), issues);
    }
    for (i, consumer) in config.consumers.iter().flatten().enumerate() {
        check_consumer_credentials(consumer, &push_index(&push_key(&[], "consumers"), i), issues);
    }
    for (i, group) in config.consumer_groups.iter().flatten().enumerate() {
        let group_path = push_index(&push_key(&[], "consumer_groups"), i);
        for (j, consumer) in group.consumers.iter().flatten().enumerate() {
            check_consumer_credentials(consumer, &push_index(&push_key(&group_path, "consumers"), j), issues);
        }
    }
}

/// No `checks.active`/`checks.passive` interlock rule here: `active` is a
/// required (non-`Option`) field on `UpstreamHealthCheck`, so any rule tying
/// it to `passive`'s presence would be unconditionally true given valid
/// input — a dead rule, not a gap.
fn check_service(service: &Service, path: &[PathSegment], issues: &mut Vec<LintIssue>) {
    if let Some(prefix) = &service.path_prefix
        && !prefix.starts_with('/')
    {
        issues.push(LintIssue {
            path: push_key(path, "path_prefix"),
            message: "must start with \"/\"".to_string(),
        });
    }
    if service.upstreams.is_some() && service.upstream.is_none() {
        issues.push(LintIssue {
            path: path.to_vec(),
            message: "the default upstream must be set with \"upstream\" when multiple upstreams are set via \"upstreams\"".to_string(),
        });
    }
    if let Some(upstream) = &service.upstream {
        check_upstream_discovery(upstream, &push_key(path, "upstream"), issues);
    }
    for (i, upstream) in service.upstreams.iter().flatten().enumerate() {
        check_upstream_discovery(upstream, &push_index(&push_key(path, "upstreams"), i), issues);
    }
}

/// `nodes` and service discovery (`discovery_type`+`service_name`) are
/// mutually exclusive, and exactly one must be set:
/// `(nodes && !discovery_type && !service_name) || (discovery_type &&
/// service_name && !nodes)`.
fn check_upstream_discovery(upstream: &Upstream, path: &[PathSegment], issues: &mut Vec<LintIssue>) {
    let nodes_only = upstream.nodes.is_some() && upstream.discovery_type.is_none() && upstream.service_name.is_none();
    let discovery_only =
        upstream.discovery_type.is_some() && upstream.service_name.is_some() && upstream.nodes.is_none();
    if !(nodes_only || discovery_only) {
        issues.push(LintIssue {
            path: path.to_vec(),
            message: "upstream must either specify nodes or use service discovery (\"discovery_type\" + \"service_name\"), not both or neither"
                .to_string(),
        });
    }
}

const ALLOWED_CREDENTIAL_TYPES: [&str; 4] = ["key-auth", "basic-auth", "jwt-auth", "hmac-auth"];

fn check_consumer_credentials(consumer: &Consumer, path: &[PathSegment], issues: &mut Vec<LintIssue>) {
    for (i, credential) in consumer.credentials.iter().flatten().enumerate() {
        if !ALLOWED_CREDENTIAL_TYPES.contains(&credential.r#type.as_str()) {
            issues.push(LintIssue {
                path: push_key(&push_index(&push_key(path, "credentials"), i), "type"),
                message: "consumer credential only supports \"key-auth\", \"basic-auth\", \"jwt-auth\" and \"hmac-auth\" types".to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{ConsumerCredential, ConsumerGroup, ServiceRoutes, UpstreamNode};

    fn minimal_upstream_with_nodes() -> Upstream {
        Upstream {
            id: None,
            name: None,
            description: None,
            labels: None,
            r#type: Default::default(),
            hash_on: None,
            key: None,
            checks: None,
            nodes: Some(vec![UpstreamNode { host: "127.0.0.1".into(), port: 80, weight: 1, priority: 0, metadata: None }]),
            scheme: Default::default(),
            retries: None,
            retry_timeout: None,
            timeout: None,
            tls: None,
            keepalive_pool: None,
            pass_host: Default::default(),
            upstream_host: None,
            service_name: None,
            discovery_type: None,
            discovery_args: None,
        }
    }

    fn minimal_service() -> Service {
        Service {
            id: None,
            name: "svc".into(),
            description: None,
            labels: None,
            upstream: Some(minimal_upstream_with_nodes()),
            upstreams: None,
            plugins: None,
            path_prefix: None,
            strip_path_prefix: None,
            hosts: None,
            routes: Some(ServiceRoutes::Http { routes: vec![] }),
        }
    }

    fn empty_config() -> Configuration {
        Configuration { services: None, ssls: None, consumers: None, consumer_groups: None, global_rules: None, plugin_metadata: None }
    }

    #[test]
    fn a_valid_configuration_lints_clean() {
        let config = Configuration { services: Some(vec![minimal_service()]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn upstream_with_both_nodes_and_discovery_is_rejected() {
        let mut upstream = minimal_upstream_with_nodes();
        upstream.discovery_type = Some("dns".into());
        upstream.service_name = Some("svc.local".into());
        let service = Service { upstream: Some(upstream), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(format_path(&issues[0].path), "services[0].upstream");
    }

    #[test]
    fn upstream_with_neither_nodes_nor_discovery_is_rejected() {
        let mut upstream = minimal_upstream_with_nodes();
        upstream.nodes = None;
        let service = Service { upstream: Some(upstream), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config).len(), 1);
    }

    #[test]
    fn upstream_with_only_discovery_is_accepted() {
        let mut upstream = minimal_upstream_with_nodes();
        upstream.nodes = None;
        upstream.discovery_type = Some("dns".into());
        upstream.service_name = Some("svc.local".into());
        let service = Service { upstream: Some(upstream), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn path_prefix_without_a_leading_slash_is_rejected() {
        let service = Service { path_prefix: Some("no-slash".into()), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(format_path(&issues[0].path), "services[0].path_prefix");
    }

    #[test]
    fn path_prefix_with_a_leading_slash_is_accepted() {
        let service = Service { path_prefix: Some("/api".into()), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn upstreams_without_a_default_upstream_is_rejected() {
        let mut named = minimal_upstream_with_nodes();
        named.name = Some("u1".into());
        let service = Service { upstream: None, upstreams: Some(vec![named]), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config).len(), 1);
    }

    #[test]
    fn upstreams_with_a_default_upstream_is_accepted() {
        let mut named = minimal_upstream_with_nodes();
        named.name = Some("u1".into());
        let service = Service { upstreams: Some(vec![named]), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn an_unnamed_entry_in_upstreams_is_rejected_by_the_schema() {
        let unnamed = minimal_upstream_with_nodes();
        let service = Service { upstreams: Some(vec![unnamed]), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert!(!lint(&config).is_empty());
    }

    /// The embedded default upstream isn't an addressable resource on its
    /// own — any `id` on it is rejected, unlike `upstreams[]` items.
    #[test]
    fn an_id_on_the_default_upstream_is_rejected_by_the_schema() {
        let mut upstream = minimal_upstream_with_nodes();
        upstream.id = Some("u1".into());
        let service = Service { upstream: Some(upstream), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert!(!lint(&config).is_empty());
    }

    #[test]
    fn an_id_on_a_named_upstreams_entry_is_accepted() {
        let mut named = minimal_upstream_with_nodes();
        named.id = Some("u1".into());
        named.name = Some("u1".into());
        let service = Service { upstreams: Some(vec![named]), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn an_id_with_disallowed_characters_is_rejected_by_the_schema() {
        let service = Service { id: Some("not valid!".into()), ..minimal_service() };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(format_path(&issues[0].path), "services[0].id");
    }

    #[test]
    fn an_ssl_certificate_shorter_than_the_minimum_length_is_rejected_by_the_schema() {
        use crate::resources::{SSL, SSLCertificate};
        let ssl = SSL {
            id: None,
            labels: None,
            r#type: Default::default(),
            snis: vec!["example.com".into()],
            certificates: vec![SSLCertificate { certificate: "short".into(), key: "x".repeat(32) }],
            client: None,
            ssl_protocols: None,
        };
        let config = Configuration { ssls: Some(vec![ssl]), ..empty_config() };
        assert!(!lint(&config).is_empty());
    }

    #[test]
    fn an_ssl_certificate_as_a_secret_reference_is_accepted() {
        use crate::resources::{SSL, SSLCertificate};
        let ssl = SSL {
            id: None,
            labels: None,
            r#type: Default::default(),
            snis: vec!["example.com".into()],
            certificates: vec![SSLCertificate { certificate: "$secret://vault/cert".into(), key: "$env://TLS_KEY".into() }],
            client: None,
            ssl_protocols: None,
        };
        let config = Configuration { ssls: Some(vec![ssl]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    /// An invalid `certificate`/`key` value fails the schema's `anyOf`
    /// check — `jsonschema`'s default error message would otherwise embed
    /// the offending value verbatim, leaking secret material into lint output.
    #[test]
    fn an_invalid_ssl_key_value_is_not_echoed_into_the_lint_message() {
        use crate::resources::{SSL, SSLCertificate};
        // Under the 32-char minimum, so it fails validation (too short to be
        // real PEM content, and doesn't match the `$secret://`/`$env://`
        // reference pattern either) — short enough on purpose, to prove the
        // failure path in particular doesn't echo it back.
        let secret_value = "sk_live_UNIQUE_MARKER_9f3a7c";
        let ssl = SSL {
            id: None,
            labels: None,
            r#type: Default::default(),
            snis: vec!["example.com".into()],
            certificates: vec![SSLCertificate { certificate: "x".repeat(128), key: secret_value.into() }],
            client: None,
            ssl_protocols: None,
        };
        let config = Configuration { ssls: Some(vec![ssl]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert!(
            !issues[0].message.contains(secret_value),
            "lint message leaked the secret value: {}",
            issues[0].message
        );
    }

    #[test]
    fn a_consumer_credential_with_a_disallowed_type_is_rejected() {
        let consumer = Consumer {
            username: "u".into(),
            description: None,
            labels: None,
            plugins: None,
            credentials: Some(vec![ConsumerCredential {
                id: None,
                name: "c".into(),
                description: None,
                labels: None,
                r#type: "totally-made-up-auth".into(),
                config: Default::default(),
            }]),
        };
        let config = Configuration { consumers: Some(vec![consumer]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(format_path(&issues[0].path), "consumers[0].credentials[0].type");
    }

    #[test]
    fn a_consumer_credential_with_an_allowed_type_is_accepted() {
        let consumer = Consumer {
            username: "u".into(),
            description: None,
            labels: None,
            plugins: None,
            credentials: Some(vec![ConsumerCredential {
                id: None,
                name: "c".into(),
                description: None,
                labels: None,
                r#type: "key-auth".into(),
                config: Default::default(),
            }]),
        };
        let config = Configuration { consumers: Some(vec![consumer]), ..empty_config() };
        assert_eq!(lint(&config), Vec::new());
    }

    #[test]
    fn a_credential_type_violation_nested_inside_a_consumer_group_is_found() {
        let consumer = Consumer {
            username: "u".into(),
            description: None,
            labels: None,
            plugins: None,
            credentials: Some(vec![ConsumerCredential {
                id: None,
                name: "c".into(),
                description: None,
                labels: None,
                r#type: "bogus".into(),
                config: Default::default(),
            }]),
        };
        let group = ConsumerGroup { id: None, name: "g".into(), description: None, labels: None, plugins: None, consumers: Some(vec![consumer]) };
        let config = Configuration { consumer_groups: Some(vec![group]), ..empty_config() };
        let issues = lint(&config);
        assert_eq!(issues.len(), 1);
        assert_eq!(format_path(&issues[0].path), "consumer_groups[0].consumers[0].credentials[0].type");
    }

    #[test]
    fn multiple_violations_are_all_collected_in_one_pass() {
        let mut upstream = minimal_upstream_with_nodes();
        upstream.discovery_type = Some("dns".into());
        upstream.service_name = Some("svc.local".into());
        let service = Service {
            id: Some("bad id!".into()),
            path_prefix: Some("no-slash".into()),
            upstream: Some(upstream),
            ..minimal_service()
        };
        let config = Configuration { services: Some(vec![service]), ..empty_config() };
        let issues = lint(&config);
        // id charset (schema), path_prefix leading slash, nodes/discovery
        // conflict — three independent violations, all reported together.
        assert_eq!(issues.len(), 3);
    }
}
