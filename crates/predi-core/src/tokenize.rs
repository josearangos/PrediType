//! Word boundaries, truncation, and not-applicable detection (research R4, FR-012, FR-029–FR-031,
//! FR-040, FR-041).
//!
//! The same tokenizer is used at runtime and by the asset builder, so build-time n-gram counts and
//! runtime context agree on what a word is.

use crate::normalize::{lower, nfc};
use unicode_normalization::char::is_combining_mark;

/// The engine considers at most this many trailing Unicode scalar values (FR-031).
pub const MAX_CONTEXT_CHARS: usize = 1000;
/// A token longer than this is not a word (FR-012).
pub const MAX_TOKEN_CHARS: usize = 30;

/// A token of a message, as seen by the n-gram model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    /// A supported word, as an original slice of the input (may be NFD; not lowercased).
    Word(&'a str),
    /// A number, email, URL, code-like string, placeholder, over-long or unsupported token. It
    /// breaks the n-gram context but doesn't end the sentence.
    NonWord,
    /// Sentence-ending punctuation (`.` `?` `!` `…`) or a line break.
    SentenceEnd,
}

/// How the text before the caret ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending<'a> {
    /// No text at all.
    Empty,
    /// The caret is inside (at the end of) a word.
    InsideWord {
        /// The partial word, as an original slice of the input.
        partial: &'a str,
        /// Byte offset of the partial word in the original input.
        start: usize,
        /// Length of the partial word in Unicode scalar values.
        chars: u32,
        /// Length of the partial word in UTF-16 code units.
        utf16: u32,
    },
    /// The caret follows whitespace or punctuation.
    Boundary,
}

/// A context slot for the n-gram model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ctx {
    /// Message or sentence start (`<s>`).
    Start,
    /// A word, NFC-normalized and lowercased.
    Word(String),
    /// Unknown: a non-word token, or the start of a truncated window.
    Gap,
}

/// Everything the engine needs to know about the text before the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis<'a> {
    pub ending: Ending<'a>,
    /// The current token is a number, URL, email, code-like, over-long or unsupported (FR-012,
    /// FR-029). The result must be empty with status `NotApplicable`.
    pub not_applicable: bool,
    /// The (truncated) text before the partial word, or before the caret at a boundary. Used for
    /// sentence-start detection.
    pub before: &'a str,
    /// The last two context slots before the caret (or before the partial word), oldest first.
    pub context: [Ctx; 2],
    /// Whether a next word needs a leading space (FR-041). Always false inside a word.
    pub leading_space: bool,
}

/// Word character (FR-040): any alphabetic character. Combining marks attach to a preceding
/// letter (decomposed accents, FR-003). Unsupported letters are detected separately.
fn is_letter(c: char) -> bool {
    c.is_alphabetic()
}

fn is_joiner(c: char) -> bool {
    matches!(c, '-' | '\'' | '’' | '‐')
}

fn is_space(c: char) -> bool {
    c.is_whitespace() || c.is_control()
}

fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

fn is_sentence_end_char(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '…')
}

/// Characters that are transparent when looking for sentence-ending punctuation at a token end.
fn is_closing_wrapper(c: char) -> bool {
    matches!(c, '"' | '\'' | '»' | '”' | '’' | ')' | ']')
}

/// Supported letters for Spanish (FR-029): a–z, A–Z and á é í ó ú ü ñ with their capitals.
fn is_supported_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
        || matches!(
            c,
            'á' | 'é' | 'í' | 'ó' | 'ú' | 'ü' | 'ñ' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú' | 'Ü' | 'Ñ'
        )
}

/// A word (after NFC) is supported when every letter is supported; joiners are allowed.
fn is_supported_word(word_nfc: &str) -> bool {
    word_nfc
        .chars()
        .all(|c| is_supported_letter(c) || is_joiner(c))
}

/// A whitespace-delimited chunk is code-like (FR-012): it contains a digit, `@ / \ _ = { } < >`,
/// or a `.`/`:` between two letters or digits.
fn is_code_like(chunk: &str) -> bool {
    let chars: Vec<char> = chunk.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_numeric() || matches!(c, '@' | '/' | '\\' | '_' | '=' | '{' | '}' | '<' | '>') {
            return true;
        }
        if (c == '.' || c == ':') && i > 0 && i + 1 < chars.len() {
            let alnum = |x: char| x.is_alphanumeric();
            if alnum(chars[i - 1]) && alnum(chars[i + 1]) {
                return true;
            }
        }
    }
    false
}

/// Byte ranges of the word runs in `chunk` (a chunk has no whitespace and isn't code-like).
fn word_runs(chunk: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = chunk.char_indices().collect();
    let mut runs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_letter(chars[i].1) {
            i += 1;
            continue;
        }
        let start = chars[i].0;
        let mut j = i + 1;
        while j < chars.len() {
            let c = chars[j].1;
            let joins = is_joiner(c)
                && j + 1 < chars.len()
                && is_letter(chars[j + 1].1)
                && is_letter_or_mark_before(&chars, j);
            if !(is_letter(c) || is_combining_mark(c) || joins) {
                break;
            }
            j += 1;
        }
        let end = if j < chars.len() {
            chars[j].0
        } else {
            chunk.len()
        };
        runs.push((start, end));
        i = j;
    }
    runs
}

fn is_letter_or_mark_before(chars: &[(usize, char)], j: usize) -> bool {
    j > 0 && (is_letter(chars[j - 1].1) || is_combining_mark(chars[j - 1].1))
}

/// Classify a word run: `Word` when supported and not over-long, else `NonWord`.
fn classify_word(word: &str) -> Token<'_> {
    let n = nfc(word);
    if n.chars().count() > MAX_TOKEN_CHARS || !is_supported_word(&n) {
        Token::NonWord
    } else {
        Token::Word(word)
    }
}

fn push_sentence_end(out: &mut Vec<Token<'_>>) {
    if out.last() != Some(&Token::SentenceEnd) {
        out.push(Token::SentenceEnd);
    }
}

/// Split `text` into word, non-word and sentence-end tokens.
pub fn tokens(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut chunk_start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if is_space(c) {
            if let Some(s) = chunk_start.take() {
                tokenize_chunk(&text[s..i], &mut out);
            }
            if is_line_break(c) {
                push_sentence_end(&mut out);
            }
        } else if chunk_start.is_none() {
            chunk_start = Some(i);
        }
    }
    if let Some(s) = chunk_start {
        tokenize_chunk(&text[s..], &mut out);
    }
    out
}

fn tokenize_chunk<'a>(chunk: &'a str, out: &mut Vec<Token<'a>>) {
    if is_code_like(chunk) {
        out.push(Token::NonWord);
        let trimmed = chunk.trim_end_matches(is_closing_wrapper);
        if trimmed
            .chars()
            .next_back()
            .is_some_and(is_sentence_end_char)
        {
            push_sentence_end(out);
        }
        return;
    }
    let runs = word_runs(chunk);
    let mut pos = 0;
    for (s, e) in runs {
        punctuation_between(&chunk[pos..s], out);
        out.push(classify_word(&chunk[s..e]));
        pos = e;
    }
    punctuation_between(&chunk[pos..], out);
}

fn punctuation_between(between: &str, out: &mut Vec<Token<'_>>) {
    if between.chars().any(is_sentence_end_char) {
        push_sentence_end(out);
    }
}

/// Truncate to the last [`MAX_CONTEXT_CHARS`] scalar values, at a word boundary (FR-031).
/// Returns the kept slice and whether anything was cut.
pub fn truncate(text: &str) -> (&str, bool) {
    let mut count = 0usize;
    let mut cut = 0usize;
    for (i, _) in text.char_indices().rev() {
        count += 1;
        if count == MAX_CONTEXT_CHARS {
            cut = i;
            break;
        }
    }
    if count < MAX_CONTEXT_CHARS || cut == 0 {
        return (text, false);
    }
    // Don't start inside a word: skip to the first non-word character.
    let window = &text[cut..];
    let preceded_by_word = text[..cut].chars().next_back().is_some_and(is_letter);
    if preceded_by_word {
        let skip = window
            .char_indices()
            .find(|&(_, c)| !(is_letter(c) || is_combining_mark(c) || is_joiner(c)))
            .map(|(i, _)| i)
            .unwrap_or(window.len());
        (&window[skip..], true)
    } else {
        (window, true)
    }
}

/// Leading space rule (FR-041) for a text that ends at a boundary.
fn needs_leading_space(text: &str) -> bool {
    let mut rev = text.chars().rev();
    let Some(last) = rev.next() else {
        return false;
    };
    if is_space(last) {
        return false;
    }
    match last {
        '¿' | '¡' | '(' | '«' | '“' | '‘' | '[' => false,
        // A straight quote opens when it's at the start or after whitespace.
        '"' | '\'' => rev.next().is_some_and(|prev| !is_space(prev)),
        _ => true,
    }
}

/// Analyze the text before the caret.
pub fn analyze(text_before_caret: &str) -> Analysis<'_> {
    let (window, truncated) = truncate(text_before_caret);
    let window_offset = text_before_caret.len() - window.len();

    if text_before_caret.is_empty() {
        return Analysis {
            ending: Ending::Empty,
            not_applicable: false,
            before: "",
            context: [Ctx::Gap, Ctx::Start],
            leading_space: false,
        };
    }

    // Truncation swallowed everything: the input ends in one giant token.
    if truncated && window.is_empty() {
        return not_applicable(window);
    }

    // The trailing whitespace-delimited chunk is the "current token" for FR-012.
    let last_char = window.chars().next_back();
    let ends_in_space = last_char.is_none_or(is_space);
    let chunk_start = window
        .char_indices()
        .rev()
        .find(|&(_, c)| is_space(c))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let chunk = &window[chunk_start..];

    if !ends_in_space && (is_code_like(chunk) || (truncated && chunk_start == 0)) {
        return not_applicable(window);
    }

    // Is the caret inside a word?
    if !ends_in_space && last_char.is_some_and(|c| is_letter(c) || is_combining_mark(c)) {
        let runs = word_runs(chunk);
        if let Some(&(s, e)) = runs.last()
            && e == chunk.len()
        {
            let partial = &chunk[s..e];
            let partial_nfc = nfc(partial);
            if partial_nfc.chars().count() > MAX_TOKEN_CHARS || !is_supported_word(&partial_nfc) {
                return not_applicable(window);
            }
            // A partial word starting right at a truncation cut is over-long by construction.
            let start_in_window = chunk_start + s;
            let before = &window[..start_in_window];
            return Analysis {
                ending: Ending::InsideWord {
                    partial,
                    start: window_offset + start_in_window,
                    chars: partial.chars().count() as u32,
                    utf16: partial.encode_utf16().count() as u32,
                },
                not_applicable: false,
                before,
                context: context_of(before, truncated),
                leading_space: false,
            };
        }
    }

    Analysis {
        ending: Ending::Boundary,
        not_applicable: false,
        before: window,
        context: context_of(window, truncated),
        leading_space: needs_leading_space(window),
    }
}

fn not_applicable(window: &str) -> Analysis<'_> {
    Analysis {
        ending: Ending::Boundary,
        not_applicable: true,
        before: window,
        context: [Ctx::Gap, Ctx::Gap],
        leading_space: false,
    }
}

/// The last two context slots of `text` (the text before the partial word or caret).
fn context_of(text: &str, truncated: bool) -> [Ctx; 2] {
    let toks = tokens(text);
    let mut slots: Vec<Ctx> = Vec::with_capacity(3);
    slots.push(if truncated { Ctx::Gap } else { Ctx::Start });
    for t in toks {
        match t {
            Token::SentenceEnd => {
                slots.clear();
                slots.push(Ctx::Start);
            }
            Token::NonWord => slots.push(Ctx::Gap),
            Token::Word(w) => slots.push(Ctx::Word(lower(&nfc(w)))),
        }
        if slots.len() > 2 {
            slots.remove(0);
        }
    }
    match slots.len() {
        0 => [Ctx::Gap, Ctx::Start],
        1 => {
            let only = slots.pop().unwrap_or(Ctx::Start);
            // Before a sentence start there is nothing usable.
            [Ctx::Gap, only]
        }
        _ => {
            let b = slots.pop().unwrap_or(Ctx::Gap);
            let a = slots.pop().unwrap_or(Ctx::Gap);
            [a, b]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(s: &str) -> Ctx {
        Ctx::Word(s.to_string())
    }

    #[test]
    fn tokens_basic() {
        let t = tokens("Hola, ¿cómo está? Bien.\nGracias");
        assert_eq!(
            t,
            vec![
                Token::Word("Hola"),
                Token::Word("cómo"),
                Token::Word("está"),
                Token::SentenceEnd,
                Token::Word("Bien"),
                Token::SentenceEnd,
                Token::Word("Gracias"),
            ]
        );
    }

    #[test]
    fn tokens_placeholder_and_numbers() {
        let t = tokens("Buenos días, {NOMBRE}. ¿En qué le ayudo? Pedido 123");
        assert_eq!(
            t,
            vec![
                Token::Word("Buenos"),
                Token::Word("días"),
                Token::NonWord,
                Token::SentenceEnd,
                Token::Word("En"),
                Token::Word("qué"),
                Token::Word("le"),
                Token::Word("ayudo"),
                Token::SentenceEnd,
                Token::Word("Pedido"),
                Token::NonWord,
            ]
        );
    }

    #[test]
    fn tokens_joiners() {
        assert_eq!(
            tokens("socio-económico"),
            vec![Token::Word("socio-económico")]
        );
        assert_eq!(
            tokens("bien- mal"),
            vec![Token::Word("bien"), Token::Word("mal")]
        );
    }

    #[test]
    fn ellipsis_is_not_code() {
        assert_eq!(
            tokens("esperar..."),
            vec![Token::Word("esperar"), Token::SentenceEnd]
        );
        assert!(is_code_like("www.ej"));
        assert!(is_code_like("10:30"));
        assert!(!is_code_like("esperar..."));
        assert!(!is_code_like("siguiente:"));
    }

    #[test]
    fn inside_word() {
        let a = analyze("Buenos dí");
        assert_eq!(
            a.ending,
            Ending::InsideWord {
                partial: "dí",
                start: 7,
                chars: 2,
                utf16: 2
            }
        );
        assert_eq!(a.context, [Ctx::Start, word("buenos")]);
        assert!(!a.leading_space);
    }

    #[test]
    fn inside_word_nfd() {
        let text = "Buenos di\u{301}";
        let a = analyze(text);
        assert_eq!(
            a.ending,
            Ending::InsideWord {
                partial: "di\u{301}",
                start: 7,
                chars: 3,
                utf16: 3
            }
        );
    }

    #[test]
    fn boundaries_and_leading_space() {
        for (text, space) in [
            ("Hola,", true),
            ("Hola, ", false),
            ("¿", false),
            ("Listo;", true),
            ("Gracias.", true),
            ("(ver anexo)", true),
            ("«Hola»", true),
            ("(", false),
            ("«", false),
            ("\"", false),
            ("Hola \"", false),
            ("Hola\n", false),
            ("", false),
        ] {
            let a = analyze(text);
            assert!(!a.not_applicable, "{text:?}");
            assert_ne!(
                std::mem::discriminant(&a.ending),
                std::mem::discriminant(&Ending::InsideWord {
                    partial: "",
                    start: 0,
                    chars: 0,
                    utf16: 0
                }),
                "{text:?}"
            );
            assert_eq!(a.leading_space, space, "{text:?}");
        }
    }

    #[test]
    fn not_applicable_cases() {
        for text in [
            "Escríbanos a soporte@emp",
            "Pedido 12345",
            "https://ej",
            "Su 2do",
            "a_b",
            "www.ej",
            "Son las 10:30",
            "garçon",
            "привет",
            "Supercalifragilisticoespialidosoextra",
        ] {
            assert!(analyze(text).not_applicable, "{text:?}");
        }
        for text in [
            "Gracias por esperar...",
            "Gracias por esperar... ",
            "Pedido 12345 ",
        ] {
            assert!(!analyze(text).not_applicable, "{text:?}");
        }
    }

    #[test]
    fn context_resets_at_sentence_end() {
        assert_eq!(analyze("Gracias. ").context, [Ctx::Gap, Ctx::Start]);
        assert_eq!(
            analyze("Muchas gracias por ").context,
            [word("gracias"), word("por")]
        );
        assert_eq!(analyze("").context, [Ctx::Gap, Ctx::Start]);
        assert_eq!(analyze("Pedido 12345 ").context, [word("pedido"), Ctx::Gap]);
        assert_eq!(
            analyze("¿En qué puedo ").context,
            [word("qué"), word("puedo")]
        );
    }

    #[test]
    fn emoji_is_a_boundary() {
        let a = analyze("Hola 😀");
        assert!(!a.not_applicable);
        assert_eq!(a.ending, Ending::Boundary);
    }

    #[test]
    fn truncation_at_word_boundary() {
        let long = "palabra ".repeat(200); // 1,600 chars
        let (w, cut) = truncate(&long);
        assert!(cut);
        assert!(w.chars().count() <= MAX_CONTEXT_CHARS);
        assert!(w.starts_with(' ') || w.starts_with('p'));
        assert!(!w.starts_with("alabra"));
        // A huge input still analyzes quickly and correctly.
        let text = format!("{long}gra");
        let a = analyze(&text);
        assert!(matches!(a.ending, Ending::InsideWord { chars: 3, .. }));
    }

    #[test]
    fn single_giant_token_is_not_applicable() {
        let s = "a".repeat(5000);
        assert!(analyze(&s).not_applicable);
    }
}
