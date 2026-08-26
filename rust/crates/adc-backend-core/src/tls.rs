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
    /// Builds a bare `reqwest::Client` from this config alone, with no
    /// server/token/timeout attached.
    pub fn build_client(&self) -> Result<reqwest::Client, BackendError> {
        self.apply(reqwest::Client::builder())?
            .build()
            .map_err(|e| BackendError::Other(format!("failed to build HTTP client: {e}").into()))
    }

    pub(crate) fn apply(&self, mut builder: reqwest::ClientBuilder) -> Result<reqwest::ClientBuilder, BackendError> {
        if self.skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if let Some(pem) = &self.ca_cert_pem {
            let cert = reqwest::Certificate::from_pem(pem)
                .map_err(|e| BackendError::Other(format!("invalid CA certificate: {e}").into()))?;
            builder = builder.add_root_certificate(cert);
        }

        match (&self.client_cert_pem, &self.client_key_pem) {
            (Some(cert_pem), Some(key_pem)) => {
                // PEM blocks must each start on their own line; the cert's
                // bytes don't reliably end with a newline (depends on where
                // the caller sourced them from), so one is inserted rather
                // than gluing the key's `-----BEGIN` onto the cert's own
                // `-----END` line.
                let mut pem = cert_pem.clone();
                if !pem.ends_with(b"\n") {
                    pem.push(b'\n');
                }
                pem.extend(key_pem);
                let identity = reqwest::Identity::from_pem(&pem)
                    .map_err(|e| BackendError::Other(format!("invalid client certificate/key: {e}").into()))?;
                builder = builder.identity(identity);
            }
            (None, None) => {}
            _ => {
                return Err(BackendError::Other(
                    "mTLS requires both a client certificate and a client key".into(),
                ));
            }
        }

        Ok(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real self-signed EC cert/key pair (generated once via `openssl req
    // -x509 -newkey ec ...`, not fetched at test time) — `Identity::from_pem`
    // actually parses this, so these tests exercise the real PEM assembly,
    // not just whether the cert/key presence check fires.
    const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIBcjCCARmgAwIBAgIUWp+abBNKuUPdUIeouYDaDgHPIO4wCgYIKoZIzj0EAwIw\n\
DzENMAsGA1UEAwwEdGVzdDAeFw0yNjA4MTgwMzIwMDJaFw0yNjA4MTkwMzIwMDJa\n\
MA8xDTALBgNVBAMMBHRlc3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARVo3/X\n\
uhOYfghuoLbag2VJvGofvgPYtXcdh4oFCmXB1MOupxSI3DqCFvMJc/QeH92Nz/qW\n\
vLW7TEWRCo2/Bay1o1MwUTAdBgNVHQ4EFgQUy80qZFI7+wryg4UyeI+YsHfSqgow\n\
HwYDVR0jBBgwFoAUy80qZFI7+wryg4UyeI+YsHfSqgowDwYDVR0TAQH/BAUwAwEB\n\
/zAKBggqhkjOPQQDAgNHADBEAiB+ddl9S2GSo8/NF37M47JI1HtxOzQQTizSoAQd\n\
tx5+SQIgKX3ASSnC8rrNGSFda+y79MOudxia/iQouBhv8Fb/hnE=\n\
-----END CERTIFICATE-----\n";
    const KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgO3caXwd/kpykMTTw\n\
+IoxA9NXadu2yhvQXw/rxkgjhZChRANCAARVo3/XuhOYfghuoLbag2VJvGofvgPY\n\
tXcdh4oFCmXB1MOupxSI3DqCFvMJc/QeH92Nz/qWvLW7TEWRCo2/Bay1\n\
-----END PRIVATE KEY-----\n";

    #[test]
    fn a_client_cert_without_a_key_is_rejected() {
        let config = TlsConfig { client_cert_pem: Some(CERT_PEM.into()), ..Default::default() };
        assert!(config.apply(reqwest::ClientBuilder::new()).is_err());
    }

    #[test]
    fn a_client_key_without_a_cert_is_rejected() {
        let config = TlsConfig { client_key_pem: Some(KEY_PEM.into()), ..Default::default() };
        assert!(config.apply(reqwest::ClientBuilder::new()).is_err());
    }

    #[test]
    fn neither_cert_nor_key_is_not_an_error() {
        let config = TlsConfig::default();
        assert!(config.apply(reqwest::ClientBuilder::new()).is_ok());
    }

    #[test]
    fn a_cert_missing_its_trailing_newline_still_assembles_into_a_valid_identity() {
        let mut cert_without_trailing_newline = CERT_PEM.as_bytes().to_vec();
        assert_eq!(cert_without_trailing_newline.pop(), Some(b'\n'), "test fixture must actually end with \\n");

        let config = TlsConfig {
            client_cert_pem: Some(cert_without_trailing_newline),
            client_key_pem: Some(KEY_PEM.into()),
            ..Default::default()
        };
        let result = config.apply(reqwest::ClientBuilder::new());
        assert!(result.is_ok(), "{result:?}");
    }
}
