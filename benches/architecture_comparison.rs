//! Positive cross-crate probe: current public APIs permit a Behavior to own a
//! repository, retain aggregate identity, and await durable command execution.
//! This is a mechanical-possibility probe, not an architecture endorsement.

use std::collections::{HashMap, hash_map::Entry};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actorpass::{
    ActorRef, DeliveryRouter, EndpointRegistry, IncarnationEndpoint, MailboxConfig, RunExit,
    System, TaskOutcome,
};
use behavior::{Actions, Behavior, Delivery, Exit, MailAddr, Never, NoBirths, Recipient, User};
use bytes::Bytes;
use mnesis::{
    Aggregate, AggregateRoot, AggregateState, DomainEvent, Events, Handle, Message, events,
};
use mnesis_inmemory::InMemoryStore;
use mnesis_store::{
    CommandRepository, Decode, Encode, EventStore, Execution, PersistedEnvelope, Repository, Store,
};
use tokio::sync::{Mutex, oneshot};
use tower::{Service, ServiceExt, service_fn};

type AggregateShard = Mutex<HashMap<MailAddr, AggregateRoot<Counter>>>;
type AggregateShards = Arc<Vec<AggregateShard>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CounterId([u8; 8]);

impl CounterId {
    fn new(value: u64) -> Self {
        Self(value.to_le_bytes())
    }
}

impl core::fmt::Display for CounterId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", u64::from_le_bytes(self.0))
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
    fn apply(mut self, event: &CounterEvent) -> Self {
        let CounterEvent::Added(amount) = event;
        self.total += amount;
        self
    }
}

#[derive(Debug)]
struct CounterError;
impl core::fmt::Display for CounterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("counter error")
    }
}
impl std::error::Error for CounterError {}

struct Counter;
impl Aggregate for Counter {
    type State = CounterState;
    type Error = CounterError;
    type Id = CounterId;
}

#[derive(Debug, Clone, Copy)]
struct Add(u64);

#[derive(Debug)]
enum ActorCommand {
    Add {
        command: Add,
        committed: Option<oneshot::Sender<()>>,
    },
    Stop,
}

impl Handle<Add> for Counter {
    fn handle(
        _: &CounterState,
        command: Add,
    ) -> Result<Option<Events<CounterEvent>>, CounterError> {
        Ok(Some(events![CounterEvent::Added(command.0)]))
    }
}

struct CounterCodec;
impl Encode<CounterEvent> for CounterCodec {
    type Error = Infallible;
    fn encode(&self, event: &CounterEvent) -> Result<Bytes, Self::Error> {
        let CounterEvent::Added(amount) = event;
        Ok(Bytes::copy_from_slice(&amount.to_le_bytes()))
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
        bytes.copy_from_slice(&envelope.payload()[..8]);
        Ok(CounterEvent::Added(u64::from_le_bytes(bytes)))
    }
}

type CounterRepository = EventStore<InMemoryStore, CounterCodec, Counter>;

fn repository() -> Arc<CounterRepository> {
    Arc::new(
        Store::new(InMemoryStore::new())
            .repository()
            .codec(CounterCodec)
            .build(),
    )
}

#[derive(Debug)]
struct DurableBehaviorError;

struct ExecuteRequest {
    command: Add,
    committed: Option<oneshot::Sender<()>>,
}

struct PureCommandBehavior;

impl Behavior for PureCommandBehavior {
    type Addr = MailAddr;
    type Msg = ActorCommand;
    type Event = User<MailAddr, ActorCommand>;
    type Sends = Vec<Delivery<MailAddr, ExecuteRequest>>;
    type Ph = Never;
    type Error = DurableBehaviorError;
    type Birth = NoBirths;

    async fn init(&mut self) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    async fn step(&mut self, event: Self::Event) -> behavior::BehaviorActed<Self> {
        match event.message {
            ActorCommand::Add { command, committed } => {
                let mut actions: Actions<Self::Addr, Self::Ph, Self::Sends, Self::Birth> =
                    Actions::cont();
                actions.sends.push(Delivery::new(
                    Recipient::global(MailAddr(0)),
                    ExecuteRequest { command, committed },
                ));
                Ok(actions)
            }
            ActorCommand::Stop => Ok(Actions::stop(Exit::Normal)),
        }
    }
}

/// This value owns both identity and repository access. Its error is deliberately
/// not `Infallible`, while its phase type remains `Never`.
struct DurableBehavior {
    id: CounterId,
    root: Option<AggregateRoot<Counter>>,
    repository: Arc<CounterRepository>,
}

impl Behavior for DurableBehavior {
    type Addr = MailAddr;
    type Msg = ActorCommand;
    type Event = User<MailAddr, ActorCommand>;
    type Sends = Vec<Delivery<MailAddr, ActorCommand>>;
    type Ph = Never;
    type Error = DurableBehaviorError;
    type Birth = NoBirths;

    async fn init(&mut self) -> behavior::BehaviorActed<Self> {
        // `&mut self` permits an immutable reborrow and Arc clone. The returned
        // repository future is awaited directly by the Behavior future.
        let repository = Arc::clone(&self.repository);
        self.root = repository.load(self.id.clone()).await.ok();
        Ok(Actions::cont())
    }

    async fn step(&mut self, event: Self::Event) -> behavior::BehaviorActed<Self> {
        let ActorCommand::Add { command, committed } = event.message else {
            return Ok(Actions::stop(Exit::Normal));
        };
        let repository = Arc::clone(&self.repository);
        let root = self.root.as_mut().ok_or(DurableBehaviorError)?;
        match repository.execute(root, command).await {
            Ok(Execution::Executed { .. } | Execution::Ignored) => {
                if let Some(committed) = committed {
                    let _ = committed.send(());
                }
                Ok(Actions::cont())
            }
            Err(_) => Err(DurableBehaviorError),
        }
    }
}

fn assert_send_static<T: Send + 'static>(_: &T) {}

#[derive(Clone, Copy)]
struct ProbeRouter;

impl DeliveryRouter<MailAddr, ActorCommand> for ProbeRouter {
    type Error = Infallible;

    async fn deliver(
        &self,
        _from: MailAddr,
        _delivery: Delivery<MailAddr, ActorCommand>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone)]
struct ServiceRouter {
    repository: Arc<CounterRepository>,
    roots: AggregateShards,
}

impl ServiceRouter {
    fn new(repository: Arc<CounterRepository>) -> Self {
        Self {
            repository,
            roots: Arc::new((0..64).map(|_| Mutex::new(HashMap::new())).collect()),
        }
    }
}

impl DeliveryRouter<MailAddr, ExecuteRequest> for ServiceRouter {
    type Error = Infallible;

    async fn deliver(
        &self,
        from: MailAddr,
        delivery: Delivery<MailAddr, ExecuteRequest>,
    ) -> Result<(), Self::Error> {
        let ExecuteRequest { command, committed } = delivery.message;
        let shard_index =
            usize::try_from(from.0 % self.roots.len() as u64).expect("shard index fits usize");
        let mut shard = self.roots[shard_index].lock().await;
        if let Entry::Vacant(slot) = shard.entry(from) {
            let loaded = self
                .repository
                .load(CounterId::new(from.0))
                .await
                .expect("service aggregate loads");
            slot.insert(loaded);
        }
        let _ = self
            .repository
            .execute(shard.get_mut(&from).expect("root initialized"), command)
            .await
            .expect("service execution succeeds");
        if let Some(committed) = committed {
            let _ = committed.send(());
        }
        Ok(())
    }
}

impl<S>
    EndpointRegistry<MailAddr, ActorCommand, IncarnationEndpoint<MailAddr, ActorRef<MailAddr, S>>>
    for ServiceRouter
{
    type Error = Infallible;
    type Registration = ();

    fn register(
        &self,
        _address: MailAddr,
        _endpoint: IncarnationEndpoint<MailAddr, ActorRef<MailAddr, S>>,
    ) -> Result<Self::Registration, Self::Error> {
        Ok(())
    }
}

fn command_service(
    repository: Arc<CounterRepository>,
) -> impl Service<(MailAddr, ExecuteRequest), Response = (), Error = Infallible, Future: Send>
+ Clone
+ Send
+ Sync {
    let roots: AggregateShards = Arc::new((0..64).map(|_| Mutex::new(HashMap::new())).collect());
    service_fn(move |(id, request): (MailAddr, ExecuteRequest)| {
        let repository = Arc::clone(&repository);
        let roots = Arc::clone(&roots);
        async move {
            let shard_index =
                usize::try_from(id.0 % roots.len() as u64).expect("shard index fits usize");
            let mut shard = roots[shard_index].lock().await;
            if let Entry::Vacant(slot) = shard.entry(id) {
                let loaded = repository
                    .load(CounterId::new(id.0))
                    .await
                    .expect("tower service aggregate loads");
                slot.insert(loaded);
            }
            let _ = repository
                .execute(
                    shard.get_mut(&id).expect("root initialized"),
                    request.command,
                )
                .await
                .expect("tower service execution succeeds");
            if let Some(committed) = request.committed {
                let _ = committed.send(());
            }
            Ok(())
        }
    })
}

#[derive(Clone)]
struct TowerRouter<S> {
    service: S,
}

impl<S> DeliveryRouter<MailAddr, ExecuteRequest> for TowerRouter<S>
where
    S: Service<(MailAddr, ExecuteRequest), Response = (), Error = Infallible> + Clone + Send + Sync,
    S::Future: Send,
{
    type Error = Infallible;

    async fn deliver(
        &self,
        from: MailAddr,
        delivery: Delivery<MailAddr, ExecuteRequest>,
    ) -> Result<(), Self::Error> {
        self.service.clone().oneshot((from, delivery.message)).await
    }
}

impl<S, Sender>
    EndpointRegistry<
        MailAddr,
        ActorCommand,
        IncarnationEndpoint<MailAddr, ActorRef<MailAddr, Sender>>,
    > for TowerRouter<S>
{
    type Error = Infallible;
    type Registration = ();

    fn register(
        &self,
        _address: MailAddr,
        _endpoint: IncarnationEndpoint<MailAddr, ActorRef<MailAddr, Sender>>,
    ) -> Result<Self::Registration, Self::Error> {
        Ok(())
    }
}

impl<S>
    EndpointRegistry<MailAddr, ActorCommand, IncarnationEndpoint<MailAddr, ActorRef<MailAddr, S>>>
    for ProbeRouter
{
    type Error = Infallible;
    type Registration = ();

    fn register(
        &self,
        _address: MailAddr,
        _endpoint: IncarnationEndpoint<MailAddr, ActorRef<MailAddr, S>>,
    ) -> Result<Self::Registration, Self::Error> {
        Ok(())
    }
}

async fn run_direct(iterations: u64) -> Duration {
    let repository = repository();
    let mut root = repository
        .load(CounterId::new(1))
        .await
        .expect("initial aggregate loads");
    let started = Instant::now();
    for _ in 0..iterations {
        let _ = repository
            .execute(&mut root, Add(1))
            .await
            .expect("direct execution succeeds");
    }
    let elapsed = started.elapsed();
    assert_eq!(root.state().total, iterations);
    elapsed
}

async fn run_actor(iterations: u64) -> Duration {
    let repository = repository();
    let behavior = DurableBehavior {
        id: CounterId::new(1),
        root: None,
        repository: Arc::clone(&repository),
    };
    let system = System::new(MailboxConfig::bounded(1024), ProbeRouter);
    let handle = system.spawn(MailAddr(1), behavior).expect("vacant address");
    let started = Instant::now();
    for _ in 0..iterations {
        handle
            .actor_ref()
            .send(
                MailAddr(2),
                ActorCommand::Add {
                    command: Add(1),
                    committed: None,
                },
            )
            .await
            .expect("mailbox accepts command");
    }
    handle
        .actor_ref()
        .send(MailAddr(2), ActorCommand::Stop)
        .await
        .expect("mailbox accepts stop");
    assert!(matches!(
        handle.outcome().await,
        TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
    ));
    let elapsed = started.elapsed();
    let reloaded = repository
        .load(CounterId::new(1))
        .await
        .expect("committed aggregate reloads");
    assert_eq!(reloaded.state().total, iterations);
    elapsed
}

async fn run_actor_roundtrip(iterations: u64) -> Duration {
    let repository = repository();
    let behavior = DurableBehavior {
        id: CounterId::new(1),
        root: None,
        repository: Arc::clone(&repository),
    };
    let system = System::new(MailboxConfig::bounded(1024), ProbeRouter);
    let handle = system.spawn(MailAddr(1), behavior).expect("vacant address");
    let started = Instant::now();
    for _ in 0..iterations {
        let (committed, receipt) = oneshot::channel();
        handle
            .actor_ref()
            .send(
                MailAddr(2),
                ActorCommand::Add {
                    command: Add(1),
                    committed: Some(committed),
                },
            )
            .await
            .expect("mailbox accepts command");
        receipt.await.expect("durable command receipt arrives");
    }
    handle
        .actor_ref()
        .send(MailAddr(2), ActorCommand::Stop)
        .await
        .expect("mailbox accepts stop");
    assert!(matches!(
        handle.outcome().await,
        TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
    ));
    let elapsed = started.elapsed();
    let reloaded = repository
        .load(CounterId::new(1))
        .await
        .expect("committed aggregate reloads");
    assert_eq!(reloaded.state().total, iterations);
    elapsed
}

async fn run_service_router(iterations: u64, roundtrip: bool) -> Duration {
    let repository = repository();
    let router = ServiceRouter::new(Arc::clone(&repository));
    let system = System::new(MailboxConfig::bounded(1024), router);
    let handle = system
        .spawn(MailAddr(1), PureCommandBehavior)
        .expect("vacant address");
    let started = Instant::now();
    for _ in 0..iterations {
        let (committed, receipt) = oneshot::channel();
        handle
            .actor_ref()
            .send(
                MailAddr(2),
                ActorCommand::Add {
                    command: Add(1),
                    committed: roundtrip.then_some(committed),
                },
            )
            .await
            .expect("mailbox accepts command");
        if roundtrip {
            receipt.await.expect("durable command receipt arrives");
        }
    }
    handle
        .actor_ref()
        .send(MailAddr(2), ActorCommand::Stop)
        .await
        .expect("mailbox accepts stop");
    assert!(matches!(
        handle.outcome().await,
        TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
    ));
    let elapsed = started.elapsed();
    let reloaded = repository
        .load(CounterId::new(1))
        .await
        .expect("committed aggregate reloads");
    assert_eq!(reloaded.state().total, iterations);
    elapsed
}

async fn run_service_router_multi(
    actor_count: u64,
    commands_per_actor: u64,
    roundtrip: bool,
) -> Duration {
    let repository = repository();
    let router = ServiceRouter::new(Arc::clone(&repository));
    let system = System::new(MailboxConfig::bounded(1024), router);
    let mut handles = Vec::new();
    for address in 1..=actor_count {
        handles.push(
            system
                .spawn(MailAddr(address), PureCommandBehavior)
                .expect("vacant address"),
        );
    }

    let started = Instant::now();
    for _ in 0..commands_per_actor {
        for (index, handle) in handles.iter().enumerate() {
            let (committed, receipt) = oneshot::channel();
            handle
                .actor_ref()
                .send(
                    MailAddr(0),
                    ActorCommand::Add {
                        command: Add(1),
                        committed: roundtrip.then_some(committed),
                    },
                )
                .await
                .unwrap_or_else(|_| panic!("actor {index} accepts command"));
            if roundtrip {
                receipt.await.expect("durable command receipt arrives");
            }
        }
    }
    for handle in &handles {
        handle
            .actor_ref()
            .send(MailAddr(0), ActorCommand::Stop)
            .await
            .expect("mailbox accepts stop");
    }
    for handle in handles {
        assert!(matches!(
            handle.outcome().await,
            TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
        ));
    }
    let elapsed = started.elapsed();
    for address in 1..=actor_count {
        let reloaded = repository
            .load(CounterId::new(address))
            .await
            .expect("committed aggregate reloads");
        assert_eq!(reloaded.state().total, commands_per_actor);
    }
    elapsed
}

async fn run_tower_router(iterations: u64, roundtrip: bool) -> Duration {
    let repository = repository();
    let router = TowerRouter {
        service: command_service(Arc::clone(&repository)),
    };
    let system = System::new(MailboxConfig::bounded(1024), router);
    let handle = system
        .spawn(MailAddr(1), PureCommandBehavior)
        .expect("vacant address");
    let started = Instant::now();
    for _ in 0..iterations {
        let (committed, receipt) = oneshot::channel();
        handle
            .actor_ref()
            .send(
                MailAddr(2),
                ActorCommand::Add {
                    command: Add(1),
                    committed: roundtrip.then_some(committed),
                },
            )
            .await
            .expect("mailbox accepts command");
        if roundtrip {
            receipt.await.expect("durable command receipt arrives");
        }
    }
    handle
        .actor_ref()
        .send(MailAddr(2), ActorCommand::Stop)
        .await
        .expect("mailbox accepts stop");
    assert!(matches!(
        handle.outcome().await,
        TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
    ));
    let elapsed = started.elapsed();
    let reloaded = repository
        .load(CounterId::new(1))
        .await
        .expect("committed aggregate reloads");
    assert_eq!(reloaded.state().total, iterations);
    elapsed
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    if let Some(iterations) = std::env::var("BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let direct = run_direct(iterations).await;
        let actor_pipeline = run_actor(iterations).await;
        let actor_roundtrip = run_actor_roundtrip(iterations).await;
        let service_pipeline = run_service_router(iterations, false).await;
        let service_roundtrip = run_service_router(iterations, true).await;
        let tower_pipeline = run_tower_router(iterations, false).await;
        let tower_roundtrip = run_tower_router(iterations, true).await;
        println!(
            "iterations={iterations} direct_ns_per_op={} actor_pipeline_ns_per_op={} actor_roundtrip_ns_per_op={} service_pipeline_ns_per_op={} service_roundtrip_ns_per_op={} tower_pipeline_ns_per_op={} tower_roundtrip_ns_per_op={} actor_pipeline_ratio={:.3} actor_roundtrip_ratio={:.3} service_pipeline_ratio={:.3} service_roundtrip_ratio={:.3} tower_pipeline_ratio={:.3} tower_roundtrip_ratio={:.3}",
            direct.as_nanos() / u128::from(iterations),
            actor_pipeline.as_nanos() / u128::from(iterations),
            actor_roundtrip.as_nanos() / u128::from(iterations),
            service_pipeline.as_nanos() / u128::from(iterations),
            service_roundtrip.as_nanos() / u128::from(iterations),
            tower_pipeline.as_nanos() / u128::from(iterations),
            tower_roundtrip.as_nanos() / u128::from(iterations),
            actor_pipeline.as_secs_f64() / direct.as_secs_f64(),
            actor_roundtrip.as_secs_f64() / direct.as_secs_f64(),
            service_pipeline.as_secs_f64() / direct.as_secs_f64(),
            service_roundtrip.as_secs_f64() / direct.as_secs_f64(),
            tower_pipeline.as_secs_f64() / direct.as_secs_f64(),
            tower_roundtrip.as_secs_f64() / direct.as_secs_f64(),
        );
        return;
    }

    if let Some(actor_count) = std::env::var("BENCH_ACTORS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        let commands_per_actor = std::env::var("BENCH_COMMANDS_PER_ACTOR")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1_000);
        let total = actor_count * commands_per_actor;
        let pipeline = run_service_router_multi(actor_count, commands_per_actor, false).await;
        let roundtrip = run_service_router_multi(actor_count, commands_per_actor, true).await;
        println!(
            "actors={actor_count} commands_per_actor={commands_per_actor} total={total} service_pipeline_ns_per_op={} service_roundtrip_ns_per_op={}",
            pipeline.as_nanos() / u128::from(total),
            roundtrip.as_nanos() / u128::from(total),
        );
        return;
    }

    let repository = repository();
    let behavior = DurableBehavior {
        id: CounterId::new(1),
        root: None,
        repository: Arc::clone(&repository),
    };
    assert_send_static(&behavior);

    let system = System::new(MailboxConfig::bounded(4), ProbeRouter);
    let handle = system.spawn(MailAddr(1), behavior).expect("vacant address");
    handle
        .actor_ref()
        .send(
            MailAddr(2),
            ActorCommand::Add {
                command: Add(7),
                committed: None,
            },
        )
        .await
        .expect("mailbox accepts command");
    handle
        .actor_ref()
        .send(MailAddr(2), ActorCommand::Stop)
        .await
        .expect("mailbox accepts stop");

    assert!(matches!(
        handle.outcome().await,
        TaskOutcome::Returned(Ok(RunExit::Stopped(Exit::Normal)))
    ));

    let reloaded = repository
        .load(CounterId::new(1))
        .await
        .expect("committed aggregate reloads");
    assert_eq!(reloaded.state().total, 7);
}
