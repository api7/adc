//! Resolves every `$ref` in a document against itself. Ported from
//! `@scalar/openapi-parser`'s `resolveReferences`, narrowed to what
//! `OpenAPIConverter` actually needs from it: `parseOAS` only ever calls
//! `dereference(specification)` on a single in-memory object, never with a
//! filesystem/fetch plugin registered, so external (cross-file) `$ref`s are
//! rejected outright rather than supported.
//!
//! Simplified relative to the upstream implementation in two ways, both
//! deliberate:
//! - Upstream resolves against the *same* object tree it mutates in place,
//!   so a `$ref` target that was itself just rewritten by an earlier step
//!   can observe that rewrite; this resolves every `$ref` against a frozen
//!   snapshot of the input instead. The two only disagree on documents with
//!   `$ref` chains through content order-dependently mutated mid-resolution
//!   — not a shape `pruneConversionDocument` ever leaves behind for this
//!   converter's own inputs.
//! - Upstream detects only *direct* self-reference chains (`processedRefs`,
//!   reset per node) and otherwise relies on a `WeakSet` of already-visited
//!   objects to silently stop re-descending into shared/cyclic structure.
//!   This tracks the full stack of `$ref` pointers currently being expanded
//!   and errors on any cycle through it — strictly more cycles are caught,
//!   never fewer — matching this project's rustify.md 7.2 call to
//!   strengthen cycle handling rather than track scalar's own edge cases
//!   1:1.

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
            // keys (mirroring `if (schema[key] === undefined) schema[key]
            // = resolved[key]`), but still get their own nested `$ref`s
            // resolved independently.
            let mut merged = match resolved? {
                Value::Object(m) => m,
                other => return Ok(other),
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
    use serde_json::json;

    use super::*;

    #[test]
    fn resolves_a_simple_pointer() {
        let doc = json!({"components": {"pathItems": {"foo": {"a": 1}}}, "target": {"$ref": "#/components/pathItems/foo"}});
        let out = dereference(&doc).unwrap();
        assert_eq!(out["target"], json!({"a": 1}));
    }

    #[test]
    fn follows_a_chain_of_refs() {
        let doc = json!({"a": {"$ref": "#/b"}, "b": {"$ref": "#/c"}, "c": {"value": 1}});
        let out = dereference(&doc).unwrap();
        assert_eq!(out["a"], json!({"value": 1}));
    }

    #[test]
    fn sibling_keys_win_over_the_targets_own_keys() {
        let doc = json!({"a": {"$ref": "#/b", "value": "own"}, "b": {"value": "target", "other": 1}});
        let out = dereference(&doc).unwrap();
        assert_eq!(out["a"], json!({"value": "own", "other": 1}));
    }

    #[test]
    fn unescapes_tilde_and_slash_in_pointer_segments() {
        let doc = json!({"a": {"b/c~d": {"value": 1}}, "target": {"$ref": "#/a/b~1c~0d"}});
        let out = dereference(&doc).unwrap();
        assert_eq!(out["target"], json!({"value": 1}));
    }

    #[test]
    fn a_direct_self_reference_is_an_error() {
        let doc = json!({"a": {"$ref": "#/a"}});
        assert!(dereference(&doc).is_err());
    }

    #[test]
    fn a_two_step_cycle_is_an_error() {
        let doc = json!({"a": {"$ref": "#/b"}, "b": {"$ref": "#/a"}});
        assert!(dereference(&doc).is_err());
    }

    #[test]
    fn an_external_reference_is_rejected() {
        let doc = json!({"a": {"$ref": "other.yaml#/b"}});
        assert!(dereference(&doc).is_err());
    }

    #[test]
    fn a_dangling_reference_is_an_error() {
        let doc = json!({"a": {"$ref": "#/does/not/exist"}});
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
