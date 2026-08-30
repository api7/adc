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
        let mut resolved = Vec::with_capacity(results.len());
        for result in results {
            // Any single probe failure fails the whole lookup, even if every
            // other server is healthy — not attempted-and-recovered-from.
            resolved.push(result?);
        }

        Ok(pick_latest(resolved))
    }
}

/// Picks whichever `(server, timestamp)` pair has the highest timestamp.
/// `>=` means a later entry in `results` overtakes an earlier one on an
/// exact tie — deterministic given a fixed input order, but `results`
/// itself comes from `concurrent_map`'s unordered completion order, so
/// which *real* server wins a genuine tie is still not a documented,
/// wall-clock-meaningful rule. `None` iff every entry is `0` (a fresh
/// cluster no server has ever accepted a write on) or `results` is empty.
fn pick_latest(results: Vec<(String, i64)>) -> Option<String> {
    let mut latest: Option<(String, i64)> = None;
    for (server, timestamp) in results {
        if latest.as_ref().is_none_or(|(_, best)| timestamp >= *best) {
            latest = Some((server, timestamp));
        }
    }
    latest.filter(|(_, timestamp)| *timestamp > 0).map(|(server, _)| server)
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
    use std::time::Duration;

    use adc_backend_core::{HttpClient, HttpClientConfig, TlsConfig};

    use super::*;

    #[test]
    fn head_is_only_used_strictly_above_the_version_cutoff() {
        assert!(!version_supports_head(&Version::new(3, 13, 0)));
        assert!(version_supports_head(&Version::new(3, 13, 1)));
        assert!(!version_supports_head(&Version::new(3, 12, 9)));
    }

    #[test]
    fn a_tie_is_broken_by_whichever_entry_comes_later_in_the_results_list() {
        assert_eq!(pick_latest(vec![("a".to_string(), 100), ("b".to_string(), 100)]), Some("b".to_string()));
        assert_eq!(pick_latest(vec![("b".to_string(), 100), ("a".to_string(), 100)]), Some("a".to_string()));
    }

    #[test]
    fn a_fresh_cluster_where_every_server_reports_zero_has_no_latest() {
        assert_eq!(pick_latest(vec![("a".to_string(), 0), ("b".to_string(), 0), ("c".to_string(), 0)]), None);
    }

    #[test]
    fn no_probes_at_all_has_no_latest() {
        assert_eq!(pick_latest(vec![]), None);
    }

    #[test]
    fn the_single_highest_timestamp_wins_regardless_of_position() {
        assert_eq!(
            pick_latest(vec![("a".to_string(), 50), ("b".to_string(), 100), ("c".to_string(), 30)]),
            Some("b".to_string())
        );
    }

    fn unreachable_server() -> StandaloneServer {
        let client = HttpClient::new(HttpClientConfig {
            server: "http://127.0.0.1:1".to_string(),
            token: "x".to_string(),
            timeout: Some(Duration::from_secs(2)),
            tls: TlsConfig::default(),
        })
        .unwrap();
        StandaloneServer { server: "http://127.0.0.1:1".to_string(), client }
    }

    /// Documents current behavior rather than asserting it's the only
    /// sensible one: a single unreachable server fails the whole `dump()`,
    /// even with zero other servers configured to fall back on. If this
    /// crate ever grows tolerance for a subset of unreachable servers, this
    /// test is the one that should change alongside it.
    #[tokio::test]
    async fn a_single_unreachable_server_fails_the_whole_dump() {
        let fetcher = Fetcher::new(vec![unreachable_server()], Version::new(999, 999, 999));
        assert!(fetcher.dump().await.is_err());
    }
}
