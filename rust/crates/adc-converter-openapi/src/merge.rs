//! Services/routes/upstreams/nodes are built by overlaying `x-adc-*-defaults`
//! blobs (and merged plugin maps) on top of an object under construction —
//! deliberately a *shallow* merge, not a recursive one: a defaults key
//! replaces whatever `target` already had for that key wholesale, even if
//! both sides are themselves objects, rather than merging their fields.

use serde_json::{Map, Value};

pub fn shallow_merge(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        target.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn overlay_keys_win_and_nested_objects_are_replaced_wholesale() {
        let mut target = json!({"a": 1, "nested": {"x": 1, "y": 2}}).as_object().unwrap().clone();
        let overlay = json!({"nested": {"y": 3}, "b": 2}).as_object().unwrap().clone();
        shallow_merge(&mut target, &overlay);
        assert_eq!(Value::Object(target), json!({"a": 1, "nested": {"y": 3}, "b": 2}));
    }
}
