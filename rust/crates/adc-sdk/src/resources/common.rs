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
    // `i64::MAX as f64` itself rounds up to `2f64.powi(63)` (an `f64` can't
    // represent `i64::MAX` exactly), so comparing against it as an upper
    // bound would let a value at or beyond `i64`'s actual range through —
    // `as i64` on that saturates to `i64::MAX` instead of preserving the
    // real value, silently corrupting it. Comparing against `2f64.powi(63)`
    // directly (exclusive) is exact.
    if value.fract() == 0.0 && value.is_finite() && value.abs() < 2f64.powi(63) {
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

    #[derive(Serialize)]
    struct WholeNumber(#[serde(serialize_with = "serialize_whole_number_as_integer")] f64);

    #[test]
    fn a_large_but_in_range_whole_number_still_serializes_as_an_integer() {
        // `2^62` is exactly representable as both `f64` and `i64`, and well
        // clear of the boundary this function has to guard.
        let value = 2f64.powi(62);
        let json = serde_json::to_string(&WholeNumber(value)).unwrap();
        assert_eq!(json, (2i64.pow(62)).to_string());
    }

    #[test]
    fn a_value_at_two_to_the_63_falls_back_to_a_float_instead_of_an_incorrect_integer() {
        // `2^63` is exactly `i64::MAX + 1` — out of `i64`'s range. Naively
        // comparing against `i64::MAX as f64` (itself rounded up to `2^63`,
        // since `i64::MAX` isn't exactly representable as an `f64`) would
        // let this through and `as i64` would silently saturate it down to
        // `i64::MAX` — a wrong value. It must fall back to `f64` instead.
        let value = 2f64.powi(63);
        let json = serde_json::to_string(&WholeNumber(value)).unwrap();
        assert_ne!(json, i64::MAX.to_string(), "{json}");
        assert_eq!(json.parse::<f64>().unwrap(), value);
    }
}
