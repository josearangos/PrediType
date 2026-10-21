//! Pause, resume, field change and no-learning guarantees (T046, FR-024–FR-026, SC-013).

mod common;

use predi_core::Status;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Deterministic pseudo-random inputs from a small alphabet (no extra dependencies).
fn sample_inputs(n: usize, seed: u64) -> Vec<String> {
    const PIECES: &[&str] = &[
        "Buenos ",
        "días",
        "dí",
        "gracias",
        " por ",
        "su ",
        "contrase",
        "Hola,",
        " ",
        ". ",
        "¿En qué ",
        "puedo ",
        "a",
        "e",
        "ñ",
        "BUE",
        "\n",
        "le ",
        "molest",
        "123",
        "@",
        "😀",
        "Lo esperamos en Bog",
    ];
    let mut state = seed;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    (0..n)
        .map(|_| {
            let len = next() % 8;
            (0..len).map(|_| PIECES[next() % PIECES.len()]).collect()
        })
        .collect()
}

#[test]
fn pause_returns_empty_paused_and_resume_restores() {
    let e = common::engine();
    let before = common::predict(&e, "Buenos dí");
    assert!(!before.suggestions.is_empty());
    e.pause();
    for text in sample_inputs(50, 1)
        .iter()
        .chain([&"Buenos dí".to_string()])
    {
        let r = common::predict(&e, text);
        assert_eq!(r.status, Status::Paused);
        assert!(r.suggestions.is_empty());
    }
    e.resume();
    assert_eq!(common::predict(&e, "Buenos dí"), before);
}

#[test]
fn field_changed_equals_fresh_engine() {
    let e = common::engine();
    for text in sample_inputs(300, 2) {
        let _ = common::predict(&e, &text);
    }
    e.field_changed();
    let fresh = common::engine();
    for text in sample_inputs(200, 3) {
        assert_eq!(common::predict(&e, &text), common::predict(&fresh, &text));
    }
}

#[test]
fn no_learning_from_requests() {
    let e = common::engine();
    let probe = "Muchas gracias por ";
    let first = common::predict(&e, probe);
    for text in sample_inputs(10_000, 4) {
        let _ = common::predict(&e, &text);
    }
    assert_eq!(common::predict(&e, probe), first);
}

#[test]
fn request_id_is_echoed() {
    let e = common::engine();
    for id in [0u64, 1, u64::MAX, 42] {
        let r = e.predict(&predi_core::PredictionRequest::new(id, "Hola"));
        assert_eq!(r.request_id, id);
    }
    e.pause();
    assert_eq!(
        e.predict(&predi_core::PredictionRequest::new(9, "Hola"))
            .request_id,
        9
    );
}
