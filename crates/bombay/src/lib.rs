//! Bombay-specific integration boundary.
//!
//! The production API remains deliberately small while failure and readiness
//! semantics are validated. The architecture is specified in ADR 0001.

#![forbid(unsafe_code)]

/// Typed request emitted by a pure Behavior for interpretation by an
/// application service route.
#[derive(Debug)]
pub struct ExecuteRequest<AggregateId, CommandId, Command, Reply> {
    /// Logical aggregate identity, distinct from the actor address.
    pub aggregate_id: AggregateId,
    /// Stable command identity for retry and reply recovery.
    pub command_id: CommandId,
    /// Typed domain command.
    pub command: Command,
    /// Runtime-specific typed reply capability.
    pub reply: Reply,
}
