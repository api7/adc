//! Converting between API7's wire shapes (`crate::typing`) and ADC's
//! resource model (`adc_sdk::resources`).
//!
//! Read direction (API7 -> ADC, used by the fetcher): `TryFrom`/`From` on
//! the ADC type, so a caller can write either `adc::Route::try_from(route)`
//! or `route.try_into()`. Write direction (ADC -> API7, used by the
//! operator/validator): plain `From` on the wire type, since nothing here
//! can fail the way parsing a live server's response can — except `SSL`,
//! whose `certificates` list can be empty (nothing stops a locally-authored
//! config from omitting it, and the read direction deliberately tolerates
//! that on the way *in*), but there's no wire representation of "no
//! certificate at all" to send back *out*: the server would just reject an
//! empty `cert`/`key` string outright. `TryFrom` rejects that case here,
//! before a doomed request is ever built.
//! `Route`/`StreamRoute` are free functions rather than `From` impls: each
//! needs its parent service's id alongside the resource itself, and `From`
//! only takes one argument. `Service`/`Route`/`StreamRoute` also each have a
//! write-direction-only id field (`service_id`/`route_id`/`stream_route_id`)
//! distinct from the `id` their read-direction conversion produces.
//!
//! `typing::Service`'s `From` impl always converts a service's embedded
//! default upstream through [`typing::Upstream`]'s own `From` impl, exactly
//! like a standalone named upstream — never assembled directly from the ADC
//! shape's own fields, since those (`description`, `type` for the
//! balancer) don't line up with what the wire format expects (`desc`,
//! `type`), and passing them through unrenamed would silently drop the
//! description.

use std::collections::HashMap;

use adc_sdk::resources::{self as adc, LabelValue};
use serde_json::Value;

use crate::typing;

fn parse_http_method(method: String) -> Result<adc::HttpMethod, String> {
    serde_json::from_value(Value::String(method.clone()))
        .map_err(|_| format!("unrecognized HTTP method {method:?}"))
}

fn http_method_to_string(method: adc::HttpMethod) -> String {
    match serde_json::to_value(method).expect("HttpMethod serialization is infallible") {
        Value::String(s) => s,
        other => unreachable!("HttpMethod must serialize to a JSON string, got {other:?}"),
    }
}

// --- Labels: every API7 resource's wire `labels` is a plain string map,
// unlike ADC's own string-or-array `Labels` — a multi-value label
// round-trips through a JSON-array-shaped string rather than a nested JSON
// array.

/// A value that's valid JSON *and* decodes to a string array round-trips
/// back to `LabelValue::Multiple`; anything else (not JSON, not an array,
/// an array with non-string elements) stays a plain string.
fn transform_labels_from_wire(labels: Option<HashMap<String, String>>) -> Option<adc::Labels> {
    labels.map(|labels| {
        labels
            .into_iter()
            .map(|(key, value)| {
                let label_value = serde_json::from_str::<Vec<String>>(&value)
                    .map(LabelValue::Multiple)
                    .unwrap_or(LabelValue::Single(value));
                (key, label_value)
            })
            .collect()
    })
}

fn stringify_label_value(value: LabelValue) -> String {
    match value {
        LabelValue::Single(s) => s,
        LabelValue::Multiple(items) => serde_json::to_string(&items).unwrap_or_default(),
    }
}

fn transform_labels_to_wire(labels: Option<adc::Labels>) -> Option<HashMap<String, String>> {
    labels.map(|labels| {
        labels
            .into_iter()
            .map(|(key, value)| (key, stringify_label_value(value)))
            .collect()
    })
}

// --- Read direction: API7 -> ADC ---

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
        let id = route.id;

        Ok(adc::Route {
            name: route.name.unwrap_or_else(|| id.clone().unwrap_or_default()),
            id,
            description: route.desc,
            labels: transform_labels_from_wire(route.labels),

            hosts: None,
            uris: route.paths.unwrap_or_default(),
            priority: route.priority,
            timeout: route.timeout,
            vars: route.vars,
            methods,
            enable_websocket: route.enable_websocket,
            remote_addrs: None,
            plugins: route.plugins,
            filter_func: None,
        })
    }
}

impl From<typing::Upstream> for adc::Upstream {
    fn from(upstream: typing::Upstream) -> Self {
        adc::Upstream {
            id: upstream.id,
            name: upstream.name,
            description: upstream.desc,
            labels: transform_labels_from_wire(upstream.labels),

            r#type: upstream.ty.unwrap_or_default(),
            hash_on: upstream.hash_on,
            key: upstream.key,
            checks: upstream.checks,
            nodes: upstream.nodes,
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
        }
    }
}

impl TryFrom<typing::Service> for adc::Service {
    type Error = String;

    fn try_from(service: typing::Service) -> Result<Self, Self::Error> {
        let id = service.id;
        let upstream = service.upstream.map(adc::Upstream::from);
        let upstreams = service.upstreams.map(|list| {
            list.into_iter()
                // Ignore the default upstream if the named-upstreams
                // collection happens to echo it back too.
                .filter(|u| u.id != id)
                .map(adc::Upstream::from)
                .collect()
        });

        Ok(adc::Service {
            name: service
                .name
                .unwrap_or_else(|| id.clone().unwrap_or_default()),
            id,
            description: service.desc,
            labels: transform_labels_from_wire(service.labels),

            upstream,
            upstreams,
            plugins: service.plugins,
            // Not an API7 wire concept on read: these only exist on
            // ADC-authored config.
            path_prefix: service.path_prefix,
            strip_path_prefix: service.strip_path_prefix,
            hosts: service.hosts,

            // Attached later, once route/stream_route fetch results are
            // available to nest under their parent service.
            routes: None,
        })
    }
}

impl From<typing::Ssl> for adc::SSL {
    fn from(ssl: typing::Ssl) -> Self {
        // Only the first certificate/key pair is ever read back, and the
        // key is always empty — a gateway server never echoes a private
        // key on read, and additional entries in `certs`/`keys` aren't
        // recovered here either (unlike `adc_backend_apisix`'s own SSL
        // read conversion, which does merge them back in). A missing
        // certificate isn't an error — it just means no certificate to
        // report, so `certificates` comes back empty instead.
        let certificates = ssl
            .cert
            .map(|certificate| {
                vec![adc::SSLCertificate {
                    certificate,
                    key: String::new(),
                }]
            })
            .unwrap_or_default();

        adc::SSL {
            id: ssl.id,
            labels: transform_labels_from_wire(ssl.labels),

            r#type: ssl.ty.unwrap_or_default(),
            snis: ssl.snis.unwrap_or_default(),
            certificates,
            client: ssl.client,
            ssl_protocols: None,
        }
    }
}

/// A credential's `type`/`config` come from its single plugin entry (API7
/// models a credential as a one-plugin `Plugins` map, same as APISIX).
/// Unlike `adc_backend_apisix`'s transformer, there's no allow-list of
/// recognized credential plugin names here — whatever single plugin entry
/// is present is accepted as-is.
impl TryFrom<typing::ConsumerCredential> for adc::ConsumerCredential {
    type Error = String;

    fn try_from(credential: typing::ConsumerCredential) -> Result<Self, Self::Error> {
        let plugins = credential
            .plugins
            .filter(|p| !p.is_empty())
            .ok_or("credential has no plugin configured")?;
        let (plugin_name, config) = plugins.into_iter().next().expect("checked non-empty above");
        let Value::Object(config) = config else {
            return Err(format!(
                "credential plugin {plugin_name:?} config is not an object"
            ));
        };

        Ok(adc::ConsumerCredential {
            id: credential.id,
            name: credential.name.unwrap_or_default(),
            description: credential.desc,
            labels: transform_labels_from_wire(credential.labels),
            r#type: plugin_name,
            config,
        })
    }
}

impl From<typing::Consumer> for adc::Consumer {
    fn from(consumer: typing::Consumer) -> Self {
        // Present-but-empty stays present-but-empty; absent stays absent —
        // matches `adc_backend_apisix::transformer`'s reasoning.
        let credentials = consumer.credentials.map(|creds| {
            creds
                .into_iter()
                .filter_map(|c| adc::ConsumerCredential::try_from(c).ok())
                .collect()
        });

        adc::Consumer {
            username: consumer.username,
            description: consumer.desc,
            labels: transform_labels_from_wire(consumer.labels),
            plugins: consumer.plugins,
            credentials,
        }
    }
}

impl From<typing::StreamRoute> for adc::StreamRoute {
    fn from(route: typing::StreamRoute) -> Self {
        let id = route.id;
        adc::StreamRoute {
            name: route.name.unwrap_or_else(|| id.clone().unwrap_or_default()),
            id,
            description: route.desc,
            labels: transform_labels_from_wire(route.labels),
            plugins: route.plugins,
            remote_addr: route.remote_addr,
            server_addr: route.server_addr,
            server_port: route.server_port,
            sni: None,
        }
    }
}

// --- Write direction: ADC -> API7 ---

pub fn transform_route(route: adc::Route, parent_id: String) -> typing::Route {
    typing::Route {
        id: None,
        route_id: route.id,
        name: Some(route.name),
        desc: route.description,
        labels: transform_labels_to_wire(route.labels),
        service_id: Some(parent_id),

        plugins: route.plugins,

        paths: Some(route.uris),
        methods: route
            .methods
            .map(|methods| methods.into_iter().map(http_method_to_string).collect()),
        vars: route.vars,

        enable_websocket: route.enable_websocket,
        priority: route.priority,
        timeout: route.timeout,
    }
}

pub fn transform_stream_route(route: adc::StreamRoute, parent_id: String) -> typing::StreamRoute {
    typing::StreamRoute {
        id: None,
        stream_route_id: route.id,
        name: Some(route.name),
        desc: route.description,
        labels: transform_labels_to_wire(route.labels),
        service_id: Some(parent_id),

        plugins: route.plugins,

        server_addr: route.server_addr,
        server_port: route.server_port,
        remote_addr: route.remote_addr,
    }
}

impl From<adc::Upstream> for typing::Upstream {
    fn from(upstream: adc::Upstream) -> Self {
        typing::Upstream {
            id: upstream.id,
            name: upstream.name,
            desc: upstream.description,
            labels: transform_labels_to_wire(upstream.labels),

            nodes: upstream.nodes,
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

impl From<adc::Service> for typing::Service {
    /// Builds a service's wire body, including its embedded default
    /// upstream (see the module doc comment for why this always goes
    /// through `typing::Upstream`'s `From` impl rather than being assembled
    /// directly). `type` is derived from the upstream's scheme: a
    /// `tcp`/`udp`/`tls` upstream makes the service a `stream` service,
    /// anything else `http`.
    fn from(service: adc::Service) -> Self {
        let ty = match service.upstream.as_ref().map(|u| u.scheme) {
            Some(adc::UpstreamScheme::Tcp | adc::UpstreamScheme::Udp | adc::UpstreamScheme::Tls) => {
                "stream"
            }
            _ => "http",
        };

        typing::Service {
            id: None,
            service_id: service.id,
            name: Some(service.name),
            desc: service.description,
            labels: transform_labels_to_wire(service.labels),
            ty: Some(ty.to_string()),

            hosts: service.hosts,
            upstream: service.upstream.map(typing::Upstream::from),
            plugins: service.plugins,
            path_prefix: service.path_prefix,
            strip_path_prefix: service.strip_path_prefix,

            routes: None,
            stream_routes: None,
            upstreams: None,
        }
    }
}

impl From<adc::Consumer> for typing::Consumer {
    fn from(consumer: adc::Consumer) -> Self {
        typing::Consumer {
            username: consumer.username,
            desc: consumer.description,
            labels: transform_labels_to_wire(consumer.labels),
            plugins: consumer.plugins,
            // Credentials sync as their own independent events, not nested
            // in the consumer body.
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
            name: Some(credential.name),
            desc: credential.description,
            labels: transform_labels_to_wire(credential.labels),
            plugins: Some(plugins),
        }
    }
}

impl TryFrom<adc::SSL> for typing::Ssl {
    type Error = String;

    fn try_from(ssl: adc::SSL) -> Result<Self, Self::Error> {
        let mut certificates = ssl.certificates.into_iter();
        let Some(first) = certificates.next() else {
            return Err(format!(
                "SSL {:?} has no certificates to write",
                ssl.id.as_deref().unwrap_or("<unknown>")
            ));
        };
        let (certs, keys): (Vec<String>, Vec<String>) =
            certificates.map(|c| (c.certificate, c.key)).unzip();

        Ok(typing::Ssl {
            id: ssl.id,
            labels: transform_labels_to_wire(ssl.labels),

            ty: Some(ssl.r#type),
            cert: Some(first.certificate),
            certs: (!certs.is_empty()).then_some(certs),
            key: Some(first.key),
            keys: (!keys.is_empty()).then_some(keys),
            client: ssl.client,
            snis: Some(ssl.snis),

            status: Some(1),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ip_restriction_plugins() -> adc::Plugins {
        json!({ "ip-restriction": { "blacklist": ["0.0.0.0/0"] } })
            .as_object()
            .unwrap()
            .clone()
    }

    /// Regression: the write-direction stream route conversion used to
    /// drop `plugins`, so a synced stream route's plugins never actually
    /// reached the wire.
    #[test]
    fn transform_stream_route_writes_plugins() {
        let route = adc::StreamRoute {
            id: Some("sr1".to_string()),
            name: "sr1".to_string(),
            description: Some("desc".to_string()),
            labels: None,
            plugins: Some(ip_restriction_plugins()),
            remote_addr: None,
            server_addr: None,
            server_port: None,
            sni: None,
        };

        let wire = transform_stream_route(route, "svc1".to_string());

        assert_eq!(wire.plugins, Some(ip_restriction_plugins()));
    }

    /// Regression: the read-direction stream route conversion used to drop
    /// `plugins`, so dumping a stream route always came back without them —
    /// the differ could then never detect a plugin removal (local empty ===
    /// remote empty), leaving stale plugins on the gateway.
    #[test]
    fn stream_route_from_wire_preserves_plugins_on_dump() {
        let wire = typing::StreamRoute {
            id: Some("sr1".to_string()),
            stream_route_id: Some("sr1".to_string()),
            name: Some("sr1".to_string()),
            desc: Some("desc".to_string()),
            labels: None,
            service_id: Some("svc1".to_string()),
            plugins: Some(ip_restriction_plugins()),
            server_addr: Some("1.1.1.1".to_string()),
            server_port: Some(80),
            remote_addr: None,
        };

        let route = adc::StreamRoute::from(wire);

        assert_eq!(route.plugins, Some(ip_restriction_plugins()));
    }

    /// `server_port` is `u16` at the wire level too (not a wider int
    /// narrowed later): a port outside 0-65535 is never a real port, so a
    /// server response containing one is rejected right at deserialization
    /// instead of being silently dropped downstream.
    #[test]
    fn an_out_of_range_wire_port_is_rejected_at_deserialization() {
        let json = serde_json::json!({ "server_port": 70_000 });
        assert!(serde_json::from_value::<typing::StreamRoute>(json).is_err());
    }
}
