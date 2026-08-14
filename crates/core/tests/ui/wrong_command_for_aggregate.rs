#[path = "../support/mod.rs"]
mod support;

use mnesis_bombay_core::CommandRequest;
use support::{Account, Ship, context};

fn main() {
    let _ = CommandRequest::<Account, _, _, _, _>::new((), Ship, context(), None::<()>);
}
