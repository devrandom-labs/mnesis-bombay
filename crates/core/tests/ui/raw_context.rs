#[path = "../support/mod.rs"]
mod support;

use mnesis_bombay_core::CommandRequest;
use support::{Account, Deposit, RawContext};

fn main() {
    let _ = CommandRequest::<Account, _, _, _, _>::new((), Deposit, RawContext, None::<()>);
}
