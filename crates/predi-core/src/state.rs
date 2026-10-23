//! Engine lifecycle state (research R9, FR-024, FR-025).
//!
//! The only state is the pause flag. The engine holds no text, so `field_changed` has nothing
//! to discard.

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct EngineState {
    paused: AtomicBool,
}

impl EngineState {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}
