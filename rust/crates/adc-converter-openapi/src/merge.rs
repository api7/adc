//! `libs/converter-openapi` builds services/routes/upstreams by spreading
//! plain JS objects (`{...a, ...b}`) — a *shallow* merge, not a recursive
//! one. This is the Rust equivalent: insert every top-level key of
//! `overlay` into `target`, overwriting whatever `target` already had.

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
