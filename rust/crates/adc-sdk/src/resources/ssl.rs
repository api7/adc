//! The `SSL` (certificate) resource.

use serde::{Deserialize, Serialize};

use super::common::Labels;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SSLCertificate {
    pub certificate: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SslClient {
    pub ca: String,
    #[serde(default = "default_client_depth")]
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_mtls_uri_regex: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SSL {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Labels>,

    #[serde(rename = "type", default)]
    pub r#type: SslType,
    pub snis: Vec<String>,
    pub certificates: Vec<SSLCertificate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<SslClient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_protocols: Option<Vec<SslProtocol>>,
}
