//! The `Service` resource. Two of its cross-field rules (path_prefix must
//! start with "/", upstream required when upstreams is set) are semantic
//! validation, not structural — left for the validation layer. A third
//! ("HTTP routes and Stream routes are mutually exclusive") is enforced here
//! instead, via `ServiceRoutes` — see its doc comment for why and how.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use super::common::{Labels, Plugins};
use super::route::{Route, StreamRoute};
use super::upstream::Upstream;

/// `Upstream.name` is optional everywhere in its own schema (it doubles as
/// `Service.upstream`, the unnamed default), but each entry in
/// `Service.upstreams[]` needs a name to be addressable — matches the TS SDK's
/// exported `schema.json`, which shows `upstreams[].items.required ==
/// ["name"]` even though the base `Upstream` schema has no required list at
/// all. `allOf` layers the extra `required` on top of `Upstream`'s own
/// (possibly `$ref`'d) schema rather than trying to mutate it in place. The
/// outer `anyOf`+`null` is needed because `schema_with` disables schemars'
/// usual `Option<T>` handling (see `common.rs`'s note on `schema_with`) —
/// `upstreams` is itself optional even though each of its items needs a name.
fn upstreams_schema(generator: &mut SchemaGenerator) -> Schema {
    let item = generator.subschema_for::<Upstream>();
    let value = serde_json::json!({
        "anyOf": [
            {"type": "array", "items": {"allOf": [item, {"required": ["name"]}]}},
            {"type": "null"}
        ]
    });
    Schema::try_from(value).expect("valid schema")
}

/// `Service.upstream` (the singular, embedded default upstream) must never
/// carry an `id`: it isn't an addressable resource of its own — the id it
/// ends up with, if the backend splits it into a real upstream sub-resource,
/// is generated internally. `Service.upstreams[]` items *are* addressable on
/// their own and do allow an explicit `id` (see `upstreams_schema`, which
/// doesn't restrict it). `not: {required: ["id"]}` rejects the key if
/// present, rather than just leaving it optional.
fn default_upstream_schema(generator: &mut SchemaGenerator) -> Schema {
    let item = generator.subschema_for::<Upstream>();
    let value = serde_json::json!({
        "anyOf": [
            {"allOf": [item, {"not": {"required": ["id"]}}]},
            {"type": "null"}
        ]
    });
    Schema::try_from(value).expect("valid schema")
}

/// `Service.routes`/`Service.stream_routes` are two sibling JSON keys, at
/// most one of which may be present — a service either proxies HTTP or a
/// stream (TCP/UDP), never both. This enum makes the other combination
/// structurally unrepresentable rather than checking for it separately.
///
/// This can't come from a plain `#[derive(Deserialize)]` + `#[serde(untagged,
/// flatten)]`: that combination doesn't actually reject "both keys present"
/// (flatten buffers unmatched keys and untagged just accepts the first
/// variant that parses, silently ignoring the other key). Enforcing the
/// exclusion instead happens via `Service`'s `#[serde(try_from = "ServiceRaw")]`
/// — `ServiceRaw` derives normally with both fields as plain
/// `Option<Vec<_>>`, and `TryFrom` is where the two are reconciled into this
/// enum (or rejected).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ServiceRoutes {
    Http { routes: Vec<Route> },
    Stream { stream_routes: Vec<StreamRoute> },
}

impl ServiceRoutes {
    pub fn http(&self) -> Option<&[Route]> {
        match self {
            ServiceRoutes::Http { routes } => Some(routes),
            ServiceRoutes::Stream { .. } => None,
        }
    }

    pub fn stream(&self) -> Option<&[StreamRoute]> {
        match self {
            ServiceRoutes::Stream { stream_routes } => Some(stream_routes),
            ServiceRoutes::Http { .. } => None,
        }
    }

    fn from_raw(
        routes: Option<Vec<Route>>,
        stream_routes: Option<Vec<StreamRoute>>,
    ) -> Result<Option<Self>, String> {
        match (routes, stream_routes) {
            (Some(routes), None) => Ok(Some(ServiceRoutes::Http { routes })),
            (None, Some(stream_routes)) => Ok(Some(ServiceRoutes::Stream { stream_routes })),
            (None, None) => Ok(None),
            (Some(_), Some(_)) => {
                Err("routes and stream_routes are mutually exclusive".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ServiceRaw")]
pub struct Service {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<Upstream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstreams: Option<Vec<Upstream>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Plugins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strip_path_prefix: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,

    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub routes: Option<ServiceRoutes>,
}

/// Plain-shape deserialization target for `Service` — see `ServiceRoutes`'s
/// doc comment for why this indirection exists. Every field here must stay
/// in sync with `Service`'s (routes/stream_routes excepted).
///
/// This is also where the `#[schemars(...)]` field attributes live rather
/// than on `Service` itself: `#[serde(try_from = "ServiceRaw")]` makes
/// `schemars` derive `Service`'s exported schema *from* `ServiceRaw`'s shape
/// (the actual wire input), not `Service`'s own internal representation —
/// putting the attributes on `Service` would silently be dead code.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ServiceRaw {
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[a-zA-Z0-9-_.]+$"))]
    id: Option<String>,
    #[schemars(length(min = 1, max = 65536))]
    name: String,
    #[schemars(length(max = 65536))]
    description: Option<String>,
    labels: Option<Labels>,

    #[serde(default)]
    #[schemars(schema_with = "default_upstream_schema")]
    upstream: Option<Upstream>,
    #[serde(default)]
    #[schemars(schema_with = "upstreams_schema")]
    upstreams: Option<Vec<Upstream>>,
    plugins: Option<Plugins>,
    path_prefix: Option<String>,
    strip_path_prefix: Option<bool>,
    #[schemars(inner(length(min = 1)))]
    hosts: Option<Vec<String>>,

    #[serde(default)]
    routes: Option<Vec<Route>>,
    #[serde(default)]
    stream_routes: Option<Vec<StreamRoute>>,
}

impl TryFrom<ServiceRaw> for Service {
    type Error = String;

    fn try_from(raw: ServiceRaw) -> Result<Self, Self::Error> {
        Ok(Service {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            labels: raw.labels,
            upstream: raw.upstream,
            upstreams: raw.upstreams,
            plugins: raw.plugins,
            path_prefix: raw.path_prefix,
            strip_path_prefix: raw.strip_path_prefix,
            hosts: raw.hosts,
            routes: ServiceRoutes::from_raw(raw.routes, raw.stream_routes)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A JSON `null` and an absent key must deserialize to the identical
    /// `Option<T>::None` for every optional field here — callers that build
    /// this document as a `serde_json::Value` in code (rather than through
    /// `Service`'s own `Serialize` impl) rely on this to fill an unset
    /// field with `Value::Null` rather than tracking which keys to omit.
    /// Goes through the `ServiceRaw` -> `Service` `TryFrom` indirection
    /// (see this module's doc comment), not a plain derive — worth pinning
    /// down on its own, separately from `Route`'s equivalent test.
    #[test]
    fn an_explicit_null_and_an_absent_key_deserialize_identically() {
        let with_null = json!({
            "name": "s",
            "description": null, "labels": null, "plugins": null,
        });
        let absent = json!({"name": "s"});
        let service_with_null: Service = serde_json::from_value(with_null).unwrap();
        let service_absent: Service = serde_json::from_value(absent).unwrap();
        assert_eq!(service_with_null, service_absent);
        assert!(service_with_null.description.is_none());
        assert!(service_with_null.labels.is_none());
        assert!(service_with_null.plugins.is_none());
        assert!(service_absent.description.is_none());
        assert!(service_absent.labels.is_none());
        assert!(service_absent.plugins.is_none());
    }
}
