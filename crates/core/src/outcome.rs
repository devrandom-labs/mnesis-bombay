use core::num::NonZeroU32;

/// Observable, factual result of attempting one durable command.
///
/// Each failure domain has its own generic type. Adapters may preserve their
/// native facts without erasure or conversion into an unstructured error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandOutcome<Position, Output, Rejection, Conflict, Storage, Overload, CommandId> {
    /// Mnesis accepted the command but decided no events; no append occurred.
    Ignored {
        /// Application-selected result of the accepted no-op decision.
        output: Output,
    },
    /// Mnesis confirmed the append at the returned read-your-writes position.
    Committed {
        /// Durable global position returned by the repository.
        position: Position,
        /// Application-selected result derived from the confirmed execution.
        output: Output,
    },
    /// The aggregate rejected the command; no append occurred.
    Rejected(Rejection),
    /// Confirmed optimistic conflicts exhausted the explicit replay budget.
    ConflictExhausted {
        /// Last confirmed conflict fact.
        source: Conflict,
        /// Non-zero number of executions attempted.
        attempts: NonZeroU32,
    },
    /// Storage failed outside the confirmed-conflict policy.
    Storage(Storage),
    /// Runtime admission refused the command before execution began.
    Overloaded(Overload),
    /// The absolute deadline elapsed before execution began.
    DeadlineBeforeExecution,
    /// Append may have committed, but no terminal storage fact was observed.
    AmbiguousCompletion {
        /// Stable application-owned identity needed for explicit recovery.
        command_id: CommandId,
    },
    /// The host rejected the command while shutting down, before execution.
    ShuttingDown,
}
