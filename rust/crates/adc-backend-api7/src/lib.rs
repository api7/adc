//! The API7 Enterprise Dashboard API gateway integration. So far: resolving
//! the human-readable gateway group name a user configures into the id
//! API7's admin API actually expects ([`GatewayGroupResolver`]), and
//! fetching a gateway group's resources in their wire shape ([`Fetcher`]).
//! The transformer/operator/validator that turn this into a full
//! `adc_sdk::Backend` land in later work.

mod fetcher;
mod gateway_group;
mod typing;

pub use fetcher::Fetcher;
pub use gateway_group::GatewayGroupResolver;
