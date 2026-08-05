//! Cascading resource queries against a gateway group's admin API: a
//! service's named upstreams and routes/stream_routes live under their own
//! collection endpoints, so listing services fans out into per-service
//! follow-up requests. Fetches wire-shape structs (`crate::typing`) —
//! converting those into ADC's model is a separate concern, layered on top
//! of this.

use adc_backend_core::{
    HttpClient, Method, RequestBuilder, ResourceFilter, concurrent_map_until_err,
};
use adc_sdk::BackendError;
use adc_sdk::ResourceType;
use adc_sdk::resources::{self as adc, Configuration};
use semver::Version;
use serde::de::DeserializeOwned;

use crate::typing;

pub struct Fetcher {
    client: HttpClient,
    version: Version,
    gateway_group_id: Option<String>,
    filter: ResourceFilter,
}

impl Fetcher {
    pub fn new(
        client: HttpClient,
        version: Version,
        gateway_group_id: Option<String>,
        filter: ResourceFilter,
    ) -> Self {
        Self {
            client,
            version,
            gateway_group_id,
            filter,
        }
    }

    fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, BackendError> {
        let mut builder = self.client.request(method, path)?;
        if let Some(id) = &self.gateway_group_id {
            builder = builder.query(&[("gateway_group_id", id)]);
        }
        Ok(builder)
    }

    /// A top-level collection request: unlike [`Fetcher::request`], this
    /// also carries `--label-selector`'s query params — the cascading
    /// per-service (upstreams/routes) and per-consumer (credentials)
    /// follow-up requests below don't call this, matching the dashboard's
    /// own admin API, which only accepts a label filter on a top-level
    /// collection endpoint.
    fn collection_request(&self, path: &str) -> Result<RequestBuilder, BackendError> {
        let builder = self.request(Method::GET, path)?;
        Ok(self.filter.attach_label_selector(builder))
    }

    async fn list<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, BackendError> {
        let builder = self.collection_request(path)?;
        let body: typing::ListResponse<T> = self.client.send_json(builder).await?;
        Ok(body.list)
    }

    pub async fn list_services(&self) -> Result<Vec<typing::Service>, BackendError> {
        if self.filter.is_skip(ResourceType::Service) {
            return Ok(Vec::new());
        }
        let services: Vec<typing::Service> = self.list("/apisix/admin/services").await?;
        concurrent_map_until_err(services, None, |service| {
            self.with_upstreams_and_routes(service)
        })
        .await
    }

    /// A service below 3.5.0 has no `/upstreams` sub-collection at all —
    /// only its own inline default `upstream` — so the fetch is skipped
    /// rather than attempted and failed. Above that, a non-2xx response is
    /// tolerated as "no named upstreams" rather than a hard error —
    /// deliberately lenient here specifically, unlike the routes/
    /// stream_routes fetch below.
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
        if self.filter.is_skip(ResourceType::Consumer) {
            return Ok(Vec::new());
        }
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
        if self.filter.is_skip(ResourceType::Ssl) {
            return Ok(Vec::new());
        }
        self.list("/apisix/admin/ssls").await
    }

    pub async fn list_global_rules(&self) -> Result<Vec<typing::GlobalRule>, BackendError> {
        if self.filter.is_skip(ResourceType::GlobalRule) {
            return Ok(Vec::new());
        }
        self.list("/apisix/admin/global_rules").await
    }

    pub async fn list_plugin_metadata(&self) -> Result<typing::PluginMetadata, BackendError> {
        if self.filter.is_skip(ResourceType::PluginMetadata) {
            return Ok(typing::PluginMetadata::default());
        }
        let builder = self.collection_request("/apisix/admin/plugin_metadata")?;
        let body: typing::ValueResponse<typing::PluginMetadata> =
            self.client.send_json(builder).await?;
        Ok(body.value)
    }

    /// Fetches every resource type (concurrently) and converts them into a
    /// single ADC `Configuration`. Unlike APISIX, a service's routes/
    /// stream_routes are already nested under it by [`Fetcher::list_services`]
    /// before this runs, so there's no separate bucketing/attaching pass —
    /// each service just needs its own wire-shape `routes`/`stream_routes`
    /// converted and reattached as ADC's `ServiceRoutes`.
    pub async fn dump(&self) -> Result<Configuration, BackendError> {
        let (services, consumers, ssls, global_rules, plugin_metadata) = tokio::try_join!(
            self.list_services(),
            self.list_consumers(),
            self.list_ssls(),
            self.list_global_rules(),
            self.list_plugin_metadata(),
        )?;

        let services = services
            .into_iter()
            .map(|mut service| {
                let routes = service.routes.take();
                let stream_routes = service.stream_routes.take();
                let mut service: adc::Service =
                    service.try_into().map_err(BackendError::Serialization)?;
                if let Some(routes) = routes {
                    let routes = routes
                        .into_iter()
                        .map(adc::Route::try_from)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(BackendError::Serialization)?;
                    service.routes = Some(adc::ServiceRoutes::Http { routes });
                } else if let Some(stream_routes) = stream_routes {
                    let stream_routes = stream_routes
                        .into_iter()
                        .map(adc::StreamRoute::from)
                        .collect();
                    service.routes = Some(adc::ServiceRoutes::Stream { stream_routes });
                }
                Ok(service)
            })
            .collect::<Result<Vec<_>, BackendError>>()?;

        let mut merged_global_rules = adc::Plugins::new();
        for rule in global_rules {
            merged_global_rules.extend(rule.plugins);
        }

        Ok(Configuration {
            services: (!services.is_empty()).then_some(services),
            ssls: (!ssls.is_empty()).then(|| ssls.into_iter().map(adc::SSL::from).collect()),
            consumers: (!consumers.is_empty())
                .then(|| consumers.into_iter().map(Into::into).collect()),
            // Not fetched: this crate has no notion of consumer groups yet,
            // the same gap noted in `adc_backend_apisix::fetcher`'s own doc
            // comment.
            consumer_groups: None,
            global_rules: (!merged_global_rules.is_empty()).then_some(merged_global_rules),
            plugin_metadata: (!plugin_metadata.is_empty()).then_some(plugin_metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use adc_backend_core::{HttpClientConfig, TlsConfig};

    use super::*;

    /// Never resolves a connection, so any request made through it fails
    /// immediately — used below to prove `is_skip` short-circuits *before*
    /// a request is built, not just that the response gets discarded.
    fn unreachable_client() -> HttpClient {
        HttpClient::new(HttpClientConfig {
            server: "http://0.0.0.0".to_string(),
            token: "test-token".to_string(),
            timeout: None,
            tls: TlsConfig::default(),
        })
        .unwrap()
    }

    #[tokio::test]
    async fn dump_makes_no_request_at_all_once_every_resource_type_is_excluded() {
        let exclude = HashSet::from([
            ResourceType::Service,
            ResourceType::Consumer,
            ResourceType::Ssl,
            ResourceType::GlobalRule,
            ResourceType::PluginMetadata,
        ]);
        let filter = ResourceFilter {
            include: HashSet::new(),
            exclude,
            label_selector: HashMap::new(),
        };
        let fetcher = Fetcher::new(
            unreachable_client(),
            Version::new(999, 999, 999),
            Some("test".to_string()),
            filter,
        );

        let configuration = fetcher.dump().await.unwrap();
        assert_eq!(
            configuration,
            Configuration {
                services: None,
                ssls: None,
                consumers: None,
                consumer_groups: None,
                global_rules: None,
                plugin_metadata: None,
            }
        );
    }

    /// A local server that records every path it's asked for and answers
    /// generically (an empty list/value satisfies every resource type's
    /// response shape without needing per-type fixtures) — used to prove a
    /// specific endpoint was *never requested*, not just that its response
    /// was discarded.
    async fn spawn_recording_server() -> (String, std::sync::Arc<tokio::sync::Mutex<Vec<String>>>) {
        let seen = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let seen_for_handler = seen.clone();
        let router = axum::Router::new().fallback(axum::routing::any(
            move |request: axum::extract::Request| {
                let seen = seen_for_handler.clone();
                async move {
                    seen.lock().await.push(request.uri().path().to_string());
                    axum::Json(serde_json::json!({ "list": [], "value": {} }))
                }
            },
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), seen)
    }

    #[tokio::test]
    async fn excluding_a_resource_type_means_its_endpoint_is_never_requested() {
        let (server, seen) = spawn_recording_server().await;
        let client = HttpClient::new(HttpClientConfig {
            server,
            token: "test-token".to_string(),
            timeout: None,
            tls: TlsConfig::default(),
        })
        .unwrap();
        let filter = ResourceFilter {
            include: HashSet::new(),
            exclude: HashSet::from([ResourceType::Service]),
            label_selector: HashMap::new(),
        };
        let fetcher = Fetcher::new(
            client,
            Version::new(999, 999, 999),
            Some("test".to_string()),
            filter,
        );

        fetcher.dump().await.unwrap();

        let seen = seen.lock().await;
        assert!(
            !seen.iter().any(|path| path == "/apisix/admin/services"),
            "{seen:?}"
        );
        assert!(
            seen.iter().any(|path| path == "/apisix/admin/consumers"),
            "{seen:?}"
        );
    }

    #[test]
    fn collection_request_carries_the_label_selector_but_a_nested_request_does_not() {
        let filter = ResourceFilter {
            include: HashSet::new(),
            exclude: HashSet::new(),
            label_selector: HashMap::from([("env".to_string(), "prod".to_string())]),
        };
        let fetcher = Fetcher::new(
            unreachable_client(),
            Version::new(999, 999, 999),
            None,
            filter,
        );

        let collection = fetcher
            .collection_request("/api/services")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(collection.url().query(), Some("labels%5Benv%5D=prod"));

        let nested = fetcher
            .request(Method::GET, "/api/services/svc/routes")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(nested.url().query(), None);
    }
}
