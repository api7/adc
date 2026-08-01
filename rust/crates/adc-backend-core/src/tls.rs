use adc_sdk::BackendError;

/// TLS options for connecting to a backend's admin API. Mirrors what the CLI
/// previously built into per-request Node `http(s).Agent` instances (custom
/// CA, mTLS client identity, skip-verify) — here they're baked directly into
/// the `reqwest::Client` at construction time instead.
///
/// Takes PEM bytes directly rather than file paths: reading config off disk
/// is the CLI/config layer's job, not this HTTP client's — keeps this crate
/// free of filesystem I/O and lets callers source certs from anywhere
/// (files, env vars, a secrets manager).
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    pub ca_cert_pem: Option<Vec<u8>>,
    pub client_cert_pem: Option<Vec<u8>>,
    pub client_key_pem: Option<Vec<u8>>,
    pub skip_verify: bool,
}

impl TlsConfig {
    pub(crate) fn apply(&self, mut builder: reqwest::ClientBuilder) -> Result<reqwest::ClientBuilder, BackendError> {
        if self.skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some(pem) = &self.ca_cert_pem {
            let cert = reqwest::Certificate::from_pem(pem)
                .map_err(|e| BackendError::Other(format!("invalid CA certificate: {e}").into()))?;
            builder = builder.add_root_certificate(cert);
        }

        if let (Some(cert_pem), Some(key_pem)) = (&self.client_cert_pem, &self.client_key_pem) {
            let mut pem = cert_pem.clone();
            pem.extend(key_pem);
            let identity = reqwest::Identity::from_pem(&pem)
                .map_err(|e| BackendError::Other(format!("invalid client certificate/key: {e}").into()))?;
            builder = builder.identity(identity);
        }

        Ok(builder)
    }
}
