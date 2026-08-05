//! Types shared across multiple resource definitions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A resource's labels: each value is a single string or a list of strings.
pub type Labels = HashMap<String, LabelValue>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LabelValue {
    Single(String),
    Multiple(Vec<String>),
}

/// An arbitrary, unvalidated plugin configuration object — its shape depends
/// entirely on which plugin it configures, so it's kept structurally open
/// (no `deny_unknown_fields`) rather than typed field-by-field.
pub type Plugin = serde_json::Map<String, Value>;

/// A plugin name to configuration map.
pub type Plugins = serde_json::Map<String, Value>;

/// An APISIX condition expression: an arbitrary nested array structure
/// evaluated by the gateway at request time.
pub type Expr = Vec<Value>;

/// Serializes a whole-number `f64` as a bare JSON integer (`60`, not
/// `60.0`) — some gateways' own admin APIs unmarshal a handful of
/// nominally-numeric fields (timeouts, health-check counts, upstream node
/// priority) into a Go `int`, and reject a float-formatted literal outright
/// even when it's numerically a whole number. The field itself stays `f64`
/// (ADC's own schema for these fields isn't integer-constrained — a
/// genuinely fractional value still round-trips normally through this),
/// this only changes how a whole number happens to be spelled on the wire.
pub fn serialize_whole_number_as_integer<S: serde::Serializer>(
    value: &f64,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if value.fract() == 0.0 && value.is_finite() && value.abs() <= i64::MAX as f64 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

/// [`serialize_whole_number_as_integer`] for an `Option<f64>` field paired
/// with `skip_serializing_if = "Option::is_none"` — only ever called with
/// `Some`, since serde skips the field entirely for `None` before this runs.
pub fn serialize_optional_whole_number_as_integer<S: serde::Serializer>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serialize_whole_number_as_integer(
        value.as_ref().expect("skip_serializing_if filters out None before this runs"),
        serializer,
    )
}

/// Connect/send/read timeouts in seconds, shared by upstream and route configs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeout {
    #[serde(serialize_with = "serialize_whole_number_as_integer")]
    pub connect: f64,
    #[serde(serialize_with = "serialize_whole_number_as_integer")]
    pub send: f64,
    #[serde(serialize_with = "serialize_whole_number_as_integer")]
    pub read: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_number_timeout_serializes_without_a_decimal_point() {
        let timeout = Timeout { connect: 111.0, send: 222.0, read: 333.0 };
        assert_eq!(
            serde_json::to_string(&timeout).unwrap(),
            r#"{"connect":111,"send":222,"read":333}"#
        );
    }

    #[test]
    fn a_genuinely_fractional_timeout_still_serializes_as_a_float() {
        let timeout = Timeout { connect: 1.5, send: 222.0, read: 333.0 };
        let json = serde_json::to_string(&timeout).unwrap();
        assert!(json.contains("\"connect\":1.5"), "{json}");
    }
}
