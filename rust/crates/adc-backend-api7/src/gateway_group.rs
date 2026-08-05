//! Users configure a gateway group by its display name, but every
//! gateway-group-scoped admin API call carries a `gateway_group_id` query
//! param instead. [`GatewayGroupResolver`] is the one place that name gets
//! turned into the id, resolved lazily and cached for the resolver's
//! lifetime (mirroring how `adc-backend-apisix`'s `Backend` caches its
//! resolved server version).

use adc_backend_core::{HttpClient, Method};
use adc_sdk::BackendError;
use serde::Deserialize;
use tokio::sync::OnceCell;

/// An `a7adm-` prefixed token is an admin token scoped across every
/// gateway group rather than one — requests made with it omit
/// `gateway_group_id` entirely instead of resolving one.
const ADMIN_TOKEN_PREFIX: &str = "a7adm-";

#[derive(Deserialize)]
struct GatewayGroupListResponse {
    list: Vec<GatewayGroupSummary>,
}

#[derive(Deserialize)]
struct GatewayGroupSummary {
    id: String,
    name: String,
}

pub struct GatewayGroupResolver {
    client: HttpClient,
    name: String,
    is_admin_token: bool,
    id: OnceCell<Option<String>>,
}

impl GatewayGroupResolver {
    pub fn new(client: HttpClient, name: String, token: &str) -> Self {
        Self {
            client,
            name,
            is_admin_token: token.starts_with(ADMIN_TOKEN_PREFIX),
            id: OnceCell::new(),
        }
    }

    /// Resolves to `None` for an admin token; otherwise looks up the
    /// group by name and errors if none matches.
    pub async fn resolve(&self) -> Result<Option<String>, BackendError> {
        let id = self
            .id
            .get_or_try_init(|| async {
                if self.is_admin_token {
                    return Ok::<_, BackendError>(None);
                }

                let request = self
                    .client
                    .request(Method::GET, "/api/gateway_groups")?
                    .query(&[("search", self.name.as_str()), ("name", self.name.as_str())]);
                let response: GatewayGroupListResponse = self.client.send_json(request).await?;

                find_exact_match(response.list, &self.name)
                    .ok_or_else(|| {
                        BackendError::Other(
                            format!("Gateway group \"{}\" does not exist", self.name).into(),
                        )
                    })
                    .map(|group| Some(group.id))
            })
            .await?;
        Ok(id.clone())
    }
}

/// `search`/`name` on `/api/gateway_groups` is a substring/fuzzy filter,
/// not an exact-match lookup — a request for `"prod"` can come back with
/// `"prod"`, `"prod-2"`, and `"non-prod"` all in the same `list`. Picks the
/// one entry whose `name` matches exactly, rather than assuming the first
/// result returned is the one that was asked for.
fn find_exact_match(groups: Vec<GatewayGroupSummary>, name: &str) -> Option<GatewayGroupSummary> {
    groups.into_iter().find(|group| group.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, name: &str) -> GatewayGroupSummary {
        GatewayGroupSummary {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn picks_the_entry_whose_name_matches_exactly() {
        let groups = vec![group("id-1", "prod-2"), group("id-2", "prod"), group("id-3", "non-prod")];
        let matched = find_exact_match(groups, "prod").unwrap();
        assert_eq!(matched.id, "id-2");
    }

    #[test]
    fn no_exact_match_among_fuzzy_results_returns_none() {
        let groups = vec![group("id-1", "prod-2"), group("id-3", "non-prod")];
        assert!(find_exact_match(groups, "prod").is_none());
    }
}
