//! Keystroke replay (T053): one request at every character position of each message, with each
//! position classified per the spec's "Prediction points" definition.

use predi_core::tokenize::{Token, tokens};
use predi_core::{Engine, PredictionRequest, PredictionResult};

/// What a position is, for the metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointKind {
    /// Inside a word token after `typed` letters (strictly before its end). Spec partial-word
    /// points are those with `typed >= 2`.
    Partial { typed: usize },
    /// Directly after whitespace that follows a word, with a word starting here.
    Boundary,
    /// Anything else (message start, punctuation, non-word tokens, placeholders, word ends).
    Other,
}

/// One replay position.
#[derive(Debug, Clone)]
pub struct Point {
    /// Byte offset of the caret in the message.
    pub pos: usize,
    pub kind: PointKind,
    /// The word being typed (partial) or the next word (boundary).
    pub intended: Option<String>,
}

impl Point {
    /// A spec prediction point (SC-001, SC-002, SC-004, SC-014).
    pub fn is_prediction_point(&self) -> bool {
        match self.kind {
            PointKind::Partial { typed } => typed >= 2,
            PointKind::Boundary => true,
            PointKind::Other => false,
        }
    }
}

/// Byte spans of the word tokens of `text`.
pub fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let base = text.as_ptr() as usize;
    tokens(text)
        .into_iter()
        .filter_map(|t| match t {
            Token::Word(w) => {
                let s = w.as_ptr() as usize - base;
                Some((s, s + w.len()))
            }
            _ => None,
        })
        .collect()
}

/// Classify every caret position `0..=len` of a message.
pub fn points(message: &str) -> Vec<Point> {
    let spans = word_spans(message);
    let mut out = Vec::new();
    let positions = message
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(message.len()));
    for pos in positions {
        let mut kind = PointKind::Other;
        let mut intended = None;
        if let Some(&(s, e)) = spans.iter().find(|&&(s, e)| s < pos && pos < e) {
            kind = PointKind::Partial {
                typed: message[s..pos].chars().count(),
            };
            intended = Some(message[s..e].to_string());
        } else if let Some(&(s, e)) = spans.iter().find(|&&(s, _)| s == pos) {
            let after_space = message[..pos]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
            let word_before = spans.iter().any(|&(_, pe)| pe <= pos);
            if after_space && word_before {
                kind = PointKind::Boundary;
                intended = Some(message[s..e].to_string());
            }
        }
        out.push(Point {
            pos,
            kind,
            intended,
        });
    }
    out
}

/// A replayed position with its result.
pub struct Replayed {
    pub point: Point,
    pub result: PredictionResult,
}

/// Replay one message through the engine, one request per caret position.
pub fn replay_message(engine: &Engine, message: &str, next_id: &mut u64) -> Vec<Replayed> {
    points(message)
        .into_iter()
        .map(|point| {
            *next_id += 1;
            let result = engine.predict(&PredictionRequest::new(*next_id, &message[..point.pos]));
            Replayed { point, result }
        })
        .collect()
}

/// Does `text` start with `prefix` followed by a non-letter (or the end)?
pub fn starts_with_word(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix)
        .is_some_and(|rest| !rest.chars().next().is_some_and(char::is_alphabetic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_positions() {
        let pts = points("Hola, buenos días");
        let kinds: Vec<_> = pts.iter().map(|p| (p.pos, p.kind.clone())).collect();
        assert_eq!(kinds[0], (0, PointKind::Other));
        assert_eq!(kinds[2], (2, PointKind::Partial { typed: 2 }));
        // "Hola," then space: position 6 is a boundary before "buenos".
        assert_eq!(kinds[6], (6, PointKind::Boundary));
        assert_eq!(pts[6].intended.as_deref(), Some("buenos"));
        // End of the message (after "días") is not a prediction point.
        assert_eq!(pts.last().map(|p| p.kind.clone()), Some(PointKind::Other));
        let n_partial = pts.iter().filter(|p| p.is_prediction_point()).count();
        // hola: 2 (ho, hol); buenos: 4; días: 2; plus 2 boundaries.
        assert_eq!(n_partial, 2 + 4 + 2 + 2);
    }

    #[test]
    fn placeholders_are_not_points() {
        let pts = points("Hola, {NOMBRE}.");
        assert!(
            pts.iter()
                .filter(|p| p.pos > 6)
                .all(|p| !p.is_prediction_point())
        );
    }

    #[test]
    fn word_prefix() {
        assert!(starts_with_word("puedo ayudarle hoy", "puedo ayudarle"));
        assert!(starts_with_word("puedo ayudarle?", "puedo ayudarle"));
        assert!(!starts_with_word("puedo ayudarles", "puedo ayudarle"));
    }
}
