//! The `Service` resource. Two of its cross-field rules (path_prefix must
//! start with "/", upstream required when upstreams is set) are semantic
//! validation, not structural — left for the validation layer. A third
//! ("HTTP routes and Stream routes are mutually exclusive") is enforced here
//! instead, via `ServiceRoutes` — see its doc comment for why and how.

use serde::{Deserialize, Serialize};

use super::common::{Labels, Plugins};
use super::route::{Route, StreamRoute};
use super::upstream::Upstream;

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
#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceRaw {
    id: Option<String>,
    name: String,
    description: Option<String>,
    labels: Option<Labels>,

    upstream: Option<Upstream>,
    upstreams: Option<Vec<Upstream>>,
    plugins: Option<Plugins>,
    path_prefix: Option<String>,
    strip_path_prefix: Option<bool>,
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
