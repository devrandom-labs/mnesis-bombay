use core::convert::Infallible;
use core::fmt;

use mnesis::{Aggregate, AggregateState, DomainEvent, Events, Handle, Message};
use mnesis_bombay_core::{Context, ValidatedContext};

#[derive(Debug, Clone)]
pub enum Event {
    Changed,
}

impl Message for Event {}

impl DomainEvent for Event {
    fn name(&self) -> &'static str {
        "changed"
    }
}

#[derive(Debug)]
pub struct State;

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
pub struct AccountId(pub &'static [u8]);

impl fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("account")
    }
}

impl AsRef<[u8]> for AccountId {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(pub &'static [u8]);

impl fmt::Display for OrderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("order")
    }
}

impl AsRef<[u8]> for OrderId {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

pub struct Account;

impl Aggregate for Account {
    type State = State;
    type Error = Infallible;
    type Id = AccountId;
}

pub struct Order;

impl Aggregate for Order {
    type State = State;
    type Error = Infallible;
    type Id = OrderId;
}

pub struct Deposit;
pub struct Ship;

pub struct RawContext;

impl Context for RawContext {
    type Error = Infallible;

    fn encoded_len(&self) -> usize {
        0
    }

    fn validate(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn fmt_redacted(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

pub type TestContext = ValidatedContext<RawContext, 0>;

pub fn context() -> TestContext {
    TestContext::try_new(RawContext).unwrap()
}

impl Handle<Deposit> for Account {
    fn handle(_: &State, _: Deposit) -> Result<Option<Events<Event>>, Infallible> {
        Ok(Some(Events::new(Event::Changed)))
    }
}

impl Handle<Ship> for Order {
    fn handle(_: &State, _: Ship) -> Result<Option<Events<Event>>, Infallible> {
        Ok(Some(Events::new(Event::Changed)))
    }
}
