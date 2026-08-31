//! Covers SSL the same way the ported suites cover every other resource
//! type: basic correctness (create/dump round-trips), extension correctness
//! (modifiedIndex/conf_version bump on a real change), and that a create
//! then an update each produce exactly one event through the real differ.
//! Real network calls against a live 3-instance standalone APISIX cluster —
//! see `common`'s module doc for how to bring one up and run this file.

use adc_backend_apisix_standalone::Backend;
use adc_sdk::resources::{self as adc, Configuration};
use adc_sdk::Backend as _;
use adc_sdk::BackendSyncOptions;

mod common;
use common::{backend, diff, empty_configuration};

const TEST_CERT: &str = include_str!("../../../../libs/backend-apisix-standalone/e2e/assets/test-ssl.cer");
const TEST_KEY: &str = include_str!("../../../../libs/backend-apisix-standalone/e2e/assets/test-ssl.key");

async fn dump(backend: &Backend) -> Configuration {
    backend.dump().await.unwrap()
}

async fn sync_ok(backend: &Backend, events: Vec<adc_sdk::Event>) {
    let results = backend.sync(events, BackendSyncOptions::default()).await.unwrap();
    for result in &results {
        assert!(result.success, "{:?}: {:?}", result.server, result.error);
    }
}

fn ssl(snis: Vec<&str>, labels: Option<adc::Labels>) -> adc::SSL {
    adc::SSL {
        id: None,
        labels,
        r#type: adc::SslType::default(),
        snis: snis.into_iter().map(String::from).collect(),
        certificates: vec![adc::SSLCertificate { certificate: TEST_CERT.to_string(), key: TEST_KEY.to_string() }],
        client: None,
        ssl_protocols: None,
    }
}

fn config_with_ssls(ssls: Vec<adc::SSL>) -> Configuration {
    Configuration { ssls: Some(ssls), ..empty_configuration() }
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_an_ssl_certificate() {
    common::restart_apisix().await;
    let backend = backend("ssl-e2e");
    dump(&backend).await;

    // --- Create ---
    let before = dump(&backend).await;
    let local = config_with_ssls(vec![ssl(vec!["test.example.com"], None)]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1, "a brand-new ssl must diff to exactly one create event");
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let ssls = config.ssls.expect("ssl was just created");
    assert_eq!(ssls.len(), 1);
    assert_eq!(ssls[0].snis, vec!["test.example.com".to_string()]);
    assert_eq!(ssls[0].certificates[0].certificate, TEST_CERT);

    let raw = common::raw_config().await;
    let created_index = raw.ssls[0].modified_index;
    assert_eq!(raw.ssls_conf_version, created_index, "a fresh ssl's conf_version must match its own modifiedIndex");

    // --- Add a second, unrelated ssl. ---
    let before = dump(&backend).await;
    let local = config_with_ssls(vec![ssl(vec!["test.example.com"], None), ssl(vec!["other.example.com"], None)]);
    let events = diff(&local, &before);
    assert_eq!(events.len(), 1);
    sync_ok(&backend, events).await;

    let raw = common::raw_config().await;
    assert_eq!(raw.ssls.len(), 2);
    let second_index = raw.ssls.iter().find(|s| s.snis == ["other.example.com"]).unwrap().modified_index;

    // --- Update: add a label to the first ssl, keeping `snis` (and so the
    //     derived id) unchanged. `adc-differ` derives an SSL's id from its
    //     joined `snis` (there's no stable name field to hash instead) —
    //     changing `snis` itself would diff as a delete-then-create of a
    //     differently-identified ssl, not an update of this one. The
    //     second, unrelated ssl must not move at all.
    let mut labels = adc::Labels::new();
    labels.insert("env".to_string(), adc::LabelValue::Single("staging".to_string()));
    let before = dump(&backend).await;
    let updated = config_with_ssls(vec![ssl(vec!["test.example.com"], Some(labels)), ssl(vec!["other.example.com"], None)]);
    let events = diff(&updated, &before);
    assert_eq!(events.len(), 1, "changing only the first ssl's label must diff to exactly one update event, same id");
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    let ssls = config.ssls.expect("ssls still exist after update");
    assert_eq!(ssls.len(), 2, "the ssl must have been updated in place, not deleted and recreated");
    let updated_ssl = ssls.iter().find(|s| s.snis == ["test.example.com"]).unwrap();
    assert_eq!(updated_ssl.labels.as_ref().and_then(|l| l.get("env")), Some(&adc::LabelValue::Single("staging".to_string())));

    let raw = common::raw_config().await;
    let updated_index = raw.ssls.iter().find(|s| s.snis == ["test.example.com"]).unwrap().modified_index;
    assert!(updated_index > created_index, "updating the ssl must bump its modifiedIndex");
    assert_eq!(raw.ssls_conf_version, updated_index);
    let untouched_index = raw.ssls.iter().find(|s| s.snis == ["other.example.com"]).unwrap().modified_index;
    assert_eq!(untouched_index, second_index, "the unrelated second ssl's modifiedIndex must not move");

    // --- Delete both. ---
    let before = dump(&backend).await;
    let events = diff(&empty_configuration(), &before);
    assert_eq!(events.len(), 2);
    sync_ok(&backend, events).await;

    let config = dump(&backend).await;
    assert_eq!(config.ssls.map(|s| s.len()).unwrap_or(0), 0);
}
