//! Adversarial executable audit of the public Bombay–Entity runtime seam.

use std::collections::HashSet;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use behavior::{
    Actions, Behavior, Delivery, Exit, MailAddr, Never, NoBirths, Recipient, ShutdownEvent,
    ShutdownRequested, TimeEvent, TimerElapsed, User, UserEvent,
};
use bombay::{
    BehaviorRetirement, DeliveryEndpoint, DeliveryRouter, EndpointRegistry, MailboxConfig,
    RootEndpoint, System, TaskOutcome,
};
use bombay_entity::{
    Activated, ActivationId, DrainFailure, DrainFenceAcknowledged, DrainStage, EntityBehavior,
    EntityId, EntityProtocol, FenceFailure, LocalEntityRuntime, RetirementMode,
};
use parking_lot::Mutex;

#[derive(Debug, PartialEq, Eq)]
struct Command(u64);

enum ProbeEvent {
    User(User<MailAddr, Command>),
    Shutdown,
}

impl UserEvent for ProbeEvent {
    type Addr = MailAddr;
    type Message = Command;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::User(user) => Ok(user),
            Self::Shutdown => Err(Self::Shutdown),
        }
    }
}

impl ShutdownEvent for ProbeEvent {
    fn shutdown_requested(_: ShutdownRequested) -> Option<Self> {
        Some(Self::Shutdown)
    }
}

impl TimeEvent for ProbeEvent {
    fn time_reached(_: TimerElapsed) -> Option<Self> {
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
struct InitializationFailed;

struct ProbeBehavior {
    initialized: Arc<AtomicBool>,
    processed: Arc<Mutex<Vec<u64>>>,
    fail_initialization: bool,
}

impl Behavior for ProbeBehavior {
    type Addr = MailAddr;
    type Msg = Command;
    type Event = ProbeEvent;
    type Sends = Vec<Delivery<MailAddr, Command>>;
    type Ph = Never;
    type Error = InitializationFailed;
    type Birth = NoBirths;

    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        if self.fail_initialization {
            return Err(InitializationFailed);
        }
        self.initialized.store(true, Ordering::SeqCst);
        Ok(Actions::cont())
    }

    fn transition(&mut self, event: Self::Event) -> behavior::BehaviorActed<Self> {
        match event {
            ProbeEvent::User(user) => {
                self.processed.lock().push(user.message.0);
                Ok(Actions::cont())
            }
            ProbeEvent::Shutdown => Ok(Actions::stop(Exit::Normal)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistrationFailure {
    Rejected,
    Occupied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcknowledgementMode {
    Send,
    Drop,
}

type FenceAcknowledgement = Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>;

struct ProbeRegistration {
    address: MailAddr,
    active: Arc<Mutex<HashSet<MailAddr>>>,
}

impl Drop for ProbeRegistration {
    fn drop(&mut self) {
        assert!(self.active.lock().remove(&self.address));
    }
}

#[derive(Clone)]
struct ProbeRouter {
    registered: Arc<AtomicBool>,
    initialized: Arc<AtomicBool>,
    reject_registration: Arc<AtomicBool>,
    active: Arc<Mutex<HashSet<MailAddr>>>,
    acknowledgement_mode: Arc<Mutex<AcknowledgementMode>>,
    fence_acknowledgement: FenceAcknowledgement,
}

impl ProbeRouter {
    fn new(initialized: Arc<AtomicBool>) -> Self {
        Self {
            registered: Arc::default(),
            initialized,
            reject_registration: Arc::default(),
            active: Arc::default(),
            acknowledgement_mode: Arc::new(Mutex::new(AcknowledgementMode::Send)),
            fence_acknowledgement: Arc::default(),
        }
    }

    fn register(&self, address: MailAddr) -> Result<ProbeRegistration, RegistrationFailure> {
        assert!(
            self.initialized.load(Ordering::SeqCst),
            "transactional registration requires completed initialization"
        );
        if self.reject_registration.load(Ordering::SeqCst) {
            return Err(RegistrationFailure::Rejected);
        }
        if !self.active.lock().insert(address) {
            return Err(RegistrationFailure::Occupied);
        }
        self.registered.store(true, Ordering::SeqCst);
        Ok(ProbeRegistration {
            address,
            active: self.active.clone(),
        })
    }
}

impl<D> EndpointRegistry<MailAddr, EntityProtocol<MailAddr, Command>, D> for ProbeRouter {
    type Error = RegistrationFailure;
    type Registration = ProbeRegistration;

    fn register(&self, address: MailAddr, _: D) -> Result<Self::Registration, Self::Error> {
        self.register(address)
    }
}

impl<D> EndpointRegistry<MailAddr, Command, D> for ProbeRouter {
    type Error = RegistrationFailure;
    type Registration = ProbeRegistration;

    fn register(&self, address: MailAddr, _: D) -> Result<Self::Registration, Self::Error> {
        self.register(address)
    }
}

impl DeliveryRouter<MailAddr, Command> for ProbeRouter {
    type Error = std::convert::Infallible;

    async fn deliver(
        &self,
        _: MailAddr,
        _: Delivery<MailAddr, Command>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl DeliveryRouter<MailAddr, DrainFenceAcknowledged> for ProbeRouter {
    type Error = std::convert::Infallible;

    async fn deliver(
        &self,
        _: MailAddr,
        _: Delivery<MailAddr, DrainFenceAcknowledged>,
    ) -> Result<(), Self::Error> {
        let acknowledgement = self.fence_acknowledgement.lock().take();
        match *self.acknowledgement_mode.lock() {
            AcknowledgementMode::Send => {
                if let Some(acknowledgement) = acknowledgement {
                    let _ = acknowledgement.send(());
                }
            }
            AcknowledgementMode::Drop => drop(acknowledgement),
        }
        Ok(())
    }
}

type ActivatedBehavior = EntityBehavior<ProbeBehavior>;

struct BombayRuntime {
    system: System<ProbeRouter>,
    router: ProbeRouter,
    initialized: Arc<AtomicBool>,
    processed: Arc<Mutex<Vec<u64>>>,
}

impl BombayRuntime {
    fn new() -> Self {
        let initialized = Arc::new(AtomicBool::new(false));
        let processed: Arc<Mutex<Vec<u64>>> = Arc::default();
        let router = ProbeRouter::new(initialized.clone());
        Self {
            system: System::new(MailboxConfig::bounded(2), router.clone()),
            router,
            initialized,
            processed,
        }
    }
}

impl LocalEntityRuntime<u64, Command> for BombayRuntime {
    type Endpoint = RootEndpoint<ActivatedBehavior>;
    type Lease = BehaviorRetirement<ProbeRouter, ActivatedBehavior>;
    type ActivationError = ();

    fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
        tokio::spawn(task);
    }

    async fn activate(
        &self,
        _: EntityId<u64>,
        activation_id: ActivationId,
    ) -> Result<Activated<Self::Endpoint, Self::Lease>, Self::ActivationError> {
        let activation = self
            .system
            .activate(
                MailAddr(activation_id.get().get()),
                EntityBehavior::new(ProbeBehavior {
                    initialized: self.initialized.clone(),
                    processed: self.processed.clone(),
                    fail_initialization: false,
                }),
            )
            .await
            .map_err(|_| ())?;
        Ok(Activated {
            endpoint: activation.endpoint,
            lease: activation.retirement,
        })
    }

    async fn deliver(&self, endpoint: Self::Endpoint, command: Command) -> Result<(), Command> {
        DeliveryEndpoint::deliver(&endpoint, MailAddr(0), EntityProtocol::Command(command))
            .await
            .map_err(|rejected| match rejected.message {
                EntityProtocol::Command(command) => command,
                EntityProtocol::DrainFence { .. } => {
                    unreachable!("the delivered command must round-trip unchanged")
                }
            })
    }

    async fn fence(&self, endpoint: Self::Endpoint) -> Result<(), FenceFailure> {
        let (acknowledge, acknowledged) = tokio::sync::oneshot::channel();
        let previous = self
            .router
            .fence_acknowledgement
            .lock()
            .replace(acknowledge);
        assert!(previous.is_none(), "the conformance probe fences serially");

        if DeliveryEndpoint::deliver(
            &endpoint,
            MailAddr(0),
            EntityProtocol::DrainFence {
                reply_to: Recipient::global(MailAddr(0)),
            },
        )
        .await
        .is_err()
        {
            self.router.fence_acknowledgement.lock().take();
            return Err(FenceFailure::Enqueue);
        }

        acknowledged
            .await
            .map_err(|_| FenceFailure::Acknowledgement)
    }

    async fn retire(&self, lease: Self::Lease, retirement: RetirementMode) {
        match retirement {
            RetirementMode::Graceful => {
                lease
                    .request_shutdown()
                    .expect("the Entity-wrapped probe supports shutdown");
            }
            RetirementMode::Forced(_) => lease.abort(),
        }
        let _ = lease.outcome().await;
    }
}

fn activation_id(value: u64) -> ActivationId {
    ActivationId::new(NonZeroU64::new(value).unwrap())
}

#[tokio::test(flavor = "current_thread")]
async fn ordinary_spawn_exposes_registration_before_initialization() {
    let behavior_initialized = Arc::new(AtomicBool::new(false));
    let router_initialized = Arc::new(AtomicBool::new(true));
    let router = ProbeRouter::new(router_initialized);
    let system = System::new(MailboxConfig::bounded(1), router.clone());

    let actor = system
        .spawn(
            MailAddr(1),
            ProbeBehavior {
                initialized: behavior_initialized.clone(),
                processed: Arc::default(),
                fail_initialization: false,
            },
        )
        .expect("the address starts vacant");

    assert!(router.registered.load(Ordering::SeqCst));
    assert!(!behavior_initialized.load(Ordering::SeqCst));
    actor.abort();
    assert!(matches!(actor.outcome().await, TaskOutcome::Cancelled));
    assert!(router.active.lock().is_empty());
}

#[tokio::test]
async fn transactional_activation_orders_commands_before_fence_and_retires_exactly() {
    let runtime = BombayRuntime::new();
    let activated = runtime
        .activate(EntityId::new(7), activation_id(1))
        .await
        .expect("transactional activation succeeds");

    assert!(runtime.initialized.load(Ordering::SeqCst));
    assert!(runtime.router.registered.load(Ordering::SeqCst));
    runtime
        .deliver(activated.endpoint.clone(), Command(1))
        .await
        .unwrap();
    runtime
        .deliver(activated.endpoint.clone(), Command(2))
        .await
        .unwrap();
    let retired_endpoint = activated.endpoint.clone();
    runtime.fence(activated.endpoint).await.unwrap();
    assert_eq!(*runtime.processed.lock(), [1, 2]);

    runtime
        .retire(activated.lease, RetirementMode::Graceful)
        .await;
    assert!(runtime.router.active.lock().is_empty());
    assert_eq!(
        runtime.fence(retired_endpoint.clone()).await,
        Err(FenceFailure::Enqueue)
    );
    assert_eq!(
        runtime.deliver(retired_endpoint, Command(3)).await,
        Err(Command(3)),
        "closed delivery returns the original non-Clone command"
    );
}

#[tokio::test]
async fn initialization_failure_never_registers_or_leaks_an_address() {
    let initialized = Arc::new(AtomicBool::new(false));
    let router = ProbeRouter::new(initialized.clone());
    let system = System::new(MailboxConfig::bounded(1), router.clone());

    let failed = system
        .activate(
            MailAddr(1),
            EntityBehavior::new(ProbeBehavior {
                initialized,
                processed: Arc::default(),
                fail_initialization: true,
            }),
        )
        .await;

    assert!(failed.is_err());
    assert!(!router.registered.load(Ordering::SeqCst));
    assert!(router.active.lock().is_empty());
}

#[tokio::test]
async fn registration_failure_rolls_back_and_the_same_identity_can_retry() {
    let runtime = BombayRuntime::new();
    runtime
        .router
        .reject_registration
        .store(true, Ordering::SeqCst);
    assert!(
        runtime
            .activate(EntityId::new(7), activation_id(1))
            .await
            .is_err()
    );
    assert!(runtime.router.active.lock().is_empty());

    runtime
        .router
        .reject_registration
        .store(false, Ordering::SeqCst);
    let activated = runtime
        .activate(EntityId::new(7), activation_id(1))
        .await
        .expect("failed registration leaves the identity reusable");
    runtime
        .retire(
            activated.lease,
            RetirementMode::Forced(DrainFailure {
                stage: DrainStage::Retirement,
                outstanding_reservations: 0,
            }),
        )
        .await;
    assert!(runtime.router.active.lock().is_empty());
}

#[tokio::test]
async fn address_collision_does_not_disturb_the_live_incarnation() {
    let runtime = BombayRuntime::new();
    let first = runtime
        .activate(EntityId::new(7), activation_id(1))
        .await
        .unwrap();
    assert!(
        runtime
            .activate(EntityId::new(8), activation_id(1))
            .await
            .is_err()
    );

    runtime
        .deliver(first.endpoint.clone(), Command(9))
        .await
        .unwrap();
    runtime.fence(first.endpoint).await.unwrap();
    assert_eq!(*runtime.processed.lock(), [9]);
    runtime
        .retire(
            first.lease,
            RetirementMode::Forced(DrainFailure {
                stage: DrainStage::Retirement,
                outstanding_reservations: 0,
            }),
        )
        .await;
}

#[tokio::test]
async fn delivered_but_unacknowledged_fence_is_not_reported_as_enqueue_failure() {
    let runtime = BombayRuntime::new();
    let activated = runtime
        .activate(EntityId::new(7), activation_id(1))
        .await
        .unwrap();
    *runtime.router.acknowledgement_mode.lock() = AcknowledgementMode::Drop;

    assert_eq!(
        runtime.fence(activated.endpoint).await,
        Err(FenceFailure::Acknowledgement)
    );
    runtime
        .retire(
            activated.lease,
            RetirementMode::Forced(DrainFailure {
                stage: DrainStage::FenceAcknowledgement,
                outstanding_reservations: 0,
            }),
        )
        .await;
}

#[tokio::test]
async fn directory_owned_tasks_are_driven_to_completion() {
    let runtime = BombayRuntime::new();
    let (completed, observed) = tokio::sync::oneshot::channel();
    runtime.spawn(async move {
        completed.send(()).unwrap();
    });
    observed.await.expect("the owned lifecycle task completes");
}
