//! Unicode normalization and accent folding (research R4).

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// NFC-normalize `s`.
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Lowercase one character. Every character this engine folds lowercases to a single char;
/// others keep the first char of their lowercase mapping, which is deterministic.
pub fn lower_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Lowercase a string (full Unicode lowercase mapping).
pub fn lower(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// Folded lookup key: lowercase, combining diacritics removed (so `ñ`→`n`, `ü`→`u`, `á`→`a`).
pub fn fold(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Fold a single character (lowercase, diacritics removed). Returns `c` lowercased if it has no
/// decomposition.
pub fn fold_char(c: char) -> char {
    let mut base = c;
    unicode_normalization::char::decompose_canonical(c, |d| {
        if base == c && !is_combining_mark(d) {
            base = d;
        }
    });
    lower_char(base)
}

/// True when `typed` (a partial word) is a case-insensitive prefix of `canonical` including all
/// diacritics. Used for the exact-diacritic ranking bonus (R4).
pub fn matches_exact_diacritics(typed: &str, canonical: &str) -> bool {
    let t = lower(&nfc(typed));
    let c = lower(&nfc(canonical));
    c.starts_with(&t)
}

/// True when completing `typed` with `canonical` restores accents, `ñ` or `ü` that the agent left
/// out: `canonical` starts with the same letters as `typed` after folding, and at least one typed
/// letter is the bare base of an accented canonical letter (`comunicacion` → `comunicación`,
/// `ano` → `año`, `informacio` → `información`). A typed accent that the canonical form lacks
/// does not count.
pub fn restores_diacritics(typed: &str, canonical: &str) -> bool {
    let typed: Vec<char> = nfc(typed).chars().map(lower_char).collect();
    let canon: Vec<char> = nfc(canonical).chars().map(lower_char).collect();
    if canon.len() < typed.len() {
        return false;
    }
    let mut restored = false;
    for (t, c) in typed.iter().zip(canon.iter()) {
        if t == c {
            continue;
        }
        // The typed letter must be the bare base letter of the canonical one.
        if fold_char(*c) == *t && fold_char(*t) == *t {
            restored = true;
        } else {
            return false;
        }
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_composes() {
        assert_eq!(nfc("di\u{301}as"), "días");
    }

    #[test]
    fn fold_removes_diacritics_and_case() {
        assert_eq!(fold("Contraseña"), "contrasena");
        assert_eq!(fold("PINGÜINO"), "pinguino");
        assert_eq!(fold("Comunicación"), "comunicacion");
        assert_eq!(fold("di\u{301}as"), "dias");
    }

    #[test]
    fn fold_char_single() {
        assert_eq!(fold_char('Ñ'), 'n');
        assert_eq!(fold_char('é'), 'e');
        assert_eq!(fold_char('x'), 'x');
    }

    #[test]
    fn exact_diacritics() {
        assert!(matches_exact_diacritics("añ", "año"));
        assert!(!matches_exact_diacritics("an", "año"));
        assert!(matches_exact_diacritics("An", "anotar"));
        assert!(matches_exact_diacritics("di\u{301}", "días"));
    }

    #[test]
    fn restores() {
        assert!(restores_diacritics("comunicacion", "comunicación"));
        assert!(restores_diacritics("ano", "año"));
        assert!(restores_diacritics("informacio", "información"));
        assert!(restores_diacritics("Comunicacion", "comunicación"));
        assert!(!restores_diacritics("comunicación", "comunicación"));
        assert!(!restores_diacritics("gracias", "gracias"));
        // A typed accent the canonical form lacks is not a restoration.
        assert!(!restores_diacritics("á", "a"));
        assert!(!restores_diacritics("xyz", "año"));
        assert!(!restores_diacritics("añoo", "año"));
    }
}
