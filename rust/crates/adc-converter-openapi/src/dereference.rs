//! Resolves every `$ref` in a document against itself — same-document JSON
//! Pointer resolution only, no external file loading, since this converter
//! only ever needs to dereference a single in-memory document, never a
//! multi-file spec split across a filesystem.
//!
//! Resolves every `$ref` against a frozen snapshot of the input rather than
//! mutating the tree in place as it goes: simpler to reason about, and
//! sufficient for what this converter actually receives (see
//! `crate::prune`'s module doc for why `$ref`s are rare at all once pruning
//! runs — there's rarely a chain deep enough for the distinction to show
//! up). Cycle detection tracks the full stack of `$ref` pointers currently
//! being expanded and errors on any repeat, so indirect cycles through
//! several hops are caught, not just a node referencing itself directly.

use adc_sdk::ConvertError;
use serde_json::Value;

/// Ceiling on how many nodes a single `dereference` call will visit.
/// Without memoization (see this module's own doc comment on that
/// tradeoff), a document with several `$ref`s that all fan out into the
/// same shared subtree re-expands that subtree once per path to it —
/// harmless for the small, mostly-`$ref`-free documents `prune` leaves
/// this converter with, but a hand-crafted document could chain enough
/// diamond-shaped sharing to blow that up multiplicatively. This bounds
/// the damage with a flat error instead of an unbounded hang.
const MAX_RESOLVED_NODES: usize = 200_000;

pub fn dereference(document: &Value) -> Result<Value, ConvertError> {
    let mut budget = MAX_RESOLVED_NODES;
    resolve_node(document, document, &mut Vec::new(), &mut budget)
}

fn resolve_node(node: &Value, root: &Value, resolving: &mut Vec<String>, budget: &mut usize) -> Result<Value, ConvertError> {
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| ConvertError("OpenAPI document has too many $ref expansions to resolve".to_string()))?;
    match node {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_node(item, root, resolving, budget)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let Some(Value::String(pointer)) = map.get("$ref") else {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (key, value) in map {
                    out.insert(key.clone(), resolve_node(value, root, resolving, budget)?);
                }
                return Ok(Value::Object(out));
            };

            if resolving.iter().any(|p| p == pointer) {
                return Err(ConvertError(format!("circular $ref detected: {pointer}")));
            }
            let target = resolve_pointer(root, pointer)?;
            resolving.push(pointer.clone());
            let resolved = resolve_node(&target, root, resolving, budget);
            resolving.pop();

            // Siblings of `$ref` on this node win over the target's own
            // keys: only keys this node doesn't already have get filled in
            // from the resolved target, but every key — inherited or
            // original — still gets its own nested `$ref`s resolved below.
            let resolved = resolved?;
            let Value::Object(mut merged) = resolved else {
                return Ok(resolved);
            };
            for (key, value) in map {
                if key == "$ref" {
                    continue;
                }
                merged.insert(key.clone(), resolve_node(value, root, resolving, budget)?);
            }
            Ok(Value::Object(merged))
        }
        other => Ok(other.clone()),
    }
}

/// Resolves a `#/a/b/c`-shaped same-document JSON Pointer reference.
fn resolve_pointer(root: &Value, uri: &str) -> Result<Value, ConvertError> {
    let path = uri
        .strip_prefix('#')
        .ok_or_else(|| ConvertError(format!("unsupported $ref (external references are not supported): {uri}")))?;
    if path.is_empty() {
        return Ok(root.clone());
    }
    let path = path.strip_prefix('/').ok_or_else(|| ConvertError(format!("invalid $ref: {uri}")))?;

    let mut current = root;
    for raw_segment in path.split('/') {
        // RFC 6901 unescaping: undo the "~1" -> "/" then "~0" -> "~"
        // encoding, in that order (reversed from how it was encoded).
        let segment = raw_segment.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(map) => {
                map.get(&segment).ok_or_else(|| ConvertError(format!("could not resolve $ref: {uri}")))?
            }
            Value::Array(items) => {
                let index: usize =
                    segment.parse().map_err(|_| ConvertError(format!("could not resolve $ref: {uri}")))?;
                items.get(index).ok_or_else(|| ConvertError(format!("could not resolve $ref: {uri}")))?
            }
            _ => return Err(ConvertError(format!("could not resolve $ref: {uri}"))),
        };
    }
    Ok(current.clone())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[rstest]
    #[case::resolves_a_simple_pointer(
        json!({"components": {"pathItems": {"foo": {"a": 1}}}, "target": {"$ref": "#/components/pathItems/foo"}}),
        "target",
        json!({"a": 1}),
    )]
    #[case::follows_a_chain_of_refs(
        json!({"a": {"$ref": "#/b"}, "b": {"$ref": "#/c"}, "c": {"value": 1}}),
        "a",
        json!({"value": 1}),
    )]
    #[case::sibling_keys_win_over_the_targets_own_keys(
        json!({"a": {"$ref": "#/b", "value": "own"}, "b": {"value": "target", "other": 1}}),
        "a",
        json!({"value": "own", "other": 1}),
    )]
    #[case::unescapes_tilde_and_slash_in_pointer_segments(
        json!({"a": {"b/c~d": {"value": 1}}, "target": {"$ref": "#/a/b~1c~0d"}}),
        "target",
        json!({"value": 1}),
    )]
    fn dereference_resolves_cases(#[case] doc: Value, #[case] key: &str, #[case] expected: Value) {
        let out = dereference(&doc).unwrap();
        assert_eq!(out[key], expected);
    }

    #[rstest]
    #[case::a_direct_self_reference_is_an_error(json!({"a": {"$ref": "#/a"}}))]
    #[case::a_two_step_cycle_is_an_error(json!({"a": {"$ref": "#/b"}, "b": {"$ref": "#/a"}}))]
    #[case::an_external_reference_is_rejected(json!({"a": {"$ref": "other.yaml#/b"}}))]
    #[case::a_dangling_reference_is_an_error(json!({"a": {"$ref": "#/does/not/exist"}}))]
    fn dereference_error_cases(#[case] doc: Value) {
        assert!(dereference(&doc).is_err());
    }

    #[test]
    fn a_diamond_shaped_ref_chain_that_would_blow_up_exponentially_is_bounded() {
        // No cycle here — L0 through L19 each fan out into two refs to the
        // next level, so resolving L0 without memoization would revisit the
        // shared tail 2^20 times, well past MAX_RESOLVED_NODES.
        let mut doc = serde_json::Map::new();
        for level in 0..20 {
            doc.insert(
                format!("L{level}"),
                json!({"a": {"$ref": format!("#/L{}", level + 1)}, "b": {"$ref": format!("#/L{}", level + 1)}}),
            );
        }
        doc.insert("L20".to_string(), json!({"value": 1}));
        let err = dereference(&Value::Object(doc)).unwrap_err();
        assert!(err.0.contains("too many"), "{}", err.0);
    }
}
