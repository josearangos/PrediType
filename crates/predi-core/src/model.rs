//! Candidate generation and Stupid Backoff scoring (research R2, R4, R6, R7).

use crate::asset::Asset;
use crate::normalize::{fold, lower, matches_exact_diacritics, nfc, restores_diacritics};
use crate::tokenize::Ctx;
use fst::{Automaton, IntoStreamer, Streamer};

/// At most this many FST keys are visited per prefix lookup (bounded work, G6).
pub const MAX_PREFIX_KEYS: usize = 5000;
/// Prefixes up to this many characters also use the precomputed prefix lists.
pub const PREFIX_LIST_CHARS: usize = 3;
/// Ranking bonus for a candidate that matches the typed letters including diacritics (R4).
pub const EXACT_DIACRITIC_BONUS: f64 = 2.0;
/// Continuations have at most this many words (FR-010).
pub const MAX_CONTINUATION_WORDS: usize = 4;

/// A scored candidate word (or continuation) before casing and ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Word ids; one for a word, 2–4 for a continuation.
    pub ids: Vec<u32>,
    pub score: f64,
}

/// The n-gram context as word ids: `None` is unknown (gap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdContext {
    pub prev2: Option<u32>,
    pub prev1: Option<u32>,
}

impl IdContext {
    pub fn push(self, id: u32) -> IdContext {
        IdContext {
            prev2: self.prev1,
            prev1: Some(id),
        }
    }
}

/// Map a lowercase NFC word to its lexicon id (exact spelling, case-insensitive).
pub fn word_id(asset: &Asset, word_lower_nfc: &str) -> Option<u32> {
    asset
        .lookup_folded(&fold(word_lower_nfc))
        .find(|&id| lower(asset.spelling(id)) == word_lower_nfc)
}

pub fn context_ids(asset: &Asset, ctx: &[Ctx; 2]) -> IdContext {
    let map = |c: &Ctx| match c {
        Ctx::Start => Some(asset.bos()),
        Ctx::Word(w) => word_id(asset, w),
        Ctx::Gap => None,
    };
    IdContext {
        prev2: map(&ctx[0]),
        prev1: map(&ctx[1]),
    }
}

/// Stupid Backoff score S(w | context): the highest-order stored n-gram, times α per backoff step.
pub fn score(asset: &Asset, ctx: IdContext, w: u32) -> f64 {
    let alpha = asset.alpha as f64;
    let mut factor = 1.0f64;
    if let (Some(a), Some(b)) = (ctx.prev2, ctx.prev1) {
        if let Some(p) = asset.trigram(a, b, w) {
            return p;
        }
        factor *= alpha;
    }
    if let Some(b) = ctx.prev1 {
        if let Some(p) = asset.bigram(b, w) {
            return factor * p;
        }
        factor *= alpha;
    }
    factor * asset.unigram(w)
}

/// Push `id` into a sorted, de-duplicated id list.
fn insert_id(ids: &mut Vec<u32>, id: u32) {
    if let Err(pos) = ids.binary_search(&id) {
        ids.insert(pos, id);
    }
}

/// The context's ranked next-word lists, most specific first.
fn context_lists(asset: &Asset, ctx: IdContext) -> Vec<u32> {
    let mut ids = Vec::new();
    if let (Some(a), Some(b)) = (ctx.prev2, ctx.prev1) {
        for id in asset.next_after2(a, b) {
            insert_id(&mut ids, id);
        }
    }
    if let Some(b) = ctx.prev1 {
        for id in asset.next_after1(b) {
            insert_id(&mut ids, id);
        }
    }
    ids
}

/// Completion candidates for a partial word (FR-008, FR-016, FR-022), scored with context
/// (R2: context candidates filtered by the folded prefix, then prefix-index candidates).
pub fn completion_candidates(asset: &Asset, ctx: IdContext, partial: &str) -> Vec<Candidate> {
    let partial_nfc = nfc(partial);
    let partial_lower = lower(&partial_nfc);
    let partial_len = partial_nfc.chars().count();
    let folded = fold(&partial_nfc);
    if folded.is_empty() {
        return Vec::new();
    }

    let mut ids: Vec<u32> = Vec::new();
    // Context lists filtered by the folded prefix.
    for id in context_lists(asset, ctx) {
        if id < asset.n_words() && fold(asset.spelling(id)).starts_with(&folded) {
            insert_id(&mut ids, id);
        }
    }
    // Precomputed frequent completions of short prefixes.
    if folded.chars().count() <= PREFIX_LIST_CHARS {
        for id in asset.prefix_list(&folded) {
            insert_id(&mut ids, id);
        }
    }
    // Prefix index: FST range over folded keys, deterministic order, bounded.
    let matcher = fst::automaton::Str::new(&folded).starts_with();
    let mut stream = asset.lexicon().search(matcher).into_stream();
    let mut visited = 0usize;
    while let Some((_, v)) = stream.next() {
        for id in asset.ids_for_value(v) {
            insert_id(&mut ids, id);
        }
        visited += 1;
        if visited >= MAX_PREFIX_KEYS {
            break;
        }
    }

    // Eligibility (FR-008): not the identical word; adds ≥ 2 characters or restores diacritics.
    let mut eligible: Vec<u32> = Vec::with_capacity(ids.len());
    let mut any_unflagged = false;
    for &id in &ids {
        let spelling = asset.spelling(id);
        if lower(spelling) == partial_lower {
            continue;
        }
        let added = spelling.chars().count() as isize - partial_len as isize;
        if added >= 2 || restores_diacritics(&partial_nfc, spelling) {
            if !asset.register_other(id) {
                any_unflagged = true;
            }
            eligible.push(id);
        }
    }
    // Register filter (FR-022): tú-form and other-variant words only once nothing else matches.
    if any_unflagged {
        eligible.retain(|&id| !asset.register_other(id));
    }

    eligible
        .into_iter()
        .map(|id| {
            let mut s = score(asset, ctx, id);
            if matches_exact_diacritics(&partial_nfc, asset.spelling(id)) {
                s *= EXACT_DIACRITIC_BONUS;
            }
            Candidate {
                ids: vec![id],
                score: s,
            }
        })
        .collect()
}

/// Next-word candidates at a boundary (FR-009, FR-011, FR-022).
///
/// The returned total includes the probability of ending the sentence, so a likely sentence end
/// lowers every word's confidence; `</s>` itself is never a candidate.
pub fn next_word_candidates(asset: &Asset, ctx: IdContext) -> (Vec<Candidate>, f64) {
    let mut ids = context_lists(asset, ctx);
    for id in asset.global_next() {
        insert_id(&mut ids, id);
    }
    // The chance of ending the sentence here counts toward the total.
    let mut total = score(asset, ctx, asset.eos());
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if id >= asset.n_words() || asset.register_other(id) {
            continue;
        }
        let s = score(asset, ctx, id);
        total += s;
        out.push(Candidate {
            ids: vec![id],
            score: s,
        });
    }
    (out, total)
}

/// The single most likely next id after `ctx` (may be `</s>`), excluding flagged words, with its
/// share of the next-word distribution (the same measure as a suggestion's confidence).
fn best_next(asset: &Asset, ctx: IdContext) -> (u32, f64) {
    let (cands, total) = next_word_candidates(asset, ctx);
    let mut best = (asset.eos(), score(asset, ctx, asset.eos()));
    for c in &cands {
        let id = c.ids[0];
        if c.score > best.1 || (c.score == best.1 && asset.spelling(id) < asset.spelling(best.0)) {
            best = (id, c.score);
        }
    }
    let share = if total > 0.0 { best.1 / total } else { 0.0 };
    (best.0, share)
}

/// Greedy continuation from `first` (R7): extend with the most likely next word while its share
/// of the next-word distribution is at least `tau_next`, up to 4 words, stopping at a sentence
/// end. The overall score is the product of the shares, starting with the first word's.
/// Returns `None` unless it has at least 2 words and an overall score of at least `tau_next`.
pub fn continuation(
    asset: &Asset,
    ctx: IdContext,
    first: u32,
    first_share: f64,
) -> Option<Candidate> {
    let tau = asset.tau_next as f64;
    let mut ids = vec![first];
    let mut overall = first_share;
    let mut cur = ctx.push(first);
    while ids.len() < MAX_CONTINUATION_WORDS {
        let (next, share) = best_next(asset, cur);
        if next == asset.eos() || share < tau || overall * share < tau {
            break;
        }
        overall *= share;
        ids.push(next);
        cur = cur.push(next);
    }
    if ids.len() >= 2 && overall >= tau {
        Some(Candidate {
            ids,
            score: overall,
        })
    } else {
        None
    }
}
