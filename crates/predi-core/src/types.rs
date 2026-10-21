//! Public request/result types (contracts/engine-api.md, data-model.md).

use std::fmt;

/// One prediction request. Lives only for the duration of a `predict` call (FR-023).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PredictionRequest<'a> {
    /// Chosen by the client and echoed unchanged in the result (FR-006).
    pub request_id: u64,
    /// Text of the current message before the caret. May be empty (FR-011).
    pub text_before_caret: &'a str,
}

impl<'a> PredictionRequest<'a> {
    pub fn new(request_id: u64, text_before_caret: &'a str) -> Self {
        Self {
            request_id,
            text_before_caret,
        }
    }
}

/// Typed text never appears in debug output (FR-023).
impl fmt::Debug for PredictionRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PredictionRequest")
            .field("request_id", &self.request_id)
            .field("text_before_caret", &"<redacted>")
            .finish()
    }
}

/// What kind of help a suggestion offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionKind {
    /// Completes the trailing partial word. Used iff the request ended inside a word (FR-008).
    Completion,
    /// A next word at a word boundary (FR-009).
    NextWord,
    /// A continuation of 2–4 words at a word boundary (FR-010).
    Continuation,
}

/// One ranked suggestion.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// Canonical spelling with output casing applied. For a continuation, 2–4 space-separated
    /// words. `text` never begins or ends with whitespace.
    pub text: String,
    pub kind: SuggestionKind,
    /// Trailing Unicode scalar values of `text_before_caret` (the partial word) that this
    /// suggestion replaces. 0 for `NextWord`/`Continuation`. `replace_chars` is never greater
    /// than the partial word's length.
    pub replace_chars: u32,
    /// The same span in UTF-16 code units.
    pub replace_utf16: u32,
    /// The client inserts one space before `text`. `leading_space` is always false for
    /// `Completion`.
    pub leading_space: bool,
    /// `rank` 1–3, contiguous; 1 is most likely.
    pub rank: u8,
    /// Share of the total candidate score (R6), at least the applicable cutoff. Informational.
    pub confidence: f32,
}

impl Suggestion {
    /// Apply this suggestion to `text_before_caret` per the data-model application rule.
    ///
    /// Returns `None` if `replace_chars` exceeds the text length (never the case for engine output).
    pub fn apply_to(&self, text_before_caret: &str) -> Option<String> {
        let mut cut = text_before_caret.len();
        let mut chars = text_before_caret.char_indices().rev();
        for _ in 0..self.replace_chars {
            let (i, _) = chars.next()?;
            cut = i;
        }
        let mut out = String::with_capacity(cut + self.text.len() + 1);
        out.push_str(&text_before_caret[..cut]);
        if self.leading_space {
            out.push(' ');
        }
        out.push_str(&self.text);
        Some(out)
    }
}

/// Why a result may be empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Ok,
    Paused,
    NotApplicable,
    LowConfidence,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "Ok",
            Status::Paused => "Paused",
            Status::NotApplicable => "NotApplicable",
            Status::LowConfidence => "LowConfidence",
        }
    }
}

/// The answer to one request: 0–3 suggestions ordered by rank.
#[derive(Debug, Clone, PartialEq)]
pub struct PredictionResult {
    pub request_id: u64,
    pub suggestions: Vec<Suggestion>,
    pub status: Status,
}

impl PredictionResult {
    pub(crate) fn empty(request_id: u64, status: Status) -> Self {
        Self {
            request_id,
            suggestions: Vec::new(),
            status,
        }
    }
}

/// Engine and asset identification (contracts/engine-api.md `info`).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineInfo {
    pub engine_version: &'static str,
    pub asset_version: String,
    pub format_version: u16,
    pub tau_completion: f32,
    pub tau_next: f32,
    pub alpha: f32,
    pub vocabulary_size: u32,
}

/// Why an asset was refused (FR-038, FR-039). Variants carry no input text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// The asset is shorter than its header or a section it declares.
    Truncated,
    /// The first 8 bytes are not `PREDITYP`.
    BadMagic,
    /// The asset uses a format version this engine can't read.
    UnsupportedFormat { found: u16, supported: u16 },
    /// The payload SHA-256 doesn't match the header.
    BadChecksum,
    /// A structural rule is violated; the string names the rule.
    Malformed(&'static str),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::Truncated => write!(f, "language asset is truncated"),
            AssetError::BadMagic => write!(f, "file is not a PrediType language asset (bad magic)"),
            AssetError::UnsupportedFormat { found, supported } => write!(
                f,
                "language asset format version {found} is not supported (this engine reads version {supported})"
            ),
            AssetError::BadChecksum => {
                write!(
                    f,
                    "language asset checksum mismatch (corrupt or modified file)"
                )
            }
            AssetError::Malformed(what) => write!(f, "language asset is malformed: {what}"),
        }
    }
}

impl std::error::Error for AssetError {}
