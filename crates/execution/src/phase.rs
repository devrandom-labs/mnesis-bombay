use core::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use mnesis_bombay_core::CommandPhase;

const RECEIVED: u8 = 0;
const ADMITTED: u8 = 1;
const LOADING: u8 = 2;
const DECIDING: u8 = 3;
const APPEND_IN_FLIGHT: u8 = 4;
const APPEND_NOT_COMMITTED: u8 = 5;
const APPEND_COMMITTED: u8 = 6;

/// Cloneable observation handle for the last established command phase.
///
/// The handle survives cancellation or task panic. A host can inspect it after
/// the execution future disappears and classify interruption without treating
/// actor lifecycle or future completion as a durability fact.
#[derive(Debug, Clone)]
pub struct PhaseTracker {
    phase: Arc<AtomicU8>,
}

impl Default for PhaseTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseTracker {
    /// Creates a tracker at [`CommandPhase::Received`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Arc::new(AtomicU8::new(RECEIVED)),
        }
    }

    /// Returns the last phase established by the executor or repository
    /// decorator.
    #[must_use]
    pub fn phase(&self) -> CommandPhase {
        match self.phase.load(Ordering::Acquire) {
            RECEIVED => CommandPhase::Received,
            ADMITTED => CommandPhase::Admitted,
            LOADING => CommandPhase::Loading,
            DECIDING => CommandPhase::Deciding,
            APPEND_IN_FLIGHT => CommandPhase::AppendInFlight,
            APPEND_NOT_COMMITTED => CommandPhase::AppendNotCommitted,
            APPEND_COMMITTED => CommandPhase::AppendCommitted,
            _ => unreachable!("PhaseTracker stores only closed CommandPhase encodings"),
        }
    }

    pub(crate) fn set(&self, phase: CommandPhase) {
        let encoded = match phase {
            CommandPhase::Received => RECEIVED,
            CommandPhase::Admitted => ADMITTED,
            CommandPhase::Loading => LOADING,
            CommandPhase::Deciding => DECIDING,
            CommandPhase::AppendInFlight => APPEND_IN_FLIGHT,
            CommandPhase::AppendNotCommitted => APPEND_NOT_COMMITTED,
            CommandPhase::AppendCommitted => APPEND_COMMITTED,
        };
        self.phase.store(encoded, Ordering::Release);
    }
}
