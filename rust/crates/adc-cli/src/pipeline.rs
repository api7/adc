//! The sequential stages every backend-talking command runs through:
//! `init_backend -> load_local -> load_remote -> diff -> {sync,validate}`.
//! Each stage is a plain function threaded together by its caller in
//! `main.rs` — there's no task-runner abstraction here (that was `listr2`'s
//! job in the TS CLI, driven by its progress-rendering needs, which this
//! CLI doesn't have yet).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use adc_backend_core::{HttpClient, HttpClientConfig, ResourceFilter, TlsConfig};
use adc_differ::DifferV4;
use adc_sdk::resources::Configuration;
use adc_sdk::{Backend, Event, InternalConfiguration, ResourceType};

use crate::cli::{BackendArgs, BackendKind};
use crate::config;
use crate::error::CliError;

pub async fn init_backend(args: &BackendArgs) -> Result<Box<dyn Backend>, CliError> {
    let filter = resource_filter(args)?;
    match args.backend {
        BackendKind::Apisix => {
            let (client, _token) = build_client(args).await?;
            Ok(Box::new(adc_backend_apisix::Backend::new(client, filter)))
        }
        BackendKind::Api7Ee => {
            let (client, token) = build_client(args).await?;
            Ok(Box::new(adc_backend_api7::Backend::new(
                client,
                args.gateway_group.clone(),
                &token,
                filter,
            )))
        }
        BackendKind::ApisixStandalone => Err(CliError::msg(format!(
            "backend \"{}\" is not yet implemented (only \"apisix\"/\"api7ee\" are supported so far)",
            args.backend.as_str()
        ))),
    }
}

/// Shared by every backend: the `X-API-KEY`/TLS-configured `HttpClient`
/// every one of them wraps. Returns the raw token alongside it — `api7ee`
/// needs it separately (to recognize an `a7adm-` admin token, which skips
/// gateway_group resolution entirely), not just baked into the client's
/// headers.
async fn build_client(args: &BackendArgs) -> Result<(HttpClient, String), CliError> {
    let token = args.token.clone().ok_or_else(|| {
        CliError::msg("a backend token is required: pass --token or set ADC_TOKEN")
    })?;
    let ca_cert_pem = read_optional(&args.ca_cert_file).await?;
    let client_cert_pem = read_optional(&args.tls_client_cert_file).await?;
    let client_key_pem = read_optional(&args.tls_client_key_file).await?;
    let client = HttpClient::new(HttpClientConfig {
        server: args.server.clone(),
        token: token.clone(),
        timeout: Some(args.timeout),
        tls: TlsConfig {
            ca_cert_pem,
            client_cert_pem,
            client_key_pem,
            skip_verify: args.tls_skip_verify,
        },
    })?;
    Ok((client, token))
}

async fn read_optional(path: &Option<PathBuf>) -> Result<Option<Vec<u8>>, CliError> {
    match path {
        Some(path) => Ok(Some(tokio::fs::read(path).await?)),
        None => Ok(None),
    }
}

pub fn resource_type_sets(args: &BackendArgs) -> (HashSet<ResourceType>, HashSet<ResourceType>) {
    let include = args
        .include_resource_type
        .iter()
        .map(|t| (*t).into())
        .collect();
    let exclude = args
        .exclude_resource_type
        .iter()
        .map(|t| (*t).into())
        .collect();
    (include, exclude)
}

/// Parses `--label-selector key=value` entries into a map. Rejects an entry
/// without a `=` rather than silently dropping it — a typo here should fail
/// loudly, not quietly select nothing.
pub fn label_selector_map(args: &BackendArgs) -> Result<HashMap<String, String>, CliError> {
    args.label_selector
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

/// The filter a backend applies at fetch time: skipping whole resource
/// types the request never needed, and (where the admin API supports it)
/// asking the server itself to narrow results by label. This is an
/// optimization only — `config::filter_resource_types`/`filter_by_labels`
/// still run afterward and are what actually guarantee the result matches.
fn resource_filter(args: &BackendArgs) -> Result<ResourceFilter, CliError> {
    let (include, exclude) = resource_type_sets(args);
    let label_selector = label_selector_map(args)?;
    Ok(ResourceFilter {
        include,
        exclude,
        label_selector,
    })
}

/// Loads, merges, and structurally parses the local configuration file(s).
/// Deserializing into `Configuration` here is the structural-validity gate
/// (unknown fields, wrong types, missing required fields all reject) — the
/// separate `--no-lint`/`Lint` step has nothing left to check yet, since
/// semantic validation (regex/cross-field rules) hasn't landed (stage 2.2).
pub async fn load_local(
    files: &[PathBuf],
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
    label_selector: &HashMap<String, String>,
    managed_by_label: bool,
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
    Ok(configuration)
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
