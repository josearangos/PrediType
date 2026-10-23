//! Unigram counting and lexicon building (T022, T045, research R4, R8, FR-014, FR-019, FR-022).

use super::pii::{Allowlist, WordStats};
use predi_core::casing::LexCasing;
use predi_core::normalize::{fold, lower, nfc};
use predi_core::tokenize::{Token, tokens};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A source text with its domain weight.
#[derive(Debug, Clone)]
pub struct Text {
    pub text: String,
    pub weight: f64,
}

/// One vocabulary entry; its index in [`Lexicon::entries`] is its word id.
#[derive(Debug, Clone, PartialEq)]
pub struct LexEntry {
    /// Lowercase for `Lower` entries; the standard form (`Bogotá`, `PSE`) otherwise.
    pub canonical: String,
    pub folded: String,
    pub casing: LexCasing,
    pub register_other: bool,
    /// Weighted unigram count.
    pub count: f64,
}

pub struct Lexicon {
    pub entries: Vec<LexEntry>,
    /// Lowercase NFC form → word id.
    pub by_lower: HashMap<String, u32>,
}

impl Lexicon {
    pub fn id_of(&self, lower_word: &str) -> Option<u32> {
        self.by_lower.get(lower_word).copied()
    }
}

/// Count words: weighted counts plus mid-sentence casing (for the proper-name filter).
/// Returns the stats and the number of non-word tokens seen.
pub fn count_words(texts: &[Text]) -> (BTreeMap<String, WordStats>, u64) {
    let mut stats: BTreeMap<String, WordStats> = BTreeMap::new();
    let mut non_word = 0u64;
    for t in texts {
        let mut sentence_initial = true;
        for tok in tokens(&t.text) {
            match tok {
                Token::Word(w) => {
                    let w = nfc(w);
                    let s = stats.entry(lower(&w)).or_default();
                    s.count += t.weight;
                    if !sentence_initial {
                        if w.chars().next().is_some_and(char::is_uppercase) {
                            s.mid_capital += 1;
                        } else {
                            s.mid_lower += 1;
                        }
                    }
                    sentence_initial = false;
                }
                Token::NonWord => {
                    non_word += 1;
                    sentence_initial = false;
                }
                Token::SentenceEnd => sentence_initial = true,
            }
        }
    }
    (stats, non_word)
}

/// Lists used to validate and flag vocabulary.
pub struct LexiconInputs<'a> {
    /// Exact spellings from the expanded `es_CO` dictionary.
    pub dictionary: &'a HashSet<String>,
    /// Legitimate domain terms (lowercase).
    pub domain_words: &'a HashSet<String>,
    pub allowlist: &'a Allowlist,
    /// Tú-form and other-variant words (lowercase).
    pub register_flags: &'a HashSet<String>,
}

/// Build the lexicon from counted words (after the proper-name filter).
///
/// Spelling validation (T045, SC-005): a word is kept only if its lowercase form is in the
/// dictionary or the domain list, or it's allowlisted. Rejected words are returned with counts
/// for the build report. Word ids are assigned in (folded, canonical) order so that ids sharing a
/// folded key are contiguous.
pub fn build_lexicon(
    stats: &BTreeMap<String, WordStats>,
    inputs: &LexiconInputs<'_>,
) -> (Lexicon, Vec<(String, f64)>) {
    let mut entries = Vec::new();
    let mut rejected = Vec::new();
    for (w, s) in stats {
        let valid = inputs.dictionary.contains(w)
            || inputs.domain_words.contains(w)
            || inputs.allowlist.contains(w);
        if !valid {
            rejected.push((w.clone(), s.count));
            continue;
        }
        let casing = inputs.allowlist.casing(w);
        let canonical = match casing {
            LexCasing::Lower => w.clone(),
            _ => inputs.allowlist.standard(w).unwrap_or(w).to_string(),
        };
        entries.push(LexEntry {
            folded: fold(&canonical),
            canonical,
            casing,
            register_other: inputs.register_flags.contains(w),
            count: s.count,
        });
    }
    entries.sort_by(|a, b| {
        a.folded
            .cmp(&b.folded)
            .then_with(|| a.canonical.cmp(&b.canonical))
    });
    let by_lower = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (lower(&e.canonical), i as u32))
        .collect();
    rejected.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    (Lexicon { entries, by_lower }, rejected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_validates() {
        let texts = vec![
            Text {
                text: "Hola, Tom. ¿Cómo está Tom en Bogotá?".into(),
                weight: 1.0,
            },
            Text {
                text: "Su contraseña es nueva, gracias.".into(),
                weight: 10.0,
            },
        ];
        let (stats, non_word) = count_words(&texts);
        assert_eq!(non_word, 0);
        assert_eq!(stats["tom"].mid_capital, 2);
        assert_eq!(stats["contraseña"].count, 10.0);

        let dict: HashSet<String> = [
            "hola",
            "cómo",
            "está",
            "en",
            "su",
            "contraseña",
            "es",
            "nueva",
            "gracias",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let domain = HashSet::new();
        let allow = Allowlist::new(&["Bogotá".to_string()]);
        let flags: HashSet<String> = ["está".to_string()].into_iter().collect();
        let inputs = LexiconInputs {
            dictionary: &dict,
            domain_words: &domain,
            allowlist: &allow,
            register_flags: &flags,
        };
        let (lex, rejected) = build_lexicon(&stats, &inputs);
        assert!(rejected.iter().any(|(w, _)| w == "tom"));
        let bog = lex.id_of("bogotá").unwrap();
        assert_eq!(lex.entries[bog as usize].canonical, "Bogotá");
        assert_eq!(lex.entries[bog as usize].casing, LexCasing::Capitalized);
        assert!(lex.entries[lex.id_of("está").unwrap() as usize].register_other);
        // Ids are ordered by folded key.
        let folded: Vec<_> = lex.entries.iter().map(|e| e.folded.clone()).collect();
        let mut sorted = folded.clone();
        sorted.sort();
        assert_eq!(folded, sorted);
    }
}
