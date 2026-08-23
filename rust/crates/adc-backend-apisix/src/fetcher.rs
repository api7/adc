use std::collections::HashMap;

use adc_backend_core::{HttpClient, Method, ResourceFilter, concurrent_map_until_err};
use adc_sdk::resources::{self as adc, Configuration, LabelValue, Plugins};
use adc_sdk::{BackendError, ResourceType};
use indexmap::IndexMap;
use semver::Version;
use serde::de::DeserializeOwned;

use crate::typing;
use crate::utils::resource_type_to_api_name;

/// Fetches an ADC-managed APISIX instance's full resource state, one
/// resource type at a time, in APISIX's own wire shape (`crate::typing`) —
/// converting that into ADC's model (`adc_sdk::resources`) and assembling it
/// into a single `Configuration` is a separate concern, layered on top of
/// this.
pub struct Fetcher {
    client: HttpClient,
    version: Version,
    filter: ResourceFilter,
    /// Bounds how many consumers' credentials `list_consumers` fetches at
    /// once, so a large consumer list doesn't fan out unboundedly against
    /// the admin API. The CLI's `--request-concurrent`.
    concurrency: usize,
}

impl Fetcher {
    pub fn new(client: HttpClient, version: Version, filter: ResourceFilter, concurrency: usize) -> Self {
        Self {
            client,
            version,
            filter,
            concurrency,
        }
    }

    async fn list<T: DeserializeOwned>(
        &self,
        resource_type: ResourceType,
    ) -> Result<Vec<T>, BackendError> {
        if self.filter.is_skip(resource_type) {
            return Ok(Vec::new());
        }
        let api_name = resource_type_to_api_name(resource_type).ok_or_else(|| {
            BackendError::Unsupported(format!(
                "{resource_type:?} has no top-level admin API collection"
            ))
        })?;
        let path = format!("/apisix/admin/{api_name}");
        let builder = self
            .filter
            .attach_label_selector(self.client.request(Method::GET, &path)?);
        let body: typing::ListResponse<T> = self.client.send_json(builder).await?;
        Ok(body.list.into_iter().map(|item| item.value).collect())
    }

    pub async fn list_services(&self) -> Result<Vec<typing::Service>, BackendError> {
        self.list(ResourceType::Service).await
    }

    pub async fn list_routes(&self) -> Result<Vec<typing::Route>, BackendError> {
        self.list(ResourceType::Route).await
    }

    pub async fn list_upstreams(&self) -> Result<Vec<typing::Upstream>, BackendError> {
        self.list(ResourceType::Upstream).await
    }

    pub async fn list_ssls(&self) -> Result<Vec<typing::Ssl>, BackendError> {
        self.list(ResourceType::Ssl).await
    }

    /// No dedicated PluginConfig resource type to check `is_skip` against —
    /// gated on Route instead, since resolving a route's `plugin_config_id`
    /// is the only thing this data is ever used for.
    pub async fn list_plugin_configs(&self) -> Result<Vec<typing::PluginConfig>, BackendError> {
        if self.filter.is_skip(ResourceType::Route) {
            return Ok(Vec::new());
        }
        let builder = self
            .filter
            .attach_label_selector(self.client.request(Method::GET, "/apisix/admin/plugin_configs")?);
        let body: typing::ListResponse<typing::PluginConfig> = self.client.send_json(builder).await?;
        Ok(body.list.into_iter().map(|item| item.value).collect())
    }

    /// A backend may define several `global_rules` entries; their `plugins`
    /// maps are merged into one (later entries win on key collision),
    /// matching how they're consumed — as a single flat set of gateway-wide
    /// plugins, not as separate rules.
    pub async fn list_global_rules(&self) -> Result<Plugins, BackendError> {
        let rules: Vec<typing::GlobalRule> = self.list(ResourceType::GlobalRule).await?;
        let mut merged = Plugins::new();
        for rule in rules {
            merged.extend(rule.plugins);
        }
        Ok(merged)
    }

    /// Plugin metadata isn't returned as a `name` field on each item — the
    /// name only appears as the last segment of the etcd key
    /// (`/apisix/plugin_metadata/http-logger`), so it's extracted from
    /// `ListItem::key` rather than `ListItem::value`.
    pub async fn list_plugin_metadata(&self) -> Result<Plugins, BackendError> {
        if self.filter.is_skip(ResourceType::PluginMetadata) {
            return Ok(Plugins::new());
        }
        let path = format!(
            "/apisix/admin/{}",
            resource_type_to_api_name(ResourceType::PluginMetadata)
                .expect("PluginMetadata always has an api name")
        );
        let builder = self
            .filter
            .attach_label_selector(self.client.request(Method::GET, &path)?);
        let body: typing::ListResponse<Plugins> = self.client.send_json(builder).await?;

        let mut merged = Plugins::new();
        for item in body.list {
            if let Some(name) = item.key.rsplit('/').next() {
                merged.insert(name.to_string(), item.value.into());
            }
        }
        Ok(merged)
    }

    /// Stream routes are an optional APISIX feature: on a version/build
    /// that doesn't support them, the endpoint itself may not exist. Only a
    /// 404 here is treated as "no stream routes" — anything else
    /// (authentication/authorization failure, a 5xx) is a real error, same
    /// as [`Fetcher::list`].
    pub async fn list_stream_routes(&self) -> Result<Vec<typing::StreamRoute>, BackendError> {
        if self.filter.is_skip(ResourceType::StreamRoute) {
            return Ok(Vec::new());
        }
        let builder = self.filter.attach_label_selector(
            self.client
                .request(Method::GET, "/apisix/admin/stream_routes")?,
        );
        let response = self.client.execute(builder).await?;
        if response.status().as_u16() == 404 {
            return Ok(Vec::new());
        }
        let response = HttpClient::require_success(response).await?;
        let body: typing::ListResponse<typing::StreamRoute> =
            response.json().await.map_err(|e| {
                BackendError::Serialization(format!(
                    "decoding response from /apisix/admin/stream_routes: {e}"
                ))
            })?;
        Ok(body.list.into_iter().map(|item| item.value).collect())
    }

    /// Consumers, plus (from APISIX 3.11.0 onward, where the credentials
    /// API exists) each consumer's credentials, fetched concurrently. A 404
    /// on a specific consumer's credentials endpoint means that consumer
    /// simply has none — anything else (network failure, 5xx) is a real
    /// error and aborts the whole call, same as any other resource type.
    pub async fn list_consumers(&self) -> Result<Vec<typing::Consumer>, BackendError> {
        let consumers: Vec<typing::Consumer> = self.list(ResourceType::Consumer).await?;

        if self.version < Version::new(3, 11, 0) {
            return Ok(consumers);
        }

        concurrent_map_until_err(consumers, Some(self.concurrency), |consumer| {
            self.with_credentials(consumer)
        })
        .await
    }

    async fn with_credentials(
        &self,
        consumer: typing::Consumer,
    ) -> Result<typing::Consumer, BackendError> {
        let path = format!("/apisix/admin/consumers/{}/credentials", consumer.username);
        let builder = self.client.request(Method::GET, &path)?;
        // Purpose isn't obvious from the URL alone in a `--verbose 2` dump
        // of N concurrent credential fetches.
        let response = self
            .client
            .execute_described(
                builder,
                &format!("Get credentials of consumer \"{}\"", consumer.username),
            )
            .await?;
        if response.status().as_u16() == 404 {
            return Ok(consumer);
        }
        let response = HttpClient::require_success(response).await?;
        let body: typing::ListResponse<typing::ConsumerCredential> =
            response.json().await.map_err(|e| {
                BackendError::Serialization(format!("decoding response from {path}: {e}"))
            })?;
        Ok(typing::Consumer {
            credentials: Some(body.list.into_iter().map(|item| item.value).collect()),
            ..consumer
        })
    }

    /// Fetches every resource type (concurrently) and assembles them into a
    /// single ADC `Configuration`: routes and stream routes nested under
    /// their owning service, a service's default upstream inlined onto it,
    /// its named upstreams (matched via
    /// [`typing::ADC_UPSTREAM_SERVICE_ID_LABEL`]) collected into
    /// `Service.upstreams`, and a route's `plugin_config_id` resolved into
    /// its `plugins` — APISIX stores each of those as a link between two
    /// separate admin-API resources; ADC's model has them nested instead.
    pub async fn dump(&self) -> Result<Configuration, BackendError> {
        // Step 1: fetch every resource type concurrently, in APISIX's own wire shape.
        let (
            services,
            mut routes,
            upstreams,
            ssls,
            consumers,
            plugin_configs,
            global_rules,
            plugin_metadata,
            stream_routes,
        ) = tokio::try_join!(
            self.list_services(),
            self.list_routes(),
            self.list_upstreams(),
            self.list_ssls(),
            self.list_consumers(),
            self.list_plugin_configs(),
            self.list_global_rules(),
            self.list_plugin_metadata(),
            self.list_stream_routes(),
        )?;

        // Step 2: resolve the two APISIX-only cross-references that don't
        // survive as-is in ADC's model — a route's `plugin_config_id`
        // becomes its resolved `plugins`, and the flat upstream list splits
        // into "this service's default upstream" and "this service's named
        // upstreams".
        resolve_plugin_config_refs(&mut routes, &plugin_configs);
        let (default_upstream_by_id, named_upstreams_by_service) = index_upstreams(upstreams)?;

        // Step 3: convert each service to ADC's model and attach its
        // upstream(s) from step 2.
        let mut services: IndexMap<String, adc::Service> = services
            .into_iter()
            .map(|service| {
                let id = service.id.clone();
                let mut service: adc::Service =
                    service.try_into().map_err(BackendError::Serialization)?;
                if let Some(upstream) = default_upstream_by_id.get(&id) {
                    let mut upstream = upstream.clone();
                    upstream.id = None;
                    upstream.name = None;
                    service.upstream = Some(upstream);
                }
                if let Some(named) = named_upstreams_by_service.get(&id) {
                    service.upstreams = Some(named.clone());
                }
                Ok((id, service))
            })
            .collect::<Result<_, BackendError>>()?;

        // Step 4: bucket routes and stream routes by their owning service —
        // an orphaned one (referencing a service id that wasn't in this
        // dump, which shouldn't normally happen) is dropped rather than
        // surfaced as an error.
        let mut routes_by_service: HashMap<String, Vec<adc::Route>> = HashMap::new();
        for route in routes {
            let Some(service_id) = route.service_id.clone() else {
                continue;
            };
            if !services.contains_key(&service_id) {
                continue;
            }
            routes_by_service
                .entry(service_id)
                .or_default()
                .push(route.try_into().map_err(BackendError::Serialization)?);
        }
        let mut stream_routes_by_service: HashMap<String, Vec<adc::StreamRoute>> = HashMap::new();
        for stream_route in stream_routes {
            let Some(service_id) = stream_route.service_id.clone() else {
                continue;
            };
            if !services.contains_key(&service_id) {
                continue;
            }
            stream_routes_by_service
                .entry(service_id)
                .or_default()
                .push(stream_route.into());
        }

        // Step 5: attach each service's bucketed routes/stream routes from
        // step 4. A service is either HTTP or stream, never both — mirrors
        // `ServiceRoutes`'s own invariant, and matches how APISIX data is
        // actually shaped (a route and a stream_route never share a
        // `service_id`).
        for (id, service) in services.iter_mut() {
            if let Some(routes) = routes_by_service.remove(id) {
                service.routes = Some(adc::ServiceRoutes::Http { routes });
            } else if let Some(stream_routes) = stream_routes_by_service.remove(id) {
                service.routes = Some(adc::ServiceRoutes::Stream { stream_routes });
            }
        }

        // Step 6: assemble the final Configuration — everything not nested
        // under a service converts independently.
        let mut configuration = Configuration {
            services: (!services.is_empty()).then(|| services.into_values().collect()),
            ssls: (!ssls.is_empty())
                .then(|| {
                    ssls.into_iter()
                        .map(adc::SSL::try_from)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()
                .map_err(BackendError::Serialization)?,
            consumers: (!consumers.is_empty())
                .then(|| consumers.into_iter().map(Into::into).collect()),
            consumer_groups: None, // apisix's fetcher doesn't fetch consumer groups at all — see `crate::transformer`'s doc comment.
            global_rules: (!global_rules.is_empty()).then_some(global_rules),
            plugin_metadata: (!plugin_metadata.is_empty()).then_some(plugin_metadata),
        };

        // Step 7: re-check every resource against the label selector
        // client-side. The `labels[key]=value` query params attached to
        // each request above (`Fetcher::list`) are a request to the server
        // to narrow its response, not a guarantee that it did — nothing
        // here can tell whether an unrecognized query param was silently
        // ignored, so the result can't be trusted as filtered until this
        // runs.
        self.filter.filter_configuration(&mut configuration);

        Ok(configuration)
    }
}

/// Resolves each route's `plugin_config_id` reference into its `plugins`,
/// in place — APISIX stores a plugin config as its own admin-API resource
/// and a route merely points at one by id; ADC's `Route` has no equivalent
/// reference field, only the resolved `plugins`. A route can carry its own
/// inline `plugins` alongside a `plugin_config_id` at the same time; APISIX
/// merges the two at request-serving time with the route's own entries
/// winning on a name collision, so the reconstructed `plugins` here does
/// the same rather than letting the plugin config clobber the route's own.
fn resolve_plugin_config_refs(
    routes: &mut [typing::Route],
    plugin_configs: &[typing::PluginConfig],
) {
    let by_id: HashMap<&str, &typing::PluginConfig> = plugin_configs
        .iter()
        .map(|pc| (pc.id.as_str(), pc))
        .collect();
    for route in routes {
        if let Some(plugin_config_id) = &route.plugin_config_id
            && let Some(plugin_config) = by_id.get(plugin_config_id.as_str())
        {
            let mut plugins = plugin_config.plugins.clone();
            plugins.extend(route.plugins.clone().unwrap_or_default());
            route.plugins = Some(plugins);
        }
    }
}

type DefaultUpstreamById = HashMap<String, adc::Upstream>;
type NamedUpstreamsByService = HashMap<String, Vec<adc::Upstream>>;

/// Splits APISIX's flat upstream list into: the default upstream for each
/// service that has one (keyed by upstream id, which for a service's
/// default upstream is always the service's own id), and each service's
/// *named* upstreams (matched via the association label ADC writes on
/// `Backend::sync` — see `typing::ADC_UPSTREAM_SERVICE_ID_LABEL`).
fn index_upstreams(
    upstreams: Vec<typing::Upstream>,
) -> Result<(DefaultUpstreamById, NamedUpstreamsByService), BackendError> {
    let mut by_id = HashMap::new();
    let mut named_by_service: HashMap<String, Vec<adc::Upstream>> = HashMap::new();

    for upstream in upstreams {
        let id = upstream.id.clone();
        let service_label = upstream
            .labels
            .as_ref()
            .and_then(|labels| labels.get(typing::ADC_UPSTREAM_SERVICE_ID_LABEL))
            .and_then(|value| match value {
                LabelValue::Single(name) => Some(name.clone()),
                LabelValue::Multiple(_) => None,
            });
        let upstream: adc::Upstream = upstream.try_into().map_err(BackendError::Serialization)?;

        if let Some(service_id) = service_label {
            named_by_service
                .entry(service_id)
                .or_default()
                .push(upstream.clone());
        }
        if let Some(id) = id {
            by_id.insert(id, upstream);
        }
    }

    Ok((by_id, named_by_service))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

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
            ResourceType::Route,
            ResourceType::Upstream,
            ResourceType::Ssl,
            ResourceType::GlobalRule,
            ResourceType::PluginMetadata,
            ResourceType::StreamRoute,
            ResourceType::Consumer,
        ]);
        let filter = ResourceFilter {
            include: HashSet::new(),
            exclude,
            label_selector: HashMap::new(),
        };
        let fetcher = Fetcher::new(unreachable_client(), Version::new(999, 999, 999), filter, 10);

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
    /// generically (an empty `list` satisfies every resource type's
    /// envelope without needing per-type fixtures) — used to prove a
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
                    axum::Json(serde_json::json!({ "list": [] }))
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
        let fetcher = Fetcher::new(client, Version::new(999, 999, 999), filter, 10);

        fetcher.dump().await.unwrap();

        let seen = seen.lock().await;
        assert!(
            !seen.iter().any(|path| path == "/apisix/admin/services"),
            "{seen:?}"
        );
        assert!(
            seen.iter().any(|path| path == "/apisix/admin/routes"),
            "{seen:?}"
        );
    }

    #[test]
    fn a_top_level_collection_request_carries_the_label_selector() {
        let filter = ResourceFilter {
            include: HashSet::new(),
            exclude: HashSet::new(),
            label_selector: HashMap::from([("env".to_string(), "prod".to_string())]),
        };
        let fetcher = Fetcher::new(unreachable_client(), Version::new(999, 999, 999), filter, 10);

        let builder = fetcher.filter.attach_label_selector(
            fetcher
                .client
                .request(Method::GET, "/apisix/admin/services")
                .unwrap(),
        );
        let request = builder.build().unwrap();
        assert_eq!(request.url().query(), Some("labels%5Benv%5D=prod"));
    }

    /// A server that ignores the `labels[...]` query param entirely and
    /// always returns every service — standing in for an admin API that
    /// doesn't actually support server-side label filtering (unverified for
    /// APISIX; the query param is sent on a best-effort basis).
    async fn spawn_server_that_ignores_the_label_query() -> String {
        let router = axum::Router::new()
            .route(
                "/apisix/admin/services",
                axum::routing::get(|| async {
                    axum::Json(serde_json::json!({
                        "list": [
                            {"key": "/apisix/admin/services/1", "value": {"id": "1", "name": "matches", "labels": {"env": "prod"}}},
                            {"key": "/apisix/admin/services/2", "value": {"id": "2", "name": "no-match", "labels": {"env": "dev"}}},
                        ]
                    }))
                }),
            )
            .fallback(axum::routing::any(|| async { axum::Json(serde_json::json!({ "list": [] })) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn dump_filters_by_label_client_side_even_when_the_server_ignores_the_query() {
        let server = spawn_server_that_ignores_the_label_query().await;
        let client = HttpClient::new(HttpClientConfig {
            server,
            token: "test-token".to_string(),
            timeout: None,
            tls: TlsConfig::default(),
        })
        .unwrap();
        let filter = ResourceFilter {
            include: HashSet::from([ResourceType::Service]),
            exclude: HashSet::new(),
            label_selector: HashMap::from([("env".to_string(), "prod".to_string())]),
        };
        let fetcher = Fetcher::new(client, Version::new(999, 999, 999), filter, 10);

        let configuration = fetcher.dump().await.unwrap();

        let names: Vec<String> = configuration.services.unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["matches"]);
    }
}
