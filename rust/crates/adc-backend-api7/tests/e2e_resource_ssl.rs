//! Real end-to-end tests against a live API7 Enterprise dashboard, not a
//! mock. Requires `docker compose up -d` in `libs/backend-api7/e2e/assets`
//! — see `tests/common/mod.rs`'s module doc.
//!
//! Ignored by default; run with `cargo test -p adc-backend-api7 --test
//! e2e_resource_ssl -- --ignored --test-threads=1`.

use adc_sdk::resources::Configuration;
use adc_sdk::{EventType, ResourceType};
use semver::Version;
use serde_json::json;

mod common;
use common::{
    assert_matches_object, create_event, delete_event, dump_configuration, read_asset, server_version, sync_events,
    update_event,
};

fn config(json: serde_json::Value) -> Configuration {
    serde_json::from_value(json).unwrap()
}

#[tokio::test]
#[ignore]
async fn syncs_and_dumps_ssls() {
    let backend = common::backend().await;
    let cert1 = read_asset("certs/test-ssl1.cer").trim().to_string();
    let key1 = read_asset("certs/test-ssl1.key").trim().to_string();
    let cert2 = read_asset("certs/test-ssl2.cer").trim().to_string();
    let key2 = read_asset("certs/test-ssl2.key").trim().to_string();

    let ssl1_snis = ["ssl1-1.com", "ssl1-2.com"];
    let mut ssl1 =
        json!({ "snis": ssl1_snis, "certificates": [{ "certificate": cert1, "key": key1 }] });
    let ssl2_snis = ["ssl2-1.com", "ssl2-2.com"];
    let ssl2 =
        json!({ "snis": ssl2_snis, "certificates": [{ "certificate": cert2, "key": key2 }] });
    let ssl_name = |snis: &[&str]| snis.join(",");

    let mut ssl1_test = ssl1.clone();
    ssl1_test["certificates"][0]
        .as_object_mut()
        .unwrap()
        .remove("key");
    let mut ssl2_test = ssl2.clone();
    ssl2_test["certificates"][0]
        .as_object_mut()
        .unwrap()
        .remove("key");

    sync_events(
        &backend,
        vec![
            create_event(ResourceType::Ssl, &ssl_name(&ssl1_snis), ssl1.clone(), None),
            create_event(ResourceType::Ssl, &ssl_name(&ssl2_snis), ssl2.clone(), None),
        ],
    )
    .await
    .unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let mut ssls = dump.ssls.unwrap();
    ssls.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(ssls.len(), 2);
    assert_matches_object(&serde_json::to_value(&ssls[0]).unwrap(), &ssl2_test);
    assert_matches_object(&serde_json::to_value(&ssls[1]).unwrap(), &ssl1_test);
    // The subset matches above don't check `key` at all (it was removed
    // from the expected object) — assert directly that it comes back as
    // the empty string, not the submitted key echoed back.
    assert_eq!(ssls[0].certificates[0].key, "");
    assert_eq!(ssls[1].certificates[0].key, "");

    ssl1["labels"] = json!({ "test": "test" });
    sync_events(
        &backend,
        vec![update_event(
            ResourceType::Ssl,
            &ssl_name(&ssl1_snis),
            ssl1.clone(),
            None,
        )],
    )
    .await
    .unwrap();

    // Not sorted, unlike the dump above: the just-updated ssl1 comes back
    // first in the dashboard's own natural order.
    let dump = dump_configuration(&backend).await.unwrap();
    let ssls = dump.ssls.unwrap();
    let mut expected = ssl1.clone();
    expected["certificates"][0]
        .as_object_mut()
        .unwrap()
        .remove("key");
    assert_matches_object(&serde_json::to_value(&ssls[0]).unwrap(), &expected);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Ssl, &ssl_name(&ssl1_snis), None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    let ssls = dump.ssls.unwrap();
    assert_eq!(ssls.len(), 1);
    assert_matches_object(&serde_json::to_value(&ssls[0]).unwrap(), &ssl2_test);

    sync_events(
        &backend,
        vec![delete_event(ResourceType::Ssl, &ssl_name(&ssl2_snis), None)],
    )
    .await
    .unwrap();
    let dump = dump_configuration(&backend).await.unwrap();
    assert!(dump.ssls.is_none_or(|s| s.is_empty()));
}

/// Multi-certificate SSL writes (`cert`/`key` alongside `certs`/`keys`)
/// landed in the same dashboard release train as the fractional-timeout fix
/// in `e2e_resource_route.rs`'s `dashboard_supports_fractional_timeout` —
/// same version thresholds, a different bug:
/// 3.9.x  => x >= 3.9.19
/// 3.10.x => x >= 3.10.6
/// 3.x    => x >  3.10
/// x      => x >  3
/// Below it, a request combining both forms is rejected outright ("input
/// matches more than one oneOf schemas", confirmed against live
/// 3.9.14/3.10.1 instances).
fn dashboard_supports_multi_cert_ssl(version: &Version) -> bool {
    match (version.major, version.minor) {
        (3, 9) => version.patch >= 19,
        (3, 10) => version.patch >= 6,
        (3, minor) => minor > 10,
        (major, _) => major > 3,
    }
}

/// No app-layer version gate on the write path itself (see
/// `transformer::TryFrom<adc::SSL> for typing::Ssl`) — a dashboard too old
/// to accept a multi-cert SSL rejects the request on its own, which is the
/// intended behavior, so this test asserts against whichever of the two
/// outcomes the configured dashboard actually produces rather than
/// special-casing an app-side rejection that doesn't exist.
#[tokio::test]
#[ignore]
async fn an_ssl_with_multiple_certificates_recovers_all_of_them_on_dump() {
    let backend = common::backend().await;
    let cert1 = read_asset("certs/test-ssl1.cer").trim().to_string();
    let key1 = read_asset("certs/test-ssl1.key").trim().to_string();
    let cert2 = read_asset("certs/test-ssl2.cer").trim().to_string();
    let key2 = read_asset("certs/test-ssl2.key").trim().to_string();

    let cfg = config(json!({
        "ssls": [{
            "snis": ["multi-cert.com"],
            "certificates": [
                { "certificate": cert1, "key": key1 },
                { "certificate": cert2, "key": key2 },
            ],
        }],
    }));

    // The real differ, not a hand-built `Event` — a wrong id/kind here
    // would be a bug in the differ itself, not this test, so it's worth
    // catching before the sync call rather than only downstream in the
    // dump assertions.
    let create_events = common::diff(&cfg, &config(json!({})), None);
    assert_eq!(create_events.len(), 1, "one SSL in, one event out");
    assert_eq!(create_events[0].resource_type, ResourceType::Ssl);
    assert_eq!(create_events[0].event_type(), EventType::Create);

    let result = sync_events(&backend, create_events).await;

    if !dashboard_supports_multi_cert_ssl(&server_version()) {
        assert!(result.is_err(), "expected the dashboard to reject a multi-cert SSL below the supported version");
        return;
    }
    result.unwrap();

    let dump = dump_configuration(&backend).await.unwrap();
    let ssls = dump.ssls.unwrap();
    assert_eq!(ssls.len(), 1);
    assert_eq!(ssls[0].certificates.len(), 2, "both certificates should come back on dump");
    assert_eq!(ssls[0].certificates[0].certificate.trim(), cert1);
    assert_eq!(ssls[0].certificates[0].key, "");
    assert_eq!(ssls[0].certificates[1].certificate.trim(), cert2);
    assert_eq!(ssls[0].certificates[1].key, "");

    let delete_events = common::diff(&config(json!({})), &cfg, None);
    assert_eq!(delete_events.len(), 1);
    assert_eq!(delete_events[0].event_type(), EventType::Delete);
    sync_events(&backend, delete_events).await.unwrap();
}
