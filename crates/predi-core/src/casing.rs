//! Output casing (research R5, FR-017–FR-020).

/// Canonical casing of a lexicon entry (word-flags bits 0–1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexCasing {
    Lower,
    Capitalized,
    Acronym,
}

/// What the agent's text implies for the casing of the word being suggested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseContext {
    /// The word starts a sentence (rule 3).
    pub sentence_start: bool,
    /// The partial word is all capitals with at least 2 letters (rule 2).
    pub partial_all_caps: bool,
    /// The partial word starts with a capital (rule 4).
    pub partial_initial_cap: bool,
}

impl CaseContext {
    pub fn new(before: &str, partial: &str) -> Self {
        let letters: Vec<char> = partial.chars().filter(|c| c.is_alphabetic()).collect();
        let partial_all_caps = letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase());
        let partial_initial_cap = letters.first().is_some_and(|c| c.is_uppercase());
        Self {
            sentence_start: is_sentence_start(before),
            partial_all_caps,
            partial_initial_cap,
        }
    }
}

/// Characters that casing looks through: quotes, parentheses and opening `¿`/`¡` (FR-017).
fn is_transparent(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '«' | '»' | '“' | '”' | '‘' | '’' | '(' | ')' | '[' | ']' | '¿' | '¡'
    )
}

/// True when a word typed right after `before` starts a sentence: at message start, or after
/// `.`, `?`, `!`, `…`, `...` or a line break, looking through whitespace, quotes, parentheses
/// and `¿`/`¡`. A colon gives lowercase unless a line break follows it (FR-017).
pub fn is_sentence_start(before: &str) -> bool {
    for c in before.chars().rev() {
        if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}' | '\u{85}') {
            return true;
        }
        if c.is_whitespace() || c.is_control() || is_transparent(c) {
            continue;
        }
        return matches!(c, '.' | '?' | '!' | '…');
    }
    true
}

/// Capitalize the first character of `word`.
pub fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn upper(word: &str) -> String {
    word.chars().flat_map(char::to_uppercase).collect()
}

/// Apply the R5 priority rules to a canonical word. `canonical` is stored lowercase for `Lower`
/// entries and in its standard form (`Bogotá`, `PSE`, `WhatsApp`) otherwise.
pub fn apply(canonical: &str, lex: LexCasing, ctx: &CaseContext) -> String {
    // Rule 2 overrides lexicon casing: an all-caps partial word forces all-caps.
    if ctx.partial_all_caps {
        return upper(canonical);
    }
    // Rule 1: lexicon casing.
    if lex != LexCasing::Lower {
        return canonical.to_string();
    }
    // Rules 3 and 4.
    if ctx.sentence_start || ctx.partial_initial_cap {
        return capitalize(canonical);
    }
    // Rule 5.
    canonical.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_start_rules() {
        assert!(is_sentence_start(""));
        assert!(is_sentence_start("Su pedido fue enviado. "));
        assert!(is_sentence_start("¿"));
        assert!(is_sentence_start("Hola.\n"));
        assert!(is_sentence_start("Gracias por esperar… "));
        assert!(is_sentence_start("Gracias por esperar... "));
        assert!(is_sentence_start("Listo. ¿"));
        assert!(is_sentence_start("Estimado cliente:\n"));
        assert!(!is_sentence_start("Gracias, "));
        assert!(!is_sentence_start("Gracias, ¿"));
        assert!(!is_sentence_start("Tenga en cuenta lo siguiente: "));
        assert!(!is_sentence_start("Le dijimos \""));
        assert!(!is_sentence_start("(ver anexo) "));
    }

    #[test]
    fn rule_priority() {
        let mid = CaseContext::new("Gracias, ", "le");
        assert_eq!(apply("bogotá", LexCasing::Lower, &mid), "bogotá");
        assert_eq!(apply("Bogotá", LexCasing::Capitalized, &mid), "Bogotá");
        assert_eq!(apply("PSE", LexCasing::Acronym, &mid), "PSE");

        let caps = CaseContext::new("", "BUE");
        assert_eq!(apply("buenos", LexCasing::Lower, &caps), "BUENOS");
        assert_eq!(apply("Bogotá", LexCasing::Capitalized, &caps), "BOGOTÁ");

        let start = CaseContext::new("Listo. ", "");
        assert_eq!(apply("gracias", LexCasing::Lower, &start), "Gracias");

        let initial = CaseContext::new("Hola, ", "BUe");
        assert_eq!(apply("buenos", LexCasing::Lower, &initial), "Buenos");
        let mixed = CaseContext::new("Hola, ", "bUe");
        assert_eq!(apply("buenos", LexCasing::Lower, &mixed), "buenos");

        // A single capital letter is not "all caps".
        let one = CaseContext::new("Hola, ", "B");
        assert_eq!(apply("buenos", LexCasing::Lower, &one), "Buenos");
    }

    #[test]
    fn capitalize_enye() {
        assert_eq!(capitalize("ñandú"), "Ñandú");
        assert_eq!(capitalize("él"), "Él");
    }
}
