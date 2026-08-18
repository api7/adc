//! The `SSL` (certificate) resource.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use super::common::Labels;

/// A PEM-ish blob (certificate) or a `$secret://`/`$env://` reference in its
/// place — an `anyOf` union, not a structural type, because the field stays
/// plain `String` (see `SSLCertificate`'s doc comment). Pattern/lengths
/// copied from the TS SDK's exported `schema.json` (Zod's own
/// `z.union([...])` for this field).
fn certificate_schema(_gen: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [
            {"type": "string", "minLength": 128, "maxLength": 65536},
            {"type": "string", "pattern": r"^\$(secret|env):\/\/"}
        ]
    })
}

/// Same shape as `certificate_schema`, but for a private key — TS requires a
/// shorter minimum length (32 vs 128) for these.
fn pem_key_or_secret_ref_schema(_gen: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "anyOf": [
            {"type": "string", "minLength": 32, "maxLength": 65536},
            {"type": "string", "pattern": r"^\$(secret|env):\/\/"}
        ]
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum SslType {
    #[default]
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "client")]
    Client,
}

fn default_client_depth() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SslProtocol {
    #[serde(rename = "TLSv1.1")]
    Tlsv1_1,
    #[serde(rename = "TLSv1.2")]
    Tlsv1_2,
    #[serde(rename = "TLSv1.3")]
    Tlsv1_3,
}

/// A certificate/key pair. Either may also be a `$secret://`/`$env://`
/// reference string instead of inline PEM content — that's a semantic
/// (regex-union) check, not a structural one, so both fields are plain
/// `String` here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SSLCertificate {
    #[schemars(schema_with = "certificate_schema")]
    pub certificate: String,
    #[schemars(schema_with = "pem_key_or_secret_ref_schema")]
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SslClient {
    #[schemars(schema_with = "certificate_schema")]
    pub ca: String,
    #[serde(default = "default_client_depth")]
    #[schemars(range(min = 0))]
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub skip_mtls_uri_regex: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SSL {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[a-zA-Z0-9-_.]+$"))]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(rename = "type", default)]
    pub r#type: SslType,
    #[schemars(length(min = 1), inner(length(min = 1)))]
    pub snis: Vec<String>,
    #[schemars(length(min = 1))]
    pub certificates: Vec<SSLCertificate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<SslClient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub ssl_protocols: Option<Vec<SslProtocol>>,
}
