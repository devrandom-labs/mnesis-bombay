//! Optional Tower interoperability boundary.
//!
//! Tower is not required by the core protocol or the Bombay adapter. This
//! crate exists so HTTP and other Tower-native software can share the same
//! typed command contract without infecting inner dependency layers.

#![forbid(unsafe_code)]

pub use tower_service::Service;
