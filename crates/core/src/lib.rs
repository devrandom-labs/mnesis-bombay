//! Runtime-, transport-, and storage-adapter-neutral application protocol.
//!
//! Mnesis owns aggregates, their application-selected identifiers, pure
//! command handling, and durable repository execution. This crate only adds
//! the request facts needed outside a domain decision and the factual outcome
//! vocabulary shared by direct and actor-hosted callers.

#![no_std]
#![forbid(unsafe_code)]

mod addressed;
mod command;
mod context;
mod outcome;
mod phase;

pub use addressed::Addressed;
pub use command::{CommandIdentity, CommandRequest};
pub use context::{Context, ContextError, ValidatedCommandContext, ValidatedContext};
pub use outcome::CommandOutcome;
pub use phase::{CommandPhase, InterruptionFact};
