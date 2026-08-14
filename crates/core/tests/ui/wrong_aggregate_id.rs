#[path = "../support/mod.rs"]
mod support;

use mnesis_bombay_core::{Addressed, CommandRequest};
use support::{Account, Deposit, OrderId, TestContext, context};

fn requires_account_id(
    _: Addressed<
        <Account as mnesis::Aggregate>::Id,
        CommandRequest<Account, Deposit, (), TestContext, ()>,
    >,
) {
}

fn main() {
    let request =
        CommandRequest::<Account, _, _, _, _>::new((), Deposit, context(), None::<()>);
    requires_account_id(Addressed::new(OrderId(b"order"), request));
}
