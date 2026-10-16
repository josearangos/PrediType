# Data Model: Spanish Prediction Engine

**Feature**: `001-spanish-prediction-engine` | **Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

All runtime entities exist only in memory for the duration of a call (see research R9). Static
entities (assets, corpus, test cases) are files produced or curated at build time and never contain
user text.

## Runtime entities

### PredictionRequest

| Field | Type | Rules |
|-------|------|-------|
| `request_id` | unsigned 64-bit integer | Chosen by the client and echoed in the result (FR-006). Not interpreted by the engine. |
| `text_before_caret` | UTF-8 string | Text of the current message before the caret. The engine considers at most the last 1,000 characters (Unicode scalar values), truncated at a word boundary (FR-031). It may be empty (FR-011). |

- **Lifetime**: it lives only for the duration of a single `predict` call, and the engine keeps no
  copy (FR-023).

### Suggestion

| Field | Type | Rules |
|-------|------|-------|
| `text` | UTF-8 string | Canonical spelling with output casing applied (FR-015, FR-017–FR-020). For a continuation, this is a space-separated sequence of 2–4 words. |
| `kind` | enum `Completion` \| `NextWord` \| `Continuation` | `Completion` iff the request ended inside a word. In that case every suggestion is a `Completion` (FR-008). |
| `replace_chars` | unsigned integer | How many trailing Unicode scalar values of `text_before_caret` (the partial word) this suggestion replaces. It is 0 for `NextWord`/`Continuation` at a boundary. It is never greater than the partial word's length (FR-004). |
| `replace_utf16` | unsigned integer | The same span measured in UTF-16 code units, for browser and Windows text APIs. |
| `leading_space` | boolean | True when the client must insert a single space before `text` (for example after `Hola,`). It is always false for `Completion`. |
| `rank` | 1–3 | 1 is most likely. Ranks are contiguous. |
| `confidence` | float 0–1 | Share-of-total score (R6). It is always at least the applicable cutoff. Informational only. |

- **Application rule**: remove `replace_chars` from the end of `text_before_caret`, append a space if
  `leading_space` is true, then append `text`.
- **Gain**: a completion's added length, `len(text) − replace_chars`, is at least 2, unless the
  completion restores missing accents or `ñ` in the partial word (FR-008, FR-016).
- **Whitespace**: `text` never begins or ends with whitespace (FR-003). Any space after acceptance is
  a client behavior.

### PredictionResult

| Field | Type | Rules |
|-------|------|-------|
| `request_id` | unsigned 64-bit integer | Equals the request's id. |
| `suggestions` | list of `Suggestion`, length 0–3 | Ordered by rank. There are no two entries whose `text` is equal after case and accent folding (FR-013). |
| `status` | enum `Ok` \| `Paused` \| `NotApplicable` \| `LowConfidence` | Says why the list may be empty. `NotApplicable` covers numbers, URLs, emails, code, tokens over 30 characters, and unsupported characters (FR-012, FR-029). |

### EngineState

| State | `predict` behavior | Transitions |
|-------|--------------------|-------------|
| `Active` | Returns suggestions normally | `pause()` → `Paused` |
| `Paused` | Returns empty with status `Paused` (FR-025) | `resume()` → `Active` |

- `field_changed()` is accepted in both states. It holds no text, so it changes nothing (R9).
- Dropping the engine ends its lifetime (exit).
- There is no state that stores text.

## Static entities

### LanguageAsset (`assets/es-CO-<version>.pta`)

| Part | Contents |
|------|----------|
| Header | Magic number, format version, asset version (semver), `tau_completion`, `tau_next`, backoff α, source-manifest hash, payload checksum |
| Lexicon | Folded-key FST → word ids; canonical spelling per id; casing flag (`Lower`, `Capitalized`, `Acronym`) |
| N-gram tables | Unigram, bigram, and trigram scores, quantized, with domain weighting applied at build time |
| Top-next lists | Up to 32 ranked next-word ids per frequent unigram or bigram context |

- **Validation**: the asset is rejected if the magic number, format version, or checksum don't match.
- **Content**: it holds no strings except vocabulary words, which pass the personal-data filter (R8).
- **Versioning**: the asset version is bumped on any change to its sources or build parameters.

The binary layout is in [contracts/asset-format.md](contracts/asset-format.md).

### SourceManifest (`data/sources/manifest.toml`)

| Field | Rules |
|-------|-------|
| `name`, `url`, `license`, `retrieved` | Required for every external source (FR-028) |
| `sha256` | Hash of the exact input used |
| `role` | `general` \| `spelling` \| `domain` |

### DomainCorpus (`data/corpus/es-co-cs/`)

| Field | Rules |
|-------|-------|
| Message | One message per line, UTF-8. Placeholders such as `{NOMBRE}` and `{NUMERO_PEDIDO}` are used instead of personal data. |
| `split` | `build` \| `dev` \| `test`, assigned once and recorded in `splits.tsv` (message hash → split). It is immutable once assigned. |
| `category` | `saludo`, `disculpa`, `pregunta`, `pedido`, `envio`, `facturacion`, `cuenta`, `cierre`, `otro` (FR-034 coverage) |

- **Minimums**: at least 1,500 messages, of which at least 500 are test and at least 250 are dev.
  The test split has at least 5,000 word positions.

### CorrectnessCase (`data/correctness/cases.jsonl`)

| Field | Rules |
|-------|-------|
| `id` | Unique, stable |
| `area` | `accents`, `enye`, `opening_punct`, `casing`, `punctuation`, `boundary`, `empty`, `deletion`, `partial`, `not_applicable`, `continuation`, `robustness` |
| `input` | `text_before_caret` |
| `expect` | Any of: `contains` (list), `not_contains` (list), `all_capitalized`, `all_lowercase`, `all_uppercase`, `empty` (bool), `status`, `max_words`, `leading_space`, `replace_chars` |

The schema is in [contracts/correctness-case.schema.json](contracts/correctness-case.schema.json).

### EvaluationReport (`reports/eval-<timestamp>.json`)

It holds:
- metric values for SC-001–SC-014, each with a threshold, pass/fail, and a `provisional` flag;
- the reference-computer description (CPU model, cores, base clock, RAM, OS build, power plan);
- the engine version, asset version, and corpus manifest hash.

The schema is in [contracts/eval-report.schema.json](contracts/eval-report.schema.json).
