//! Source readers: Tatoeba, the expanded `es_CO` word list, and the team corpus (T020).

use crate::util::{Result, ToolError, message_hash, read_list, read_to_string};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// One team-written corpus message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub category: String,
    pub text: String,
    /// SHA-256 of the NFC-normalized text.
    pub hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Split {
    Build,
    Dev,
    Test,
}

impl Split {
    pub fn as_str(self) -> &'static str {
        match self {
            Split::Build => "build",
            Split::Dev => "dev",
            Split::Test => "test",
        }
    }

    pub fn parse(s: &str) -> Option<Split> {
        match s {
            "build" => Some(Split::Build),
            "dev" => Some(Split::Dev),
            "test" => Some(Split::Test),
            _ => None,
        }
    }
}

/// Read `messages.tsv` (`category<TAB>text`, with a header line).
pub fn read_messages(path: &Path) -> Result<Vec<Message>> {
    let text = read_to_string(path)?;
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if n == 0 && line.starts_with("category\t") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let Some((category, msg)) = line.split_once('\t') else {
            return Err(ToolError::invalid(format!(
                "{}:{}: expected category<TAB>text",
                path.display(),
                n + 1
            )));
        };
        if msg.contains('\t') {
            return Err(ToolError::invalid(format!(
                "{}:{}: tab inside a message",
                path.display(),
                n + 1
            )));
        }
        out.push(Message {
            category: category.to_string(),
            text: msg.to_string(),
            hash: message_hash(msg),
        });
    }
    Ok(out)
}

/// Read `splits.tsv` (`message_sha256<TAB>split`). A missing file is an empty map.
pub fn read_splits(path: &Path) -> Result<BTreeMap<String, Split>> {
    let mut map = BTreeMap::new();
    if !path.exists() {
        return Ok(map);
    }
    for (n, line) in read_to_string(path)?.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with("message_sha256\t") {
            continue;
        }
        let parsed = line
            .split_once('\t')
            .and_then(|(h, s)| Some((h, Split::parse(s.trim())?)));
        let Some((hash, split)) = parsed else {
            return Err(ToolError::invalid(format!(
                "{}:{}: expected message_sha256<TAB>split",
                path.display(),
                n + 1
            )));
        };
        map.insert(hash.to_string(), split);
    }
    Ok(map)
}

/// The team corpus with its frozen split assignment.
pub struct Corpus {
    pub messages: Vec<Message>,
    pub splits: BTreeMap<String, Split>,
}

impl Corpus {
    pub fn load(dir: &Path) -> Result<Corpus> {
        let messages = read_messages(&dir.join("messages.tsv"))?;
        let splits = read_splits(&dir.join("splits.tsv"))?;
        Ok(Corpus { messages, splits })
    }

    /// Messages of one split, in file order. Fails if any message has no assignment.
    pub fn split(&self, which: Split) -> Result<Vec<&Message>> {
        if self.splits.is_empty() {
            return Err(ToolError::invalid(
                "the corpus has no splits.tsv; run `predi-build split` first",
            ));
        }
        let mut out = Vec::new();
        for m in &self.messages {
            match self.splits.get(&m.hash) {
                Some(&s) if s == which => out.push(m),
                Some(_) => {}
                None => {
                    return Err(ToolError::invalid(format!(
                        "a message in category '{}' has no split assignment; run `predi-build split`",
                        m.category
                    )));
                }
            }
        }
        Ok(out)
    }

    pub fn test_hashes(&self) -> HashSet<&str> {
        self.splits
            .iter()
            .filter(|(_, s)| **s == Split::Test)
            .map(|(h, _)| h.as_str())
            .collect()
    }
}

/// Read Tatoeba sentences, keeping only Spanish rows. Accepts the per-language export
/// (`spa_sentences.tsv`) and the full `sentences.csv` (both `id<TAB>lang<TAB>text`).
pub fn read_tatoeba(path: &Path) -> Result<Vec<String>> {
    let text = read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (_, lang, sentence) = (parts.next(), parts.next(), parts.next());
        if let (Some("spa"), Some(s)) = (lang, sentence) {
            out.push(s.to_string());
        }
    }
    if out.is_empty() {
        return Err(ToolError::invalid(format!(
            "{}: no Spanish (spa) rows",
            path.display()
        )));
    }
    Ok(out)
}

/// Read the expanded dictionary word list (one word per line).
pub fn read_words(path: &Path) -> Result<HashSet<String>> {
    Ok(read_list(path)?.into_iter().collect())
}

/// Drop general sentences identical to a test message, and fail if any test hash remains in the
/// build input (FR-035, exit code 3). Returns how many general sentences were dropped.
pub fn check_leakage(
    general: &mut Vec<String>,
    domain: &[&Message],
    test_hashes: &HashSet<&str>,
) -> Result<usize> {
    let before = general.len();
    general.retain(|s| !test_hashes.contains(message_hash(s).as_str()));
    let dropped = before - general.len();
    if let Some(m) = domain
        .iter()
        .find(|m| test_hashes.contains(m.hash.as_str()))
    {
        return Err(ToolError::leakage(format!(
            "test-split message (category '{}') found in the build input",
            m.category
        )));
    }
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leakage_is_detected() {
        let m = Message {
            category: "saludo".into(),
            text: "Hola".into(),
            hash: message_hash("Hola"),
        };
        let test: HashSet<&str> = [m.hash.as_str()].into_iter().collect();
        let mut general = vec!["Hola".to_string(), "Adiós".to_string()];
        let err = check_leakage(&mut general, &[&m], &test)
            .err()
            .map(|e| e.code);
        assert_eq!(err, Some(3));
        let mut general = vec!["Hola".to_string(), "Adiós".to_string()];
        assert_eq!(check_leakage(&mut general, &[], &test).ok(), Some(1));
        assert_eq!(general, vec!["Adiós".to_string()]);
    }
}
