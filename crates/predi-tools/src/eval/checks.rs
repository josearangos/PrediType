//! Correctness-set, determinism and field-change checks (T056, SC-006, SC-008, SC-013).

#[path = "../../../predi-core/tests/common/cases.rs"]
#[allow(dead_code)]
mod cases;

use crate::util::{Result, ToolError};
use predi_core::{Engine, PredictionRequest};
use sha2::{Digest, Sha256};

/// SC-006: run the correctness set. Returns (passed, total, failing case ids).
pub fn correctness(engine: &Engine, jsonl: &str) -> Result<(usize, usize, Vec<String>)> {
    let parsed = cases::parse_cases(jsonl).map_err(ToolError::invalid)?;
    let mut failing = Vec::new();
    for (i, case) in parsed.iter().enumerate() {
        let r = engine.predict(&PredictionRequest::new(i as u64, &case.input));
        if !cases::check(case, &r).is_empty() {
            failing.push(case.id.clone());
        }
    }
    Ok((parsed.len() - failing.len(), parsed.len(), failing))
}

/// SHA-256 over the canonical results of `inputs` (request ids are their indices).
pub fn results_digest(engine: &Engine, inputs: &[&str]) -> [u8; 32] {
    let mut h = Sha256::new();
    for (i, text) in inputs.iter().enumerate() {
        let r = engine.predict(&PredictionRequest::new(i as u64, text));
        h.update(cases::canonical(&r).as_bytes());
        h.update(b"\n");
    }
    h.finalize().into()
}

/// SC-008: replay `inputs` on two independently loaded engines and compare every result.
/// Returns the number of differing requests.
pub fn determinism(asset: &[u8], inputs: &[&str]) -> Result<u64> {
    let load = || Engine::from_bytes(asset).map_err(|e| ToolError::invalid(e.to_string()));
    let (a, b) = (load()?, load()?);
    let mut diffs = 0;
    for (i, text) in inputs.iter().enumerate() {
        let req = PredictionRequest::new(i as u64, text);
        if a.predict(&req) != b.predict(&req) {
            diffs += 1;
        }
    }
    Ok(diffs)
}

/// SC-013: after `field_changed()`, results equal a fresh engine's. Returns (equal, compared).
pub fn field_change(engine: &Engine, asset: &[u8], inputs: &[&str]) -> Result<(u64, u64)> {
    let fresh = Engine::from_bytes(asset).map_err(|e| ToolError::invalid(e.to_string()))?;
    let mut equal = 0;
    for (i, text) in inputs.iter().enumerate() {
        engine.field_changed();
        let req = PredictionRequest::new(i as u64, text);
        if engine.predict(&req) == fresh.predict(&req) {
            equal += 1;
        }
    }
    Ok((equal, inputs.len() as u64))
}
