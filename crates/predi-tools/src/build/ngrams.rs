//! N-gram counting, pruning, quantization and top-next lists (T036, research R2, R10).

use super::lexicon::{Lexicon, Text};
use predi_core::asset::layout::{MAX_LIST, STEPS_PER_OCTAVE, pack2, pack3};
use predi_core::model::PREFIX_LIST_CHARS;
use predi_core::normalize::{lower, nfc};
use predi_core::tokenize::{Token, tokens};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy)]
pub struct NgramParams {
    /// Minimum weighted count for a stored bigram.
    pub min_bigram: f64,
    /// Minimum weighted count for a stored trigram.
    pub min_trigram: f64,
}

impl Default for NgramParams {
    fn default() -> Self {
        Self {
            min_bigram: 2.0,
            min_trigram: 2.0,
        }
    }
}

/// Everything the writer needs for the n-gram and top-next sections.
pub struct NgramModel {
    pub dequant: Vec<f64>,
    /// Quantized unigram probability per id (words, then `<s>`, then `</s>`).
    pub unigram: Vec<u16>,
    pub bigrams: Vec<(u64, u16)>,
    pub trigrams: Vec<(u64, u16)>,
    /// Shared pool of ranked word-id lists. The global list comes first.
    pub pool: Vec<u32>,
    pub global_len: u32,
    /// Offsets into `pool` of the next-word list per one-id context (n_ids + 1 entries).
    pub uni_offsets: Vec<u32>,
    /// Two-id contexts (packed, sorted) and offsets into `pool` (len + 1 entries).
    pub bictx_keys: Vec<u64>,
    pub bictx_offsets: Vec<u32>,
    /// Folded prefix (1–3 chars) → (pool offset, length).
    pub prefix_lists: BTreeMap<String, (u32, u8)>,
    pub stats: NgramStats,
}

#[derive(Debug, Clone, Default)]
pub struct NgramStats {
    pub tokens: f64,
    pub bigrams_seen: usize,
    pub trigrams_seen: usize,
    pub bigrams_kept: usize,
    pub trigrams_kept: usize,
}

/// q = round(-log2(p) · 1024), clamped to u16. Build time only.
pub fn quantize(p: f64) -> u16 {
    if p.is_nan() || p <= 0.0 {
        return u16::MAX;
    }
    let q = (-p.log2() * STEPS_PER_OCTAVE as f64).round();
    q.clamp(0.0, u16::MAX as f64) as u16
}

/// frac[k] = 2^(-k/1024), stored in the asset so runtime needs no platform math (G4).
pub fn dequant_table() -> Vec<f64> {
    (0..STEPS_PER_OCTAVE)
        .map(|k| 2f64.powf(-(k as f64) / STEPS_PER_OCTAVE as f64))
        .collect()
}

/// Map each text to sentences of ids (`None` = gap), each starting with `<s>` and ending
/// with `</s>`.
fn sentences(texts: &[Text], lex: &Lexicon, bos: u32, eos: u32) -> Vec<(Vec<Option<u32>>, f64)> {
    let mut out = Vec::new();
    for t in texts {
        let mut cur: Vec<Option<u32>> = vec![Some(bos)];
        for tok in tokens(&t.text) {
            match tok {
                Token::Word(w) => cur.push(lex.id_of(&lower(&nfc(w)))),
                Token::NonWord => cur.push(None),
                Token::SentenceEnd => {
                    if cur.len() > 1 {
                        cur.push(Some(eos));
                        out.push((std::mem::replace(&mut cur, vec![Some(bos)]), t.weight));
                    }
                }
            }
        }
        if cur.len() > 1 {
            cur.push(Some(eos));
            out.push((cur, t.weight));
        }
    }
    out
}

fn ranked_list(mut items: Vec<(u32, f64)>, lex: &Lexicon) -> Vec<u32> {
    let spelling = |id: u32| {
        lex.entries
            .get(id as usize)
            .map(|e| e.canonical.as_str())
            .unwrap_or("")
    };
    items.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| spelling(a.0).cmp(spelling(b.0)))
    });
    items.into_iter().take(MAX_LIST).map(|(id, _)| id).collect()
}

/// Count, prune and quantize n-grams, and precompute the top-next and prefix lists.
pub fn build_ngrams(texts: &[Text], lex: &Lexicon, params: NgramParams) -> NgramModel {
    let n_words = lex.entries.len() as u32;
    let (bos, eos) = (n_words, n_words + 1);
    let n_ids = n_words as usize + 2;
    let usable = |id: u32| id < n_words && !lex.entries[id as usize].register_other;

    let mut uni = vec![0f64; n_ids];
    let mut bi: HashMap<u64, f64> = HashMap::new();
    let mut tri: HashMap<u64, f64> = HashMap::new();
    for (sent, w) in sentences(texts, lex, bos, eos) {
        for (i, id) in sent.iter().enumerate() {
            if let Some(id) = id
                && *id != bos
            {
                uni[*id as usize] += w;
            }
            if i >= 1
                && let (Some(a), Some(b)) = (sent[i - 1], sent[i])
            {
                *bi.entry(pack2(a, b)).or_default() += w;
            }
            if i >= 2
                && let (Some(a), Some(b), Some(c)) = (sent[i - 2], sent[i - 1], sent[i])
            {
                *tri.entry(pack3(a, b, c)).or_default() += w;
            }
        }
    }

    let total: f64 = uni.iter().sum();
    let mut unigram: Vec<u16> = uni
        .iter()
        .map(|&c| {
            if total > 0.0 {
                quantize(c / total)
            } else {
                u16::MAX
            }
        })
        .collect();
    unigram[bos as usize] = u16::MAX;

    // Context totals before pruning, so probabilities stay true conditionals.
    let mut ctx1: HashMap<u32, f64> = HashMap::new();
    for (&k, &c) in &bi {
        *ctx1.entry((k >> 21) as u32).or_default() += c;
    }
    let mut ctx2: HashMap<u64, f64> = HashMap::new();
    for (&k, &c) in &tri {
        *ctx2.entry(k >> 21).or_default() += c;
    }

    let mut stats = NgramStats {
        tokens: total,
        bigrams_seen: bi.len(),
        trigrams_seen: tri.len(),
        ..Default::default()
    };

    let mut bigrams: Vec<(u64, u16)> = Vec::new();
    let mut uni_lists: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n_ids];
    for (&k, &c) in &bi {
        if c < params.min_bigram {
            continue;
        }
        let a = (k >> 21) as u32;
        let b = (k & ((1 << 21) - 1)) as u32;
        let p = c / ctx1[&a];
        bigrams.push((k, quantize(p)));
        if usable(b) {
            uni_lists[a as usize].push((b, p));
        }
    }
    bigrams.sort_unstable_by_key(|e| e.0);
    stats.bigrams_kept = bigrams.len();

    let mut trigrams: Vec<(u64, u16)> = Vec::new();
    let mut bi_lists: BTreeMap<u64, Vec<(u32, f64)>> = BTreeMap::new();
    for (&k, &c) in &tri {
        if c < params.min_trigram {
            continue;
        }
        let ctx = k >> 21;
        let w = (k & ((1 << 21) - 1)) as u32;
        let p = c / ctx2[&ctx];
        trigrams.push((k, quantize(p)));
        if usable(w) {
            bi_lists.entry(ctx).or_default().push((w, p));
        }
    }
    trigrams.sort_unstable_by_key(|e| e.0);
    stats.trigrams_kept = trigrams.len();

    // Pool: global list, then one-id contexts by id, then two-id contexts, then prefix lists.
    let global: Vec<(u32, f64)> = (0..n_words)
        .filter(|&id| usable(id))
        .map(|id| (id, uni[id as usize]))
        .collect();
    let global = ranked_list(global, lex);
    let mut pool: Vec<u32> = global.clone();
    let global_len = pool.len() as u32;

    let mut uni_offsets = Vec::with_capacity(n_ids + 1);
    uni_offsets.push(pool.len() as u32);
    for list in uni_lists {
        pool.extend(ranked_list(list, lex));
        uni_offsets.push(pool.len() as u32);
    }

    let mut bictx_keys = Vec::new();
    let mut bictx_offsets = vec![pool.len() as u32];
    for (ctx, list) in bi_lists {
        bictx_keys.push(ctx);
        pool.extend(ranked_list(list, lex));
        bictx_offsets.push(pool.len() as u32);
    }

    let mut by_prefix: BTreeMap<String, Vec<(u32, f64)>> = BTreeMap::new();
    for (id, e) in lex.entries.iter().enumerate() {
        if e.register_other {
            continue;
        }
        let chars: Vec<char> = e.folded.chars().collect();
        for len in 1..=PREFIX_LIST_CHARS.min(chars.len()) {
            let p: String = chars[..len].iter().collect();
            by_prefix.entry(p).or_default().push((id as u32, e.count));
        }
    }
    let mut prefix_lists = BTreeMap::new();
    for (p, list) in by_prefix {
        let ids = ranked_list(list, lex);
        prefix_lists.insert(p, (pool.len() as u32, ids.len() as u8));
        pool.extend(ids);
    }

    NgramModel {
        dequant: dequant_table(),
        unigram,
        bigrams,
        trigrams,
        pool,
        global_len,
        uni_offsets,
        bictx_keys,
        bictx_offsets,
        prefix_lists,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantization_round_trips_closely() {
        let table = dequant_table();
        for p in [1.0, 0.5, 0.3, 1e-3, 1e-7] {
            let q = quantize(p) as u32;
            let back = table[(q % 1024) as usize] * 2f64.powi(-((q / 1024) as i32));
            assert!((back - p).abs() / p < 0.001, "{p} -> {back}");
        }
        assert_eq!(quantize(0.0), u16::MAX);
    }
}
