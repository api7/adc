//! Shared HTTP client plumbing for the backend integrations
//! (`adc-backend-apisix`, `adc-backend-api7`, `adc-backend-apisix-standalone`):
//! building a `reqwest::Client` with auth headers and TLS settings,
//! classifying request failures into `adc_sdk::BackendError` uniformly, and
//! the retry/concurrency helpers each backend's operator/fetcher builds
//! request fan-out on top of.

mod client;
mod concurrency;
mod retry;
mod tls;

pub use client::{HttpClient, HttpClientConfig, encode_path_segment};
pub use concurrency::{concurrent_map, concurrent_map_until_err};
pub use retry::RetryPolicy;
pub use tls::TlsConfig;

pub use reqwest::{Method, Response};
