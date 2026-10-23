//! Correctness set against the fixture asset (T027, FR-036). Runs natively and on WASM; the
//! digest test checks both builds produce identical results (SC-008, FR-007).

#[path = "common/cases.rs"]
mod cases;
mod common;

use predi_core::PredictionRequest;
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const CASES: &str = include_str!("../../../data/correctness/cases.jsonl");
/// SHA-256 of the canonical results of every case on the fixture, as produced natively.
/// Regenerate after changing the cases or the fixture: the failure message prints the new value.
const DIGEST: &str = include_str!("../fixtures/correctness.digest");

#[test]
fn correctness_cases_pass_on_fixture() {
    let engine = common::engine();
    let parsed = match cases::parse_cases(CASES) {
        Ok(c) => c,
        Err(e) => panic!("{e}"),
    };
    assert!(!parsed.is_empty());
    let mut failures = Vec::new();
    for case in &parsed {
        let r = engine.predict(&PredictionRequest::new(1, &case.input));
        let failed = cases::check(case, &r);
        if !failed.is_empty() {
            failures.push(format!("{} ({})", case.id, failed.join(", ")));
        }
    }
    assert!(
        failures.is_empty(),
        "failing cases: {}",
        failures.join("; ")
    );
}

#[test]
fn results_match_native_digest() {
    let engine = common::engine();
    let parsed = cases::parse_cases(CASES).unwrap_or_default();
    let mut h = Sha256::new();
    for (i, case) in parsed.iter().enumerate() {
        let r = engine.predict(&PredictionRequest::new(i as u64, &case.input));
        h.update(cases::canonical(&r).as_bytes());
        h.update(b"\n");
    }
    let got: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        got,
        DIGEST.trim(),
        "result digest differs; if the change is intended, write it to fixtures/correctness.digest"
    );
}
