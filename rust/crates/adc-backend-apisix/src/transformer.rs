//! Converting between APISIX's wire shapes (`crate::typing`) and ADC's
//! resource model (`adc_sdk::resources`).
//!
//! **Read direction** (APISIX -> ADC, used by the fetcher): a `TryFrom`/
//! `From` impl *on the ADC type*, with the wire type as the source (e.g.
//! `TryFrom<typing::Route> for adc::Route`); `Into` comes for free from the
//! standard library's blanket impl, so callers can write either
//! `adc::Route::try_from(route)` or `route.try_into()`.
//!
//! **Write direction** (ADC -> APISIX, used by the operator): the reverse
//! `From` impls, on the wire type this time (`From<adc::Consumer> for
//! typing::Consumer`) — plain `From` throughout, since nothing here can fail
//! the way parsing a live server's response can. Two conversions need more
//! than the resource itself (a route needs its parent service's id; a
//! service needs its own id split out into a matching upstream) and are
//! free functions instead, since `From` only takes one argument.
//!
//! `TryFrom` is used on the read direction wherever a conversion can
//! genuinely fail or elect not to apply (HTTP method strings APISIX didn't
//! validate, a discovery-map node with an unparsable port, a credential
//! plugin ADC doesn't support); `From` is used where it can't.
//!
//! Resource types with no ToADC conversion here (`PluginConfig`,
//! `GlobalRule`, `PluginMetadata`) either never go through a dedicated
//! per-item transform on the read path (`global_rules`/`plugin_metadata`
//! already come out of the fetcher shaped as ADC's flat `Plugins` map;
//! `plugin_configs` gets merged into a route's `plugins` rather than
//! transformed on its own), or (`ConsumerGroup`) aren't reachable from what
//! the fetcher currently fetches — APISIX's fetcher doesn't fetch consumer
//! groups at all, though sync can still write them, so `ConsumerGroup` gets
//! a write-direction conversion despite having no read-direction one.

use std::collections::HashMap;

use adc_sdk::resources::{self as adc, LabelValue};
use serde_json::Value;

use crate::typing;

const ALLOWED_CREDENTIAL_PLUGINS: &[&str] = &["key-auth", "basic-auth", "jwt-auth", "hmac-auth"];

fn parse_http_method(method: String) -> Result<adc::HttpMethod, String> {
    serde_json::from_value(Value::String(method.clone()))
        .map_err(|_| format!("unrecognized HTTP method {method:?}"))
}

impl TryFrom<typing::Route> for adc::Route {
    type Error = String;

    fn try_from(route: typing::Route) -> Result<Self, Self::Error> {
        let methods = route
            .methods
            .map(|methods| {
                methods
                    .into_iter()
                    .map(parse_http_method)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(adc::Route {
            id: Some(route.id.clone()),
            name: route.name.unwrap_or(route.id),
            description: route.desc,
            labels: route.labels.map(|labels| {
                labels
                    .into_iter()
                    .map(|(k, v)| (k, LabelValue::Single(v)))
                    .collect()
            }),

            hosts: route.host.map(|h| vec![h]).or(route.hosts),
            uris: route
                .uri
                .map(|u| vec![u])
                .or(route.uris)
                .unwrap_or_default(),
            priority: route.priority,
            timeout: route.timeout,
            vars: route.vars,
            methods,
            enable_websocket: route.enable_websocket,
            remote_addrs: route.remote_addr.map(|a| vec![a]).or(route.remote_addrs),
            plugins: route.plugins,
            filter_func: route.filter_func,
        })
    }
}

impl TryFrom<typing::Service> for adc::Service {
    type Error = String;

    fn try_from(service: typing::Service) -> Result<Self, Self::Error> {
        let upstream = service.upstream.map(adc::Upstream::try_from).transpose()?;
        let upstreams = service
            .upstreams
            .map(|list| {
                list.into_iter()
                    .map(adc::Upstream::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(adc::Service {
            id: Some(service.id.clone()),
            name: service.name.unwrap_or(service.id),
            description: service.desc,
            labels: service.labels,

            upstream,
            upstreams,
            plugins: service.plugins,
            // Not an APISIX concept: these only exist on ADC-authored config.
            path_prefix: None,
            strip_path_prefix: None,
            hosts: service.hosts,

            // Attached later, once route/stream_route fetch results are
            // available to nest under their parent service — out of scope
            // for a single resource's conversion.
            routes: None,
        })
    }
}

fn default_upstream_port(scheme: Option<adc::UpstreamScheme>) -> u32 {
    match scheme {
        Some(adc::UpstreamScheme::Http) | Some(adc::UpstreamScheme::Grpc) => 80,
        Some(adc::UpstreamScheme::Https) | Some(adc::UpstreamScheme::Grpcs) => 443,
        _ => 80,
    }
}

/// Parses APISIX's legacy `"host:port": weight` node map into ADC's node
/// list. The TS transformer's own naive `host.split(':')` mishandles any
/// host with more than one colon, including a *bracketed* IPv6 address
/// (`"[::1]:9000"`) — unlike TS, that specific case is recognized and
/// parsed correctly here (see the `strip_prefix('[')` branch below). A
/// bare, bracket-less IPv6 host is inherently ambiguous with a
/// `host:port` pair and still falls through to the "no port" branch.
fn parse_discovery_map_nodes(
    map: HashMap<String, i64>,
    scheme: Option<adc::UpstreamScheme>,
) -> Result<Vec<adc::UpstreamNode>, String> {
    map.into_iter()
        .map(|(node, weight)| {
            let (host, port) = if let Some(rest) = node.strip_prefix('[') {
                // Bracketed IPv6, e.g. "[::1]:9000" or "[::1]" — the host
                // itself contains colons, so unlike an IPv4/hostname:port
                // pair it can't be split on ':' directly.
                let (host, after_bracket) = rest
                    .split_once(']')
                    .ok_or_else(|| format!("unterminated \"[\" in upstream node {node:?}"))?;
                let port = match after_bracket.strip_prefix(':') {
                    Some(port) => port
                        .parse::<u32>()
                        .map_err(|_| format!("invalid upstream node port in {node:?}"))?,
                    None => default_upstream_port(scheme),
                };
                (host.to_string(), port)
            } else {
                let parts: Vec<&str> = node.split(':').collect();
                if parts.len() == 2 {
                    let port = parts[1]
                        .parse::<u32>()
                        .map_err(|_| format!("invalid upstream node port in {node:?}"))?;
                    (parts[0].to_string(), port)
                } else {
                    (parts[0].to_string(), default_upstream_port(scheme))
                }
            };
            Ok(adc::UpstreamNode {
                host,
                port,
                weight,
                priority: 0.0,
                metadata: None,
            })
        })
        .collect()
}

impl TryFrom<typing::Upstream> for adc::Upstream {
    type Error = String;

    fn try_from(upstream: typing::Upstream) -> Result<Self, Self::Error> {
        let nodes = match upstream.nodes {
            None => None,
            Some(typing::UpstreamNodes::List(nodes)) => Some(nodes),
            Some(typing::UpstreamNodes::Map(map)) => {
                Some(parse_discovery_map_nodes(map, upstream.scheme)?)
            }
        };

        // The service-association label is fetcher-internal bookkeeping
        // (see `typing::ADC_UPSTREAM_SERVICE_ID_LABEL`), not something a
        // consumer of the ADC model should see.
        let labels = upstream
            .labels
            .map(|mut labels| {
                labels.remove(typing::ADC_UPSTREAM_SERVICE_ID_LABEL);
                labels
            })
            .filter(|labels| !labels.is_empty());

        Ok(adc::Upstream {
            id: upstream.id,
            name: upstream.name,
            description: upstream.desc,
            labels,

            r#type: upstream.ty.unwrap_or_default(),
            hash_on: upstream.hash_on,
            key: upstream.key,
            checks: upstream.checks,
            nodes,
            scheme: upstream.scheme.unwrap_or_default(),
            retries: upstream.retries,
            retry_timeout: upstream.retry_timeout,
            timeout: upstream.timeout,
            tls: upstream.tls,
            keepalive_pool: upstream.keepalive_pool,
            pass_host: upstream.pass_host.unwrap_or_default(),
            upstream_host: upstream.upstream_host,

            service_name: upstream.service_name,
            discovery_type: upstream.discovery_type,
            discovery_args: upstream.discovery_args.and_then(|v| v.as_object().cloned()),
        })
    }
}

impl TryFrom<typing::Ssl> for adc::SSL {
    type Error = String;

    fn try_from(ssl: typing::Ssl) -> Result<Self, Self::Error> {
        let id = ssl.id.clone();
        let cert = ssl
            .cert
            .ok_or_else(|| format!("ssl {id:?} is missing a certificate"))?;

        // APISIX never echoes a private key back on read, on any admin API
        // shape (list or single-resource GET) — confirmed against a real
        // instance, not just this fixture. A missing key here means
        // "redacted by the server", not "this SSL resource is broken", so
        // it degrades to an empty placeholder rather than failing the
        // conversion (and with it, the whole dump — one SSL resource
        // shouldn't be able to take down `dump()` for everything else).
        let mut keys = ssl.keys.unwrap_or_default().into_iter();
        let mut certificates = vec![adc::SSLCertificate {
            certificate: cert,
            key: ssl.key.unwrap_or_default(),
        }];
        if let Some(certs) = ssl.certs {
            certificates.extend(certs.into_iter().map(|certificate| adc::SSLCertificate {
                certificate,
                key: keys.next().unwrap_or_default(),
            }));
        }

        Ok(adc::SSL {
            id: Some(ssl.id),
            labels: ssl.labels,

            r#type: ssl.ty.unwrap_or_default(),
            snis: ssl.sni.map(|s| vec![s]).or(ssl.snis).unwrap_or_default(),
            certificates,
            client: ssl.client,
            ssl_protocols: ssl.ssl_protocols,
        })
    }
}

/// A credential's `type`/`config` come from its single plugin entry (APISIX
/// models a credential as a one-plugin `Plugins` map); credentials configured
/// with a plugin outside ADC's supported credential types, or with none at
/// all, don't convert — callers filter these out with `.ok()` rather than
/// treating them as fatal, matching the TS transformer silently skipping them.
impl TryFrom<typing::ConsumerCredential> for adc::ConsumerCredential {
    type Error = String;

    fn try_from(credential: typing::ConsumerCredential) -> Result<Self, Self::Error> {
        let plugins = credential
            .plugins
            .filter(|p| !p.is_empty())
            .ok_or("credential has no plugin configured")?;
        let (plugin_name, config) = plugins.into_iter().next().expect("checked non-empty above");
        if !ALLOWED_CREDENTIAL_PLUGINS.contains(&plugin_name.as_str()) {
            return Err(format!("unsupported credential plugin {plugin_name:?}"));
        }
        let Value::Object(config) = config else {
            return Err(format!(
                "credential plugin {plugin_name:?} config is not an object"
            ));
        };

        Ok(adc::ConsumerCredential {
            id: credential.id,
            name: credential.name,
            description: credential.desc,
            labels: credential.labels,
            r#type: plugin_name,
            config,
        })
    }
}

impl From<typing::Consumer> for adc::Consumer {
    fn from(consumer: typing::Consumer) -> Self {
        // Present-but-empty stays present-but-empty (matches APISIX having
        // returned a credentials array, even if none of its entries
        // converted); absent (pre-3.11 APISIX, no credentials fetched at
        // all) stays absent — the two are different facts.
        let credentials = consumer.credentials.map(|creds| {
            creds
                .into_iter()
                .filter_map(|c| adc::ConsumerCredential::try_from(c).ok())
                .collect()
        });

        adc::Consumer {
            username: consumer.username,
            description: consumer.desc,
            labels: consumer.labels,
            plugins: consumer.plugins,
            credentials,
        }
    }
}

fn extract_name_label(labels: &Option<adc::Labels>, key: &str) -> Option<String> {
    labels
        .as_ref()
        .and_then(|labels| labels.get(key))
        .and_then(|value| match value {
            LabelValue::Single(name) => Some(name.clone()),
            LabelValue::Multiple(_) => None,
        })
}

impl From<typing::StreamRoute> for adc::StreamRoute {
    fn from(route: typing::StreamRoute) -> Self {
        // APISIX's stream routes have no `name` field at all; ADC smuggles
        // one through a magic label when writing, and recovers it here when
        // reading back, falling back to `id` if it was never set that way.
        let name = extract_name_label(&route.labels, typing::ADC_NAME_LABEL)
            .unwrap_or_else(|| route.id.clone().unwrap_or_default());
        let labels = route
            .labels
            .map(|mut labels| {
                labels.remove(typing::ADC_NAME_LABEL);
                labels
            })
            .filter(|labels| !labels.is_empty());

        adc::StreamRoute {
            id: route.id,
            name,
            description: route.desc,
            labels,
            plugins: route.plugins,
            remote_addr: route.remote_addr,
            server_addr: route.server_addr,
            server_port: route.server_port.map(|port| port as u32),
            sni: route.sni,
        }
    }
}

// --- Write direction: ADC -> APISIX ---

/// A label value that's already a string is written as-is; anything else
/// (ADC labels can be string-or-array, APISIX labels are always plain
/// strings) is JSON-stringified — APISIX's admin API only ever accepts
/// plain string label values on write, regardless of how permissive a given
/// resource's read-side type is.
fn stringify_label_value(value: LabelValue) -> String {
    match value {
        LabelValue::Single(s) => s,
        LabelValue::Multiple(items) => serde_json::to_string(&items).unwrap_or_default(),
    }
}

/// For every resource whose wire `labels` field is typed `Labels` (all of
/// them except `Route` — see `typing::Route`'s doc comment): stringify each
/// value, then re-wrap as `Labels` to fit the field's declared shape.
fn transform_labels_to_apisix(labels: Option<adc::Labels>) -> Option<adc::Labels> {
    labels.map(|labels| {
        labels
            .into_iter()
            .map(|(key, value)| (key, LabelValue::Single(stringify_label_value(value))))
            .collect()
    })
}

/// `Route.labels` is the one wire field genuinely typed `Record<string,
/// string>` rather than `Labels`, so its stringified labels don't get
/// re-wrapped.
fn transform_route_labels_to_apisix(
    labels: Option<adc::Labels>,
) -> Option<HashMap<String, String>> {
    labels.map(|labels| {
        labels
            .into_iter()
            .map(|(key, value)| (key, stringify_label_value(value)))
            .collect()
    })
}

/// Derived from `adc::HttpMethod`'s own `#[serde(rename = ...)]` names
/// rather than a hand-written match, so this can't drift from
/// [`parse_http_method`]'s (the read-direction counterpart) idea of what
/// each variant's wire string is.
fn http_method_to_string(method: adc::HttpMethod) -> String {
    match serde_json::to_value(method).expect("HttpMethod serialization is infallible") {
        Value::String(s) => s,
        other => unreachable!("HttpMethod must serialize to a JSON string, got {other:?}"),
    }
}

/// Builds a route's wire body. Takes `parent_id` (the owning service's id)
/// separately since ADC's `Route` doesn't carry it — a route only knows its
/// parent by virtue of being nested under `Service.routes` in the model.
pub fn transform_route(route: adc::Route, parent_id: String) -> typing::Route {
    typing::Route {
        id: route.id.unwrap_or_default(),
        name: Some(route.name),
        desc: route.description,
        labels: transform_route_labels_to_apisix(route.labels),

        uri: None,
        uris: Some(route.uris),
        host: None,
        hosts: route.hosts,
        methods: route
            .methods
            .map(|methods| methods.into_iter().map(http_method_to_string).collect()),
        remote_addr: None,
        remote_addrs: route.remote_addrs,
        vars: route.vars,
        filter_func: route.filter_func,

        script: None,
        script_id: None,
        plugins: route.plugins,
        plugin_config_id: None,
        upstream: None,
        upstream_id: None,
        service_id: Some(parent_id),
        timeout: route.timeout,

        enable_websocket: route.enable_websocket,
        priority: route.priority,
        // Always written active; ADC's model has no notion of a disabled route.
        status: Some(1),
    }
}

impl From<adc::Upstream> for typing::Upstream {
    fn from(upstream: adc::Upstream) -> Self {
        typing::Upstream {
            // Left unset: a standalone upstream is always addressed by the
            // URL path it's PUT to, and an inlined one (under a service)
            // gets its id set separately by the caller.
            id: None,
            name: upstream.name,
            desc: upstream.description,
            labels: transform_labels_to_apisix(upstream.labels),

            nodes: upstream.nodes.map(typing::UpstreamNodes::List),
            scheme: Some(upstream.scheme),
            ty: Some(upstream.r#type),
            hash_on: upstream.hash_on,
            key: upstream.key,
            checks: upstream.checks,

            discovery_type: upstream.discovery_type,
            service_name: upstream.service_name,
            discovery_args: upstream.discovery_args.map(Value::Object),

            pass_host: Some(upstream.pass_host),
            upstream_host: upstream.upstream_host,
            retries: upstream.retries,
            retry_timeout: upstream.retry_timeout,
            timeout: upstream.timeout,
            tls: upstream.tls,
            keepalive_pool: upstream.keepalive_pool,
        }
    }
}

/// Builds a service's wire body and, if it has a default upstream, that
/// upstream's own wire body — APISIX stores a service's default upstream as
/// a *separate* `/apisix/admin/upstreams/{id}` resource sharing the
/// service's id, not embedded inline, which is why this returns two bodies
/// rather than one with a nested `upstream` field. (A service's *named*
/// upstreams, `Service.upstreams`, don't go through this at all: ADC's
/// flattened config representation surfaces each as its own top-level
/// `Upstream` resource with a `parent_id`, so they arrive at the operator as
/// independent `Event`s, handled by [`transform_route`]'s sibling for
/// upstreams — see the operator's `UPSTREAM` dispatch.)
pub fn transform_service(service: adc::Service) -> (typing::Service, Option<typing::Upstream>) {
    let id = service.id.unwrap_or_default();

    let upstream = service.upstream.map(|upstream| {
        let mut wire = typing::Upstream::from(upstream);
        wire.id = Some(id.clone());
        wire.name = Some(service.name.clone());
        wire
    });

    let wire_service = typing::Service {
        id: id.clone(),
        name: Some(service.name),
        desc: service.description,
        labels: transform_labels_to_apisix(service.labels),

        hosts: service.hosts,
        upstream: None,
        // Only reference an upstream_id when there's actually an upstream
        // resource to reference — APISIX validates this at write time and
        // rejects a service pointing at a nonexistent upstream (confirmed
        // against a real instance), which a service with no `upstream`
        // field at all would otherwise always hit.
        upstream_id: upstream.as_ref().map(|_| id.clone()),
        plugins: service.plugins,
        script: None,
        enable_websocket: None,
        upstreams: None,
    };

    (wire_service, upstream)
}

impl From<adc::Consumer> for typing::Consumer {
    fn from(consumer: adc::Consumer) -> Self {
        typing::Consumer {
            username: consumer.username,
            desc: consumer.description,
            labels: transform_labels_to_apisix(consumer.labels),
            group_id: None,
            plugins: consumer.plugins,
            // Credentials are synced as their own independent Events, not
            // nested in the consumer body.
            credentials: None,
        }
    }
}

impl From<adc::ConsumerCredential> for typing::ConsumerCredential {
    fn from(credential: adc::ConsumerCredential) -> Self {
        let mut plugins = adc::Plugins::new();
        plugins.insert(credential.r#type, Value::Object(credential.config));

        typing::ConsumerCredential {
            id: credential.id,
            name: credential.name,
            desc: credential.description,
            labels: transform_labels_to_apisix(credential.labels),
            plugins: Some(plugins),
        }
    }
}

impl From<adc::SSL> for typing::Ssl {
    fn from(ssl: adc::SSL) -> Self {
        let mut certificates = ssl.certificates.into_iter();
        let first = certificates.next().unwrap_or(adc::SSLCertificate {
            certificate: String::new(),
            key: String::new(),
        });
        let (certs, keys): (Vec<String>, Vec<String>) =
            certificates.map(|c| (c.certificate, c.key)).unzip();

        typing::Ssl {
            id: ssl.id.unwrap_or_default(),
            labels: transform_labels_to_apisix(ssl.labels),

            ty: Some(ssl.r#type),
            sni: None,
            snis: Some(ssl.snis),
            cert: Some(first.certificate),
            certs: (!certs.is_empty()).then_some(certs),
            key: Some(first.key),
            keys: (!keys.is_empty()).then_some(keys),
            client: ssl.client,
            ssl_protocols: ssl.ssl_protocols,

            status: 1,
        }
    }
}

/// Builds a stream route's wire body. `inject_name` gates whether ADC's name
/// gets smuggled through the `__ADC_NAME` label (older APISIX versions
/// don't support labels on stream routes at all, so the caller only sets
/// this once it's confirmed the target version does — APISIX >= 3.8.0).
pub fn transform_stream_route(
    route: adc::StreamRoute,
    parent_id: String,
    inject_name: bool,
) -> typing::StreamRoute {
    let mut labels = transform_labels_to_apisix(route.labels).unwrap_or_default();
    if inject_name {
        labels.insert(
            typing::ADC_NAME_LABEL.to_string(),
            LabelValue::Single(route.name),
        );
    }

    typing::StreamRoute {
        id: None,
        desc: route.description,
        labels: (!labels.is_empty()).then_some(labels),

        remote_addr: route.remote_addr,
        server_addr: route.server_addr,
        server_port: route.server_port.map(i64::from),
        sni: route.sni,
        upstream: None,
        upstream_id: None,
        service_id: Some(parent_id),

        plugins: route.plugins,
        protocol: None,
    }
}

/// Builds a consumer group's wire body, plus its member consumers' own
/// bodies (each stamped with the group's id) — mirrors the TS transformer's
/// return shape, though the operator currently only writes the group itself
/// (member consumers reach it as their own separate `Event`s already).
/// Unlike every other write-direction conversion, the id here is *derived*
/// (`generate_id(name)`), not taken from the caller — APISIX's consumer
/// group has no natural identity of its own beyond its plugin set, so ADC
/// manufactures one from the name and recovers the name back from a label
/// on read, the same trick used for stream routes.
pub fn transform_consumer_group(
    group: adc::ConsumerGroup,
) -> (typing::ConsumerGroup, Vec<typing::Consumer>) {
    let id = adc_sdk::utils::generate_id(&group.name);

    let consumers = group
        .consumers
        .unwrap_or_default()
        .into_iter()
        .map(|consumer| {
            let mut wire = typing::Consumer::from(consumer);
            wire.group_id = Some(id.clone());
            wire
        })
        .collect();

    let mut labels = transform_labels_to_apisix(group.labels).unwrap_or_default();
    labels.insert("ADC_NAME".to_string(), LabelValue::Single(group.name));

    let wire_group = typing::ConsumerGroup {
        id,
        desc: group.description,
        labels: Some(labels),
        plugins: group.plugins.unwrap_or_default(),
    };

    (wire_group, consumers)
}
