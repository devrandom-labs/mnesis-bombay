/// Application/store classification of a non-conflict append error.
///
/// Mnesis can prove optimistic conflicts, but a generic repository cannot know
/// whether an adapter connection error happened before or after its durable
/// commit point. The adapter or application must state that fact explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitFailure {
    /// The store proves that this append did not commit.
    NotCommitted,
    /// The append may have committed; transparent retry is unsafe.
    Ambiguous,
}
