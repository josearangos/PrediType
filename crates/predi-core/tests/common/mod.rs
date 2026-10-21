//! Shared helpers for predi-core integration tests. Everything is embedded, so the tests also
//! run on WebAssembly with no filesystem.

#![allow(dead_code)]

use predi_core::{Engine, PredictionRequest, PredictionResult};

pub const FIXTURE: &[u8] = include_bytes!("../../fixtures/mini.pta");

pub fn engine() -> Engine {
    match Engine::from_bytes(FIXTURE) {
        Ok(e) => e,
        Err(e) => panic!("fixture asset does not load: {e}"),
    }
}

pub fn predict(engine: &Engine, text: &str) -> PredictionResult {
    engine.predict(&PredictionRequest::new(7, text))
}
