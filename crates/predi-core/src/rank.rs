//! Confidence, cutoff, de-duplication and ranking (research R6, R7, FR-002, FR-007, FR-013).

use crate::normalize::fold;
use crate::types::{Status, Suggestion, SuggestionKind};

/// Maximum suggestions per result (FR-002).
pub const MAX_SUGGESTIONS: usize = 3;

/// A cased candidate ready for ranking.
#[derive(Debug, Clone)]
pub struct Ranked {
    /// Final text (casing applied).
    pub text: String,
    /// Canonical text (before casing), the deterministic tie-breaker.
    pub canonical: String,
    pub kind: SuggestionKind,
    pub score: f64,
}

/// Output span shared by every suggestion of one request.
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub replace_chars: u32,
    pub replace_utf16: u32,
    pub leading_space: bool,
}

fn order(a: &Ranked, b: &Ranked) -> std::cmp::Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| a.canonical.cmp(&b.canonical))
        .then_with(|| a.text.cmp(&b.text))
}

/// Rank candidates: confidence = share of `total` (R6), cutoff `tau`, fold-duplicates dropped
/// (FR-013), at most 3 with contiguous ranks, ties broken by canonical text (FR-007).
///
/// `continuation`, if any, takes the slot of the suggestion with the same first word (R7).
pub fn rank(
    mut cands: Vec<Ranked>,
    total: f64,
    tau: f32,
    span: Span,
    continuation: Option<Ranked>,
) -> (Vec<Suggestion>, Status) {
    if total <= 0.0 || !total.is_finite() {
        return (Vec::new(), Status::LowConfidence);
    }
    cands.sort_by(order);

    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<Suggestion> = Vec::new();
    let mut cont = continuation;
    for c in cands {
        if out.len() == MAX_SUGGESTIONS {
            break;
        }
        let key = fold(&c.text);
        if seen.contains(&key) {
            continue;
        }
        let confidence = (c.score / total) as f32;
        if confidence < tau {
            // Sorted by score: nothing later qualifies.
            break;
        }
        seen.push(key);
        let (text, kind, confidence) = match cont.take_if(|k| first_word_matches(&k.text, &c.text))
        {
            Some(k) => (k.text, k.kind, (k.score as f32).min(confidence)),
            None => (c.text, c.kind, confidence),
        };
        out.push(Suggestion {
            text,
            kind,
            replace_chars: span.replace_chars,
            replace_utf16: span.replace_utf16,
            leading_space: span.leading_space,
            rank: out.len() as u8 + 1,
            confidence,
        });
    }
    let status = if out.is_empty() {
        Status::LowConfidence
    } else {
        Status::Ok
    };
    (out, status)
}

fn first_word_matches(continuation: &str, word: &str) -> bool {
    continuation.split(' ').next() == Some(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(text: &str, score: f64) -> Ranked {
        Ranked {
            text: text.to_string(),
            canonical: text.to_lowercase(),
            kind: SuggestionKind::NextWord,
            score,
        }
    }

    const SPAN: Span = Span {
        replace_chars: 0,
        replace_utf16: 0,
        leading_space: false,
    };

    #[test]
    fn cutoff_dedupe_and_limit() {
        let cands = vec![
            r("si", 3.0),
            r("sí", 2.0),
            r("no", 2.0),
            r("ya", 1.0),
            r("x", 0.1),
        ];
        let (s, st) = rank(cands, 10.0, 0.1, SPAN, None);
        assert_eq!(st, Status::Ok);
        let texts: Vec<_> = s.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(texts, ["si", "no", "ya"]);
        assert_eq!(s.iter().map(|x| x.rank).collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn ties_break_by_text() {
        let (s, _) = rank(vec![r("b", 1.0), r("a", 1.0)], 2.0, 0.0, SPAN, None);
        assert_eq!(s[0].text, "a");
    }

    #[test]
    fn low_confidence() {
        let (s, st) = rank(vec![r("a", 1.0)], 100.0, 0.5, SPAN, None);
        assert!(s.is_empty());
        assert_eq!(st, Status::LowConfidence);
    }

    #[test]
    fn continuation_takes_slot() {
        let cont = Ranked {
            text: "puedo ayudarle".into(),
            canonical: "puedo ayudarle".into(),
            kind: SuggestionKind::Continuation,
            score: 0.3,
        };
        let (s, _) = rank(
            vec![r("puedo", 5.0), r("le", 3.0)],
            10.0,
            0.1,
            SPAN,
            Some(cont),
        );
        assert_eq!(s[0].text, "puedo ayudarle");
        assert_eq!(s[0].kind, SuggestionKind::Continuation);
        assert_eq!(s[1].text, "le");
        assert_eq!(s.len(), 2);
    }
}
