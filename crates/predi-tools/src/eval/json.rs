//! JSON views of engine results (for `predi-cli` and determinism checks).

use predi_core::{PredictionResult, Suggestion, SuggestionKind};
use serde::Serialize;

#[derive(Serialize)]
pub struct SuggestionJson<'a> {
    pub text: &'a str,
    pub kind: &'static str,
    pub replace_chars: u32,
    pub replace_utf16: u32,
    pub leading_space: bool,
    pub rank: u8,
    pub confidence: f32,
}

#[derive(Serialize)]
pub struct ResultJson<'a> {
    pub request_id: u64,
    pub suggestions: Vec<SuggestionJson<'a>>,
    pub status: &'static str,
}

pub fn kind_str(k: SuggestionKind) -> &'static str {
    match k {
        SuggestionKind::Completion => "Completion",
        SuggestionKind::NextWord => "NextWord",
        SuggestionKind::Continuation => "Continuation",
    }
}

fn suggestion(s: &Suggestion) -> SuggestionJson<'_> {
    SuggestionJson {
        text: &s.text,
        kind: kind_str(s.kind),
        replace_chars: s.replace_chars,
        replace_utf16: s.replace_utf16,
        leading_space: s.leading_space,
        rank: s.rank,
        confidence: s.confidence,
    }
}

pub fn result(r: &PredictionResult) -> ResultJson<'_> {
    ResultJson {
        request_id: r.request_id,
        suggestions: r.suggestions.iter().map(suggestion).collect(),
        status: r.status.as_str(),
    }
}

pub fn to_string(r: &PredictionResult) -> String {
    serde_json::to_string(&result(r)).unwrap_or_default()
}
