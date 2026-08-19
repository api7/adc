//! The sequential stages every backend-talking command runs through:
//! `init_backend -> load_local -> load_remote -> diff -> {sync,validate}`.
//! Each stage is a plain function threaded together by its caller in
//! `main.rs`, wrapped in `progress::stage` for display — no separate
//! task-runner abstraction on top of that.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use adc_backend_core::{HttpClient, HttpClientConfig, ResourceFilter, TlsConfig};
use adc_differ::DifferV4;
use adc_sdk::resources::Configuration;
use adc_sdk::{Backend, Converter, Event, InternalConfiguration, ResourceType};

use crate::cli::BackendArgs;
use crate::config;
use crate::error::CliError;

/// Everything [`init_backend`] needs to pick and construct a backend,
/// independent of where the caller sourced it from (CLI args or an
/// ingress-server request body).
pub struct BackendSpec {
    pub kind: String,
    pub servers: Vec<String>,
    pub tokens: Vec<String>,
    pub gateway_group: Option<String>,
    pub filter: ResourceFilter,
    pub concurrency: usize,
    pub cache_key: String,
    pub bypass_cache: bool,
    pub timeout: Option<Duration>,
    pub tls: TlsConfig,
}

/// CLI args resolve into a `BackendSpec` with no I/O — `--ca-cert-file` and
/// friends already read their file's bytes at argument-parse time (see
/// `cli::read_file_bytes`), so this is a plain, synchronous field mapping.
/// Fallible only because `label_selector_map` rejects `managed-by` as a
/// selector key.
impl TryFrom<&BackendArgs> for BackendSpec {
    type Error = CliError;

    fn try_from(args: &BackendArgs) -> Result<Self, CliError> {
        let (include, exclude) = resource_type_sets(args);
        let label_selector = label_selector_map(args)?;
        let token = args.token.as_deref().ok_or_else(|| {
            CliError::msg("a backend token is required: pass --token or set ADC_TOKEN")
        })?;

        Ok(BackendSpec {
            kind: args.backend.as_str().to_string(),
            servers: args.server.split(',').map(str::to_string).collect(),
            tokens: token.split(',').map(str::to_string).collect(),
            gateway_group: Some(args.gateway_group.clone()),
            filter: ResourceFilter {
                include,
                exclude,
                label_selector,
            },
            concurrency: args.request_concurrent,
            cache_key: "default".to_string(),
            bypass_cache: false,
            timeout: Some(args.timeout),
            tls: TlsConfig {
                ca_cert_pem: args.ca_cert_pem.clone(),
                client_cert_pem: args.tls_client_cert_pem.clone(),
                client_key_pem: args.tls_client_key_pem.clone(),
                skip_verify: args.tls_skip_verify,
            },
        })
    }
}

/// `shared_client`: `None` builds a fresh `HttpClient` from `spec.tls` (the
/// one-shot CLI); `Some` reuses an already-built, pooled `reqwest::Client`
/// (the ingress-server daemon). Only `apisix`/`api7ee` look at it —
/// `apisix-standalone` always builds its own clients from `spec.tls`.
pub fn init_backend(
    spec: BackendSpec,
    shared_client: Option<reqwest::Client>,
) -> Result<Box<dyn Backend>, CliError> {
    let server = spec.servers.first().cloned().unwrap_or_default();
    let token = spec.tokens.first().cloned().unwrap_or_default();

    match spec.kind.as_str() {
        "api7ee" => {
            let client = http_client(shared_client, &server, &token, spec.timeout, &spec.tls)?;
            let gateway_group = spec.gateway_group.unwrap_or_else(|| "default".to_string());
            Ok(Box::new(adc_backend_api7::Backend::new(
                client,
                gateway_group,
                &token,
                spec.filter,
                spec.concurrency,
            )))
        }
        "apisix-standalone" => Ok(Box::new(adc_backend_apisix_standalone::Backend::new(
            adc_backend_apisix_standalone::BackendOptions {
                servers: spec.servers,
                tokens: spec.tokens,
                cache_key: spec.cache_key,
                bypass_cache: spec.bypass_cache,
                timeout: spec.timeout,
                tls: spec.tls,
            },
        )?)),
        // "apisix" and anything unrecognized both default to apisix.
        _ => {
            let client = http_client(shared_client, &server, &token, spec.timeout, &spec.tls)?;
            Ok(Box::new(adc_backend_apisix::Backend::new(
                client,
                spec.filter,
                spec.concurrency,
            )))
        }
    }
}

fn http_client(
    shared_client: Option<reqwest::Client>,
    server: &str,
    token: &str,
    timeout: Option<Duration>,
    tls: &TlsConfig,
) -> Result<HttpClient, CliError> {
    Ok(match shared_client {
        None => HttpClient::new(HttpClientConfig {
            server: server.to_string(),
            token: token.to_string(),
            timeout,
            tls: tls.clone(),
        })?,
        Some(client) => {
            HttpClient::with_shared_client(client, server.to_string(), token.to_string())?
        }
    })
}

/// Resource types nested under a service/consumer rather than a top-level
/// `Configuration` field — `config::filter_resource_types` only ever drops
/// or keeps whole top-level fields, so naming one of these here has no
/// effect at all (see the CLI's own doc comment on the two flags below).
const UNFILTERABLE_RESOURCE_TYPES: &[(&str, ResourceType)] = &[
    ("route", ResourceType::Route),
    ("upstream", ResourceType::Upstream),
    ("stream_route", ResourceType::StreamRoute),
    ("consumer_credential", ResourceType::ConsumerCredential),
    ("plugin_config", ResourceType::PluginConfig),
];

fn warn_on_unfilterable_resource_types(flag: &str, set: &HashSet<ResourceType>) {
    let named: Vec<&str> = UNFILTERABLE_RESOURCE_TYPES
        .iter()
        .filter(|(_, rt)| set.contains(rt))
        .map(|(name, _)| *name)
        .collect();
    if !named.is_empty() {
        tracing::warn!(
            "{flag} does not support {}: these are nested under a service or consumer, not a \
             top-level resource, so filtering by them has no effect. Support for naming them here \
             will be removed in a future release.",
            named.join(", ")
        );
    }
}

pub fn resource_type_sets(args: &BackendArgs) -> (HashSet<ResourceType>, HashSet<ResourceType>) {
    let mut include: HashSet<ResourceType> = args
        .include_resource_type
        .iter()
        .map(|t| (*t).into())
        .collect();
    let mut exclude: HashSet<ResourceType> = args
        .exclude_resource_type
        .iter()
        .map(|t| (*t).into())
        .collect();
    warn_on_unfilterable_resource_types("--include-resource-type", &include);
    warn_on_unfilterable_resource_types("--exclude-resource-type", &exclude);
    // Stripped here, after warning above, so "has no effect" is actually
    // true downstream: `ResourceFilter::is_skip`/`config::filter_resource_types`
    // only ever check top-level types, so a nested one left in `include`
    // would make every real top-level type look excluded instead of doing
    // nothing.
    let is_unfilterable = |rt: &ResourceType| {
        UNFILTERABLE_RESOURCE_TYPES
            .iter()
            .any(|(_, unfilterable)| unfilterable == rt)
    };
    include.retain(|rt| !is_unfilterable(rt));
    exclude.retain(|rt| !is_unfilterable(rt));
    (include, exclude)
}

/// Parses `--label-selector key=value` entries into a map. Unconditionally
/// rejects `managed-by` as a key.
pub fn label_selector_map(args: &BackendArgs) -> Result<HashMap<String, String>, CliError> {
    let selector = parse_label_selector(&args.label_selector)?;
    if selector.contains_key(config::MANAGED_BY_LABEL_KEY) {
        return Err(CliError::msg(format!(
            "--label-selector cannot filter on \"{}\"",
            config::MANAGED_BY_LABEL_KEY
        )));
    }
    Ok(selector)
}

/// Rejects an entry without a `=` rather than silently dropping it — a typo
/// here should fail loudly, not quietly select nothing.
fn parse_label_selector(entries: &[String]) -> Result<HashMap<String, String>, CliError> {
    entries
        .iter()
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .ok_or_else(|| {
                    CliError::msg(format!(
                        "invalid --label-selector \"{entry}\": expected \"key=value\""
                    ))
                })
        })
        .collect()
}

/// Loads, merges, and structurally parses the local configuration file(s),
/// then (unless `lint` is `false`, i.e. `--no-lint`) runs semantic
/// validation on top. Deserializing into `Configuration` is the
/// structural-validity gate (unknown fields, wrong types, missing required
/// fields all reject — except inside a plugin config body: `Plugin`/
/// `Plugins` are bare maps, deliberately not `deny_unknown_fields`, since
/// ADC can't know every plugin's own schema) and always runs, regardless of
/// `lint` — only the semantic pass (`adc_sdk::lint::lint`) is skippable.
pub async fn load_local(
    files: &[PathBuf],
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
    label_selector: &HashMap<String, String>,
    managed_by_label: bool,
    lint: bool,
) -> Result<Configuration, CliError> {
    let files = config::read_files(files).await?;
    let mut merged = config::merge_files(files)?;
    config::fill_labels(&mut merged, label_selector);
    if managed_by_label {
        config::inject_managed_by_label(&mut merged);
    }
    let mut configuration: Configuration = serde_json::from_value(merged)
        .map_err(|e| CliError::msg(format!("invalid configuration: {e}")))?;
    config::filter_resource_types(&mut configuration, include, exclude);
    if lint {
        let issues = adc_sdk::lint::lint(&configuration);
        if !issues.is_empty() {
            return Err(CliError::msg(format_lint_issues(&issues)));
        }
    }
    Ok(configuration)
}

/// Collects every lint violation into one multi-line message.
fn format_lint_issues(issues: &[adc_sdk::lint::LintIssue]) -> String {
    let mut message =
        "Lint configuration\nThe following errors were found in configuration:\n".to_string();
    for issue in issues {
        message.push_str(&format!("  - {issue}\n"));
    }
    message.pop();
    message
}

/// Converts each OpenAPI document into its own `Configuration`, then
/// flattens their `services` into one — rejecting outright if two
/// documents produce a same-named service, since a resource's id is
/// derived from its name (`generate_id`) and two same-named services would
/// silently collide into one on sync.
pub async fn convert_openapi(files: &[PathBuf]) -> Result<Configuration, CliError> {
    let paths = config::resolve_files(files, None).await?;
    let mut per_file = Vec::with_capacity(paths.len());
    for path in &paths {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| CliError::msg(format!("{}: {e}", path.display())))?;
        let converted = adc_converter_openapi::OpenApiConverter
            .to_adc(&content)
            .map_err(|e| {
                CliError::msg(format!(
                    "failed to convert OpenAPI document \"{}\": {e}",
                    path.display()
                ))
            })?;
        per_file.push((path.clone(), converted.services.unwrap_or_default()));
    }
    Ok(Configuration {
        services: Some(merge_openapi_services(per_file)?),
        ssls: None,
        consumers: None,
        consumer_groups: None,
        global_rules: None,
        plugin_metadata: None,
    })
}

/// Flattens each input file's services into one list, in file order —
/// rejecting outright on the first same-named service instead of silently
/// keeping both (a resource's id is derived from its name via
/// `generate_id`, so two same-named services would collide into one
/// ambiguous resource on sync).
fn merge_openapi_services(
    per_file: Vec<(PathBuf, Vec<adc_sdk::resources::Service>)>,
) -> Result<Vec<adc_sdk::resources::Service>, CliError> {
    let mut services = Vec::new();
    let mut seen_names: HashMap<String, PathBuf> = HashMap::new();
    for (path, file_services) in per_file {
        for service in file_services {
            if let Some(first_path) = seen_names.insert(service.name.clone(), path.clone()) {
                return Err(CliError::msg(format!(
                    "{}: duplicate service name \"{}\" (already produced by {})",
                    path.display(),
                    service.name,
                    first_path.display()
                )));
            }
            services.push(service);
        }
    }
    Ok(services)
}

pub async fn load_remote(
    backend: &dyn Backend,
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
    label_selector: &HashMap<String, String>,
) -> Result<Configuration, CliError> {
    let mut configuration = backend.dump().await?;
    config::filter_resource_types(&mut configuration, include, exclude);
    config::filter_by_labels(&mut configuration, label_selector);
    Ok(configuration)
}

pub async fn diff(
    backend: &dyn Backend,
    local: &Configuration,
    remote: &Configuration,
) -> Result<Vec<Event>, CliError> {
    let default_value = backend.default_value().await?;
    let local_map = to_diff_map(local)?;
    let remote_map = to_diff_map(remote)?;
    Ok(DifferV4::diff(
        &local_map,
        &remote_map,
        Some(&default_value),
        None,
    ))
}

fn to_diff_map(configuration: &Configuration) -> Result<InternalConfiguration, CliError> {
    match serde_json::to_value(configuration)? {
        serde_json::Value::Object(map) => Ok(map),
        _ => unreachable!("Configuration always serializes to a JSON object"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::cli::{BackendKind, ResourceTypeArg};

    use super::*;

    #[test]
    fn parses_key_value_entries_into_a_map() {
        let selector =
            parse_label_selector(&["env=prod".to_string(), "team=core".to_string()]).unwrap();
        assert_eq!(selector.get("env"), Some(&"prod".to_string()));
        assert_eq!(selector.get("team"), Some(&"core".to_string()));
    }

    #[test]
    fn rejects_an_entry_with_no_equals_sign() {
        assert!(parse_label_selector(&["not-a-pair".to_string()]).is_err());
    }

    #[test]
    fn an_empty_selector_is_fine() {
        assert!(parse_label_selector(&[]).unwrap().is_empty());
    }

    fn service(name: &str) -> adc_sdk::resources::Service {
        adc_sdk::resources::Service {
            id: None,
            name: name.to_string(),
            description: None,
            labels: None,
            upstream: None,
            upstreams: None,
            plugins: None,
            path_prefix: None,
            strip_path_prefix: None,
            hosts: None,
            routes: None,
        }
    }

    #[test]
    fn merge_openapi_services_concatenates_in_file_order() {
        let per_file = vec![
            (PathBuf::from("a.yaml"), vec![service("svc-a")]),
            (PathBuf::from("b.yaml"), vec![service("svc-b")]),
        ];
        let merged = merge_openapi_services(per_file).unwrap();
        assert_eq!(
            merged.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["svc-a", "svc-b"]
        );
    }

    #[test]
    fn merge_openapi_services_rejects_a_duplicate_name_across_files() {
        let per_file = vec![
            (PathBuf::from("a.yaml"), vec![service("shared")]),
            (PathBuf::from("b.yaml"), vec![service("shared")]),
        ];
        let err = merge_openapi_services(per_file).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("b.yaml"), "{message}");
        assert!(
            message.contains("a.yaml"),
            "{message}: should name the file that first produced this service"
        );
        assert!(message.contains("shared"), "{message}");
    }

    #[test]
    fn merge_openapi_services_rejects_a_duplicate_name_within_the_same_file() {
        let per_file = vec![(
            PathBuf::from("a.yaml"),
            vec![service("shared"), service("shared")],
        )];
        assert!(merge_openapi_services(per_file).is_err());
    }

    fn backend_args(label_selector: Vec<String>) -> BackendArgs {
        BackendArgs {
            backend: BackendKind::Apisix,
            server: "http://localhost:9180".to_string(),
            token: None,
            gateway_group: "default".to_string(),
            label_selector,
            include_resource_type: vec![],
            exclude_resource_type: vec![],
            timeout: Duration::from_secs(10),
            request_concurrent: 10,
            ca_cert_pem: None,
            tls_client_cert_pem: None,
            tls_client_key_pem: None,
            tls_skip_verify: false,
            managed_by_label: true,
        }
    }

    #[test]
    fn backend_spec_splits_comma_joined_servers_and_tokens() {
        let mut args = backend_args(vec![]);
        args.server = "http://a:9180,http://b:9180".to_string();
        args.token = Some("tok-a,tok-b".to_string());
        let spec = BackendSpec::try_from(&args).unwrap();
        assert_eq!(spec.servers, vec!["http://a:9180", "http://b:9180"]);
        assert_eq!(spec.tokens, vec!["tok-a", "tok-b"]);
    }

    #[test]
    fn backend_spec_accepts_a_single_server_and_token() {
        let mut args = backend_args(vec![]);
        args.token = Some("tok".to_string());
        let spec = BackendSpec::try_from(&args).unwrap();
        assert_eq!(spec.servers, vec!["http://localhost:9180"]);
        assert_eq!(spec.tokens, vec!["tok"]);
    }

    #[test]
    fn backend_spec_rejects_a_missing_token() {
        let args = backend_args(vec![]);
        assert!(BackendSpec::try_from(&args).is_err());
    }

    #[test]
    fn rejects_managed_by_as_a_selector_key_regardless_of_the_value_supplied() {
        let args = backend_args(vec!["managed-by=custom".to_string()]);
        let error = label_selector_map(&args).unwrap_err();
        assert!(error.to_string().contains("managed-by"), "{error}");
    }

    #[test]
    fn a_managed_by_label_selector_regression_is_rejected_outright() {
        // --managed-by-label (the default) together with
        // --label-selector managed-by=<value> — this used to let the
        // automatic stamp silently win over the selector's value; now it's
        // rejected outright instead.
        let args = backend_args(vec!["managed-by=custom".to_string()]);
        assert!(label_selector_map(&args).is_err());
    }

    #[derive(Clone, Default)]
    struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn captured_warning(set: &HashSet<ResourceType>) -> String {
        let buffer = SharedBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            warn_on_unfilterable_resource_types("--include-resource-type", set);
        });
        String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn warns_when_a_nested_resource_type_is_named() {
        let output = captured_warning(&HashSet::from([ResourceType::Route, ResourceType::Service]));
        assert!(output.contains("route"), "{output}");
        assert!(output.contains("--include-resource-type"), "{output}");
    }

    #[test]
    fn no_warning_when_only_top_level_types_are_named() {
        let output = captured_warning(&HashSet::from([
            ResourceType::Service,
            ResourceType::Consumer,
        ]));
        assert!(output.is_empty(), "{output}");
    }

    #[test]
    fn resource_type_sets_strips_unfilterable_types_but_keeps_real_ones() {
        let mut args = backend_args(vec![]);
        args.include_resource_type = vec![ResourceTypeArg::Route, ResourceTypeArg::Service];
        let (include, exclude) = resource_type_sets(&args);
        assert_eq!(include, HashSet::from([ResourceType::Service]));
        assert!(exclude.is_empty());
    }

    #[test]
    fn resource_type_sets_with_only_unfilterable_types_ends_up_empty() {
        let mut args = backend_args(vec![]);
        args.include_resource_type = vec![ResourceTypeArg::Route, ResourceTypeArg::Upstream];
        let (include, _) = resource_type_sets(&args);
        assert!(
            include.is_empty(),
            "an include set with nothing but unfilterable types must behave like no --include-resource-type was passed at all"
        );
    }
}
