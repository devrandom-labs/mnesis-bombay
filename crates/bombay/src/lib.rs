//! Bombay-specific integration boundary.
//!
//! Bombay Entity owns stable local routing and activation lifecycle. This
//! crate is the only place where an application aggregate ID is translated
//! into that runtime identity.

#![forbid(unsafe_code)]

mod routing;
mod transport;

pub use routing::into_entity_delivery;
pub use transport::ExecuteRequest;
