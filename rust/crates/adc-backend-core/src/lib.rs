//! Shared HTTP client plumbing for the backend integrations
//! (`adc-backend-apisix`, `adc-backend-api7`, `adc-backend-apisix-standalone`):
//! building a `reqwest::Client` with auth headers and TLS settings,
//! classifying request failures into `adc_sdk::BackendError` uniformly, and
//! the retry/concurrency helpers each backend's operator/fetcher builds
//! request fan-out on top of.

mod client;
mod concurrency;
mod event;
mod resource_filter;
mod resource_path;
mod retry;
mod tls;

pub use client::{HTTP_REQUEST_SPAN_NAME, HttpClient, HttpClientConfig, encode_path_segment};
pub use concurrency::{concurrent_map, concurrent_map_until_err, concurrent_map_until_ok};
pub use event::{deserialize_event_value, missing_parent, to_request_body};
pub use resource_filter::{ResourceFilter, filter_configuration_by_labels};
pub use resource_path::resource_type_collection_name;
pub use retry::RetryPolicy;
pub use tls::TlsConfig;

pub use reqwest::{Method, RequestBuilder, Response};
