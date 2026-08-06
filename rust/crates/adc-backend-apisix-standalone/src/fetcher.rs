//! Fetching a standalone cluster's current config: which server has the
//! most recently accepted write (`find_latest`), then pulling that server's
//! full config document (`dump`).

use adc_backend_core::{Method, concurrent_map};
use adc_sdk::BackendError;
use adc_sdk::resources::Configuration;
use semver::Version;

use crate::backend::StandaloneServer;
use crate::transformer;
use crate::typing::ApisixStandalone;

const ENDPOINT_CONFIG: &str = "/apisix/admin/configs";
const HEADER_LAST_MODIFIED: &str = "x-last-modified";

/// APISIX standalone versions above 3.13.0 accept `HEAD` on the config
/// endpoint (a cheaper way to read just the `X-Last-Modified` header);
/// older ones don't implement `HEAD` for it at all, so `find_latest` falls
/// back to a full `GET` there.
const HEAD_SUPPORTED_SINCE: (u64, u64, u64) = (3, 13, 0);

pub struct Fetcher {
    servers: Vec<StandaloneServer>,
    version: Version,
}

impl Fetcher {
    pub fn new(servers: Vec<StandaloneServer>, version: Version) -> Self {
        Self { servers, version }
    }

    /// Pulls the full config document from whichever server
    /// [`Self::find_latest`] picks (or the first configured server, if none
    /// of them has ever accepted a write yet), and converts it into ADC's
    /// model.
    pub async fn dump(&self) -> Result<(Configuration, ApisixStandalone), BackendError> {
        let target = match self.find_latest().await? {
            Some(server) => server,
            None => self
                .servers
                .first()
                .ok_or_else(no_servers_configured)?
                .server
                .clone(),
        };
        let client = &self
            .servers
            .iter()
            .find(|s| s.server == target)
            .expect("find_latest only ever returns a server from self.servers")
            .client;

        let request = client.request(Method::GET, ENDPOINT_CONFIG)?;
        let raw_config: ApisixStandalone = client.send_json(request).await?;
        let config = transformer::to_adc(&raw_config);
        Ok((config, raw_config))
    }

    /// Finds which server holds the most recently accepted write, going by
    /// each one's `X-Last-Modified` response header (a timestamp the server
    /// stamps on every config it stores). `None` means no server has ever
    /// accepted a write (every one reports timestamp `0` — a fresh
    /// cluster), not that a request failed — a request failure is still
    /// propagated as `Err`.
    async fn find_latest(&self) -> Result<Option<String>, BackendError> {
        let method = if version_supports_head(&self.version) { Method::HEAD } else { Method::GET };

        let probe = |server: StandaloneServer| {
            let method = method.clone();
            async move {
                let request = server.client.request(method, ENDPOINT_CONFIG)?;
                let response = server.client.send(request).await?;
                let timestamp = response
                    .headers()
                    .get(HEADER_LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                Ok::<_, BackendError>((server.server, timestamp))
            }
        };
        let results = concurrent_map(self.servers.clone(), None, probe).await;

        let mut latest: Option<(String, i64)> = None;
        for result in results {
            let (server, timestamp) = result?;
            if latest.as_ref().is_none_or(|(_, best)| timestamp >= *best) {
                latest = Some((server, timestamp));
            }
        }

        Ok(latest.filter(|(_, timestamp)| *timestamp > 0).map(|(server, _)| server))
    }
}

fn version_supports_head(version: &Version) -> bool {
    let (major, minor, patch) = HEAD_SUPPORTED_SINCE;
    *version > Version::new(major, minor, patch)
}

fn no_servers_configured() -> BackendError {
    BackendError::Other("apisix-standalone backend has no servers configured".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_is_only_used_strictly_above_the_version_cutoff() {
        assert!(!version_supports_head(&Version::new(3, 13, 0)));
        assert!(version_supports_head(&Version::new(3, 13, 1)));
        assert!(!version_supports_head(&Version::new(3, 12, 9)));
    }
}
