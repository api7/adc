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

                let group = response.list.into_iter().next().ok_or_else(|| {
                    BackendError::Other(
                        format!("Gateway group \"{}\" does not exist", self.name).into(),
                    )
                })?;
                Ok(Some(group.id))
            })
            .await?;
        Ok(id.clone())
    }
}
