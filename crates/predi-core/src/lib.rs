//! PrediType Spanish prediction engine (contracts/engine-api.md, version 1).
//!
//! The engine turns the text before the caret into 0–3 ranked suggestions. It does no I/O, keeps
//! no text between calls, and compiles to native code and WebAssembly from the same source.

#![forbid(unsafe_code)]
#![deny(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::dbg_macro,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

pub mod asset;
pub mod casing;
pub mod model;
pub mod normalize;
pub mod rank;
pub mod state;
pub mod tokenize;
pub mod types;

pub use types::{
    AssetError, EngineInfo, PredictionRequest, PredictionResult, Status, Suggestion, SuggestionKind,
};

use asset::Asset;
use casing::CaseContext;
use model::{Candidate, IdContext};
use rank::{Ranked, Span};
use state::EngineState;
use tokenize::Ending;

/// Version of this engine, reported in evaluation reports.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The prediction engine. `Send + Sync`: safe to call from a worker thread.
pub struct Engine {
    asset: Asset,
    state: EngineState,
}

const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Engine>();
};

impl Engine {
    /// Build an engine from asset bytes. No filesystem or network access (G5). Fails with an
    /// [`AssetError`] on a bad magic number, unsupported format, bad checksum or malformed
    /// content; no engine exists after a failure (G9).
    pub fn from_bytes(asset: &[u8]) -> Result<Engine, AssetError> {
        Ok(Engine {
            asset: Asset::from_bytes(asset)?,
            state: EngineState::default(),
        })
    }

    /// Predict for one request. Pure with respect to text: nothing from `req` is retained (G8).
    pub fn predict(&self, req: &PredictionRequest<'_>) -> PredictionResult {
        let id = req.request_id;
        if self.state.is_paused() {
            return PredictionResult::empty(id, Status::Paused);
        }
        let analysis = tokenize::analyze(req.text_before_caret);
        if analysis.not_applicable {
            return PredictionResult::empty(id, Status::NotApplicable);
        }
        let ctx = model::context_ids(&self.asset, &analysis.context);
        let (suggestions, status) = match analysis.ending {
            Ending::InsideWord {
                partial,
                chars,
                utf16,
                ..
            } => {
                let case = CaseContext::new(analysis.before, partial);
                let cands = model::completion_candidates(&self.asset, ctx, partial);
                let total: f64 = cands.iter().map(|c| c.score).sum();
                let ranked = cands
                    .iter()
                    .map(|c| self.ranked(c, &case, SuggestionKind::Completion))
                    .collect();
                let span = Span {
                    replace_chars: chars,
                    replace_utf16: utf16,
                    leading_space: false,
                };
                rank::rank(ranked, total, self.asset.tau_completion, span, None)
            }
            Ending::Boundary | Ending::Empty => {
                let case = CaseContext::new(analysis.before, "");
                let (cands, total) = model::next_word_candidates(&self.asset, ctx);
                let ranked: Vec<Ranked> = cands
                    .iter()
                    .map(|c| self.ranked(c, &case, SuggestionKind::NextWord))
                    .collect();
                let cont = self.continuation(&ranked, &cands, total, ctx, &case);
                let span = Span {
                    replace_chars: 0,
                    replace_utf16: 0,
                    leading_space: analysis.leading_space,
                };
                rank::rank(ranked, total, self.asset.tau_next, span, cont)
            }
        };
        PredictionResult {
            request_id: id,
            suggestions,
            status,
        }
    }

    /// Subsequent `predict` calls return an empty result with status `Paused` (G7).
    pub fn pause(&self) {
        self.state.pause();
    }

    pub fn resume(&self) {
        self.state.resume();
    }

    /// Clients call this whenever focus moves to another field. The engine holds no text, so
    /// results afterwards already equal those of a fresh engine (FR-024, R9).
    pub fn field_changed(&self) {}

    pub fn info(&self) -> EngineInfo {
        EngineInfo {
            engine_version: ENGINE_VERSION,
            asset_version: self.asset.asset_version.clone(),
            format_version: asset::layout::FORMAT_VERSION,
            tau_completion: self.asset.tau_completion,
            tau_next: self.asset.tau_next,
            alpha: self.asset.alpha,
            vocabulary_size: self.asset.n_words(),
        }
    }

    fn ranked(&self, c: &Candidate, case: &CaseContext, kind: SuggestionKind) -> Ranked {
        let id = c.ids[0];
        let canonical = self.asset.spelling(id);
        Ranked {
            text: casing::apply(canonical, self.asset.casing(id), case),
            canonical: canonical.to_string(),
            kind,
            score: c.score,
        }
    }

    /// The continuation built from the top next word, if it qualifies (R7).
    fn continuation(
        &self,
        ranked: &[Ranked],
        cands: &[Candidate],
        total: f64,
        ctx: IdContext,
        case: &CaseContext,
    ) -> Option<Ranked> {
        if total <= 0.0 {
            return None;
        }
        let top = (0..ranked.len()).min_by(|&a, &b| {
            ranked[b]
                .score
                .total_cmp(&ranked[a].score)
                .then_with(|| ranked[a].canonical.cmp(&ranked[b].canonical))
        })?;
        let share = cands[top].score / total;
        if share < self.asset.tau_next as f64 {
            return None;
        }
        let cont = model::continuation(&self.asset, ctx, cands[top].ids[0], share)?;
        let rest_case = CaseContext {
            sentence_start: false,
            partial_all_caps: false,
            partial_initial_cap: false,
        };
        let mut words = Vec::with_capacity(cont.ids.len());
        let mut canonical = Vec::with_capacity(cont.ids.len());
        for (i, &id) in cont.ids.iter().enumerate() {
            let spelling = self.asset.spelling(id);
            // Only the first word follows sentence casing; the rest keep lexicon casing (R5).
            let c = if i == 0 { case } else { &rest_case };
            words.push(casing::apply(spelling, self.asset.casing(id), c));
            canonical.push(spelling);
        }
        Some(Ranked {
            text: words.join(" "),
            canonical: canonical.join(" "),
            kind: SuggestionKind::Continuation,
            score: cont.score,
        })
    }
}
