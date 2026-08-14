use core::num::NonZeroU32;

mod sealed {
    pub trait Sealed {}
}

/// Supplies one command value for every permitted decision attempt.
///
/// Implementations make replay eligibility explicit. The executor never
/// clones or reconstructs a command without the application choosing a source
/// that permits it. This protocol is closed: applications select [`Once`] or
/// [`Replay`] instead of supplying an implementation that could violate the
/// attempt budget.
pub trait CommandAttempts<C>: sealed::Sealed {
    /// Maximum number of executions, including the initial attempt.
    fn max_attempts(&self) -> NonZeroU32;

    /// Produces the command for `attempt`.
    ///
    /// The executor calls this exactly once for each value from one through
    /// [`Self::max_attempts`], in increasing order, and only after a confirmed
    /// conflict for attempts after the first.
    fn command(&mut self, attempt: NonZeroU32) -> C;
}

/// A command that is not eligible for replay after a conflict.
#[derive(Debug)]
pub struct Once<C> {
    command: Option<C>,
}

impl<C> Once<C> {
    /// Wraps a command for exactly one execution attempt.
    pub const fn new(command: C) -> Self {
        Self {
            command: Some(command),
        }
    }
}

impl<C> sealed::Sealed for Once<C> {}

impl<C> CommandAttempts<C> for Once<C> {
    fn max_attempts(&self) -> NonZeroU32 {
        NonZeroU32::MIN
    }

    fn command(&mut self, attempt: NonZeroU32) -> C {
        debug_assert_eq!(attempt, NonZeroU32::MIN);
        self.command
            .take()
            .expect("Once is consumed only by its single declared attempt")
    }
}

/// An explicitly replayable command factory with a finite attempt budget.
#[derive(Debug)]
pub struct Replay<F> {
    max_attempts: NonZeroU32,
    make: F,
}

impl<F> Replay<F> {
    /// Creates a bounded replay source.
    ///
    /// The factory receives the non-zero, one-based attempt number. It is
    /// called again only after the preceding attempt produced a confirmed
    /// optimistic-concurrency conflict.
    pub const fn new(max_attempts: NonZeroU32, make: F) -> Self {
        Self { max_attempts, make }
    }
}

impl<F> sealed::Sealed for Replay<F> {}

impl<C, F> CommandAttempts<C> for Replay<F>
where
    F: FnMut(NonZeroU32) -> C,
{
    fn max_attempts(&self) -> NonZeroU32 {
        self.max_attempts
    }

    fn command(&mut self, attempt: NonZeroU32) -> C {
        (self.make)(attempt)
    }
}
