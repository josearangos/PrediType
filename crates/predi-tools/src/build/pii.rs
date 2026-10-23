//! Personal-data filter and build-time scan (T021, research R8, FR-014, FR-028, SC-005).
//!
//! - `{PLACEHOLDER}` tokens, and tokens with digits, `@` or URL-like form, are non-word tokens for
//!   the shared tokenizer, so they never become vocabulary; they're counted here.
//! - A word seen capitalized mid-sentence but never lowercase mid-sentence is a likely proper
//!   name and is dropped unless it's on the allowlist.
//! - After assembly, the finished vocabulary is scanned, and the build fails on any match.

use crate::util::{Result, ToolError};
use predi_core::casing::LexCasing;
use predi_core::normalize::lower;
use std::collections::{BTreeMap, HashSet};

/// How a word was seen in the sources.
#[derive(Debug, Clone, Default)]
pub struct WordStats {
    /// Weighted occurrence count.
    pub count: f64,
    /// Mid-sentence occurrences written in lowercase.
    pub mid_lower: u64,
    /// Mid-sentence occurrences starting with a capital.
    pub mid_capital: u64,
}

/// Counts reported in the build report.
#[derive(Debug, Clone, Default)]
pub struct PiiReport {
    /// Non-word tokens (placeholders, numbers, emails, URLs, code) seen in the sources.
    pub non_word_tokens: u64,
    /// Words dropped as likely proper names, with their weighted counts.
    pub dropped_names: Vec<(String, f64)>,
}

/// The approved list, keyed by lowercase form, mapping to the standard spelling.
pub struct Allowlist {
    pub by_lower: BTreeMap<String, String>,
}

impl Allowlist {
    pub fn new(entries: &[String]) -> Allowlist {
        Allowlist {
            by_lower: entries.iter().map(|e| (lower(e), e.clone())).collect(),
        }
    }

    pub fn contains(&self, lower_word: &str) -> bool {
        self.by_lower.contains_key(lower_word)
    }

    pub fn standard(&self, lower_word: &str) -> Option<&str> {
        self.by_lower.get(lower_word).map(String::as_str)
    }

    /// Casing for an allowlisted entry: all-capitals with ≥ 2 letters is an acronym.
    pub fn casing(&self, lower_word: &str) -> LexCasing {
        match self.standard(lower_word) {
            Some(s)
                if s.chars().filter(|c| c.is_alphabetic()).count() >= 2
                    && s.chars()
                        .filter(|c| c.is_alphabetic())
                        .all(char::is_uppercase) =>
            {
                LexCasing::Acronym
            }
            Some(_) => LexCasing::Capitalized,
            None => LexCasing::Lower,
        }
    }
}

/// Drop likely proper names (R8). Returns the dropped words, sorted by count then text.
pub fn drop_proper_names(
    stats: &mut BTreeMap<String, WordStats>,
    allow: &Allowlist,
) -> Vec<(String, f64)> {
    let mut dropped: Vec<(String, f64)> = Vec::new();
    stats.retain(|w, s| {
        let name_like = s.mid_capital > 0 && s.mid_lower == 0;
        if name_like && !allow.contains(w) {
            dropped.push((w.clone(), s.count));
            false
        } else {
            true
        }
    });
    dropped.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    dropped
}

fn looks_like_contact_or_number(word: &str) -> bool {
    word.chars()
        .any(|c| c.is_numeric() || matches!(c, '@' | '.' | '/' | ':' | '_' | '+'))
        || word.contains("www")
        || word.starts_with("http")
}

/// Scan the finished vocabulary (FR-028): fail on email/phone/URL patterns, digit runs, or a
/// `Capitalized` entry that is a given name and not on the allowlist. Lowercase words that are
/// also names (`luz`, `paz`) pass.
pub fn scan_vocabulary(
    entries: &[(String, LexCasing)],
    given_names: &[String],
    allow: &Allowlist,
) -> Result<()> {
    let problems = find_personal_data(entries, given_names, allow);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ToolError::invalid(format!(
            "personal-data scan failed ({} entries): {}",
            problems.len(),
            problems.join("; ")
        )))
    }
}

/// The scan behind [`scan_vocabulary`]: one message per matching entry.
pub fn find_personal_data(
    entries: &[(String, LexCasing)],
    given_names: &[String],
    allow: &Allowlist,
) -> Vec<String> {
    let names: HashSet<String> = given_names.iter().map(|n| lower(n)).collect();
    let mut problems = Vec::new();
    for (spelling, casing) in entries {
        let low = lower(spelling);
        if looks_like_contact_or_number(spelling) {
            problems.push(format!("'{spelling}' looks like a number, email or URL"));
        } else if *casing != LexCasing::Lower && names.contains(&low) && !allow.contains(&low) {
            problems.push(format!("'{spelling}' is a capitalized given name"));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_mid_sentence_capitalized_only() {
        let allow = Allowlist::new(&["Bogotá".to_string()]);
        let mut stats = BTreeMap::new();
        stats.insert(
            "tom".to_string(),
            WordStats {
                count: 9.0,
                mid_lower: 0,
                mid_capital: 4,
            },
        );
        stats.insert(
            "bogotá".to_string(),
            WordStats {
                count: 5.0,
                mid_lower: 0,
                mid_capital: 5,
            },
        );
        stats.insert(
            "casa".to_string(),
            WordStats {
                count: 5.0,
                mid_lower: 5,
                mid_capital: 1,
            },
        );
        stats.insert(
            "hola".to_string(),
            WordStats {
                count: 5.0,
                mid_lower: 0,
                mid_capital: 0,
            },
        );
        let dropped = drop_proper_names(&mut stats, &allow);
        assert_eq!(dropped, vec![("tom".to_string(), 9.0)]);
        assert!(stats.contains_key("bogotá") && stats.contains_key("casa"));
        assert!(stats.contains_key("hola"));
    }

    #[test]
    fn allowlist_casing() {
        let allow = Allowlist::new(&["PSE".into(), "Bogotá".into(), "WhatsApp".into()]);
        assert_eq!(allow.casing("pse"), LexCasing::Acronym);
        assert_eq!(allow.casing("bogotá"), LexCasing::Capitalized);
        assert_eq!(allow.casing("whatsapp"), LexCasing::Capitalized);
        assert_eq!(allow.casing("casa"), LexCasing::Lower);
    }

    #[test]
    fn scan() {
        let allow = Allowlist::new(&["Cali".into()]);
        let names = vec!["Tom".to_string(), "Luz".to_string()];
        let ok = vec![
            ("luz".to_string(), LexCasing::Lower),
            ("Cali".to_string(), LexCasing::Capitalized),
        ];
        assert!(scan_vocabulary(&ok, &names, &allow).is_ok());
        let bad = vec![("Tom".to_string(), LexCasing::Capitalized)];
        assert!(scan_vocabulary(&bad, &names, &allow).is_err());
        let bad = vec![("abc123".to_string(), LexCasing::Lower)];
        assert!(scan_vocabulary(&bad, &names, &allow).is_err());
    }
}
