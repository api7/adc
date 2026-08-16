//! The `Converter` trait: the interface every source-format converter
//! (OpenAPI, ...) implements to produce a `Configuration` from raw input.
//! `adc-sdk` only defines the contract — concrete converters live in their
//! own crates and depend on this one.

use crate::resources::Configuration;

/// Failure converting input into a `Configuration`. Stays a flat,
/// message-carrying error rather than inventing variants nothing downstream
/// needs yet.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ConvertError(pub String);

impl From<&str> for ConvertError {
    fn from(message: &str) -> Self {
        ConvertError(message.to_string())
    }
}

impl From<String> for ConvertError {
    fn from(message: String) -> Self {
        ConvertError(message)
    }
}

pub trait Converter {
    fn to_adc(&self, input: &str) -> Result<Configuration, ConvertError>;
}
