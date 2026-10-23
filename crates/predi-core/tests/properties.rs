//! Property tests (T030, T060; constitution §IX, FR-003, FR-004, FR-007, FR-013, FR-029, G10).

#![cfg(not(target_arch = "wasm32"))]

mod common;

use predi_core::normalize::fold;
use predi_core::tokenize::{Ending, analyze};
use predi_core::{Engine, PredictionResult, SuggestionKind};
use proptest::prelude::*;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(common::engine)
}

/// Arbitrary Unicode text up to 2,000 scalar values.
fn any_text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..2000).prop_map(|v| v.into_iter().collect())
}

/// Spanish-looking text: words from the fixture vocabulary, partial words, punctuation.
fn spanish_text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("Buenos ".to_string()),
        Just("días".to_string()),
        Just("dí".to_string()),
        Just("gracias".to_string()),
        Just("por ".to_string()),
        Just("su ".to_string()),
        Just("contrase".to_string()),
        Just("comunicacion".to_string()),
        Just("¿En qué puedo ".to_string()),
        Just("Hola,".to_string()),
        Just(" ".to_string()),
        Just(". ".to_string()),
        Just("\n".to_string()),
        Just("…".to_string()),
        Just("¡".to_string()),
        Just("«".to_string()),
        Just("\"".to_string()),
        Just("BUE".to_string()),
        Just("ñ".to_string()),
        Just("123".to_string()),
        Just("😀".to_string()),
        "[a-záéíóúñü]{1,8}",
        "[A-ZÁÉÍÓÚÑ]{1,3}",
    ];
    prop::collection::vec(piece, 0..40).prop_map(|v| v.concat())
}

fn arbitrary_or_spanish() -> impl Strategy<Value = String> {
    prop_oneof![any_text(), spanish_text()]
}

/// Byte offset where the `replace_chars` span starts.
fn cut_at(text: &str, replace_chars: u32) -> Option<usize> {
    if replace_chars == 0 {
        return Some(text.len());
    }
    text.char_indices()
        .rev()
        .nth(replace_chars as usize - 1)
        .map(|(i, _)| i)
}

fn check_invariants(text: &str, r: &PredictionResult) -> Result<(), TestCaseError> {
    prop_assert!(r.suggestions.len() <= 3);
    for (i, s) in r.suggestions.iter().enumerate() {
        prop_assert_eq!(s.rank as usize, i + 1);
        prop_assert!(!s.text.is_empty());
        prop_assert_eq!(s.text.trim(), s.text.as_str());
        prop_assert!(s.confidence.is_finite() && (0.0..=1.0).contains(&s.confidence));
        if s.kind == SuggestionKind::Completion {
            prop_assert!(!s.leading_space);
        }
        // FR-004: everything before the replaced span is unchanged.
        let cut = cut_at(text, s.replace_chars);
        prop_assert!(cut.is_some(), "replace_chars beyond the text");
        let cut = cut.unwrap_or(0);
        let replaced = &text[cut..];
        prop_assert_eq!(replaced.encode_utf16().count() as u32, s.replace_utf16);
        prop_assert!(
            !replaced.chars().any(char::is_whitespace),
            "span crosses a word boundary"
        );
        let applied = s.apply_to(text);
        prop_assert!(applied.is_some());
        let applied = applied.unwrap_or_default();
        prop_assert_eq!(&applied.as_bytes()[..cut], &text.as_bytes()[..cut]);
        if s.kind == SuggestionKind::Continuation {
            let words = s.text.split(' ').count();
            prop_assert!((2..=4).contains(&words), "continuation of {} words", words);
            prop_assert!(!s.text.contains(['.', '?', '!', '…']));
        }
    }
    // FR-013: no case- or accent-folded duplicates.
    let mut keys: Vec<String> = r.suggestions.iter().map(|s| fold(&s.text)).collect();
    keys.sort();
    keys.dedup();
    prop_assert_eq!(keys.len(), r.suggestions.len());
    // G10/FR-008: inside a word, every suggestion is a completion; otherwise none is.
    let inside = matches!(analyze(text).ending, Ending::InsideWord { .. });
    for s in &r.suggestions {
        prop_assert_eq!(s.kind == SuggestionKind::Completion, inside);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 384, ..ProptestConfig::default() })]

    #[test]
    fn invariants_hold_for_any_input(text in arbitrary_or_spanish()) {
        let r = common::predict(engine(), &text);
        check_invariants(&text, &r)?;
    }

    #[test]
    fn deterministic(text in arbitrary_or_spanish()) {
        let a = common::predict(engine(), &text);
        let b = common::predict(engine(), &text);
        let fresh = common::predict(&common::engine(), &text);
        prop_assert_eq!(&a, &b);
        prop_assert_eq!(&a, &fresh);
    }

    /// Constitution §IX: after deleting characters, results equal a fresh engine's, even after
    /// the engine served the longer text.
    #[test]
    fn deletion_matches_fresh_engine(text in spanish_text(), cut in 0usize..40) {
        let e = engine();
        let _ = common::predict(e, &text);
        let n = text.chars().count();
        let keep: String = text.chars().take(n.saturating_sub(cut)).collect();
        let after = common::predict(e, &keep);
        let fresh = common::predict(&common::engine(), &keep);
        prop_assert_eq!(after, fresh);
    }

    /// FR-003, SC-007: decomposed (NFD) input is replaced correctly by its own counts.
    #[test]
    fn decomposed_input_applies_cleanly(text in spanish_text()) {
        let nfd: String = text.nfd().collect();
        let r = common::predict(engine(), &nfd);
        check_invariants(&nfd, &r)?;
        let r_nfc = common::predict(engine(), &text.nfc().collect::<String>());
        let texts = |r: &PredictionResult| r.suggestions.iter().map(|s| s.text.clone()).collect::<Vec<_>>();
        prop_assert_eq!(texts(&r), texts(&r_nfc));
        for s in &r.suggestions {
            let applied = s.apply_to(&nfd).unwrap_or_default();
            let cut = cut_at(&nfd, s.replace_chars).unwrap_or(0);
            let expected: String = format!("{}{}{}", &nfd[..cut], if s.leading_space { " " } else { "" }, s.text);
            prop_assert_eq!(applied, expected);
        }
    }
}

#[test]
fn nfd_example_buenos_dias() {
    let text = "Buenos di\u{301}";
    let r = common::predict(engine(), text);
    let s = r.suggestions.iter().find(|s| s.text == "días");
    assert!(s.is_some(), "días not suggested");
    let s = s.unwrap_or(&r.suggestions[0]);
    assert_eq!(s.replace_chars, 3);
    assert_eq!(s.apply_to(text).as_deref(), Some("Buenos días"));
}
