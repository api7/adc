//! The API7 Enterprise Dashboard API gateway integration. Currently only
//! models the `gateway_group` concept — resolving the human-readable
//! gateway group name a user configures into the id API7's admin API
//! actually expects on every scoped request. The fetcher/operator/
//! validator/transformer that turn this into a full `adc_sdk::Backend`
//! land in later work.

mod gateway_group;

pub use gateway_group::GatewayGroupResolver;
