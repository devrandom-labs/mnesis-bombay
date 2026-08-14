/// Last command phase established by the application interpreter.
///
/// These phases describe execution and durability, not actor lifecycle. In
/// particular, mailbox delivery and supervision cannot advance this state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandPhase {
    /// The command exists but has not been accepted for execution.
    Received,
    /// The command was admitted but execution has not started.
    Admitted,
    /// Aggregate reconstruction is in progress.
    Loading,
    /// Pure domain decision is in progress; append has not started.
    Deciding,
    /// The append was invoked and no terminal storage fact is known.
    AppendInFlight,
    /// The store confirmed that no append occurred.
    AppendNotCommitted,
    /// The store confirmed a successful append.
    AppendCommitted,
}

/// What interruption at a [`CommandPhase`] permits the caller to conclude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterruptionFact {
    /// This interpreter has not invoked append.
    NotAppended,
    /// The transaction outcome is unknown; blind retry is unsafe.
    Ambiguous,
    /// The store already established that no append occurred.
    NotCommitted,
    /// The store already confirmed durable append success.
    Committed,
}

impl CommandPhase {
    /// Classifies interruption solely from the last established durability
    /// fact.
    #[must_use]
    pub const fn interruption_fact(self) -> InterruptionFact {
        match self {
            Self::Received | Self::Admitted | Self::Loading | Self::Deciding => {
                InterruptionFact::NotAppended
            }
            Self::AppendInFlight => InterruptionFact::Ambiguous,
            Self::AppendNotCommitted => InterruptionFact::NotCommitted,
            Self::AppendCommitted => InterruptionFact::Committed,
        }
    }
}
