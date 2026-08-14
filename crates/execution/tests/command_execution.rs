//! Contract tests over the Counter domain adapted from Mnesis's own
//! `CommandRepository::execute` examples and tests.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "tests"
)]

use core::convert::Infallible;
use core::num::NonZeroU32;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bytes::Bytes;
use mnesis::{
    Aggregate, AggregateRoot, AggregateState, DomainEvent, ErrorId, EventOf, Events, Handle,
    Message, events,
};
use mnesis_bombay_core::{CommandOutcome, CommandPhase, InterruptionFact};
use mnesis_bombay_execution::{CommitFailure, Once, PhaseTracker, Replay, execute_command};
use mnesis_inmemory::{InMemoryAllPos, InMemoryStore, InMemoryStoreError};
use mnesis_store::{Decode, Encode, EventStore, PersistedEnvelope, Repository, Store, StoreError};
use parking_lot::Mutex;
use tokio::sync::{Barrier, Notify};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CounterId([u8; 8]);

impl CounterId {
    const fn new(value: u64) -> Self {
        Self(value.to_le_bytes())
    }
}

impl fmt::Display for CounterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", u64::from_le_bytes(self.0))
    }
}

impl AsRef<[u8]> for CounterId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CounterEvent {
    Added(u64),
}

impl Message for CounterEvent {}

impl DomainEvent for CounterEvent {
    fn name(&self) -> &'static str {
        "Added"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterState {
    total: u64,
}

impl AggregateState for CounterState {
    type Event = CounterEvent;

    fn initial() -> Self {
        Self { total: 0 }
    }

    fn apply(mut self, event: &Self::Event) -> Self {
        let CounterEvent::Added(value) = event;
        self.total += value;
        self
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum CounterError {
    #[error("cannot add zero")]
    Zero,
}

struct Counter;

impl Aggregate for Counter {
    type State = CounterState;
    type Error = CounterError;
    type Id = CounterId;
}

struct Add(u64);

impl Handle<Add> for Counter {
    fn handle(
        _state: &CounterState,
        command: Add,
    ) -> Result<Option<Events<CounterEvent>>, CounterError> {
        if command.0 == 0 {
            return Err(CounterError::Zero);
        }
        Ok(Some(events![CounterEvent::Added(command.0)]))
    }
}

struct RaiseTo(u64);

impl Handle<RaiseTo> for Counter {
    fn handle(
        state: &CounterState,
        command: RaiseTo,
    ) -> Result<Option<Events<CounterEvent>>, CounterError> {
        match command.0.checked_sub(state.total) {
            None | Some(0) => Ok(None),
            Some(delta) => Ok(Some(events![CounterEvent::Added(delta)])),
        }
    }
}

struct PanicWhileDeciding;

impl Handle<PanicWhileDeciding> for Counter {
    fn handle(
        _state: &CounterState,
        _command: PanicWhileDeciding,
    ) -> Result<Option<Events<CounterEvent>>, CounterError> {
        panic!("injected decision panic")
    }
}

struct CounterCodec;

impl Encode<CounterEvent> for CounterCodec {
    type Error = Infallible;

    fn encode(&self, event: &CounterEvent) -> Result<Bytes, Self::Error> {
        let CounterEvent::Added(value) = event;
        Ok(Bytes::copy_from_slice(&value.to_le_bytes()))
    }
}

impl Decode<CounterEvent> for CounterCodec {
    type Output<'a> = CounterEvent;
    type Error = Infallible;

    fn decode<'a>(
        &'a self,
        envelope: &'a PersistedEnvelope,
    ) -> Result<Self::Output<'a>, Self::Error> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(envelope.payload());
        Ok(CounterEvent::Added(u64::from_le_bytes(bytes)))
    }
}

type CounterRepository = EventStore<InMemoryStore, CounterCodec, Counter>;
type CounterStoreError = StoreError<InMemoryStoreError, Infallible, Infallible>;

fn repository() -> CounterRepository {
    Store::new(InMemoryStore::new())
        .repository()
        .codec(CounterCodec)
        .build()
}

fn output(root: &AggregateRoot<Counter>) -> u64 {
    root.state().total
}

fn definite(_error: &CounterStoreError) -> CommitFailure {
    CommitFailure::NotCommitted
}

#[derive(Debug, Clone, Copy)]
enum SaveAction {
    Pass,
    Conflict,
    Fail,
}

struct ScriptedRepository {
    inner: CounterRepository,
    actions: Mutex<VecDeque<SaveAction>>,
    loads: AtomicU32,
    saves: AtomicU32,
}

impl ScriptedRepository {
    fn new(actions: impl IntoIterator<Item = SaveAction>) -> Self {
        Self {
            inner: repository(),
            actions: Mutex::new(actions.into_iter().collect()),
            loads: AtomicU32::new(0),
            saves: AtomicU32::new(0),
        }
    }
}

impl Repository<Counter> for ScriptedRepository {
    type Error = CounterStoreError;
    type Position = InMemoryAllPos;

    async fn load(&self, id: CounterId) -> Result<AggregateRoot<Counter>, Self::Error> {
        self.loads.fetch_add(1, Ordering::Relaxed);
        self.inner.load(id).await
    }

    async fn save<const N: usize>(
        &self,
        aggregate: &mut AggregateRoot<Counter>,
        events: &Events<EventOf<Counter>, N>,
    ) -> Result<Self::Position, Self::Error> {
        self.saves.fetch_add(1, Ordering::Relaxed);
        let action = self.actions.lock().pop_front().unwrap_or(SaveAction::Pass);
        match action {
            SaveAction::Pass => self.inner.save(aggregate, events).await,
            SaveAction::Conflict => Err(StoreError::Conflict {
                stream_id: ErrorId::from_display(aggregate.id()),
                expected: aggregate.version(),
                actual: aggregate.version(),
            }),
            SaveAction::Fail => Err(StoreError::Adapter(InMemoryStoreError::CorruptVersion)),
        }
    }
}

struct BlockingSaveRepository {
    inner: CounterRepository,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

struct BlockingLoadRepository {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl Repository<Counter> for BlockingLoadRepository {
    type Error = CounterStoreError;
    type Position = InMemoryAllPos;

    async fn load(&self, _id: CounterId) -> Result<AggregateRoot<Counter>, Self::Error> {
        self.entered.notify_one();
        self.release.notified().await;
        unreachable!("the loading-cancellation test never releases the repository")
    }

    async fn save<const N: usize>(
        &self,
        _aggregate: &mut AggregateRoot<Counter>,
        _events: &Events<EventOf<Counter>, N>,
    ) -> Result<Self::Position, Self::Error> {
        unreachable!("a blocked load cannot reach save")
    }
}

impl Repository<Counter> for BlockingSaveRepository {
    type Error = CounterStoreError;
    type Position = InMemoryAllPos;

    async fn load(&self, id: CounterId) -> Result<AggregateRoot<Counter>, Self::Error> {
        self.inner.load(id).await
    }

    async fn save<const N: usize>(
        &self,
        aggregate: &mut AggregateRoot<Counter>,
        events: &Events<EventOf<Counter>, N>,
    ) -> Result<Self::Position, Self::Error> {
        self.entered.notify_one();
        self.release.notified().await;
        self.inner.save(aggregate, events).await
    }
}

struct FailingLoadRepository;

impl Repository<Counter> for FailingLoadRepository {
    type Error = CounterStoreError;
    type Position = InMemoryAllPos;

    async fn load(&self, _id: CounterId) -> Result<AggregateRoot<Counter>, Self::Error> {
        Err(StoreError::Adapter(InMemoryStoreError::CorruptVersion))
    }

    async fn save<const N: usize>(
        &self,
        _aggregate: &mut AggregateRoot<Counter>,
        _events: &Events<EventOf<Counter>, N>,
    ) -> Result<Self::Position, Self::Error> {
        unreachable!("a load failure cannot reach save")
    }
}

struct BarrierRepository {
    inner: CounterRepository,
    saves_ready: Arc<Barrier>,
}

impl Repository<Counter> for BarrierRepository {
    type Error = CounterStoreError;
    type Position = InMemoryAllPos;

    async fn load(&self, id: CounterId) -> Result<AggregateRoot<Counter>, Self::Error> {
        self.inner.load(id).await
    }

    async fn save<const N: usize>(
        &self,
        aggregate: &mut AggregateRoot<Counter>,
        events: &Events<EventOf<Counter>, N>,
    ) -> Result<Self::Position, Self::Error> {
        self.saves_ready.wait().await;
        self.inner.save(aggregate, events).await
    }
}

#[tokio::test]
async fn mnesis_example_domain_commits_and_returns_read_your_writes_position() {
    let repository = repository();
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(1),
        101_u64,
        Once::new(Add(7)),
        output,
        definite,
        &phases,
    )
    .await;

    let CommandOutcome::Committed {
        position,
        output: total,
    } = outcome
    else {
        panic!("expected committed outcome")
    };
    assert_eq!(position, InMemoryAllPos::INITIAL);
    assert_eq!(total, 7);
    assert_eq!(phases.phase(), CommandPhase::AppendCommitted);
    assert_eq!(
        phases.phase().interruption_fact(),
        InterruptionFact::Committed
    );
    assert_eq!(
        repository
            .load(CounterId::new(1))
            .await
            .unwrap()
            .state()
            .total,
        7
    );
}

#[tokio::test]
async fn accepted_noop_is_ignored_without_append_or_position() {
    let repository = repository();
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(2),
        102_u64,
        Once::new(RaiseTo(0)),
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(outcome, CommandOutcome::Ignored { output: 0 }));
    assert_eq!(phases.phase(), CommandPhase::AppendNotCommitted);
    assert_eq!(
        repository.load(CounterId::new(2)).await.unwrap().version(),
        None
    );
}

#[tokio::test]
async fn domain_rejection_is_preserved_and_never_reaches_save() {
    let repository = ScriptedRepository::new([]);
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(3),
        103_u64,
        Once::new(Add(0)),
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(
        outcome,
        CommandOutcome::Rejected(CounterError::Zero)
    ));
    assert_eq!(repository.saves.load(Ordering::Relaxed), 0);
    assert_eq!(phases.phase(), CommandPhase::AppendNotCommitted);
}

#[tokio::test]
async fn non_replayable_command_exhausts_on_first_confirmed_conflict() {
    let repository = ScriptedRepository::new([SaveAction::Conflict]);
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(4),
        104_u64,
        Once::new(Add(1)),
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(
        outcome,
        CommandOutcome::ConflictExhausted { attempts, .. }
            if attempts == NonZeroU32::MIN
    ));
    assert_eq!(repository.loads.load(Ordering::Relaxed), 1);
    assert_eq!(repository.saves.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn replayable_command_reloads_and_redecides_within_exact_budget() {
    let repository =
        ScriptedRepository::new([SaveAction::Conflict, SaveAction::Conflict, SaveAction::Pass]);
    let phases = PhaseTracker::new();
    let made = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&made);
    let attempts = Replay::new(NonZeroU32::new(3).unwrap(), move |attempt: NonZeroU32| {
        observed.lock().push(attempt.get());
        Add(2)
    });
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(5),
        105_u64,
        attempts,
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(
        outcome,
        CommandOutcome::Committed { output: 2, .. }
    ));
    assert_eq!(*made.lock(), vec![1, 2, 3]);
    assert_eq!(repository.loads.load(Ordering::Relaxed), 3);
    assert_eq!(repository.saves.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn definite_append_failure_is_storage_and_is_never_retried() {
    let repository = ScriptedRepository::new([SaveAction::Fail, SaveAction::Pass]);
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(6),
        106_u64,
        Replay::new(NonZeroU32::new(2).unwrap(), |_| Add(3)),
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(outcome, CommandOutcome::Storage(_)));
    assert_eq!(repository.saves.load(Ordering::Relaxed), 1);
    assert_eq!(phases.phase(), CommandPhase::AppendNotCommitted);
}

#[tokio::test]
async fn uncertain_append_failure_returns_command_identity_without_retry() {
    let repository = ScriptedRepository::new([SaveAction::Fail, SaveAction::Pass]);
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &repository,
        CounterId::new(7),
        0xA11_u64,
        Replay::new(NonZeroU32::new(2).unwrap(), |_| Add(4)),
        output,
        |_| CommitFailure::Ambiguous,
        &phases,
    )
    .await;

    assert!(matches!(
        outcome,
        CommandOutcome::AmbiguousCompletion { command_id: 0xA11 }
    ));
    assert_eq!(repository.saves.load(Ordering::Relaxed), 1);
    assert_eq!(phases.phase(), CommandPhase::AppendInFlight);
    assert_eq!(
        phases.phase().interruption_fact(),
        InterruptionFact::Ambiguous
    );
}

#[tokio::test]
async fn load_failure_is_storage_with_proof_append_never_started() {
    let phases = PhaseTracker::new();
    let outcome = execute_command::<Counter, _, _, _, _, _, _, _, 0>(
        &FailingLoadRepository,
        CounterId::new(8),
        108_u64,
        Once::new(Add(1)),
        output,
        definite,
        &phases,
    )
    .await;

    assert!(matches!(outcome, CommandOutcome::Storage(_)));
    assert_eq!(phases.phase(), CommandPhase::AppendNotCommitted);
}

#[tokio::test]
async fn cancellation_during_save_leaves_externally_observable_ambiguity() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let repository = Arc::new(BlockingSaveRepository {
        inner: repository(),
        entered: Arc::clone(&entered),
        release,
    });
    let phases = PhaseTracker::new();
    let observed_phases = phases.clone();
    let task = tokio::spawn(async move {
        execute_command::<Counter, _, _, _, _, _, _, _, 0>(
            repository.as_ref(),
            CounterId::new(9),
            109_u64,
            Once::new(Add(1)),
            output,
            definite,
            &observed_phases,
        )
        .await
    });

    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(phases.phase(), CommandPhase::AppendInFlight);
    assert_eq!(
        phases.phase().interruption_fact(),
        InterruptionFact::Ambiguous
    );
}

#[tokio::test]
async fn cancellation_during_load_proves_append_never_started() {
    let entered = Arc::new(Notify::new());
    let repository = Arc::new(BlockingLoadRepository {
        entered: Arc::clone(&entered),
        release: Arc::new(Notify::new()),
    });
    let phases = PhaseTracker::new();
    let observed_phases = phases.clone();
    let task = tokio::spawn(async move {
        execute_command::<Counter, _, _, _, _, _, _, _, 0>(
            repository.as_ref(),
            CounterId::new(12),
            112_u64,
            Once::new(Add(1)),
            output,
            definite,
            &observed_phases,
        )
        .await
    });

    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(phases.phase(), CommandPhase::Loading);
    assert_eq!(
        phases.phase().interruption_fact(),
        InterruptionFact::NotAppended
    );
}

#[tokio::test]
async fn panic_during_pure_decision_proves_append_never_started() {
    let repository = Arc::new(repository());
    let phases = PhaseTracker::new();
    let observed_phases = phases.clone();
    let task = tokio::spawn(async move {
        execute_command::<Counter, _, _, _, _, _, _, _, 0>(
            repository.as_ref(),
            CounterId::new(10),
            110_u64,
            Once::new(PanicWhileDeciding),
            output,
            definite,
            &observed_phases,
        )
        .await
    });

    assert!(task.await.unwrap_err().is_panic());
    assert_eq!(phases.phase(), CommandPhase::Deciding);
    assert_eq!(
        phases.phase().interruption_fact(),
        InterruptionFact::NotAppended
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_direct_writers_preserve_single_winner_semantics() {
    let repository = Arc::new(BarrierRepository {
        inner: repository(),
        saves_ready: Arc::new(Barrier::new(2)),
    });
    let tasks = [11_u64, 12].map(|command_id| {
        let repository = Arc::clone(&repository);
        tokio::spawn(async move {
            execute_command::<Counter, _, _, _, _, _, _, _, 0>(
                repository.as_ref(),
                CounterId::new(11),
                command_id,
                Once::new(Add(command_id)),
                output,
                definite,
                &PhaseTracker::new(),
            )
            .await
        })
    });
    let results = futures_join(tasks).await;
    let committed = results
        .iter()
        .filter(|outcome| matches!(outcome, CommandOutcome::Committed { .. }))
        .count();
    let conflicts = results
        .iter()
        .filter(|outcome| matches!(outcome, CommandOutcome::ConflictExhausted { .. }))
        .count();
    assert_eq!(committed, 1);
    assert_eq!(conflicts, 1);
}

async fn futures_join<T>(tasks: [tokio::task::JoinHandle<T>; 2]) -> [T; 2] {
    let [first, second] = tasks;
    [first.await.unwrap(), second.await.unwrap()]
}
