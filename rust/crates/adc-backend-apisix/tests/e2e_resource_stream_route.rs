use adc_backend_apisix::tests::Fetcher;
use adc_backend_core::ResourceFilter;

mod common;
use common::{apisix_version, client_no_stream};

/// `client_no_stream` points at an APISIX started without stream proxy,
/// where `GET /apisix/admin/stream_routes` answers 400.
fn fetcher() -> Fetcher {
    Fetcher::new(client_no_stream(), apisix_version(), ResourceFilter::default(), 10)
}

#[tokio::test]
#[ignore]
async fn list_stream_routes_reports_none_when_stream_mode_is_disabled() {
    assert!(fetcher().list_stream_routes().await.unwrap().is_empty());
}

#[tokio::test]
#[ignore]
async fn dump_succeeds_when_stream_mode_is_disabled() {
    fetcher().dump().await.unwrap();
}
