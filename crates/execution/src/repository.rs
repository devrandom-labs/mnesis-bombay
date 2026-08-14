use mnesis::{Aggregate, AggregateRoot, EventOf, Events};
use mnesis_bombay_core::CommandPhase;
use mnesis_store::Repository;

use crate::PhaseTracker;

/// Transparent repository decorator that observes the exact `save` boundary
/// used by Mnesis's blanket `CommandRepository` implementation.
pub(crate) struct PhaseRepository<'a, R> {
    inner: &'a R,
    phases: PhaseTracker,
}

impl<'a, R> PhaseRepository<'a, R> {
    pub(crate) const fn new(inner: &'a R, phases: PhaseTracker) -> Self {
        Self { inner, phases }
    }
}

impl<A, R> Repository<A> for PhaseRepository<'_, R>
where
    A: Aggregate,
    R: Repository<A>,
{
    type Error = R::Error;
    type Position = R::Position;

    async fn load(&self, id: A::Id) -> Result<AggregateRoot<A>, Self::Error> {
        self.inner.load(id).await
    }

    async fn save<const N: usize>(
        &self,
        aggregate: &mut AggregateRoot<A>,
        events: &Events<EventOf<A>, N>,
    ) -> Result<Self::Position, Self::Error> {
        self.phases.set(CommandPhase::AppendInFlight);
        let result = self.inner.save(aggregate, events).await;
        if result.is_ok() {
            self.phases.set(CommandPhase::AppendCommitted);
        }
        result
    }
}
