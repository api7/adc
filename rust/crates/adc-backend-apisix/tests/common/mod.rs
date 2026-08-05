//! Shared scaffolding for this crate's real-e2e test files: a live APISIX
//! admin API at `SERVER`, reachable with `TOKEN`. See `e2e_apisix.rs`'s
//! module doc for how to bring one up and run these tests. Not every test
//! file uses every item here, so dead-code warnings are suppressed at the
//! module level rather than per item.
#![allow(dead_code)]

use adc_backend_apisix::Backend as ApisixBackend;
use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};

pub const SERVER: &str = "http://localhost:19180";
pub const TOKEN: &str = "edd1c9f034335f136f87ad84b625c8f1";

pub fn client() -> HttpClient {
    HttpClient::new(HttpClientConfig {
        server: SERVER.to_string(),
        token: TOKEN.to_string(),
        timeout: None,
        tls: TlsConfig::default(),
    })
    .unwrap()
}

pub fn backend() -> ApisixBackend {
    ApisixBackend::new(client(), adc_backend_core::ResourceFilter::default())
}

/// The CI matrix runs this suite against every supported APISIX release
/// (`BACKEND_APISIX_VERSION`, same env var the TS e2e suite reads) — falls
/// back to a version high enough to exercise every version-gated code path
/// when unset, for local runs against whatever's in the compose file. A
/// value that's *present* but doesn't parse as a semver is almost
/// certainly a CI misconfiguration, so that panics loudly instead of
/// silently falling back the same way "unset" does.
pub fn apisix_version() -> semver::Version {
    match std::env::var("BACKEND_APISIX_VERSION") {
        Ok(v) => semver::Version::parse(&v)
            .unwrap_or_else(|e| panic!("BACKEND_APISIX_VERSION={v:?} is not a valid semver: {e}")),
        Err(_) => semver::Version::new(999, 999, 999),
    }
}
