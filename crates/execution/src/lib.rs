//! Runtime-independent execution of Mnesis aggregate commands.
//!
//! This crate composes Mnesis's existing [`mnesis_store::CommandRepository`]
//! rather than defining another repository or command handler. It adds only
//! application policy that Mnesis deliberately leaves to its consumer:
//! bounded conflict replay, commit-uncertainty classification, factual
//! outcomes, and an externally inspectable durability phase.
//!
//! Bombay aggregate actors use this exact execution path, but the crate has no
//! actor-runtime, Tokio, Tower, transport, or concrete-store dependency.

#![forbid(unsafe_code)]

mod attempts;
mod execute;
mod failure;
mod phase;
mod repository;

pub use attempts::{CommandAttempts, Once, Replay};
pub use execute::{DirectCommandOutcome, execute_command};
pub use failure::CommitFailure;
pub use phase::PhaseTracker;
