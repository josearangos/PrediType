//! Corpus split assignment (T014a, `predi-build split`, data-model DomainCorpus).
//!
//! Each message's NFC text is hashed with SHA-256. Hashes are ranked ascending: the first ⌈n/3⌉
//! are `test`, the next ⌈n/6⌉ `dev`, the rest `build`. A hash already in `splits.tsv` is never
//! reassigned.

use super::sources::{Split, read_messages, read_splits};
use crate::util::{Result, ToolError, write_bytes};
use predi_core::tokenize::{Token, tokens};
use std::collections::BTreeMap;
use std::path::Path;

pub const MIN_TEST_MESSAGES: usize = 500;
pub const MIN_DEV_MESSAGES: usize = 250;
pub const MIN_TEST_WORDS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitSummary {
    pub messages: usize,
    pub test: usize,
    pub dev: usize,
    pub build: usize,
    pub test_words: usize,
    pub newly_assigned: usize,
}

fn word_count(text: &str) -> usize {
    tokens(text)
        .iter()
        .filter(|t| matches!(t, Token::Word(_)))
        .count()
}

/// Assign splits for `corpus_dir/messages.tsv` and write `corpus_dir/splits.tsv`.
/// Fails (exit code 2) if the minimums aren't met; nothing is written then.
pub fn assign(corpus_dir: &Path) -> Result<SplitSummary> {
    let messages = read_messages(&corpus_dir.join("messages.tsv"))?;
    let splits_path = corpus_dir.join("splits.tsv");
    let existing = read_splits(&splits_path)?;

    let mut hashes: Vec<&str> = messages.iter().map(|m| m.hash.as_str()).collect();
    hashes.sort_unstable();
    hashes.dedup();
    let n = hashes.len();
    let n_test = n.div_ceil(3);
    let n_dev = n.div_ceil(6);

    let mut assigned: BTreeMap<String, Split> = existing.clone();
    let mut newly = 0;
    for (rank, h) in hashes.iter().enumerate() {
        if assigned.contains_key(*h) {
            continue;
        }
        let split = if rank < n_test {
            Split::Test
        } else if rank < n_test + n_dev {
            Split::Dev
        } else {
            Split::Build
        };
        assigned.insert(h.to_string(), split);
        newly += 1;
    }

    let mut summary = SplitSummary {
        messages: messages.len(),
        test: 0,
        dev: 0,
        build: 0,
        test_words: 0,
        newly_assigned: newly,
    };
    for m in &messages {
        match assigned.get(&m.hash) {
            Some(Split::Test) => {
                summary.test += 1;
                summary.test_words += word_count(&m.text);
            }
            Some(Split::Dev) => summary.dev += 1,
            Some(Split::Build) => summary.build += 1,
            None => {}
        }
    }

    if summary.test < MIN_TEST_MESSAGES
        || summary.dev < MIN_DEV_MESSAGES
        || summary.test_words < MIN_TEST_WORDS
    {
        return Err(ToolError::invalid(format!(
            "split minimums not met: test {} (min {MIN_TEST_MESSAGES}), dev {} (min {MIN_DEV_MESSAGES}), \
             test word positions {} (min {MIN_TEST_WORDS})",
            summary.test, summary.dev, summary.test_words
        )));
    }

    // Keep entries for hashes no longer in messages.tsv: assignments are immutable.
    let mut out = String::from("message_sha256\tsplit\n");
    for (h, s) in &assigned {
        out.push_str(h);
        out.push('\t');
        out.push_str(s.as_str());
        out.push('\n');
    }
    write_bytes(&splits_path, out.as_bytes())?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("predi-splits-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_corpus(dir: &Path, n: usize, words_per_msg: usize) {
        let mut s = String::from("category\ttext\n");
        for i in 0..n {
            let words: Vec<String> = (0..words_per_msg)
                .map(|w| format!("palabra{}", letters(i * 31 + w)))
                .collect();
            s.push_str(&format!("otro\t{}.\n", words.join(" ")));
        }
        fs::write(dir.join("messages.tsv"), s).unwrap();
    }

    /// A letters-only unique tag (digits would make tokens non-words).
    fn letters(mut i: usize) -> String {
        let mut s = String::new();
        loop {
            s.push((b'a' + (i % 26) as u8) as char);
            i /= 26;
            if i == 0 {
                break s;
            }
        }
    }

    #[test]
    fn assigns_by_rank_with_exact_counts() {
        let dir = temp_dir("exact");
        write_corpus(&dir, 1629, 11);
        let s = assign(&dir).unwrap();
        assert_eq!((s.test, s.dev, s.build), (543, 272, 814));
        assert_eq!(s.newly_assigned, 1629);
        assert!(s.test_words >= 5000);
        let written = fs::read_to_string(dir.join("splits.tsv")).unwrap();
        assert!(written.starts_with("message_sha256\tsplit\n"));
        assert_eq!(written.lines().count(), 1630);
    }

    #[test]
    fn never_reassigns() {
        let dir = temp_dir("immutable");
        write_corpus(&dir, 1629, 11);
        assign(&dir).unwrap();
        let first = read_splits(&dir.join("splits.tsv")).unwrap();
        // Add messages: existing assignments must not change.
        let mut s = fs::read_to_string(dir.join("messages.tsv")).unwrap();
        for i in 0..60 {
            s.push_str(&format!(
                "otro\tnuevo mensaje {} aqui.\n",
                letters(100_000 + i)
            ));
        }
        fs::write(dir.join("messages.tsv"), s).unwrap();
        let summary = assign(&dir).unwrap();
        assert_eq!(summary.newly_assigned, 60);
        let second = read_splits(&dir.join("splits.tsv")).unwrap();
        for (h, sp) in &first {
            assert_eq!(second.get(h), Some(sp));
        }
    }

    #[test]
    fn fails_below_minimums() {
        let dir = temp_dir("small");
        write_corpus(&dir, 300, 11);
        let err = assign(&dir).unwrap_err();
        assert_eq!(err.code, 2);
        assert!(!dir.join("splits.tsv").exists());
    }

    #[test]
    fn fails_below_word_minimum() {
        let dir = temp_dir("fewwords");
        write_corpus(&dir, 1629, 3);
        assert_eq!(assign(&dir).unwrap_err().code, 2);
    }
}
