use core::convert::Infallible;
use core::num::NonZeroU32;

use mnesis::{Aggregate, AggregateRoot, Handle};
use mnesis_bombay_core::{CommandOutcome, CommandPhase};
use mnesis_store::{CommandRepository, ConflictPredicate, ExecuteError, Execution, Repository};

use crate::attempts::CommandAttempts;
use crate::failure::CommitFailure;
use crate::phase::PhaseTracker;
use crate::repository::PhaseRepository;

/// Factual outcome returned by direct Mnesis command execution.
///
/// Actor admission has not happened at this layer, so the overload domain is
/// uninhabited. Conflict and storage variants preserve the repository's native
/// error type without erasure.
pub type DirectCommandOutcome<A, R, Output, CommandId> = CommandOutcome<
    <R as Repository<A>>::Position,
    Output,
    <A as Aggregate>::Error,
    <R as Repository<A>>::Error,
    <R as Repository<A>>::Error,
    Infallible,
    CommandId,
>;

/// Loads, decides, and durably appends one aggregate command.
///
/// Confirmed conflicts discard the stale root and reload before the next
/// explicitly permitted attempt. No other failure is retried. The `output`
/// function derives an application reply from the accepted aggregate state
/// after either a durable append or an accepted no-op.
///
/// `classify_commit_failure` is called only for a non-conflict error returned
/// after `Repository::save` began. It must report whether that adapter error
/// proves non-commit or leaves the transaction ambiguous.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "each argument and outcome parameter names a distinct execution policy or failure domain"
)]
pub async fn execute_command<
    A,
    R,
    C,
    Output,
    CommandId,
    Attempts,
    MapOutput,
    Classify,
    const N: usize,
>(
    repository: &R,
    aggregate_id: A::Id,
    command_id: CommandId,
    mut attempts: Attempts,
    mut output: MapOutput,
    classify_commit_failure: Classify,
    phases: &PhaseTracker,
) -> DirectCommandOutcome<A, R, Output, CommandId>
where
    A: Aggregate + Handle<C, N>,
    R: Repository<A>,
    R::Error: ConflictPredicate,
    C: Send,
    Attempts: CommandAttempts<C>,
    MapOutput: FnMut(&AggregateRoot<A>) -> Output,
    Classify: Fn(&R::Error) -> CommitFailure,
{
    phases.set(CommandPhase::Admitted);
    let max_attempts = attempts.max_attempts();
    let mut attempt = NonZeroU32::MIN;

    loop {
        phases.set(CommandPhase::Loading);
        let root = repository.load(aggregate_id.clone()).await;
        let mut root = match root {
            Ok(root) => root,
            Err(source) => {
                phases.set(CommandPhase::AppendNotCommitted);
                return CommandOutcome::Storage(source);
            }
        };

        let command = attempts.command(attempt);
        phases.set(CommandPhase::Deciding);
        let observed = PhaseRepository::new(repository, phases.clone());
        match observed.execute(&mut root, command).await {
            Ok(Execution::Ignored) => {
                phases.set(CommandPhase::AppendNotCommitted);
                return CommandOutcome::Ignored {
                    output: output(&root),
                };
            }
            Ok(Execution::Executed { position, .. }) => {
                debug_assert_eq!(phases.phase(), CommandPhase::AppendCommitted);
                return CommandOutcome::Committed {
                    position,
                    output: output(&root),
                };
            }
            Err(ExecuteError::Decide(rejection)) => {
                phases.set(CommandPhase::AppendNotCommitted);
                return CommandOutcome::Rejected(rejection);
            }
            Err(ExecuteError::Store(source)) if source.is_conflict() => {
                phases.set(CommandPhase::AppendNotCommitted);
                if attempt >= max_attempts {
                    return CommandOutcome::ConflictExhausted {
                        source,
                        attempts: attempt,
                    };
                }
                attempt =
                    NonZeroU32::new(attempt.get().saturating_add(1)).unwrap_or(NonZeroU32::MAX);
            }
            Err(ExecuteError::Store(source)) => match classify_commit_failure(&source) {
                CommitFailure::NotCommitted => {
                    phases.set(CommandPhase::AppendNotCommitted);
                    return CommandOutcome::Storage(source);
                }
                CommitFailure::Ambiguous => {
                    debug_assert_eq!(phases.phase(), CommandPhase::AppendInFlight);
                    return CommandOutcome::AmbiguousCompletion { command_id };
                }
            },
        }
    }
}
