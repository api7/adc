//! Converting between `typing::ApisixStandalone` (the whole config document)
//! and ADC's nested `Configuration` model, both ways:
//!
//! - `to_adc`: wire -> model. Used by `crate::fetcher::Fetcher::dump` (and,
//!   at bootstrap only, to seed `crate::cache::Cache`'s cached desired
//!   config from a real cluster read — see `transform_to_wire`'s doc
//!   comment for why every *later* sync seeds it from the desired
//!   `Configuration` directly instead).
//! - `transform_to_wire`: model -> wire. Used by `crate::operator::Operator`
//!   to build the document a sync PUTs, directly off the full reconstructed
//!   desired `Configuration` — no folding onto a prior wire document.
//!
//! Ids round-trip through both directions unchanged: `to_adc` copies a wire
//! item's `id` straight into the model, and `transform_to_wire` copies it
//! straight back out, on the assumption every resource reaching it already
//! carries the same id the differ derived for it (`adc_differ::apply`'s
//! reconstruction stamps every resource with that id — see its own doc
//! comment). Neither direction ever calls `adc_sdk::utils::generate_id`
//! itself.

use std::collections::{BTreeMap, HashMap};

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
    let credentials: Vec<&typing::ConsumerCredential> =
        input.consumers.iter().filter_map(typing::ConsumerOrCredential::as_credential).collect();

    // Grouped once up front rather than re-scanned per service: with S
    // services and U/R/T upstreams/routes/stream_routes, filtering inside
    // the services closure below costs O(S*(U+R+T)); a single grouping
    // pass costs O(U+R+T) plus an O(1) lookup per service.
    let upstream_by_id: HashMap<&str, &typing::Upstream> =
        input.upstreams.iter().map(|upstream| (upstream.id.as_str(), upstream)).collect();

    let mut named_upstreams_by_service: HashMap<&str, Vec<&typing::Upstream>> = HashMap::new();
    for upstream in &input.upstreams {
        if let Some(owner) = upstream.labels.as_ref().and_then(|labels| labels.get(typing::ADC_UPSTREAM_SERVICE_ID_LABEL)) {
            named_upstreams_by_service.entry(owner.as_str()).or_default().push(upstream);
        }
    }

    let mut routes_by_service: HashMap<&str, Vec<&typing::Route>> = HashMap::new();
    for route in &input.routes {
        routes_by_service.entry(route.service_id.as_str()).or_default().push(route);
    }

    let mut stream_routes_by_service: HashMap<&str, Vec<&typing::StreamRoute>> = HashMap::new();
    for route in &input.stream_routes {
        stream_routes_by_service.entry(route.service_id.as_str()).or_default().push(route);
    }

    let services = input.services.iter().map(|service| {
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

    let ssls: Vec<adc::SSL> = input.ssls.iter().map(ssl_to_adc).collect();

    let mut global_rules = adc::Plugins::new();
    for entry in &input.global_rules {
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
    for entry in &input.plugin_metadata {
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

// ---------------------------------------------------------------------
// Write direction: ADC's `Configuration` -> the wire document.
// ---------------------------------------------------------------------

fn from_adc_labels(labels: Option<adc::Labels>) -> Option<typing::StandaloneLabels> {
    labels.map(|labels| labels.into_iter().map(|(key, value)| (key, stringify_label_value(value))).collect())
}

fn stringify_label_value(value: LabelValue) -> String {
    match value {
        LabelValue::Single(s) => s,
        LabelValue::Multiple(items) => serde_json::to_string(&items).unwrap_or_default(),
    }
}

/// ADC's `http_req_headers` maps to the wire's `req_headers`; every other
/// active health check field is named the same.
fn health_check_from_adc(checks: adc::UpstreamHealthCheck) -> typing::UpstreamHealthCheck {
    typing::UpstreamHealthCheck {
        active: typing::UpstreamHealthCheckActive {
            ty: Some(checks.active.r#type),
            timeout: Some(checks.active.timeout),
            concurrency: Some(checks.active.concurrency),
            host: checks.active.host,
            port: checks.active.port,
            http_method: Some(checks.active.http_method),
            http_path: Some(checks.active.http_path),
            req_headers: checks.active.http_req_headers,
            http_req_body: Some(checks.active.http_req_body),
            https_verify_certificate: Some(checks.active.https_verify_certificate),
            healthy: checks.active.healthy,
            unhealthy: checks.active.unhealthy,
        },
        passive: checks.passive,
    }
}

/// Builds an upstream's wire body from its ADC shape, minus `id`/
/// `modifiedIndex`/`name` — every caller overwrites those with values that
/// come from context (an owning service's id/name for its inline default
/// upstream, or the resource's own id/name for a named one), not from the
/// upstream value itself. `parent_id`, when set, stamps the
/// service-association bookkeeping label onto a *named* upstream; a
/// service's own inline default upstream is never passed one (see
/// `typing::ADC_UPSTREAM_SERVICE_ID_LABEL`'s doc comment).
fn from_adc_upstream_wire(res: &adc::Upstream, parent_id: Option<&str>) -> typing::Upstream {
    let mut labels = from_adc_labels(res.labels.clone());
    if let Some(parent_id) = parent_id {
        labels
            .get_or_insert_with(BTreeMap::new)
            .insert(typing::ADC_UPSTREAM_SERVICE_ID_LABEL.to_string(), parent_id.to_string());
    }

    typing::Upstream {
        modified_index: 0,
        id: String::new(),
        name: res.name.clone().unwrap_or_default(),
        desc: res.description.clone(),
        labels,

        nodes: res.nodes.clone(),
        scheme: Some(res.scheme),
        ty: Some(res.r#type),
        hash_on: res.hash_on.clone(),
        key: res.key.clone(),

        pass_host: Some(res.pass_host),
        upstream_host: res.upstream_host.clone(),
        retries: res.retries,
        retry_timeout: res.retry_timeout,
        timeout: res.timeout.clone(),
        tls: res.tls.clone(),
        keepalive_pool: res.keepalive_pool.clone(),

        checks: res.checks.clone().map(health_check_from_adc),
        discovery_type: res.discovery_type.clone(),
        service_name: res.service_name.clone(),
        discovery_args: res.discovery_args.clone(),
    }
}

fn route_to_wire(route: &adc::Route, service_id: &str) -> typing::Route {
    typing::Route {
        modified_index: 0,
        id: route.id.clone().unwrap_or_default(),
        name: route.name.clone(),
        desc: route.description.clone(),
        labels: from_adc_labels(route.labels.clone()),

        uris: route.uris.clone(),
        hosts: route.hosts.clone(),
        methods: route.methods.clone(),
        remote_addrs: route.remote_addrs.clone(),
        vars: route.vars.clone(),
        filter_func: route.filter_func.clone(),

        plugins: route.plugins.clone(),
        service_id: service_id.to_string(),

        timeout: route.timeout.clone(),
        enable_websocket: route.enable_websocket,
        priority: route.priority,
        status: Some(1),
    }
}

fn stream_route_to_wire(route: &adc::StreamRoute, service_id: &str) -> typing::StreamRoute {
    typing::StreamRoute {
        modified_index: 0,
        id: route.id.clone().unwrap_or_default(),
        name: route.name.clone(),
        desc: route.description.clone(),
        labels: from_adc_labels(route.labels.clone()),

        plugins: route.plugins.clone(),
        remote_addr: route.remote_addr.clone(),
        server_addr: route.server_addr.clone(),
        server_port: route.server_port,
        sni: route.sni.clone(),
        service_id: service_id.to_string(),

        protocol: None,
    }
}

fn consumer_to_wire(consumer: &adc::Consumer) -> typing::Consumer {
    typing::Consumer {
        modified_index: 0,
        username: consumer.username.clone(),
        desc: consumer.description.clone(),
        labels: from_adc_labels(consumer.labels.clone()),
        plugins: consumer.plugins.clone(),
    }
}

/// `id` is the standalone-specific composite `<username>/credentials/<id>`
/// scheme (matches `crate::operator::generate_id_from_event`'s equivalent
/// for a differ event) — not something `adc::ConsumerCredential.id` itself
/// carries, so it's built here from the owning consumer's `username` plus
/// the credential's own (differ-derived) id.
fn credential_to_wire(username: &str, credential: &adc::ConsumerCredential) -> typing::ConsumerCredential {
    let mut plugins = adc::Plugins::new();
    plugins.insert(credential.r#type.clone(), Value::Object(credential.config.clone()));

    typing::ConsumerCredential {
        modified_index: 0,
        id: format!("{username}/credentials/{}", credential.id.clone().unwrap_or_default()),
        name: credential.name.clone(),
        desc: credential.description.clone(),
        labels: from_adc_labels(credential.labels.clone()),
        plugins: Some(plugins),
    }
}

/// Zips a certificate list with its matching key list positionally — the
/// inverse of `ssl_to_adc`'s own zip: `certificates[0]` becomes `cert`/`key`,
/// every entry after that becomes one `certs[]`/`keys[]` pair.
fn ssl_to_wire(ssl: &adc::SSL) -> typing::Ssl {
    let mut certificates = ssl.certificates.iter();
    let first = certificates.next();
    let (certs, keys): (Vec<String>, Vec<String>) =
        certificates.map(|c| (c.certificate.clone(), c.key.clone())).unzip();

    typing::Ssl {
        modified_index: 0,
        id: ssl.id.clone().unwrap_or_default(),
        desc: None,
        labels: from_adc_labels(ssl.labels.clone()),

        ty: Some(ssl.r#type),
        snis: ssl.snis.clone(),
        cert: first.map(|c| c.certificate.clone()).unwrap_or_default(),
        key: first.map(|c| c.key.clone()).unwrap_or_default(),
        certs: (!certs.is_empty()).then_some(certs),
        keys: (!keys.is_empty()).then_some(keys),
        client: ssl.client.clone(),
        ssl_protocols: ssl.ssl_protocols.clone(),

        status: 1,
    }
}

/// Builds the whole standalone wire document directly from a full ADC
/// `Configuration` — this crate's write-direction counterpart to `to_adc`,
/// and the direct replacement for what used to be a fold of differ
/// `Event`s onto a cached wire document (see `crate::operator::Operator`).
///
/// `modifiedIndex`/`*_conf_version` are all left at `0` here —
/// `crate::operator::stamp_versions` fills them in over this function's
/// output afterward (the real sync timestamp for a resource/type the
/// differ's events say changed, the previously-synced value carried over
/// for one that didn't). That decision needs the differ's `Event`s, which
/// this function never sees — it only ever sees the fully reconstructed
/// desired state, changed or not.
///
/// A service's inline default upstream (`service.upstream`) is synthesized
/// into its own top-level wire `Upstream` entry with `id`/`name` copied from
/// the *service* (not the upstream value itself, which carries neither) —
/// mirrors `to_adc`'s own reverse lookup (`ADC_UPSTREAM_SERVICE_ID_LABEL`'s
/// doc comment). `Service.upstream_id` is always set to the service's own
/// id, whether or not it actually has a default upstream — a service with
/// none simply references a wire upstream document that was never written;
/// standalone tolerates the dangling reference.
pub(crate) fn transform_to_wire(config: &adc::Configuration) -> typing::ApisixStandalone {
    let mut services = Vec::new();
    let mut upstreams = Vec::new();
    let mut routes = Vec::new();
    let mut stream_routes = Vec::new();

    for service in config.services.iter().flatten() {
        let service_id = service.id.clone().unwrap_or_default();

        if let Some(upstream) = &service.upstream {
            let mut wire = from_adc_upstream_wire(upstream, None);
            wire.id = service_id.clone();
            wire.name = service.name.clone();
            upstreams.push(wire);
        }
        for named in service.upstreams.iter().flatten() {
            let mut wire = from_adc_upstream_wire(named, Some(&service_id));
            wire.id = named.id.clone().unwrap_or_default();
            upstreams.push(wire);
        }

        match &service.routes {
            Some(adc::ServiceRoutes::Http { routes: service_routes }) => {
                routes.extend(service_routes.iter().map(|route| route_to_wire(route, &service_id)));
            }
            Some(adc::ServiceRoutes::Stream { stream_routes: service_stream_routes }) => {
                stream_routes.extend(service_stream_routes.iter().map(|route| stream_route_to_wire(route, &service_id)));
            }
            None => {}
        }

        services.push(typing::Service {
            modified_index: 0,
            id: service_id.clone(),
            name: service.name.clone(),
            desc: service.description.clone(),
            labels: from_adc_labels(service.labels.clone()),
            hosts: service.hosts.clone(),
            upstream_id: Some(service_id),
            plugins: service.plugins.clone(),
        });
    }

    let mut consumers = Vec::new();
    for consumer in config.consumers.iter().flatten() {
        consumers.push(typing::ConsumerOrCredential::Consumer(consumer_to_wire(consumer)));
        for credential in consumer.credentials.iter().flatten() {
            consumers.push(typing::ConsumerOrCredential::Credential(credential_to_wire(&consumer.username, credential)));
        }
    }

    let ssls: Vec<typing::Ssl> = config.ssls.iter().flatten().map(ssl_to_wire).collect();

    let global_rules: Vec<typing::GlobalRule> = config
        .global_rules
        .iter()
        .flatten()
        .map(|(name, value)| {
            let mut plugins = adc::Plugins::new();
            plugins.insert(name.clone(), value.clone());
            typing::GlobalRule { modified_index: 0, id: name.clone(), plugins: Some(plugins) }
        })
        .collect();

    let plugin_metadata: Vec<typing::PluginMetadata> = config
        .plugin_metadata
        .iter()
        .flatten()
        .map(|(name, value)| typing::PluginMetadata {
            modified_index: 0,
            id: name.clone(),
            extra: value.as_object().cloned().unwrap_or_default(),
        })
        .collect();

    typing::ApisixStandalone {
        routes,
        services,
        consumers,
        ssls,
        global_rules,
        plugin_metadata,
        upstreams,
        stream_routes,

        routes_conf_version: 0,
        services_conf_version: 0,
        consumers_conf_version: 0,
        ssls_conf_version: 0,
        global_rules_conf_version: 0,
        plugin_metadata_conf_version: 0,
        upstreams_conf_version: 0,
        stream_routes_conf_version: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty `Configuration` must still serialize every one of the 16
    /// wire fields explicitly (`[]`/`0`, never omitted) — this is
    /// serialization-level and can only be checked here: APISIX's own admin
    /// API doesn't preserve the distinction on the way back out (an empty
    /// collection we sent as `[]` gets dropped from the document entirely
    /// on a later `GET`), so an e2e round-trip can't observe what this
    /// crate actually put on the wire.
    #[test]
    fn transform_to_wire_always_emits_every_collection_and_conf_version_explicitly() {
        let wire = transform_to_wire(&adc::Configuration::default());
        let body = serde_json::to_value(&wire).unwrap();

        for field in ["routes", "services", "consumers", "ssls", "global_rules", "plugin_metadata", "upstreams", "stream_routes"] {
            assert_eq!(body.get(field), Some(&Value::Array(vec![])), "{field} must be an explicit empty array, not omitted");
        }
        for field in [
            "routes_conf_version",
            "services_conf_version",
            "consumers_conf_version",
            "ssls_conf_version",
            "global_rules_conf_version",
            "plugin_metadata_conf_version",
            "upstreams_conf_version",
            "stream_routes_conf_version",
        ] {
            assert_eq!(body.get(field), Some(&Value::from(0)), "{field} must be an explicit 0, not omitted");
        }
    }

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
            plugin_metadata: vec![typing::PluginMetadata {
                modified_index: 42,
                id: "http-logger".to_string(),
                extra,
            }],
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

    /// Property test: `to_adc(transform_to_wire(config)) == config` for a
    /// randomly generated `Configuration`, instead of only the handful of
    /// shapes the hand-written unit tests (and whatever each e2e resource
    /// file happens to construct) exercise. Deliberately scoped, not a
    /// generator for *every* `Configuration` this crate can express:
    ///
    /// - Every id (`Service`/named-`Upstream`/`Route`/`StreamRoute`/`Ssl`/
    ///   `ConsumerCredential`) is always `Some(unique string)`, never
    ///   `None` — matches this module's own documented precondition (ids
    ///   round-trip unchanged on the assumption they're already stamped by
    ///   `adc_differ::apply`'s reconstruction before reaching here); a
    ///   service's own *inline default* upstream is the one exception,
    ///   whose `id`/`name` are always `None` on the ADC side by design (see
    ///   `wire_upstream_to_adc`'s doc comment) and generated that way here.
    /// - Labels, discovery-based upstreams, health checks, and stream-route
    ///   `protocol` blocks aren't generated — each has its own narrower,
    ///   already-documented lossiness (labels: `LabelValue::Multiple` isn't
    ///   representable in standalone's flat string labels; discovery
    ///   upstreams and health checks would need their own dedicated
    ///   strategies; `protocol` is unconditionally dropped going to wire).
    /// - `path_prefix`/`strip_path_prefix` are always `None` — `transform_to_wire`
    ///   drops them unconditionally (standalone's wire `Service` has no
    ///   field for either), so any other value could never round-trip and
    ///   isn't this test's concern.
    mod proptests {
        use proptest::prelude::*;

        use super::*;

        fn base_upstream() -> adc::Upstream {
            adc::Upstream {
                id: None,
                name: None,
                description: None,
                labels: None,
                r#type: adc::UpstreamBalancer::default(),
                hash_on: None,
                key: None,
                checks: None,
                nodes: None,
                scheme: adc::UpstreamScheme::default(),
                retries: None,
                retry_timeout: None,
                timeout: None,
                tls: None,
                keepalive_pool: None,
                pass_host: adc::UpstreamPassHost::default(),
                upstream_host: None,
                service_name: None,
                discovery_type: None,
                discovery_args: None,
            }
        }

        #[derive(Debug, Clone)]
        struct ServiceSpec {
            inline_nodes: Option<u8>,
            named_upstream_count: u8,
            route_kind: u8,
            route_count: u8,
        }

        fn service_spec_strategy() -> impl Strategy<Value = ServiceSpec> {
            (prop::option::of(1u8..3), 0u8..3, 0u8..3, 1u8..3).prop_map(|(inline_nodes, named_upstream_count, route_kind, route_count)| {
                ServiceSpec { inline_nodes, named_upstream_count, route_kind, route_count }
            })
        }

        fn build_service(index: usize, spec: &ServiceSpec) -> adc::Service {
            let upstream = spec.inline_nodes.map(|n| adc::Upstream {
                nodes: Some((0..n).map(|i| node_strategy_fixed(index, 1000 + i as u16)).collect()),
                ..base_upstream()
            });
            let upstreams = (spec.named_upstream_count > 0).then(|| {
                (0..spec.named_upstream_count)
                    .map(|j| adc::Upstream {
                        id: Some(format!("nd-{index}-{j}")),
                        name: Some(format!("nd{index}-{j}")),
                        nodes: Some(vec![node_strategy_fixed(index, 2000 + j as u16)]),
                        ..base_upstream()
                    })
                    .collect()
            });
            let routes = match spec.route_kind {
                1 => Some(adc::ServiceRoutes::Http {
                    routes: (0..spec.route_count)
                        .map(|k| adc::Route {
                            id: Some(format!("route-{index}-{k}")),
                            name: format!("route{index}-{k}"),
                            description: None,
                            labels: None,
                            hosts: None,
                            uris: vec![format!("/route{index}-{k}")],
                            priority: None,
                            timeout: None,
                            vars: None,
                            methods: None,
                            enable_websocket: None,
                            remote_addrs: None,
                            plugins: None,
                            filter_func: None,
                        })
                        .collect(),
                }),
                2 => Some(adc::ServiceRoutes::Stream {
                    stream_routes: (0..spec.route_count)
                        .map(|k| adc::StreamRoute {
                            id: Some(format!("stream-{index}-{k}")),
                            name: format!("stream{index}-{k}"),
                            description: None,
                            labels: None,
                            plugins: None,
                            remote_addr: None,
                            server_addr: None,
                            server_port: Some(3000 + index as u16 * 10 + k as u16),
                            sni: None,
                        })
                        .collect(),
                }),
                _ => None,
            };

            adc::Service {
                id: Some(format!("svc-{index}")),
                name: format!("svc{index}"),
                description: None,
                labels: None,
                upstream,
                upstreams,
                plugins: None,
                path_prefix: None,
                strip_path_prefix: None,
                hosts: None,
                routes,
            }
        }

        fn node_strategy_fixed(service_index: usize, port: u16) -> adc::UpstreamNode {
            adc::UpstreamNode { host: format!("10.0.{service_index}.1"), port, weight: 100, priority: 0, metadata: None }
        }

        #[derive(Debug, Clone)]
        struct ConfigSpec {
            services: Vec<ServiceSpec>,
            credential_counts: Vec<u8>,
            ssl_cert_count: Option<u8>,
            global_rule_count: u8,
            plugin_metadata_count: u8,
        }

        fn config_spec_strategy() -> impl Strategy<Value = ConfigSpec> {
            (
                prop::collection::vec(service_spec_strategy(), 0..3),
                prop::collection::vec(0u8..3, 0..3),
                prop::option::of(1u8..4),
                0u8..3,
                0u8..3,
            )
                .prop_map(|(services, credential_counts, ssl_cert_count, global_rule_count, plugin_metadata_count)| ConfigSpec {
                    services,
                    credential_counts,
                    ssl_cert_count,
                    global_rule_count,
                    plugin_metadata_count,
                })
        }

        fn build_configuration(spec: &ConfigSpec) -> adc::Configuration {
            let services =
                (!spec.services.is_empty()).then(|| spec.services.iter().enumerate().map(|(i, s)| build_service(i, s)).collect());

            let consumers = (!spec.credential_counts.is_empty()).then(|| {
                spec.credential_counts
                    .iter()
                    .enumerate()
                    .map(|(i, &credential_count)| adc::Consumer {
                        username: format!("consumer{i}"),
                        description: None,
                        labels: None,
                        plugins: None,
                        credentials: Some(
                            (0..credential_count)
                                .map(|j| {
                                    let mut config = adc::Plugin::new();
                                    config.insert("key".to_string(), Value::String(format!("key-{i}-{j}")));
                                    adc::ConsumerCredential {
                                        id: Some(format!("cred-{i}-{j}")),
                                        name: format!("cred{i}-{j}"),
                                        description: None,
                                        labels: None,
                                        r#type: "key-auth".to_string(),
                                        config,
                                    }
                                })
                                .collect(),
                        ),
                    })
                    .collect()
            });

            let ssls = spec.ssl_cert_count.map(|count| {
                vec![adc::SSL {
                    id: Some("ssl-0".to_string()),
                    labels: None,
                    r#type: adc::SslType::default(),
                    snis: vec!["example.test".to_string()],
                    certificates: (0..count)
                        .map(|i| adc::SSLCertificate { certificate: format!("cert-{i}"), key: format!("key-{i}") })
                        .collect(),
                    client: None,
                    ssl_protocols: None,
                }]
            });

            let global_rules = (spec.global_rule_count > 0).then(|| {
                (0..spec.global_rule_count)
                    .map(|i| (format!("global-rule-{i}"), Value::Object(Map::new())))
                    .collect()
            });

            let plugin_metadata = (spec.plugin_metadata_count > 0).then(|| {
                (0..spec.plugin_metadata_count)
                    .map(|i| {
                        let mut m = Map::new();
                        m.insert("value".to_string(), Value::String(format!("v{i}")));
                        (format!("plugin-{i}"), Value::Object(m))
                    })
                    .collect()
            });

            adc::Configuration { services, ssls, consumers, consumer_groups: None, global_rules, plugin_metadata }
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1024))]

            #[test]
            fn transform_to_wire_and_to_adc_round_trip(spec in config_spec_strategy()) {
                let config = build_configuration(&spec);
                let round_tripped = to_adc(&transform_to_wire(&config));
                prop_assert_eq!(round_tripped, config);
            }
        }
    }
}
