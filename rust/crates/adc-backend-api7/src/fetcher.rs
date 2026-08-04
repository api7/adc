//! Cascading resource queries against a gateway group's admin API: a
//! service's named upstreams and routes/stream_routes live under their own
//! collection endpoints, so listing services fans out into per-service
//! follow-up requests. Fetches wire-shape structs (`crate::typing`) —
//! converting those into ADC's model is a separate concern, layered on top
//! of this.

use adc_backend_core::{HttpClient, Method, RequestBuilder, concurrent_map_until_err};
use adc_sdk::BackendError;
use semver::Version;
use serde::de::DeserializeOwned;

use crate::typing;

pub struct Fetcher {
    client: HttpClient,
    version: Version,
    gateway_group_id: Option<String>,
}

impl Fetcher {
    pub fn new(client: HttpClient, version: Version, gateway_group_id: Option<String>) -> Self {
        Self {
            client,
            version,
            gateway_group_id,
        }
    }

    fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let mut builder = self.client.request(method, path)?;
        if let Some(id) = &self.gateway_group_id {
            builder = builder.query(&[("gateway_group_id", id)]);
        }
        Ok(builder)
    }

    async fn list<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, BackendError> {
        let builder = self.request(Method::GET, path)?;
        let body: typing::ListResponse<T> = self.client.send_json(builder).await?;
        Ok(body.list)
    }

    pub async fn list_services(&self) -> Result<Vec<typing::Service>, BackendError> {
        let services: Vec<typing::Service> = self.list("/apisix/admin/services").await?;
        concurrent_map_until_err(services, None, |service| {
            self.with_upstreams_and_routes(service)
        })
        .await
    }

    /// A service below 3.5.0 has no `/upstreams` sub-collection at all —
    /// only its own inline default `upstream` — so the fetch is skipped
    /// rather than attempted and failed. Above that, a non-2xx response is
    /// tolerated as "no named upstreams" rather than a hard error, matching
    /// the TS fetcher's lenient status handling here specifically (as
    /// opposed to the routes/stream_routes fetch below, which isn't
    /// lenient).
    async fn with_upstreams_and_routes(
        &self,
        mut service: typing::Service,
    ) -> Result<typing::Service, BackendError> {
        let id = service.id.clone().ok_or_else(|| {
            BackendError::Serialization("a fetched service is missing its id".into())
        })?;

        if self.version >= Version::new(3, 5, 0) {
            let builder = self.request(
                Method::GET,
                &format!("/apisix/admin/services/{id}/upstreams"),
            )?;
            let response = self.client.execute(builder).await?;
            if response.status().is_success() {
                let body: typing::ListResponse<typing::Upstream> =
                    response.json().await.map_err(|e| {
                        BackendError::Serialization(format!(
                            "decoding response from /apisix/admin/services/{id}/upstreams: {e}"
                        ))
                    })?;
                service.upstreams = Some(body.list);
            }
        }

        if service.ty.as_deref() == Some("stream") {
            let builder = self
                .request(Method::GET, "/apisix/admin/stream_routes")?
                .query(&[("service_id", &id)]);
            let body: typing::ListResponse<typing::StreamRoute> =
                self.client.send_json(builder).await?;
            service.stream_routes = Some(body.list);
        } else {
            let builder = self
                .request(Method::GET, "/apisix/admin/routes")?
                .query(&[("service_id", &id)]);
            let body: typing::ListResponse<typing::Route> = self.client.send_json(builder).await?;
            service.routes = Some(body.list);
        }

        Ok(service)
    }

    pub async fn list_consumers(&self) -> Result<Vec<typing::Consumer>, BackendError> {
        let consumers: Vec<typing::Consumer> = self.list("/apisix/admin/consumers").await?;
        concurrent_map_until_err(consumers, None, |consumer| self.with_credentials(consumer)).await
    }

    async fn with_credentials(
        &self,
        mut consumer: typing::Consumer,
    ) -> Result<typing::Consumer, BackendError> {
        let path = format!("/apisix/admin/consumers/{}/credentials", consumer.username);
        let builder = self.request(Method::GET, &path)?;
        // Purpose isn't obvious from the URL alone in a `--verbose 2` dump
        // of N concurrent credential fetches.
        let response = self
            .client
            .execute_described(
                builder,
                &format!("Get credentials of consumer \"{}\"", consumer.username),
            )
            .await?;
        let response = HttpClient::require_success(response).await?;
        let body: typing::ListResponse<typing::ConsumerCredential> =
            response.json().await.map_err(|e| {
                BackendError::Serialization(format!("decoding response from {path}: {e}"))
            })?;
        consumer.credentials = Some(body.list);
        Ok(consumer)
    }

    pub async fn list_ssls(&self) -> Result<Vec<typing::Ssl>, BackendError> {
        self.list("/apisix/admin/ssls").await
    }

    pub async fn list_global_rules(&self) -> Result<Vec<typing::GlobalRule>, BackendError> {
        self.list("/apisix/admin/global_rules").await
    }

    pub async fn list_plugin_metadata(&self) -> Result<typing::PluginMetadata, BackendError> {
        let builder = self.request(Method::GET, "/apisix/admin/plugin_metadata")?;
        let body: typing::ValueResponse<typing::PluginMetadata> =
            self.client.send_json(builder).await?;
        Ok(body.value)
    }
}
