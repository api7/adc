//! Self-researched replacement for the TS side's third-party `datum-diff` (a
//! `deep-diff` fork) dependency — see `libs/differ/src/differv4.ts`'s use of
//! `objectDiff` from `datum-diff`.
//!
//! `datum-diff`'s `diff(lhs, rhs)` is a plain recursive structural diff over
//! JS values: it walks both sides depth-first and emits one entry per changed
//! leaf/key/array-slot. It is *not* content-addressed or order-independent —
//! object keys are compared in `lhs`-then-`rhs` insertion order, and arrays
//! are compared positionally (with any length difference collapsed onto the
//! tail). ADC configs are always plain trees from deserialized JSON/YAML
//! (no shared/cyclic references), so the reference-cycle bookkeeping
//! `datum-diff` does (its `stack` parameter) is intentionally not ported —
//! it can never trigger here.
//!
//! `serde_json`'s `preserve_order` feature (enabled workspace-wide) is
//! required for `Value::Object`'s key order to match the source document,
//! which is what makes `path` order below match the TS/JS output byte-for-byte.

use serde::Serialize;
use serde_json::Value;

/// One path segment: an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

pub type DiffPath = Vec<PathSegment>;

/// Mirrors `datum-diff`'s `Diff<LHS, RHS>` union (kinds `N`/`D`/`E`/`A`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum ValueDiff {
    /// New: present in `rhs`, absent in `lhs`.
    #[serde(rename = "N")]
    New { path: DiffPath, rhs: Value },
    /// Deleted: present in `lhs`, absent in `rhs`.
    #[serde(rename = "D")]
    Deleted { path: DiffPath, lhs: Value },
    /// Edit: present (with a different value or type) on both sides.
    #[serde(rename = "E")]
    Edit { path: DiffPath, lhs: Value, rhs: Value },
    /// Array: a tail element was added/removed relative to the other side's length.
    #[serde(rename = "A")]
    Array {
        path: DiffPath,
        index: usize,
        item: Box<ValueDiff>,
    },
}

fn real_type_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Structural diff of two JSON values, mirroring `datum-diff`'s `diff(lhs, rhs)`.
/// Returns `None` when there is no difference (matching `datum-diff`'s
/// `changes.length ? changes : undefined` behavior, which callers rely on).
pub fn diff_value(lhs: &Value, rhs: &Value) -> Option<Vec<ValueDiff>> {
    let mut changes = Vec::new();
    deep_diff(Some(lhs), Some(rhs), &[], &mut changes);
    if changes.is_empty() { None } else { Some(changes) }
}

fn deep_diff(lhs: Option<&Value>, rhs: Option<&Value>, path: &[PathSegment], changes: &mut Vec<ValueDiff>) {
    match (lhs, rhs) {
        (None, Some(r)) => changes.push(ValueDiff::New { path: path.to_vec(), rhs: r.clone() }),
        (Some(l), None) => changes.push(ValueDiff::Deleted { path: path.to_vec(), lhs: l.clone() }),
        (None, None) => {}
        (Some(l), Some(r)) => {
            if real_type_of(l) != real_type_of(r) {
                changes.push(ValueDiff::Edit { path: path.to_vec(), lhs: l.clone(), rhs: r.clone() });
                return;
            }
            match (l, r) {
                (Value::Array(la), Value::Array(ra)) => diff_array(la, ra, path, changes),
                (Value::Object(lo), Value::Object(ro)) => diff_object(lo, ro, path, changes),
                _ => {
                    if l != r {
                        changes.push(ValueDiff::Edit { path: path.to_vec(), lhs: l.clone(), rhs: r.clone() });
                    }
                }
            }
        }
    }
}

fn diff_object(lo: &serde_json::Map<String, Value>, ro: &serde_json::Map<String, Value>, path: &[PathSegment], changes: &mut Vec<ValueDiff>) {
    // lhs's own keys first, in insertion order (matches `Object.keys(lObj)`).
    for (key, lv) in lo {
        let mut sub_path = path.to_vec();
        sub_path.push(PathSegment::Key(key.clone()));
        deep_diff(Some(lv), ro.get(key), &sub_path, changes);
    }
    // then rhs-only keys, in rhs's insertion order.
    for (key, rv) in ro {
        if lo.contains_key(key) {
            continue;
        }
        let mut sub_path = path.to_vec();
        sub_path.push(PathSegment::Key(key.clone()));
        deep_diff(None, Some(rv), &sub_path, changes);
    }
}

fn diff_array(la: &[Value], ra: &[Value], path: &[PathSegment], changes: &mut Vec<ValueDiff>) {
    let mut i = ra.len() as isize - 1;
    let mut j = la.len() as isize - 1;

    while i > j {
        let idx = i as usize;
        changes.push(ValueDiff::Array {
            path: path.to_vec(),
            index: idx,
            item: Box::new(ValueDiff::New { path: vec![], rhs: ra[idx].clone() }),
        });
        i -= 1;
    }
    while j > i {
        let idx = j as usize;
        changes.push(ValueDiff::Array {
            path: path.to_vec(),
            index: idx,
            item: Box::new(ValueDiff::Deleted { path: vec![], lhs: la[idx].clone() }),
        });
        j -= 1;
    }

    // i == j now: the common prefix, compared pairwise from the tail down.
    let mut k = i;
    while k >= 0 {
        let idx = k as usize;
        let mut sub_path = path.to_vec();
        sub_path.push(PathSegment::Index(idx));
        deep_diff(Some(&la[idx]), Some(&ra[idx]), &sub_path, changes);
        k -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_diff_returns_none() {
        assert_eq!(diff_value(&json!({"a": 1}), &json!({"a": 1})), None);
    }

    #[test]
    fn new_key() {
        assert_eq!(
            diff_value(&json!({}), &json!({"a": 1})),
            Some(vec![ValueDiff::New { path: vec![PathSegment::Key("a".into())], rhs: json!(1) }])
        );
    }

    #[test]
    fn deleted_key() {
        assert_eq!(
            diff_value(&json!({"a": 1}), &json!({})),
            Some(vec![ValueDiff::Deleted { path: vec![PathSegment::Key("a".into())], lhs: json!(1) }])
        );
    }

    #[test]
    fn edited_scalar() {
        assert_eq!(
            diff_value(&json!({"a": 1}), &json!({"a": 2})),
            Some(vec![ValueDiff::Edit { path: vec![PathSegment::Key("a".into())], lhs: json!(1), rhs: json!(2) }])
        );
    }

    #[test]
    fn nested_object() {
        assert_eq!(
            diff_value(&json!({"a": {"b": 1}}), &json!({"a": {"b": 2}})),
            Some(vec![ValueDiff::Edit {
                path: vec![PathSegment::Key("a".into()), PathSegment::Key("b".into())],
                lhs: json!(1),
                rhs: json!(2)
            }])
        );
    }

    #[test]
    fn array_tail_growth() {
        assert_eq!(
            diff_value(&json!([1, 2]), &json!([1, 2, 3])),
            Some(vec![ValueDiff::Array {
                path: vec![],
                index: 2,
                item: Box::new(ValueDiff::New { path: vec![], rhs: json!(3) })
            }])
        );
    }

    #[test]
    fn array_tail_shrink() {
        assert_eq!(
            diff_value(&json!([1, 2, 3]), &json!([1, 2])),
            Some(vec![ValueDiff::Array {
                path: vec![],
                index: 2,
                item: Box::new(ValueDiff::Deleted { path: vec![], lhs: json!(3) })
            }])
        );
    }

    #[test]
    fn array_element_edit() {
        assert_eq!(
            diff_value(&json!([1, 2]), &json!([1, 5])),
            Some(vec![ValueDiff::Edit { path: vec![PathSegment::Index(1)], lhs: json!(2), rhs: json!(5) }])
        );
    }
}
