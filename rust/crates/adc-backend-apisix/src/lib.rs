//! The Apache APISIX gateway integration. The supported public API is just
//! [`Backend`] — the fetcher, operator, and validator it's built from are
//! internal orchestration pieces, not things a real consumer should reach
//! for directly (call `Backend::dump`/`sync`/`validate` instead). They're
//! still reachable via [`tests`] for this crate's own test suite and for
//! other crates' e2e tests that want to exercise one piece in isolation.

mod backend;
mod fetcher;
mod operator;
mod transformer;
mod typing;
mod utils;
mod validator;

pub use backend::Backend;

/// Internal building blocks, exposed only for tests — see the crate-level
/// doc comment. Not part of the supported API: gated behind the
/// `test-utils` feature (on by default only via this crate's own
/// self-referencing dev-dependency), so it doesn't leak into a normal
/// build's public surface.
#[cfg(feature = "test-utils")]
#[doc(hidden)]
pub mod tests {
    pub use crate::fetcher::Fetcher;
    pub use crate::operator::Operator;
    pub use crate::validator::Validator;

    pub mod transformer {
        pub use crate::transformer::*;
    }
    pub mod typing {
        pub use crate::typing::*;
    }
}
