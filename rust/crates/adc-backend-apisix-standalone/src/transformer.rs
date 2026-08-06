//! Converting `typing::ApisixStandalone` (the whole config document) into
//! ADC's nested `Configuration` model — the read direction used by
//! `crate::fetcher::Fetcher::dump` and, after a sync, to refresh
//! `crate::cache::Cache`'s cached `Configuration` from the just-written raw
//! document. There is no write-direction counterpart module here the way
//! `adc-backend-apisix` has one: standalone's write path
//! (`crate::operator::Operator`) builds each resource's wire body directly
//! off the differ's `Event`, not off a full `Configuration`.

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
        checks: upstream.checks.clone(),
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
/// (rather than TS's `keys?.[idx]` producing `undefined`, which would leave
/// a certificate entry with no key at all) — mirrors
/// `adc-backend-apisix::transformer`'s identical fix for the same
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
/// doesn't reject an unrecognized plugin name or a non-object config —
/// matching the TS transformer, which casts the plugin name and passes the
/// config through without validating either.
fn credential_to_adc(credential: &typing::ConsumerCredential, username: &str) -> Option<adc::ConsumerCredential> {
    let plugins = credential.plugins.clone()?;
    let (plugin_name, config) = plugins.into_iter().next()?;
    let config = match config {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    let prefix = format!("{username}/credentials/");
    let id = credential.id.strip_prefix(&prefix).unwrap_or(&credential.id).to_string();

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

    let services = input.services.iter().flatten().map(|service| {
        let upstream = service
            .upstream_id
            .as_deref()
            .and_then(|upstream_id| {
                input
                    .upstreams
                    .iter()
                    .flatten()
                    .find(|upstream| upstream.id == upstream_id)
            })
            .map(|upstream| adc::Upstream {
                name: None,
                ..wire_upstream_to_adc(upstream)
            });

        let named_upstreams: Vec<adc::Upstream> = input
            .upstreams
            .iter()
            .flatten()
            .filter(|upstream| {
                upstream
                    .labels
                    .as_ref()
                    .and_then(|labels| labels.get(typing::ADC_UPSTREAM_SERVICE_ID_LABEL))
                    .is_some_and(|owner| owner == &service.id)
            })
            .map(|upstream| adc::Upstream {
                id: Some(upstream.id.clone()),
                labels: strip_service_id_label(to_adc_labels(upstream.labels.clone())),
                ..wire_upstream_to_adc(upstream)
            })
            .collect();

        let routes: Vec<adc::Route> = input
            .routes
            .iter()
            .flatten()
            .filter(|route| route.service_id == service.id)
            .map(route_to_adc)
            .collect();
        let stream_routes: Vec<adc::StreamRoute> = input
            .stream_routes
            .iter()
            .flatten()
            .filter(|route| route.service_id == service.id)
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
            let owned: Vec<adc::ConsumerCredential> = credentials
                .iter()
                .filter(|credential| credential.id.starts_with(&format!("{}/credentials/", consumer.username)))
                .filter_map(|credential| credential_to_adc(credential, &consumer.username))
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

    // `rest` here intentionally keeps `modifiedIndex` alongside each
    // plugin's own config keys — matches the TS transformer's `const {id,
    // ...rest} = pluginMetadata` destructure, which only pulls `id` out and
    // leaves `modifiedIndex` in `rest`.
    let mut plugin_metadata = adc::Plugins::new();
    for entry in input.plugin_metadata.iter().flatten() {
        let mut rest = entry.extra.clone();
        rest.insert("modifiedIndex".to_string(), Value::from(entry.modified_index));
        plugin_metadata.insert(entry.id.clone(), Value::Object(rest));
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
