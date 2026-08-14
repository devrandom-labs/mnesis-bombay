//! Runtime tests for the runtime-neutral typed protocol.

use core::convert::Infallible;
use core::fmt;
use core::num::NonZeroU32;

use mnesis::{Aggregate, AggregateState, DomainEvent, Events, Handle, Message};
use mnesis_bombay_core::{
    Addressed, CommandIdentity, CommandOutcome, CommandPhase, Context, ContextError,
    InterruptionFact, ValidatedContext,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Changed,
}

impl Message for Event {}

impl DomainEvent for Event {
    fn name(&self) -> &'static str {
        "changed"
    }
}

#[derive(Debug)]
struct State;

impl AggregateState for State {
    type Event = Event;

    fn initial() -> Self {
        Self
    }

    fn apply(self, _: &Self::Event) -> Self {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AggregateId(&'static [u8]);

impl fmt::Display for AggregateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("aggregate")
    }
}

impl AsRef<[u8]> for AggregateId {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

struct Account;

impl Aggregate for Account {
    type State = State;
    type Error = Infallible;
    type Id = AggregateId;
}

struct Change;

impl Handle<Change> for Account {
    fn handle(_: &State, _: Change) -> Result<Option<Events<Event>>, Infallible> {
        Ok(Some(Events::new(Event::Changed)))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SecretContext {
    tenant: &'static str,
    principal: &'static str,
    valid: bool,
    encoded_len: usize,
}

impl Context for SecretContext {
    type Error = &'static str;

    fn encoded_len(&self) -> usize {
        self.encoded_len
    }

    fn validate(&self) -> Result<(), Self::Error> {
        self.valid.then_some(()).ok_or("invalid principal")
    }

    fn fmt_redacted(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretContext")
            .field("tenant", &self.tenant)
            .field("principal", &"<redacted>")
            .finish()
    }
}

#[test]
fn addressed_keeps_routing_separate_and_preserves_non_clone_message() {
    struct NonClone(u8);

    let addressed = Addressed::new(AggregateId(b"a"), NonClone(7));
    assert_eq!(addressed.id().as_ref(), b"a");
    assert_eq!(addressed.message().0, 7);
    let (id, message) = addressed.into_parts();
    assert_eq!(id.as_ref(), b"a");
    assert_eq!(message.0, 7);
}

#[test]
fn identity_roles_round_trip_without_conversion() {
    struct CommandId(u8);
    struct CausationId(u16);
    struct CorrelationId(u32);

    let identity = CommandIdentity::new(CommandId(1), CausationId(2), CorrelationId(3));
    assert_eq!(identity.command_id().0, 1);
    assert_eq!(identity.causation_id().0, 2);
    assert_eq!(identity.correlation_id().0, 3);
    let (command, causation, correlation) = identity.into_parts();
    assert_eq!((command.0, causation.0, correlation.0), (1, 2, 3));
}

#[test]
fn identity_debug_redacts_every_application_value() {
    let identity = CommandIdentity::new("command-secret", "cause-secret", "correlation-secret");
    let rendered = format!("{identity:?}");
    assert!(!rendered.contains("command-secret"));
    assert!(!rendered.contains("cause-secret"));
    assert!(!rendered.contains("correlation-secret"));
    assert_eq!(rendered.matches("<redacted>").count(), 3);
}

#[test]
fn context_establishes_size_and_semantic_validation() {
    type Bounded = ValidatedContext<SecretContext, 16>;

    let valid = Bounded::try_new(SecretContext {
        tenant: "public",
        principal: "secret",
        valid: true,
        encoded_len: 16,
    })
    .unwrap();
    assert_eq!(valid.get().principal, "secret");

    let too_large = Bounded::try_new(SecretContext {
        tenant: "public",
        principal: "secret",
        valid: true,
        encoded_len: 17,
    })
    .unwrap_err();
    assert_eq!(
        too_large,
        ContextError::TooLarge {
            actual: 17,
            maximum: 16
        }
    );

    let invalid = Bounded::try_new(SecretContext {
        tenant: "public",
        principal: "secret",
        valid: false,
        encoded_len: 1,
    })
    .unwrap_err();
    assert_eq!(invalid, ContextError::Invalid("invalid principal"));
}

#[test]
fn validated_context_debug_uses_only_application_redaction_policy() {
    let context = ValidatedContext::<_, 32>::try_new(SecretContext {
        tenant: "safe-tenant",
        principal: "principal-secret",
        valid: true,
        encoded_len: 4,
    })
    .unwrap();
    let rendered = format!("{context:?}");
    assert!(rendered.contains("safe-tenant"));
    assert!(!rendered.contains("principal-secret"));
}

#[test]
fn every_phase_has_the_only_honest_interruption_fact() {
    let cases = [
        (CommandPhase::Received, InterruptionFact::NotAppended),
        (CommandPhase::Admitted, InterruptionFact::NotAppended),
        (CommandPhase::Loading, InterruptionFact::NotAppended),
        (CommandPhase::Deciding, InterruptionFact::NotAppended),
        (CommandPhase::AppendInFlight, InterruptionFact::Ambiguous),
        (
            CommandPhase::AppendNotCommitted,
            InterruptionFact::NotCommitted,
        ),
        (CommandPhase::AppendCommitted, InterruptionFact::Committed),
    ];

    for (phase, expected) in cases {
        assert_eq!(phase.interruption_fact(), expected, "phase {phase:?}");
    }
}

#[test]
fn outcome_variants_preserve_each_failure_domain() {
    type Outcome = CommandOutcome<u64, &'static str, u8, u16, u32, u64, u128>;

    assert_eq!(
        Outcome::Ignored { output: "same" },
        Outcome::Ignored { output: "same" }
    );
    assert!(matches!(
        Outcome::Committed {
            position: 9,
            output: "ok"
        },
        Outcome::Committed {
            position: 9,
            output: "ok"
        }
    ));
    assert!(matches!(Outcome::Rejected(1), Outcome::Rejected(1)));
    assert!(matches!(
        Outcome::ConflictExhausted {
            source: 2,
            attempts: NonZeroU32::MIN,
        },
        Outcome::ConflictExhausted { source: 2, attempts } if attempts == NonZeroU32::MIN
    ));
    assert!(matches!(Outcome::Storage(3), Outcome::Storage(3)));
    assert!(matches!(Outcome::Overloaded(4), Outcome::Overloaded(4)));
    assert!(matches!(
        Outcome::DeadlineBeforeExecution,
        Outcome::DeadlineBeforeExecution
    ));
    assert!(matches!(
        Outcome::AmbiguousCompletion { command_id: 5 },
        Outcome::AmbiguousCompletion { command_id: 5 }
    ));
    assert!(matches!(Outcome::ShuttingDown, Outcome::ShuttingDown));
}

#[test]
fn request_debug_does_not_require_or_disclose_payload_fields() {
    struct Identity(&'static str);
    struct Deadline(&'static str);

    let context = ValidatedContext::<_, 32>::try_new(SecretContext {
        tenant: "tenant",
        principal: "context-secret",
        valid: true,
        encoded_len: 4,
    })
    .unwrap();
    let request = mnesis_bombay_core::CommandRequest::<Account, _, _, _, _>::new(
        Identity("identity-secret"),
        Change,
        context,
        Some(Deadline("deadline-secret")),
    );
    let rendered = format!("{request:?}");
    assert!(rendered.contains("Account"));
    assert!(rendered.contains("Change"));
    assert!(!rendered.contains("identity-secret"));
    assert!(!rendered.contains("context-secret"));
    assert!(!rendered.contains("deadline-secret"));

    let (identity, _, context, deadline) = request.into_parts();
    assert_eq!(identity.0, "identity-secret");
    assert_eq!(context.get().principal, "context-secret");
    assert_eq!(deadline.unwrap().0, "deadline-secret");
}
