//! A process-wide, LRU-bounded pool of `reqwest::Client`s keyed by TLS
//! material, so requests to the same backend gateway share a connection
//! pool. Eviction is plain `Arc` drop — no manual refcounting needed.

use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock, Mutex};

use adc_backend_core::TlsConfig;
use adc_sdk::BackendError;
use lru::LruCache;

/// Distinguishes one pooled client from another — `Hash`/`Eq` let it double as the pool key.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct TlsMaterial {
    pub skip_verify: bool,
    pub ca_cert: Option<String>,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
}

impl std::fmt::Debug for TlsMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsMaterial")
            .field("skip_verify", &self.skip_verify)
            .field("ca_cert", &self.ca_cert)
            .field("client_cert", &self.client_cert)
            .field("client_key", &self.client_key.is_some())
            .finish()
    }
}

const DEFAULT_MAX_ENTRIES: usize = 16;

fn env_max_entries() -> NonZeroUsize {
    std::env::var("ADC_INGRESS_TLS_AGENT_POOL_MAX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(NonZeroUsize::new)
        .unwrap_or(NonZeroUsize::new(DEFAULT_MAX_ENTRIES).expect("16 is nonzero"))
}

pub struct AgentPool {
    entries: Mutex<LruCache<TlsMaterial, Arc<reqwest::Client>>>,
}

impl AgentPool {
    pub fn with_capacity(max_entries: usize) -> Self {
        let capacity = NonZeroUsize::new(max_entries).unwrap_or(NonZeroUsize::new(1).expect("1 is nonzero"));
        Self { entries: Mutex::new(LruCache::new(capacity)) }
    }

    pub fn global() -> &'static AgentPool {
        static GLOBAL: LazyLock<AgentPool> = LazyLock::new(|| AgentPool::with_capacity(env_max_entries().get()));
        &GLOBAL
    }

    pub fn get_client(&self, tls: &TlsMaterial) -> Result<Arc<reqwest::Client>, BackendError> {
        let mut entries = self.entries.lock().expect("agent pool mutex poisoned");
        if let Some(client) = entries.get(tls) {
            return Ok(client.clone());
        }
        let client = Arc::new(build_client(tls)?);
        entries.put(tls.clone(), client.clone());
        Ok(client)
    }
}

fn build_client(tls: &TlsMaterial) -> Result<reqwest::Client, BackendError> {
    TlsConfig {
        ca_cert_pem: tls.ca_cert.clone().map(String::into_bytes),
        client_cert_pem: tls.client_cert.clone().map(String::into_bytes),
        client_key_pem: tls.client_key.clone().map(String::into_bytes),
        skip_verify: tls.skip_verify,
    }
    .build_client()
}

pub fn get_client(tls: &TlsMaterial) -> Result<Arc<reqwest::Client>, BackendError> {
    AgentPool::global().get_client(tls)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(ca_cert: &str) -> TlsMaterial {
        TlsMaterial { ca_cert: Some(ca_cert.to_string()), ..Default::default() }
    }

    #[test]
    fn reuses_the_same_client_for_identical_material() {
        let pool = AgentPool::with_capacity(16);
        let a = pool.get_client(&material("shared")).unwrap();
        let b = pool.get_client(&material("shared")).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn different_material_gets_isolated_clients() {
        let pool = AgentPool::with_capacity(16);
        let a = pool.get_client(&material("a")).unwrap();
        let b = pool.get_client(&material("b")).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn no_tls_material_and_some_tls_material_are_isolated_too() {
        let pool = AgentPool::with_capacity(16);
        let default_client = pool.get_client(&TlsMaterial::default()).unwrap();
        let skip_verify = pool.get_client(&TlsMaterial { skip_verify: true, ..Default::default() }).unwrap();
        assert!(!Arc::ptr_eq(&default_client, &skip_verify));
    }

    #[test]
    fn evicts_the_least_recently_used_entry_not_merely_the_first_inserted() {
        let pool = AgentPool::with_capacity(2);
        let a = pool.get_client(&material("a")).unwrap();
        let b = pool.get_client(&material("b")).unwrap();
        let _ = pool.get_client(&material("a")).unwrap(); // refreshes "a"'s recency

        let _c = pool.get_client(&material("c")).unwrap(); // exceeds capacity(2), evicts "b"

        let a_again = pool.get_client(&material("a")).unwrap();
        assert!(Arc::ptr_eq(&a, &a_again), "\"a\" should not have been evicted");

        let b_again = pool.get_client(&material("b")).unwrap();
        assert!(!Arc::ptr_eq(&b, &b_again), "\"b\" should have been evicted and rebuilt");
    }

    #[test]
    fn debug_never_prints_the_private_key_but_the_cert_is_fine() {
        let tls = TlsMaterial {
            client_key: Some("-----BEGIN PRIVATE KEY-----\nSECRET-KEY\n-----END PRIVATE KEY-----".to_string()),
            client_cert: Some("-----BEGIN CERTIFICATE-----\nPUBLIC-CERT\n-----END CERTIFICATE-----".to_string()),
            ..Default::default()
        };
        let debug = format!("{tls:?}");
        assert!(!debug.contains("SECRET-KEY"), "{debug}");
        assert!(debug.contains("PUBLIC-CERT"), "{debug}");
    }
}
