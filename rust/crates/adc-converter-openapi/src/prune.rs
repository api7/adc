//! Strips fields from the document before conversion reads it. Runs before
//! `dereference` (see `crate::dereference`'s module doc for why
//! that ordering matters): strips every field this converter never reads —
//! `requestBody`/`responses`/`parameters`/schema `components.*`/... — so the
//! `$ref` surface `dereference` has to walk is whatever's left: `info`,
//! `servers`, and the `paths.*.{method}` fields `crate::parser` actually
//! consumes, plus any `x-adc-*` extension blob a user wrote.

use serde_json::{Map, Value};

use crate::HTTP_METHODS;

const OPERATION_SCHEMA_FIELDS: &[&str] =
    &["parameters", "requestBody", "responses", "callbacks", "security", "tags", "deprecated", "externalDocs"];

const COMPONENT_SCHEMA_FIELDS: &[&str] =
    &["schemas", "responses", "requestBodies", "headers", "parameters", "examples", "links", "callbacks"];

const ROOT_SCHEMA_FIELDS: &[&str] = &["tags", "externalDocs", "security", "webhooks"];

pub fn prune_conversion_document(spec: &mut Map<String, Value>) {
    for field in ROOT_SCHEMA_FIELDS {
        spec.remove(*field);
    }
    if let Some(Value::Object(components)) = spec.get_mut("components") {
        for field in COMPONENT_SCHEMA_FIELDS {
            components.remove(*field);
        }
        if let Some(Value::Object(path_items)) = components.get_mut("pathItems") {
            prune_paths(path_items);
        }
    }
    if let Some(Value::Object(paths)) = spec.get_mut("paths") {
        prune_paths(paths);
    }
}

fn prune_paths(paths: &mut Map<String, Value>) {
    for path_item in paths.values_mut() {
        let Value::Object(path_item) = path_item else { continue };
        path_item.remove("parameters");
        for method in HTTP_METHODS {
            let Some(Value::Object(operation)) = path_item.get_mut(*method) else { continue };
            for field in OPERATION_SCHEMA_FIELDS {
                operation.remove(*field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn strips_root_and_component_schema_fields() {
        let mut spec = json!({
            "tags": [{"name": "x"}],
            "components": {"schemas": {"Foo": {}}, "securitySchemes": {"bearer": {}}},
            "paths": {},
        })
        .as_object()
        .unwrap()
        .clone();
        prune_conversion_document(&mut spec);
        assert!(!spec.contains_key("tags"));
        let components = spec["components"].as_object().unwrap();
        assert!(!components.contains_key("schemas"));
        assert!(components.contains_key("securitySchemes"), "fields outside the prune list must survive");
    }

    #[test]
    fn strips_operation_schema_fields_but_keeps_what_the_converter_reads() {
        let mut spec = json!({
            "paths": {
                "/foo": {
                    "parameters": [{"name": "id"}],
                    "get": {
                        "operationId": "getFoo",
                        "summary": "Get foo",
                        "requestBody": {},
                        "responses": {"200": {}},
                        "x-adc-name": "custom-name",
                    },
                },
            },
        })
        .as_object()
        .unwrap()
        .clone();
        prune_conversion_document(&mut spec);
        let operation = &spec["paths"]["/foo"]["get"];
        assert!(!spec["paths"]["/foo"].as_object().unwrap().contains_key("parameters"));
        assert!(!operation.as_object().unwrap().contains_key("requestBody"));
        assert!(!operation.as_object().unwrap().contains_key("responses"));
        assert_eq!(operation["operationId"], json!("getFoo"));
        assert_eq!(operation["x-adc-name"], json!("custom-name"));
    }
}
