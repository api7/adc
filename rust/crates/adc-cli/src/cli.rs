use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "adc",
    version,
    about = "Sync declarative configuration to Apache APISIX / API7 Enterprise"
)]
pub struct Cli {
    /// set the verbosity level for logs (0: no logs, 1: basic logs, 2: debug logs)
    #[arg(long, global = true, default_value_t = 1, value_parser = clap::value_parser!(u8).range(0..=2))]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// check connectivity with the backend
    Ping(BackendArgs),
    /// save the configuration of the backend to a file
    Dump(DumpArgs),
    /// show differences between the local and the backend configurations
    Diff(DiffArgs),
    /// sync the local configuration to the backend
    Sync(SyncArgs),
    /// lint the local configuration
    Lint(LintArgs),
    /// validate the local configuration against the backend
    Validate(ValidateArgs),
    /// convert API definitions in other formats to ADC configuration
    Convert(ConvertArgs),
    /// synchronize configuration via the ingress controller
    #[command(hide = true)]
    IngressSync,
    /// run the local ingress server
    #[command(hide = true)]
    IngressServer(IngressServerArgs),
}

#[derive(Args, Debug)]
pub struct IngressServerArgs {
    /// listen address of the ADC server, in the form scheme://host:port
    /// (http/https/unix are supported)
    #[arg(long, default_value = "http://127.0.0.1:3000", value_parser = parse_listen_url)]
    pub listen: url::Url,

    /// status listen port (exposes GET /healthz/ready)
    #[arg(long, default_value_t = 3001, value_parser = clap::value_parser!(u16).range(1..=65535))]
    pub listen_status: u16,

    /// path to the CA certificate used to verify client certificates (enables mTLS)
    #[arg(long, value_parser = existing_file)]
    pub ca_cert_file: Option<PathBuf>,

    /// path to the TLS server certificate (required for https:// listen addresses)
    #[arg(long, value_parser = existing_file)]
    pub tls_cert_file: Option<PathBuf>,

    /// path to the TLS server key (required for https:// listen addresses)
    #[arg(long, value_parser = existing_file)]
    pub tls_key_file: Option<PathBuf>,
}

fn parse_listen_url(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw).map_err(|e| e.to_string())?;
    match url.scheme() {
        "http" | "https" | "unix" => Ok(url),
        other => Err(format!(
            "unsupported --listen scheme \"{other}\": expected http, https, or unix"
        )),
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    #[value(name = "apisix")]
    Apisix,
    #[value(name = "api7ee")]
    Api7Ee,
    #[value(name = "apisix-standalone")]
    ApisixStandalone,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendKind::Apisix => "apisix",
            BackendKind::Api7Ee => "api7ee",
            BackendKind::ApisixStandalone => "apisix-standalone",
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub enum ResourceTypeArg {
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

impl From<ResourceTypeArg> for adc_sdk::ResourceType {
    fn from(value: ResourceTypeArg) -> Self {
        match value {
            ResourceTypeArg::Route => adc_sdk::ResourceType::Route,
            ResourceTypeArg::Service => adc_sdk::ResourceType::Service,
            ResourceTypeArg::Upstream => adc_sdk::ResourceType::Upstream,
            ResourceTypeArg::Ssl => adc_sdk::ResourceType::Ssl,
            ResourceTypeArg::GlobalRule => adc_sdk::ResourceType::GlobalRule,
            ResourceTypeArg::PluginConfig => adc_sdk::ResourceType::PluginConfig,
            ResourceTypeArg::PluginMetadata => adc_sdk::ResourceType::PluginMetadata,
            ResourceTypeArg::Consumer => adc_sdk::ResourceType::Consumer,
            ResourceTypeArg::ConsumerGroup => adc_sdk::ResourceType::ConsumerGroup,
            ResourceTypeArg::ConsumerCredential => adc_sdk::ResourceType::ConsumerCredential,
            ResourceTypeArg::StreamRoute => adc_sdk::ResourceType::StreamRoute,
        }
    }
}

/// Options shared by every command that talks to a backend (all except `lint`).
#[derive(Args, Debug)]
pub struct BackendArgs {
    /// type of backend to connect to
    #[arg(long, env = "ADC_BACKEND", default_value = "apisix")]
    pub backend: BackendKind,

    /// HTTP address of the backend
    #[arg(long, env = "ADC_SERVER", default_value = "http://localhost:9180")]
    pub server: String,

    /// token for ADC to connect to the backend
    #[arg(long, env = "ADC_TOKEN")]
    pub token: Option<String>,

    /// gateway group to operate on (only supported for the "api7ee" backend)
    #[arg(long, env = "ADC_GATEWAY_GROUP", default_value = "default")]
    pub gateway_group: String,

    /// filter resources by labels (key=value, comma-separated or repeatable)
    #[arg(long, value_delimiter = ',')]
    pub label_selector: Vec<String>,

    /// filter resources that only contain the specified type. Applies to
    /// top-level types only — route/upstream/stream_route/consumer_credential/plugin_config
    /// live nested under a service or consumer, so naming them has no
    /// effect (support for them will be removed)
    #[arg(long, conflicts_with = "exclude_resource_type")]
    pub include_resource_type: Vec<ResourceTypeArg>,

    /// filter resources that do not contain the specified type (top-level
    /// types only — see `--include-resource-type`)
    #[arg(long, conflicts_with = "include_resource_type")]
    pub exclude_resource_type: Vec<ResourceTypeArg>,

    /// timeout for adc to connect with the backend (examples: 10s, 1h10m)
    #[arg(long, default_value = "10s", value_parser = parse_timeout)]
    pub timeout: Duration,

    /// number of concurrent requests to the backend (both fetching remote
    /// state and, for `sync`, applying it)
    #[arg(long, default_value_t = 10)]
    pub request_concurrent: usize,

    /// path to the CA certificate to verify the backend
    #[arg(long = "ca-cert-file", env = "ADC_CA_CERT_FILE", value_parser = read_file_bytes)]
    pub ca_cert_pem: Option<Vec<u8>>,

    /// path to the mutual TLS client certificate to verify the backend
    #[arg(long = "tls-client-cert-file", env = "ADC_TLS_CLIENT_CERT_FILE", value_parser = read_file_bytes, requires = "tls_client_key_pem")]
    pub tls_client_cert_pem: Option<Vec<u8>>,

    /// path to the mutual TLS client key to verify the backend
    #[arg(long = "tls-client-key-file", env = "ADC_TLS_CLIENT_KEY_FILE", value_parser = read_file_bytes, requires = "tls_client_cert_pem")]
    pub tls_client_key_pem: Option<Vec<u8>>,

    /// disable the verification of the backend TLS certificate
    #[arg(long, env = "ADC_TLS_SKIP_VERIFY")]
    pub tls_skip_verify: bool,

    /// disable injecting the "managed-by=adc" label into synced resources
    #[arg(long = "no-managed-by-label", action = clap::ArgAction::SetFalse)]
    pub managed_by_label: bool,
}

#[derive(Args, Debug)]
pub struct DumpArgs {
    #[command(flatten)]
    pub backend: BackendArgs,

    /// path of the file to save the configuration
    #[arg(short, long, default_value = "adc.yaml")]
    pub output: PathBuf,

    /// dump remote resources id
    #[arg(long)]
    pub with_id: bool,
}

#[derive(Args, Debug)]
pub struct DiffArgs {
    #[command(flatten)]
    pub backend: BackendArgs,

    /// file to compare
    #[arg(short, long = "file")]
    pub files: Vec<PathBuf>,

    /// disable lint check
    #[arg(long = "no-lint", action = clap::ArgAction::SetFalse)]
    pub lint: bool,
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    #[command(flatten)]
    pub backend: BackendArgs,

    /// file to synchronize
    #[arg(short, long = "file")]
    pub files: Vec<PathBuf>,

    /// disable lint check
    #[arg(long = "no-lint", action = clap::ArgAction::SetFalse)]
    pub lint: bool,
}

#[derive(Args, Debug)]
pub struct LintArgs {
    /// file to lint
    #[arg(short, long = "file")]
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ConvertArgs {
    #[command(subcommand)]
    pub format: ConvertFormat,
}

#[derive(Subcommand, Debug)]
pub enum ConvertFormat {
    /// convert an OpenAPI specification to ADC configuration
    #[command(name = "openapi")]
    OpenApi(ConvertOpenApiArgs),
}

#[derive(Args, Debug)]
pub struct ConvertOpenApiArgs {
    /// OpenAPI specification file path
    #[arg(short, long = "file", required = true)]
    pub files: Vec<PathBuf>,

    /// output file path
    #[arg(short, long, default_value = "adc.yaml")]
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct ValidateArgs {
    #[command(flatten)]
    pub backend: BackendArgs,

    /// file to validate
    #[arg(short, long = "file")]
    pub files: Vec<PathBuf>,

    /// disable lint check
    #[arg(long = "no-lint", action = clap::ArgAction::SetFalse)]
    pub lint: bool,
}

fn parse_timeout(raw: &str) -> Result<Duration, String> {
    humantime::parse_duration(raw).map_err(|err| err.to_string())
}

fn existing_file(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !path.is_file() {
        return Err(format!("path does not exist or is not a file: {raw}"));
    }
    Ok(path)
}

/// Reads the file's content at parse time — so downstream code (`BackendSpec`)
/// only ever deals with bytes already in memory, never a path.
fn read_file_bytes(raw: &str) -> Result<Vec<u8>, String> {
    std::fs::read(raw).map_err(|e| format!("{raw}: {e}"))
}
