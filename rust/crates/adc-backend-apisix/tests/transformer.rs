use std::collections::HashMap;

use adc_backend_apisix::tests::transformer::{transform_consumer_group, transform_route, transform_service, transform_stream_route};
use adc_backend_apisix::tests::typing;
use adc_sdk::resources::{self as adc, LabelValue};
use serde_json::json;

fn adc_route(name: &str) -> adc::Route {
    adc::Route {
        id: None,
        name: name.to_string(),
        description: None,
        labels: None,
        hosts: None,
        uris: vec![],
        priority: None,
        timeout: None,
        vars: None,
        methods: None,
        enable_websocket: None,
        remote_addrs: None,
        plugins: None,
        filter_func: None,
    }
}

fn adc_service(name: &str) -> adc::Service {
    adc::Service {
        id: None,
        name: name.to_string(),
        description: None,
        labels: None,
        upstream: None,
        upstreams: None,
        plugins: None,
        path_prefix: None,
        strip_path_prefix: None,
        hosts: None,
        routes: None,
    }
}

fn adc_upstream() -> adc::Upstream {
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

fn adc_ssl() -> adc::SSL {
    adc::SSL { id: None, labels: None, r#type: adc::SslType::default(), snis: vec![], certificates: vec![], client: None, ssl_protocols: None }
}

fn adc_credential(name: &str, ty: &str) -> adc::ConsumerCredential {
    adc::ConsumerCredential { id: None, name: name.to_string(), description: None, labels: None, r#type: ty.to_string(), config: serde_json::Map::new() }
}

fn adc_stream_route(name: &str) -> adc::StreamRoute {
    adc::StreamRoute { id: None, name: name.to_string(), description: None, labels: None, plugins: None, remote_addr: None, server_addr: None, server_port: None, sni: None }
}

fn adc_consumer_group(name: &str) -> adc::ConsumerGroup {
    adc::ConsumerGroup { id: None, name: name.to_string(), description: None, labels: None, plugins: None, consumers: None }
}

fn route(id: &str) -> typing::Route {
    typing::Route { id: id.to_string(), ..Default::default() }
}

fn service(id: &str) -> typing::Service {
    typing::Service { id: id.to_string(), ..Default::default() }
}

fn upstream() -> typing::Upstream {
    typing::Upstream::default()
}

#[test]
fn route_falls_back_to_id_when_name_is_absent() {
    let route = route("r1");
    let adc_route: adc::Route = route.try_into().unwrap();
    assert_eq!(adc_route.name, "r1");
}

#[test]
fn route_prefers_singular_uri_over_plural() {
    let mut route = route("r1");
    route.uri = Some("/single".into());
    route.uris = Some(vec!["/a".into(), "/b".into()]);
    let adc_route: adc::Route = route.try_into().unwrap();
    assert_eq!(adc_route.uris, vec!["/single".to_string()]);
}

#[test]
fn route_with_no_uri_at_all_gets_an_empty_list_not_missing() {
    let route = route("r1");
    let adc_route: adc::Route = route.try_into().unwrap();
    assert_eq!(adc_route.uris, Vec::<String>::new());
}

#[test]
fn route_rejects_unrecognized_http_methods() {
    let mut route = route("r1");
    route.methods = Some(vec!["GET".into(), "MAGIC".into()]);
    let err = adc::Route::try_from(route).unwrap_err();
    assert!(err.contains("MAGIC"), "{err}");
}

#[test]
fn route_parses_recognized_http_methods() {
    let mut route = route("r1");
    route.methods = Some(vec!["GET".into(), "POST".into()]);
    let adc_route: adc::Route = route.try_into().unwrap();
    assert_eq!(adc_route.methods, Some(vec![adc::HttpMethod::Get, adc::HttpMethod::Post]));
}

#[test]
fn upstream_list_nodes_pass_through_unchanged() {
    let mut upstream = upstream();
    upstream.nodes =
        Some(typing::UpstreamNodes::List(vec![adc::UpstreamNode { host: "10.0.0.1".into(), port: 8080, weight: 1, priority: 0.0, metadata: None }]));
    let adc_upstream: adc::Upstream = upstream.try_into().unwrap();
    let nodes = adc_upstream.nodes.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].host, "10.0.0.1");
    assert_eq!(nodes[0].port, 8080);
}

#[test]
fn upstream_discovery_map_nodes_parse_host_and_port() {
    let mut upstream = upstream();
    upstream.nodes = Some(typing::UpstreamNodes::Map(HashMap::from([("10.0.0.1:9000".to_string(), 5)])));
    let adc_upstream: adc::Upstream = upstream.try_into().unwrap();
    let nodes = adc_upstream.nodes.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].host, "10.0.0.1");
    assert_eq!(nodes[0].port, 9000);
    assert_eq!(nodes[0].weight, 5);
}

#[test]
fn upstream_discovery_map_nodes_without_a_port_fall_back_to_scheme_default() {
    let mut upstream = upstream();
    upstream.scheme = Some(adc::UpstreamScheme::Https);
    upstream.nodes = Some(typing::UpstreamNodes::Map(HashMap::from([("10.0.0.1".to_string(), 1)])));
    let adc_upstream: adc::Upstream = upstream.try_into().unwrap();
    assert_eq!(adc_upstream.nodes.unwrap()[0].port, 443);
}

#[test]
fn upstream_strips_the_service_association_label_but_keeps_others() {
    let mut upstream = upstream();
    upstream.labels = Some(HashMap::from([
        (typing::ADC_UPSTREAM_SERVICE_ID_LABEL.to_string(), LabelValue::Single("svc1".into())),
        ("env".to_string(), LabelValue::Single("prod".into())),
    ]));
    let adc_upstream: adc::Upstream = upstream.try_into().unwrap();
    let labels = adc_upstream.labels.unwrap();
    assert!(!labels.contains_key(typing::ADC_UPSTREAM_SERVICE_ID_LABEL));
    assert_eq!(labels.get("env"), Some(&LabelValue::Single("prod".into())));
}

#[test]
fn upstream_with_only_the_service_association_label_ends_up_with_no_labels() {
    let mut upstream = upstream();
    upstream.labels = Some(HashMap::from([(
        typing::ADC_UPSTREAM_SERVICE_ID_LABEL.to_string(),
        LabelValue::Single("svc1".into()),
    )]));
    let adc_upstream: adc::Upstream = upstream.try_into().unwrap();
    assert!(adc_upstream.labels.is_none());
}

#[test]
fn service_falls_back_to_id_when_name_is_absent_and_converts_its_upstream() {
    let mut service = service("svc1");
    let mut inline_upstream = upstream();
    inline_upstream.scheme = Some(adc::UpstreamScheme::Http);
    service.upstream = Some(inline_upstream);

    let adc_service: adc::Service = service.try_into().unwrap();
    assert_eq!(adc_service.name, "svc1");
    assert!(adc_service.upstream.is_some());
    assert!(adc_service.routes.is_none());
}

#[test]
fn ssl_pairs_the_primary_and_additional_certificates() {
    let ssl = typing::Ssl {
        id: "ssl1".into(),
        labels: None,
        ty: None,
        sni: Some("a.example.com".into()),
        snis: None,
        cert: Some("CERT_A".into()),
        certs: Some(vec!["CERT_B".into()]),
        key: Some("KEY_A".into()),
        keys: Some(vec!["KEY_B".into()]),
        client: None,
        ssl_protocols: None,
        status: 1,
    };
    let adc_ssl: adc::SSL = ssl.try_into().unwrap();
    assert_eq!(adc_ssl.snis, vec!["a.example.com".to_string()]);
    assert_eq!(
        adc_ssl.certificates,
        vec![
            adc::SSLCertificate { certificate: "CERT_A".into(), key: "KEY_A".into() },
            adc::SSLCertificate { certificate: "CERT_B".into(), key: "KEY_B".into() },
        ]
    );
}

#[test]
fn ssl_missing_a_certificate_is_rejected() {
    let ssl = typing::Ssl {
        id: "ssl1".into(),
        labels: None,
        ty: None,
        sni: None,
        snis: None,
        cert: None,
        certs: None,
        key: Some("KEY_A".into()),
        keys: None,
        client: None,
        ssl_protocols: None,
        status: 1,
    };
    let err = adc::SSL::try_from(ssl).unwrap_err();
    assert!(err.contains("ssl1"), "{err}");
}

#[test]
fn ssl_with_a_redacted_key_degrades_to_an_empty_placeholder_instead_of_failing() {
    // Apisix never echoes a private key back on any read (list or
    // single-resource GET), confirmed against a real instance — a missing
    // `key` here means "redacted by the server", not "broken resource".
    let ssl = typing::Ssl {
        id: "ssl1".into(),
        labels: None,
        ty: None,
        sni: None,
        snis: Some(vec!["example.com".into()]),
        cert: Some("CERT_A".into()),
        certs: None,
        key: None,
        keys: None,
        client: None,
        ssl_protocols: None,
        status: 1,
    };
    let adc_ssl: adc::SSL = ssl.try_into().unwrap();
    assert_eq!(adc_ssl.certificates, vec![adc::SSLCertificate { certificate: "CERT_A".into(), key: String::new() }]);
}

#[test]
fn consumer_credential_only_converts_recognized_plugins() {
    let mut plugins = adc::Plugins::new();
    plugins.insert("key-auth".into(), json!({ "key": "secret" }));
    let credential = typing::ConsumerCredential { id: Some("c1".into()), name: "c1".into(), desc: None, labels: None, plugins: Some(plugins) };

    let adc_credential: adc::ConsumerCredential = credential.try_into().unwrap();
    assert_eq!(adc_credential.r#type, "key-auth");
    assert_eq!(adc_credential.config.get("key"), Some(&json!("secret")));
}

#[test]
fn consumer_credential_rejects_unsupported_plugins() {
    let mut plugins = adc::Plugins::new();
    plugins.insert("proxy-rewrite".into(), json!({}));
    let credential = typing::ConsumerCredential { id: None, name: "c1".into(), desc: None, labels: None, plugins: Some(plugins) };

    assert!(adc::ConsumerCredential::try_from(credential).is_err());
}

#[test]
fn consumer_drops_credentials_that_fail_to_convert_but_keeps_the_rest() {
    let mut good = adc::Plugins::new();
    good.insert("key-auth".into(), json!({}));
    let mut bad = adc::Plugins::new();
    bad.insert("proxy-rewrite".into(), json!({}));

    let consumer = typing::Consumer {
        username: "alice".into(),
        desc: None,
        labels: None,
        group_id: None,
        plugins: None,
        credentials: Some(vec![
            typing::ConsumerCredential { id: Some("good".into()), name: "good".into(), desc: None, labels: None, plugins: Some(good) },
            typing::ConsumerCredential { id: Some("bad".into()), name: "bad".into(), desc: None, labels: None, plugins: Some(bad) },
        ]),
    };

    let adc_consumer: adc::Consumer = consumer.into();
    let credentials = adc_consumer.credentials.unwrap();
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].id.as_deref(), Some("good"));
}

#[test]
fn consumer_with_credentials_never_fetched_stays_none_not_empty() {
    let consumer = typing::Consumer { username: "alice".into(), desc: None, labels: None, group_id: None, plugins: None, credentials: None };
    let adc_consumer: adc::Consumer = consumer.into();
    assert!(adc_consumer.credentials.is_none());
}

#[test]
fn stream_route_recovers_its_name_from_the_magic_label_and_strips_it() {
    let route = typing::StreamRoute {
        id: Some("sr1".into()),
        desc: None,
        labels: Some(HashMap::from([
            ("__ADC_NAME".to_string(), LabelValue::Single("my-stream-route".into())),
            ("env".to_string(), LabelValue::Single("prod".into())),
        ])),
        remote_addr: None,
        server_addr: None,
        server_port: Some(9000),
        sni: None,
        upstream: None,
        upstream_id: None,
        service_id: None,
        plugins: None,
        protocol: None,
    };

    let adc_route: adc::StreamRoute = route.into();
    assert_eq!(adc_route.name, "my-stream-route");
    let labels = adc_route.labels.unwrap();
    assert!(!labels.contains_key("__ADC_NAME"));
    assert_eq!(labels.get("env"), Some(&LabelValue::Single("prod".into())));
}

#[test]
fn stream_route_without_the_magic_label_falls_back_to_id() {
    let route = typing::StreamRoute {
        id: Some("sr1".into()),
        desc: None,
        labels: None,
        remote_addr: None,
        server_addr: None,
        server_port: None,
        sni: None,
        upstream: None,
        upstream_id: None,
        service_id: None,
        plugins: None,
        protocol: None,
    };
    let adc_route: adc::StreamRoute = route.into();
    assert_eq!(adc_route.name, "sr1");
}

#[test]
fn write_route_carries_parent_id_and_stringifies_methods() {
    let mut route = adc_route("r1");
    route.uris = vec!["/foo".into()];
    route.methods = Some(vec![adc::HttpMethod::Get, adc::HttpMethod::Post]);

    let wire = transform_route(route, "svc1".into());
    assert_eq!(wire.service_id.as_deref(), Some("svc1"));
    assert_eq!(wire.uris, Some(vec!["/foo".to_string()]));
    assert_eq!(wire.methods, Some(vec!["GET".to_string(), "POST".to_string()]));
    assert_eq!(wire.status, Some(1));
}

#[test]
fn write_route_labels_are_plain_strings_arrays_get_json_stringified() {
    let mut route = adc_route("r1");
    route.labels = Some(HashMap::from([
        ("env".to_string(), LabelValue::Single("prod".into())),
        ("team".to_string(), LabelValue::Multiple(vec!["a".into(), "b".into()])),
    ]));

    let wire = transform_route(route, "svc1".into());
    let labels = wire.labels.unwrap();
    assert_eq!(labels.get("env"), Some(&"prod".to_string()));
    assert_eq!(labels.get("team"), Some(&"[\"a\",\"b\"]".to_string()));
}

#[test]
fn write_service_splits_into_service_and_matching_upstream() {
    let mut service = adc_service("svc1");
    service.id = Some("svc1".into());
    service.upstream = Some(adc_upstream());

    let (wire_service, wire_upstream) = transform_service(service);
    assert!(wire_service.upstream.is_none(), "upstream must not be inlined into the service body");
    assert_eq!(wire_service.upstream_id.as_deref(), Some("svc1"));

    let wire_upstream = wire_upstream.expect("service had a default upstream");
    assert_eq!(wire_upstream.id.as_deref(), Some("svc1"));
    assert_eq!(wire_upstream.name.as_deref(), Some("svc1"));
}

#[test]
fn write_service_without_a_default_upstream_returns_none() {
    let service = adc_service("svc1");
    let (_wire_service, wire_upstream) = transform_service(service);
    assert!(wire_upstream.is_none());
}

#[test]
fn write_upstream_never_carries_its_own_id() {
    let upstream = adc_upstream();
    let wire: typing::Upstream = upstream.into();
    assert!(wire.id.is_none());
}

#[test]
fn write_ssl_splits_certificates_into_primary_and_additional() {
    let mut ssl = adc_ssl();
    ssl.certificates = vec![
        adc::SSLCertificate { certificate: "CERT_A".into(), key: "KEY_A".into() },
        adc::SSLCertificate { certificate: "CERT_B".into(), key: "KEY_B".into() },
    ];

    let wire: typing::Ssl = ssl.into();
    assert_eq!(wire.cert.as_deref(), Some("CERT_A"));
    assert_eq!(wire.key.as_deref(), Some("KEY_A"));
    assert_eq!(wire.certs, Some(vec!["CERT_B".to_string()]));
    assert_eq!(wire.keys, Some(vec!["KEY_B".to_string()]));
    assert_eq!(wire.status, 1);
}

#[test]
fn write_ssl_with_a_single_certificate_omits_certs_and_keys() {
    let mut ssl = adc_ssl();
    ssl.certificates = vec![adc::SSLCertificate { certificate: "CERT_A".into(), key: "KEY_A".into() }];

    let wire: typing::Ssl = ssl.into();
    assert!(wire.certs.is_none());
    assert!(wire.keys.is_none());
}

#[test]
fn write_consumer_credential_wraps_type_and_config_into_a_plugins_map() {
    let mut credential = adc_credential("c1", "key-auth");
    credential.config.insert("key".into(), json!("secret"));

    let wire: typing::ConsumerCredential = credential.into();
    let plugins = wire.plugins.unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins.get("key-auth").and_then(|v| v.get("key")), Some(&json!("secret")));
}

#[test]
fn write_stream_route_injects_the_name_label_when_requested() {
    let route = adc_stream_route("my-stream-route");
    let wire = transform_stream_route(route, "svc1".into(), true);
    let labels = wire.labels.unwrap();
    assert_eq!(labels.get("__ADC_NAME"), Some(&LabelValue::Single("my-stream-route".to_string())));
    assert_eq!(wire.service_id.as_deref(), Some("svc1"));
}

#[test]
fn write_stream_route_omits_the_name_label_when_not_requested() {
    let route = adc_stream_route("my-stream-route");
    let wire = transform_stream_route(route, "svc1".into(), false);
    assert!(wire.labels.is_none());
}

#[test]
fn write_consumer_group_derives_its_id_from_the_name_and_injects_adc_name() {
    let group = adc_consumer_group("my-group");
    let (wire, _consumers) = transform_consumer_group(group);
    assert_eq!(wire.id, adc_sdk::utils::generate_id("my-group"));
    assert_eq!(wire.labels.unwrap().get("ADC_NAME"), Some(&LabelValue::Single("my-group".to_string())));
}

#[test]
fn write_consumer_group_stamps_its_id_onto_member_consumers() {
    let mut group = adc_consumer_group("my-group");
    group.consumers = Some(vec![adc::Consumer { username: "alice".into(), description: None, labels: None, plugins: None, credentials: None }]);

    let (wire_group, consumers) = transform_consumer_group(group);
    assert_eq!(consumers.len(), 1);
    assert_eq!(consumers[0].group_id, Some(wire_group.id));
}
