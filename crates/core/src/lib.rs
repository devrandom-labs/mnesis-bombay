//! Runtime-, transport-, and storage-adapter-neutral application protocol.
//!
//! This crate intentionally does not depend on Actorpass, Tower, Tokio, HTTP,
//! or a concrete Mnesis store adapter.

#![no_std]
#![forbid(unsafe_code)]

/// A command with independent aggregate, command, and payload identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandRequest<AggregateId, CommandId, Command> {
    /// Logical aggregate identity. This is not an actor address.
    pub aggregate_id: AggregateId,
    /// Stable identity used for retry and reply recovery.
    pub command_id: CommandId,
    /// Typed domain command.
    pub command: Command,
}

/// Observable result of attempting one durable command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome<Position, DomainError, StoreError> {
    /// The command was accepted but decided no events.
    Ignored,
    /// Events were durably appended at the returned read-your-writes position.
    Committed {
        /// Durable global position returned by the repository.
        position: Position,
    },
    /// The aggregate rejected the command without persistence.
    Rejected(DomainError),
    /// Optimistic conflict retry exhausted its explicit budget.
    ConflictExhausted {
        /// Last repository conflict.
        source: StoreError,
        /// Number of executions attempted.
        attempts: u32,
    },
    /// Persistence failed for a reason not classified as retryable conflict.
    StorageFailed(StoreError),
    /// Append may have succeeded but its terminal reply was not observed.
    Ambiguous,
}
