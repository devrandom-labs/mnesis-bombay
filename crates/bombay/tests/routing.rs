//! Tests for the sole application-ID to Bombay Entity routing translation.

use core::convert::Infallible;
use core::fmt;

use mnesis::{Aggregate, AggregateState, DomainEvent, Message};
use mnesis_bombay::{ExecuteRequest, into_entity_delivery};
use mnesis_bombay_core::Addressed;

#[derive(Debug, Clone)]
struct Event;

impl Message for Event {}

impl DomainEvent for Event {
    fn name(&self) -> &'static str {
        "event"
    }
}

#[derive(Debug)]
struct State;

impl AggregateState for State {
    type Event = Event;

    fn initial() -> Self {
        Self
    }

    fn apply(self, _: &Event) -> Self {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AccountId(&'static [u8]);

impl fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("account-7")
    }
}

impl AsRef<[u8]> for AccountId {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

struct Account;

impl Aggregate for Account {
    type State = State;
    type Error = Infallible;
    type Id = AccountId;
}

#[test]
fn sole_runtime_translation_wraps_exact_application_id_and_preserves_payload() {
    struct NonCloneCommand(u64);

    let addressed = Addressed::new(AccountId(b"account-7"), NonCloneCommand(9));
    let (entity_id, command) = into_entity_delivery::<Account, _>(addressed);

    assert_eq!(entity_id.into_inner().as_ref(), b"account-7");
    assert_eq!(command.0, 9);
}

#[test]
fn reply_capability_stays_in_bombay_envelope_and_preserves_ownership() {
    struct NonCloneRequest(u8);
    struct NonCloneReply(u16);

    let envelope = ExecuteRequest::new(NonCloneRequest(3), NonCloneReply(5));
    assert_eq!(envelope.request().0, 3);
    assert_eq!(envelope.reply().0, 5);
    let (request, reply) = envelope.into_parts();
    assert_eq!((request.0, reply.0), (3, 5));
}
