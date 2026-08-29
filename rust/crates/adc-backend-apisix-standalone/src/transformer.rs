//! Converting `typing::ApisixStandalone` (the whole config document) into
//! ADC's nested `Configuration` model — the read direction used by
//! `crate::fetcher::Fetcher::dump` and, after a sync, to refresh
//! `crate::cache::Cache`'s cached `Configuration` from the just-written raw
//! document. There is no write-direction counterpart module here the way
//! `adc-backend-apisix` has one: standalone's write path
//! (`crate::operator::Operator`) builds each resource's wire body directly
//! off the differ's `Event`, not off a full `Configuration`.

use std::collections::HashMap;

use serde_json::{Map, Value};

use adc_sdk::resources::{self as adc, LabelValue};

use crate::typing;

fn to_adc_labels(labels: Option<typing::StandaloneLabels>) -> Option<adc::Labels> {
    labels.map(|labels| {
        labels
            .into_iter()
            .map(|(key, value)| (key, LabelValue::Single(value)))
            .collect()
    })
}

/// Drops the service-association bookkeeping label a named upstream is
/// stamped with (see `typing::ADC_UPSTREAM_SERVICE_ID_LABEL`) — not
/// something a consumer of the ADC model should see. A service's own
/// inline default upstream never carries this label in the first place
/// (see `crate::operator::Operator::apply_event_for_service_inlined_upstream`),
/// so this is only ever called for named upstreams.
fn strip_service_id_label(labels: Option<adc::Labels>) -> Option<adc::Labels> {
    labels
        .map(|mut labels| {
            labels.remove(typing::ADC_UPSTREAM_SERVICE_ID_LABEL);
            labels
        })
        .filter(|labels| !labels.is_empty())
}

/// `req_headers` on the wire is `http_req_headers` in ADC; every other
/// active health check field is named the same.
fn health_check_to_adc(checks: typing::UpstreamHealthCheck) -> adc::UpstreamHealthCheck {
    let default = adc::UpstreamHealthCheckActive::default();
    adc::UpstreamHealthCheck {
        active: adc::UpstreamHealthCheckActive {
            r#type: checks.active.ty.unwrap_or(default.r#type),
            timeout: checks.active.timeout.unwrap_or(default.timeout),
            concurrency: checks.active.concurrency.unwrap_or(default.concurrency),
            host: checks.active.host,
            port: checks.active.port,
            http_method: checks.active.http_method.unwrap_or(default.http_method),
            http_path: checks.active.http_path.unwrap_or(default.http_path),
            http_req_headers: checks.active.req_headers,
            http_req_body: checks.active.http_req_body.unwrap_or(default.http_req_body),
            https_verify_certificate: checks
                .active
                .https_verify_certificate
                .unwrap_or(default.https_verify_certificate),
            healthy: checks.active.healthy,
            unhealthy: checks.active.unhealthy,
        },
        passive: checks.passive,
    }
}

/// Builds an upstream's ADC shape, minus `id` (callers that need one set
/// it themselves — a service's own default upstream never gets one, a
/// named upstream does) and always carrying `name` (callers building a
/// service's own default upstream override it back to `None` afterward,
/// since a service's default upstream has no independent name in ADC's
/// model).
fn wire_upstream_to_adc(upstream: &typing::Upstream) -> adc::Upstream {
    adc::Upstream {
        id: None,
        name: Some(upstream.name.clone()),
        description: upstream.desc.clone(),
        labels: to_adc_labels(upstream.labels.clone()),

        r#type: upstream.ty.unwrap_or_default(),
        hash_on: upstream.hash_on.clone(),
        key: upstream.key.clone(),
        checks: upstream.checks.clone().map(health_check_to_adc),
        nodes: upstream.nodes.clone(),
        scheme: upstream.scheme.unwrap_or_default(),
        retries: upstream.retries,
        retry_timeout: upstream.retry_timeout,
        timeout: upstream.timeout.clone(),
        tls: upstream.tls.clone(),
        keepalive_pool: upstream.keepalive_pool.clone(),
        pass_host: upstream.pass_host.unwrap_or_default(),
        upstream_host: upstream.upstream_host.clone(),

        service_name: upstream.service_name.clone(),
        discovery_type: upstream.discovery_type.clone(),
        discovery_args: upstream.discovery_args.clone(),
    }
}

fn route_to_adc(route: &typing::Route) -> adc::Route {
    adc::Route {
        id: Some(route.id.clone()),
        name: route.name.clone(),
        description: route.desc.clone(),
        labels: to_adc_labels(route.labels.clone()),

        hosts: route.hosts.clone(),
        uris: route.uris.clone(),
        priority: route.priority,
        timeout: route.timeout.clone(),
        vars: route.vars.clone(),
        methods: route.methods.clone(),
        enable_websocket: route.enable_websocket,
        remote_addrs: route.remote_addrs.clone(),
        plugins: route.plugins.clone(),
        filter_func: route.filter_func.clone(),
    }
}

fn stream_route_to_adc(route: &typing::StreamRoute) -> adc::StreamRoute {
    adc::StreamRoute {
        id: Some(route.id.clone()),
        name: route.name.clone(),
        description: route.desc.clone(),
        labels: to_adc_labels(route.labels.clone()),

        plugins: route.plugins.clone(),
        remote_addr: route.remote_addr.clone(),
        server_addr: route.server_addr.clone(),
        server_port: route.server_port,
        sni: route.sni.clone(),
    }
}

/// Zips a certificate list with its matching key list positionally,
/// falling back to an empty key past the end of a shorter `keys` list
/// rather than leaving a certificate entry with no key at all — the same
/// fix `adc-backend-apisix::transformer` applies for this identical
/// mismatched-length edge case.
fn ssl_to_adc(ssl: &typing::Ssl) -> adc::SSL {
    let mut keys = ssl.keys.clone().unwrap_or_default().into_iter();
    let mut certificates = vec![adc::SSLCertificate {
        certificate: ssl.cert.clone(),
        key: ssl.key.clone(),
    }];
    if let Some(certs) = &ssl.certs {
        certificates.extend(certs.iter().map(|certificate| adc::SSLCertificate {
            certificate: certificate.clone(),
            key: keys.next().unwrap_or_default(),
        }));
    }

    adc::SSL {
        id: Some(ssl.id.clone()),
        labels: to_adc_labels(ssl.labels.clone()),
        r#type: ssl.ty.unwrap_or_default(),
        snis: ssl.snis.clone(),
        certificates,
        client: ssl.client.clone(),
        ssl_protocols: ssl.ssl_protocols.clone(),
    }
}

/// A credential's `type`/`config` come from its single plugin entry
/// (standalone models a credential as a one-plugin `Plugins` map, same as
/// `adc-backend-apisix`); a credential with no plugin configured has
/// nothing to convert. Unlike `adc-backend-apisix`'s equivalent, this
/// doesn't reject an unrecognized plugin name — it passes through
/// unvalidated. A non-object config isn't rejected either, but it isn't
/// passed through as-is: it's replaced with an empty map.
fn credential_to_adc(credential: &typing::ConsumerCredential, prefix: &str) -> Option<adc::ConsumerCredential> {
    let plugins = credential.plugins.clone()?;
    let (plugin_name, config) = plugins.into_iter().next()?;
    let config = match config {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    let id = credential.id.strip_prefix(prefix).unwrap_or(&credential.id).to_string();

    Some(adc::ConsumerCredential {
        id: Some(id),
        name: credential.name.clone(),
        description: credential.desc.clone(),
        labels: to_adc_labels(credential.labels.clone()),
        r#type: plugin_name,
        config,
    })
}

/// Converts the whole standalone config document into ADC's nested
/// `Configuration` model: routes/stream_routes/named-upstreams get nested
/// under their owning service, consumer credentials under their owning
/// consumer, and `global_rules`/`plugin_metadata` (each already a flat
/// per-plugin map on the wire, just split across possibly-multiple entries)
/// get merged into one map apiece.
pub fn to_adc(input: &typing::ApisixStandalone) -> adc::Configuration {
    let credentials: Vec<&typing::ConsumerCredential> = input
        .consumers
        .iter()
        .flatten()
        .filter_map(typing::ConsumerOrCredential::as_credential)
        .collect();

    // Grouped once up front rather than re-scanned per service: with S
    // services and U/R/T upstreams/routes/stream_routes, filtering inside
    // the services closure below costs O(S*(U+R+T)); a single grouping
    // pass costs O(U+R+T) plus an O(1) lookup per service.
    let upstream_by_id: HashMap<&str, &typing::Upstream> =
        input.upstreams.iter().flatten().map(|upstream| (upstream.id.as_str(), upstream)).collect();

    let mut named_upstreams_by_service: HashMap<&str, Vec<&typing::Upstream>> = HashMap::new();
    for upstream in input.upstreams.iter().flatten() {
        if let Some(owner) = upstream.labels.as_ref().and_then(|labels| labels.get(typing::ADC_UPSTREAM_SERVICE_ID_LABEL)) {
            named_upstreams_by_service.entry(owner.as_str()).or_default().push(upstream);
        }
    }

    let mut routes_by_service: HashMap<&str, Vec<&typing::Route>> = HashMap::new();
    for route in input.routes.iter().flatten() {
        routes_by_service.entry(route.service_id.as_str()).or_default().push(route);
    }

    let mut stream_routes_by_service: HashMap<&str, Vec<&typing::StreamRoute>> = HashMap::new();
    for route in input.stream_routes.iter().flatten() {
        stream_routes_by_service.entry(route.service_id.as_str()).or_default().push(route);
    }

    let services = input.services.iter().flatten().map(|service| {
        let upstream = service
            .upstream_id
            .as_deref()
            .and_then(|upstream_id| upstream_by_id.get(upstream_id).copied())
            .map(|upstream| adc::Upstream {
                name: None,
                ..wire_upstream_to_adc(upstream)
            });

        let named_upstreams: Vec<adc::Upstream> = named_upstreams_by_service
            .get(service.id.as_str())
            .into_iter()
            .flatten()
            .copied()
            .map(|upstream| adc::Upstream {
                id: Some(upstream.id.clone()),
                labels: strip_service_id_label(to_adc_labels(upstream.labels.clone())),
                ..wire_upstream_to_adc(upstream)
            })
            .collect();

        let routes: Vec<adc::Route> =
            routes_by_service.get(service.id.as_str()).into_iter().flatten().copied().map(route_to_adc).collect();
        let stream_routes: Vec<adc::StreamRoute> = stream_routes_by_service
            .get(service.id.as_str())
            .into_iter()
            .flatten()
            .copied()
            .map(stream_route_to_adc)
            .collect();

        // A service is either HTTP or stream, never both — matches
        // `ServiceRoutes`'s own invariant, and how standalone data is
        // actually shaped (a route and a stream_route never share a
        // `service_id`).
        let routes = if !routes.is_empty() {
            Some(adc::ServiceRoutes::Http { routes })
        } else if !stream_routes.is_empty() {
            Some(adc::ServiceRoutes::Stream { stream_routes })
        } else {
            None
        };

        adc::Service {
            id: Some(service.id.clone()),
            name: service.name.clone(),
            description: service.desc.clone(),
            labels: to_adc_labels(service.labels.clone()),
            upstream,
            upstreams: (!named_upstreams.is_empty()).then_some(named_upstreams),
            plugins: service.plugins.clone(),
            path_prefix: None,
            strip_path_prefix: None,
            hosts: service.hosts.clone(),
            routes,
        }
    });
    let services: Vec<adc::Service> = services.collect();

    let consumers: Vec<adc::Consumer> = input
        .consumers
        .iter()
        .flatten()
        .filter_map(typing::ConsumerOrCredential::as_consumer)
        .map(|consumer| {
            let prefix = format!("{}/credentials/", consumer.username);
            let owned: Vec<adc::ConsumerCredential> = credentials
                .iter()
                .filter(|credential| credential.id.starts_with(&prefix))
                .filter_map(|credential| credential_to_adc(credential, &prefix))
                .collect();

            adc::Consumer {
                username: consumer.username.clone(),
                description: consumer.desc.clone(),
                labels: to_adc_labels(consumer.labels.clone()),
                plugins: consumer.plugins.clone(),
                credentials: Some(owned),
            }
        })
        .collect();

    let ssls: Vec<adc::SSL> = input.ssls.iter().flatten().map(ssl_to_adc).collect();

    let mut global_rules = adc::Plugins::new();
    for entry in input.global_rules.iter().flatten() {
        if let Some(plugins) = &entry.plugins {
            global_rules.extend(plugins.clone());
        }
    }

    // `modifiedIndex` is APISIX's own per-entry version counter, not part of
    // the declarative model — a client never sends it. Kept out of the
    // modelled view so a re-dump diffs equal against the config that was
    // last synced, instead of showing a phantom `plugin_metadata` change
    // (and bumping `plugin_metadata_conf_version`) on every sync.
    let mut plugin_metadata = adc::Plugins::new();
    for entry in input.plugin_metadata.iter().flatten() {
        plugin_metadata.insert(entry.id.clone(), Value::Object(entry.extra.clone()));
    }

    adc::Configuration {
        services: (!services.is_empty()).then_some(services),
        ssls: (!ssls.is_empty()).then_some(ssls),
        consumers: (!consumers.is_empty()).then_some(consumers),
        consumer_groups: None,
        global_rules: (!global_rules.is_empty()).then_some(global_rules),
        plugin_metadata: (!plugin_metadata.is_empty()).then_some(plugin_metadata),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `server_port` is `u16` at the wire level too (not a wider int
    /// narrowed later): a port outside 0-65535 is never a real port, so a
    /// document containing one is rejected right at deserialization
    /// instead of being silently dropped downstream.
    #[test]
    fn an_out_of_range_wire_port_is_rejected_at_deserialization() {
        let json = serde_json::json!({
            "modifiedIndex": 1,
            "id": "sr1",
            "name": "sr1",
            "service_id": "svc1",
            "server_port": 70_000,
        });
        assert!(serde_json::from_value::<typing::StreamRoute>(json).is_err());
    }

    /// `modifiedIndex` is APISIX's own version counter; a client never
    /// sends it, so leaking it into the modelled view made every re-dump
    /// diff unequal against the last-synced config — a phantom
    /// `plugin_metadata` change on every sync.
    #[test]
    fn to_adc_omits_plugin_metadata_modified_index() {
        let extra = serde_json::json!({ "log_format": { "host": "$host" } })
            .as_object()
            .unwrap()
            .clone();
        let wire = typing::ApisixStandalone {
            plugin_metadata: Some(vec![typing::PluginMetadata {
                modified_index: 42,
                id: "http-logger".to_string(),
                extra,
            }]),
            ..Default::default()
        };

        let entry = to_adc(&wire).plugin_metadata.unwrap().get("http-logger").unwrap().clone();

        assert!(entry.get("modifiedIndex").is_none(), "modifiedIndex leaked into the modelled view: {entry}");
        assert_eq!(entry["log_format"]["host"], "$host");
    }

    #[test]
    fn active_health_check_req_headers_maps_to_adc_http_req_headers() {
        let checks = typing::UpstreamHealthCheck {
            active: typing::UpstreamHealthCheckActive {
                ty: Some(adc_sdk::resources::UpstreamHealthCheckType::Http),
                req_headers: Some(vec!["X-Foo: bar".to_string()]),
                http_req_body: Some("ping".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let adc = health_check_to_adc(checks);
        assert_eq!(adc.active.http_req_headers, Some(vec!["X-Foo: bar".to_string()]));
        assert_eq!(adc.active.http_req_body, "ping");
    }
}
