//! τ tuning on the dev split only (T058, research R6).
//!
//! The dev messages are replayed once with both cutoffs at 0. Raising τ only removes results
//! whose first-ranked confidence is below it, so for each τ the first-rank precision among
//! non-empty results follows from that single replay. We pick the smallest τ (per kind) with a
//! precision of at least 0.55.

use crate::eval::replay::{PointKind, replay_message};
use crate::util::{Result, ToolError};
use predi_core::{Engine, SuggestionKind};

pub const TARGET_PRECISION: f64 = 0.55;
/// τ grid: 0.00, 0.01, …, 0.99.
const GRID: usize = 100;

pub struct Tuned {
    pub tau_completion: f32,
    pub tau_next: f32,
    pub notes: Vec<String>,
}

/// First-ranked suggestion of a replayed prediction point: its confidence and correctness.
#[derive(Debug, Clone, Copy)]
struct Top {
    confidence: f32,
    correct: bool,
}

fn choose(tops: &[Top], label: &str, notes: &mut Vec<String>) -> f32 {
    let mut best: Option<(usize, f64, usize)> = None;
    let mut chosen = None;
    for step in 0..GRID {
        let tau = step as f32 / GRID as f32;
        let shown: Vec<&Top> = tops.iter().filter(|t| t.confidence >= tau).collect();
        if shown.is_empty() {
            break;
        }
        let correct = shown.iter().filter(|t| t.correct).count();
        let precision = correct as f64 / shown.len() as f64;
        if best.is_none_or(|(_, p, _)| precision > p) {
            best = Some((step, precision, shown.len()));
        }
        if precision >= TARGET_PRECISION {
            chosen = Some((step, precision, shown.len()));
            break;
        }
    }
    let (step, precision, shown, reached) = match (chosen, best) {
        (Some((s, p, n)), _) => (s, p, n, true),
        (None, Some((s, p, n))) => (s, p, n, false),
        (None, None) => (0, 0.0, 0, false),
    };
    let tau = step as f32 / GRID as f32;
    notes.push(format!(
        "{label}: {} dev prediction points; tau {tau:.2} gives first-rank precision {precision:.3} \
         on {shown} non-empty results ({:.1}% coverage){}",
        tops.len(),
        if tops.is_empty() {
            0.0
        } else {
            100.0 * shown as f64 / tops.len() as f64
        },
        if reached {
            ""
        } else {
            "; TARGET 0.55 NOT REACHED, using the most precise tau"
        }
    ));
    tau
}

/// Tune both cutoffs on the dev messages using `asset` (whose own τ values are ignored).
pub fn tune(asset: &[u8], dev: &[String]) -> Result<Tuned> {
    let mut untuned = asset.to_vec();
    super::writer::set_tau(&mut untuned, 0.0, 0.0);
    let engine = Engine::from_bytes(&untuned)
        .map_err(|e| ToolError::invalid(format!("asset does not load for tuning: {e}")))?;

    let mut completion = Vec::new();
    let mut next = Vec::new();
    let mut id = 0u64;
    for msg in dev {
        for r in replay_message(&engine, msg, &mut id) {
            if !r.point.is_prediction_point() {
                continue;
            }
            let Some(intended) = r.point.intended.as_deref() else {
                continue;
            };
            let Some(top) = r.result.suggestions.first() else {
                continue;
            };
            match r.point.kind {
                PointKind::Partial { .. } => completion.push(Top {
                    confidence: top.confidence,
                    correct: top.text == intended,
                }),
                PointKind::Boundary => {
                    let first = match top.kind {
                        SuggestionKind::Continuation => top.text.split(' ').next().unwrap_or(""),
                        _ => top.text.as_str(),
                    };
                    next.push(Top {
                        confidence: top.confidence,
                        correct: first == intended,
                    });
                }
                PointKind::Other => {}
            }
        }
    }
    let mut notes = Vec::new();
    let tau_completion = choose(&completion, "completions", &mut notes);
    let tau_next = choose(&next, "next words", &mut notes);
    Ok(Tuned {
        tau_completion,
        tau_next,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_smallest_tau_reaching_target() {
        let tops = vec![
            Top {
                confidence: 0.9,
                correct: true,
            },
            Top {
                confidence: 0.6,
                correct: true,
            },
            Top {
                confidence: 0.3,
                correct: false,
            },
            Top {
                confidence: 0.2,
                correct: false,
            },
            Top {
                confidence: 0.1,
                correct: false,
            },
        ];
        let mut notes = Vec::new();
        // tau 0.0: 2/5; 0.11: 2/4; 0.21: 2/3 ≥ 0.55.
        let tau = choose(&tops, "x", &mut notes);
        assert!((tau - 0.21).abs() < 1e-6, "{tau}");
    }
}
