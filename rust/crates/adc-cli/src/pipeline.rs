//! The sequential stages every backend-talking command runs through:
//! `init_backend -> load_local -> load_remote -> diff -> {sync,validate}`.
//! Each stage is a plain function threaded together by its caller in
//! `main.rs` — there's no task-runner abstraction here (that was `listr2`'s
//! job in the TS CLI, driven by its progress-rendering needs, which this
//! CLI doesn't have yet).

use std::collections::HashSet;
use std::path::PathBuf;

use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};
use adc_differ::DifferV4;
use adc_sdk::resources::Configuration;
use adc_sdk::{Backend, Event, InternalConfiguration, ResourceType};

use crate::cli::{BackendArgs, BackendKind};
use crate::config;
use crate::error::CliError;

pub async fn init_backend(args: &BackendArgs) -> Result<Box<dyn Backend>, CliError> {
    match args.backend {
        BackendKind::Apisix => {
            let ca_cert_pem = read_optional(&args.ca_cert_file).await?;
            let client_cert_pem = read_optional(&args.tls_client_cert_file).await?;
            let client_key_pem = read_optional(&args.tls_client_key_file).await?;
            let client = HttpClient::new(HttpClientConfig {
                server: args.server.clone(),
                token: args.token.clone().unwrap_or_default(),
                timeout: Some(args.timeout),
                tls: TlsConfig {
                    ca_cert_pem,
                    client_cert_pem,
                    client_key_pem,
                    skip_verify: args.tls_skip_verify,
                },
            })?;
            Ok(Box::new(adc_backend_apisix::Backend::new(client)))
        }
        BackendKind::Api7Ee | BackendKind::ApisixStandalone => Err(CliError::msg(format!(
            "backend \"{}\" is not yet implemented (only \"apisix\" is supported so far)",
            args.backend.as_str()
        ))),
    }
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

/// Loads, merges, and structurally parses the local configuration file(s).
/// Deserializing into `Configuration` here is the structural-validity gate
/// (unknown fields, wrong types, missing required fields all reject) — the
/// separate `--no-lint`/`Lint` step has nothing left to check yet, since
/// semantic validation (regex/cross-field rules) hasn't landed (stage 2.2).
pub async fn load_local(
    files: &[PathBuf],
    include: &HashSet<ResourceType>,
    exclude: &HashSet<ResourceType>,
    managed_by_label: bool,
) -> Result<Configuration, CliError> {
    let files = config::read_files(files).await?;
    let mut merged = config::merge_files(files)?;
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
) -> Result<Configuration, CliError> {
    let mut configuration = backend.dump().await?;
    config::filter_resource_types(&mut configuration, include, exclude);
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
