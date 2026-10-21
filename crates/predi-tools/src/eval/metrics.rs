//! Quality and correctness metrics from the keystroke replay (T054).
//!
//! | Criterion | What it measures |
//! |-----------|------------------|
//! | SC-001 | Partial-word points: intended word in the suggestions (empty = miss) |
//! | SC-002 | Boundary points: actual next word in the suggestions |
//! | SC-003 | Simulated keystroke savings (spec formula) |
//! | SC-004 | First rank given that a correct suggestion is present |
//! | SC-014 | First-rank precision among non-empty results |
//! | SC-005 | Suggestions not in the dictionary/domain list, or matching personal-data patterns |
//! | SC-007 | Prefix preservation for every applied suggestion |
//! | SC-015 | Continuations shown that match the following text in full |

use super::replay::{PointKind, Replayed, starts_with_word};
use predi_core::normalize::lower;
use predi_core::{Engine, PredictionRequest, Suggestion, SuggestionKind};
use std::collections::HashSet;

/// Accumulated counts over a replay.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QualityCounts {
    pub partial_points: u64,
    pub partial_hits: u64,
    pub boundary_points: u64,
    pub boundary_hits: u64,
    /// Prediction points where a correct suggestion was present, and where it ranked first.
    pub correct_present: u64,
    pub correct_first: u64,
    /// Prediction points with a non-empty result, and where rank 1 was correct.
    pub nonempty: u64,
    pub nonempty_first_correct: u64,
    pub suggestions_checked: u64,
    pub misspelled_or_personal: u64,
    pub applied: u64,
    pub prefix_violations: u64,
    pub continuations_shown: u64,
    pub continuations_full_match: u64,
    pub requests: u64,
}

/// Spelling reference for SC-005.
pub struct SpellingReference {
    pub words: HashSet<String>,
    pub domain: HashSet<String>,
    /// Allowlisted standard forms (`Bogotá`, `PSE`).
    pub allowlist: HashSet<String>,
}

impl SpellingReference {
    fn word_ok(&self, w: &str) -> bool {
        let low = lower(w);
        self.words.contains(w)
            || self.words.contains(&low)
            || self.domain.contains(&low)
            || self.allowlist.iter().any(|a| lower(a) == low)
    }

    /// True when `text` is correctly spelled and has no personal-data pattern.
    pub fn suggestion_ok(&self, text: &str) -> bool {
        let personal = text
            .chars()
            .any(|c| c.is_numeric() || matches!(c, '@' | '/' | ':' | '_'));
        !personal && text.split(' ').all(|w| self.word_ok(w))
    }
}

fn first_word(s: &Suggestion) -> &str {
    s.text.split(' ').next().unwrap_or("")
}

/// Is suggestion `s` exactly what follows in the message at this position?
fn matches_following(message: &str, pos: usize, s: &Suggestion) -> bool {
    let cut = cut_at(&message[..pos], s.replace_chars);
    let expected = format!("{}{}", if s.leading_space { " " } else { "" }, s.text);
    starts_with_word(&message[cut..], &expected)
}

fn cut_at(text: &str, replace_chars: u32) -> usize {
    if replace_chars == 0 {
        return text.len();
    }
    text.char_indices()
        .rev()
        .nth(replace_chars as usize - 1)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Is suggestion `s` the intended text at this point (SC-001/SC-002/SC-014)?
fn correct(message: &str, r: &Replayed, s: &Suggestion) -> bool {
    let Some(intended) = r.point.intended.as_deref() else {
        return false;
    };
    match (&r.point.kind, s.kind) {
        (PointKind::Partial { .. }, _) => s.text == intended,
        (PointKind::Boundary, SuggestionKind::Continuation) => {
            matches_following(message, r.point.pos, s)
        }
        (PointKind::Boundary, _) => s.text == intended,
        (PointKind::Other, _) => false,
    }
}

impl QualityCounts {
    /// Add one replayed message.
    pub fn add_message(
        &mut self,
        message: &str,
        replayed: &[Replayed],
        spelling: &SpellingReference,
    ) {
        for r in replayed {
            self.requests += 1;
            let sugg = &r.result.suggestions;
            let prefix = &message[..r.point.pos];
            for s in sugg {
                self.suggestions_checked += 1;
                if !spelling.suggestion_ok(&s.text) {
                    self.misspelled_or_personal += 1;
                }
                // SC-007: the text before the replaced span is unchanged.
                self.applied += 1;
                let cut = cut_at(prefix, s.replace_chars);
                let ok = s
                    .apply_to(prefix)
                    .is_some_and(|a| a.as_bytes().get(..cut) == Some(&prefix.as_bytes()[..cut]));
                if !ok {
                    self.prefix_violations += 1;
                }
                if s.kind == SuggestionKind::Continuation {
                    self.continuations_shown += 1;
                    if matches_following(message, r.point.pos, s) {
                        self.continuations_full_match += 1;
                    }
                }
            }
            if !r.point.is_prediction_point() {
                continue;
            }
            let hit = match r.point.kind {
                PointKind::Partial { .. } => {
                    self.partial_points += 1;
                    let hit = sugg
                        .iter()
                        .any(|s| Some(s.text.as_str()) == r.point.intended.as_deref());
                    self.partial_hits += hit as u64;
                    hit
                }
                PointKind::Boundary => {
                    self.boundary_points += 1;
                    let hit = sugg
                        .iter()
                        .any(|s| Some(first_word(s)) == r.point.intended.as_deref());
                    self.boundary_hits += hit as u64;
                    hit
                }
                PointKind::Other => false,
            };
            if hit {
                self.correct_present += 1;
                let first = sugg.first();
                let first_hit = first.is_some_and(|s| match r.point.kind {
                    PointKind::Boundary => Some(first_word(s)) == r.point.intended.as_deref(),
                    _ => Some(s.text.as_str()) == r.point.intended.as_deref(),
                });
                self.correct_first += first_hit as u64;
            }
            if let Some(first) = sugg.first() {
                self.nonempty += 1;
                self.nonempty_first_correct += correct(message, r, first) as u64;
            }
        }
    }
}

pub fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}

/// Keystroke-savings simulation (SC-003). Returns (characters, keystrokes typed, acceptances).
///
/// The simulated agent types the message one character at a time and accepts a suggestion
/// whenever applying it yields exactly the following text of the message (ending at a word
/// boundary), preferring the longest. A leading space inserted by a suggestion counts as saved;
/// the space after an accepted suggestion is typed. Restoring accents saves nothing, because the
/// replaced characters were already typed.
pub fn simulate_savings(engine: &Engine, message: &str, next_id: &mut u64) -> (u64, u64, u64) {
    let total = message.chars().count() as u64;
    let (mut pos, mut typed, mut accepted) = (0usize, 0u64, 0u64);
    while pos < message.len() {
        *next_id += 1;
        let r = engine.predict(&PredictionRequest::new(*next_id, &message[..pos]));
        let mut best: Option<usize> = None;
        for s in &r.suggestions {
            if !matches_following(message, pos, s) {
                continue;
            }
            let cut = cut_at(&message[..pos], s.replace_chars);
            let end = cut + usize::from(s.leading_space) + s.text.len();
            if end > pos && best.is_none_or(|b| end > b) {
                best = Some(end);
            }
        }
        match best {
            Some(end) => {
                accepted += 1;
                pos = end;
            }
            None => {
                typed += 1;
                pos += message[pos..].chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    (total, typed, accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use predi_core::{PredictionResult, Status};

    fn sugg(text: &str, kind: SuggestionKind, replace: u32, space: bool) -> Suggestion {
        Suggestion {
            text: text.into(),
            kind,
            replace_chars: replace,
            replace_utf16: replace,
            leading_space: space,
            rank: 1,
            confidence: 1.0,
        }
    }

    #[test]
    fn following_match() {
        let m = "Hola, buenos días";
        assert!(matches_following(
            m,
            5,
            &sugg("buenos", SuggestionKind::NextWord, 0, true)
        ));
        assert!(matches_following(
            m,
            8,
            &sugg("buenos", SuggestionKind::Completion, 2, false)
        ));
        assert!(!matches_following(
            m,
            8,
            &sugg("buena", SuggestionKind::Completion, 2, false)
        ));
        assert!(matches_following(
            m,
            6,
            &sugg("buenos días", SuggestionKind::Continuation, 0, false)
        ));
    }

    #[test]
    fn counts_points() {
        let msg = "Hola, buenos";
        let replayed: Vec<Replayed> = super::super::replay::points(msg)
            .into_iter()
            .map(|p| {
                let suggestions = if p.pos == 8 {
                    vec![sugg("buenos", SuggestionKind::Completion, 2, false)]
                } else {
                    vec![]
                };
                Replayed {
                    point: p,
                    result: PredictionResult {
                        request_id: 0,
                        suggestions,
                        status: Status::Ok,
                    },
                }
            })
            .collect();
        let spelling = SpellingReference {
            words: ["buenos".to_string()].into_iter().collect(),
            domain: HashSet::new(),
            allowlist: HashSet::new(),
        };
        let mut q = QualityCounts::default();
        q.add_message(msg, &replayed, &spelling);
        // hola: 2 partial points; buenos: 4 partial points; 1 boundary.
        assert_eq!((q.partial_points, q.partial_hits), (6, 1));
        assert_eq!(q.boundary_points, 1);
        assert_eq!((q.nonempty, q.nonempty_first_correct), (1, 1));
        assert_eq!(q.prefix_violations, 0);
        assert_eq!(q.misspelled_or_personal, 0);
    }
}
