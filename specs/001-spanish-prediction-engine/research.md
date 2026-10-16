# Research: Spanish Prediction Engine

**Feature**: `001-spanish-prediction-engine` | **Date**: 2026-10-16 | **Plan**: [plan.md](plan.md)

Each section records a decision, why it was chosen, and what else was considered. Together they
resolve every open item in the plan's Technical Context.

---

## R1. Implementation language for a shared engine

**Decision**: Rust (stable, edition 2024; toolchain 1.99 is installed locally). The engine core
compiles to a native Windows library and to WebAssembly from the same source.

**Rationale**:
- Principle VI asks both clients to share one engine. A Rust core links directly into a Windows
  client, and it runs inside a Chrome extension as WebAssembly, with no native-messaging host and
  no extra extension permissions. One codebase means identical suggestions in both clients.
- There's no garbage collector or runtime. The typical working set for this kind of engine is
  20–50 MB, comfortably under the 100 MB target (Principle V).
- Memory safety matters for software that sees keystrokes (Principle VII).
- Strong typing and built-in testing suit Principle X.

**Alternatives considered**:

| Option | Why rejected |
|--------|--------------|
| TypeScript | Native in Chrome, but the Windows widget would need Node/Electron, which costs well over 100 MB |
| C# / .NET | Excellent on Windows, but Chrome would need a native-messaging host (more permissions and parts) or a heavy Blazor WASM runtime |
| Python | Easy for language modelling, but packaging and memory on Windows are poor, and it can't run in Chrome |
| C++ | Works native and WASM, but has more memory-safety risk and harder packaging than Rust for the same benefit |

**Risk**: Rust has a steep learning curve. It's mitigated by a small module surface and by keeping
only two crates (see plan, Project Structure).

---

## R2. Baseline prediction approach (FR-033)

**Decision**: Use an **interpolated word n-gram model (up to trigrams) with Stupid Backoff
scoring**, plus a **prefix index** over the vocabulary for partial-word completion. Both are
precomputed into a static binary asset.

- **Next word**: candidates come from the trigram context (w₋₂ w₋₁), backing off to the bigram (w₋₁)
  and then to unigrams, with the backoff factor α = 0.4 applied at each level.
- **Partial word**: the same candidate lists are filtered by the accent-folded typed prefix (R4). If
  fewer than 3 candidates survive, the prefix index supplies frequent vocabulary words starting
  with that prefix.
- **Speed**: for each frequent unigram and bigram context, the top 32 next words are precomputed so
  a request is a handful of lookups instead of a search.
- **Domain weighting**: counts from the team-written Colombian set are up-weighted (an
  interpolation weight, tuned on the dev split) so customer-service phrasing beats general Spanish.

**Rationale**: This is the simplest approach known to give useful next-word and completion quality
in a narrow domain. It is deterministic, CPU-only, and runs in microseconds to milliseconds. Stupid
Backoff needs no normalization, which suits ranking plus a confidence cutoff.

**Alternatives considered**:

| Option | Why not for the baseline |
|--------|--------------------------|
| Modified Kneser-Ney | Slightly better perplexity, but more complex to build. It can replace Stupid Backoff later using the same asset layout |
| Small neural model (LSTM / tiny transformer on CPU) | Possibly better quality, but adds an inference runtime, larger memory, and harder determinism. It's a candidate for a later feature, measured against this baseline |
| Unigram frequency only | Too weak for next-word prediction (SC-002) |

---

## R3. Language sources and licenses (FR-028, clarification Q1)

**Decision**:

| Source | Use | License (verify before bundling) |
|--------|-----|----------------------------------|
| **Tatoeba** Spanish sentences | General conversational n-grams | CC BY 2.0 FR (attribution) |
| **RLA-ES `es_CO` Hunspell dictionary** | Spelling validation of vocabulary and suggestions, accent restoration | Tri-licensed GPL/LGPL/MPL 1.1 (MPL used) |
| **Team-written Colombian customer-service set** | Domain n-grams (build split), threshold tuning (dev split), evaluation (test split) | Owned by the project |

- **Split of the team-written set**: at least 1,500 messages, divided into at least 500 for test,
  250 for dev, and the rest for build. The split is assigned once, by message, recorded in a
  manifest, and never changed. Test messages never enter the asset build (FR-035).
- **Attribution and license texts** are shipped in `docs/data-sources.md` and alongside the asset.

**Rationale**: Tatoeba is short, conversational, sentence-level Spanish, close to chat-style
customer service, and its license allows redistribution with attribution. The `es_CO` dictionary
gives a Colombian-specific spelling reference, which SC-005 needs.

**Alternatives considered**:
- Spanish Wikipedia (CC BY-SA): encyclopedic register, and share-alike obligations on derived data.
- OpenSubtitles: licensing is unclear for redistribution.
- Europarl: formal parliamentary register, and Spain-centric.

**Follow-up**: confirm both licenses during the first implementation task, before any asset is
distributed.

---

## R4. Text normalization, tokenization, and accent tolerance (FR-015–FR-016, FR-029)

**Decision**:
- **Normalization**: a copy of the input is normalized to Unicode NFC for lookups only. The
  partial word's span is always measured on the original input, so replacement counts are correct
  for decomposed (NFD) text. Control characters other than line breaks
  are treated as separators.
- **Word characters**: letters (including á é í ó ú ü ñ and their capitals). A hyphen or apostrophe
  counts only between letters.
- **Everything else** is a boundary. A digit, `@`, `/`, `\`, `_`, `=`, `{`, `}`, `<`, `>`, or a `.`
  or `:` between two letters or digits inside a token marks it as non-word (number, email, URL,
  code; spec FR-012, FR-040). So does a token longer than 30 characters. Either case gives zero
  suggestions (FR-012). A partial word with a letter other than a–z, A–Z, and áéíóúüñ is
  unsupported; emoji and other symbols are boundaries (FR-029).
- **Folded key**: lookups use a folded form (lowercase, diacritics removed, `ñ`→`n`). Suggestions
  always use the canonical dictionary spelling.
- **Ranking**: when the typed letters exactly match the canonical spelling, including accents, that
  candidate gets a small bonus. Typing `an` therefore still finds `año`, but typing `añ` prefers
  `año` over `anotar`.
- **Replacement**: a completion that changes accents (for example `comunicacion` → `comunicación`)
  replaces only the partial word. The replacement length is reported in UTF-16 code units **and**
  in Unicode scalar values, so both browser and Windows clients can apply it safely (see the engine
  API contract).

**Rationale**: Folding `ñ` too handles agents typing on keyboards without `ñ`, while the
exact-match bonus keeps behavior intuitive for those who type it. Reporting both length units avoids
off-by-one corruption with accented characters (Principle IX: corrupting text is release-blocking).

**Alternatives considered**: exact-diacritic matching only (fails FR-016); general typo tolerance
via edit distance (out of scope: not a spelling corrector).

---

## R5. Capitalization rules (FR-017–FR-020)

**Decision**: The engine works in lowercase internally. On output, it applies these casing rules in
priority order:

1. **Lexicon casing**: a lexicon entry marked with canonical casing (proper noun or acronym) keeps
   that casing. The exception is an all-caps partial word, which forces all-caps.
2. **All-caps**: a partial word that is all-caps with at least 2 letters makes the output all-caps.
3. **Sentence start**: at message start, or after `.`, `?`, `!`, `…`, `...` or a line break
   (optionally followed by `¿`/`¡` and whitespace), only the first word is capitalized. A `¿`/`¡`
   mid-sentence does not start a sentence. After a colon, lowercase unless a line break follows.
   Quotes and parentheses are transparent: casing follows the surrounding sentence.
4. **Partial-word initial capital**: a partial word with a capital first letter keeps that capital. Other
   mixed case (for example `bUe`) is ignored.
5. Otherwise, lowercase.

Continuations (R7) apply rule 3 or 4 only to their first word.

**Rationale**: The rules are deterministic and directly testable in the correctness set.

---

## R6. Confidence cutoff (FR-002, SC-014, clarification Q3)

**Decision**:
- **Score**: each candidate gets a confidence score, which is its share of the total score of all
  candidates found for this request.
- **Cutoff**: a candidate is returned only if its score is at least a fixed cutoff τ, which is
  stored in the asset header and versioned with it.
- **Tuning τ**: on the **dev split** only, choose the smallest τ that gives a first-rank precision of
  at least 0.55 among non-empty results. The 0.05 margin above SC-014's 0.50 absorbs the difference
  between dev and test.
- **Two cutoffs**: τ is tuned separately for completions (`τ_c`) and next words (`τ_n`), because
  their score distributions differ.
- **Short completions**: completions adding fewer than 2 characters are discarded before scoring,
  unless they restore missing accents or `ñ` in the partial word (FR-008).
- **No special cases**: empty-context openers are scored and cut off like any next-word candidate
  (FR-011).
- **Register filter**: *tú*-form and other-variant words carry a lexicon flag. They're excluded from
  next words and continuations, and from completions while any *usted*/Colombian candidate still
  matches the folded prefix (FR-022).

**Rationale**: A share-of-total score is cheap, monotonic, and easy to calibrate. Tuning on dev only
keeps the test split honest (FR-035, FR-002).

**Alternatives considered**: an absolute probability cutoff (unstable across context lengths); a
learned calibrator (unnecessary for the baseline).

---

## R7. Brief continuations (FR-010)

**Decision**:
- **Building**: starting from the top next word, extend greedily up to 4 words. Each step picks the
  most likely next word given the trigram context, and the chain stops when that word's conditional
  score falls below τ_n or a sentence-ending token is reached.
- **Offering**: a continuation is returned only if it has at least 2 words and its overall score is
  at least τ_n.
- **Slot**: it takes the slot of its first word if that word is already among the candidates, so
  results contain no duplicates (FR-013).

**Rationale**: It reuses the same model with no extra data and stays within the 4-word limit by
construction.

---

## R8. Personal-data exclusion in assets (FR-014, FR-028)

**Decision**: The asset builder drops a token, and any n-gram containing it, when the token:

- has digits, `@`, or URL-like form, or
- appears only capitalized mid-sentence in the sources (a likely proper name, for example Tatoeba's
  "Tom") and isn't in a small curated allowlist (`data/allowlist-proper-nouns.txt`, for example
  `Colombia`, `Bogotá`, `Medellín`, `PSE`).

The team-written set uses placeholders such as `{NOMBRE}` and `{NUMERO_PEDIDO}`, which are removed at
build time. A build-time report lists dropped tokens for review.

**Rationale**: This prevents names, numbers, and emails from becoming suggestions, while keeping
legitimate capitalized words (FR-019). No offensive-word blocklist is applied, per clarification.

---

## R9. Context lifecycle and concurrency (FR-023–FR-026, clarification Q4)

**Decision**: The engine is **stateless with respect to text**.

- **No retained text**: each request carries the text before the caret (truncated to 1,000
  characters at a word boundary), and nothing derived from it is kept after the call returns.
- **Signals**: `field_changed` and `pause`/`resume` are still part of the contract. `pause` flips a
  flag that makes `predict` return empty results, and `field_changed` is a no-op that clients must
  still call, so future engines that hold context stay compatible.
- **Concurrency**: the engine is `Send + Sync` and can be called from a worker thread. It holds no
  locks on the request path and has bounded work per request.

**Rationale**: Holding no context at all is the strongest way to satisfy "at most one field" and
"discard on field change", and it makes SC-013 true by construction. It's also the simplest design.

**Alternatives considered**: keeping a per-field context buffer (rejected: adds privacy surface and
no quality benefit, since the request already contains the full current message).

---

## R10. Asset format and loading (SC-010, SC-011)

**Decision**:
- **Format**: a single versioned binary file `assets/es-CO-<version>.pta`. It contains:
  - a header (magic number, format version, asset version, τ values, checksum, source manifest hash);
  - an FST map from folded word to word-id list (`fst` crate);
  - a canonical-spelling table;
  - n-gram tables as sorted packed keys with quantized scores;
  - precomputed top-32 next-word lists.
- **Loading**: the core loads the asset from a byte slice (`&[u8]`), so the core never touches the
  filesystem. A thin loader in the tools crate (and later in each client) reads the file. Unknown
  versions or bad checksums are refused with an error, and the engine never runs on a corrupt asset.
- **Size**: pruning thresholds (minimum counts) are tuned so the asset is ≤ 40 MB and the loaded
  working set is ≤ 70 MB, leaving headroom under 100 MB.

**Rationale**: FST lookups work directly on the byte buffer, so loading is fast (well under the
3-second target) and memory-compact. A byte-slice API also works in WebAssembly, where there is no
filesystem.

**Alternatives considered**: SQLite or another embedded database (forbidden by Principle IV);
JSON or text assets (too large and slow to parse); memory-mapped files only (not available in
WASM).

---

## R11. Measuring latency, memory, privacy (SC-009–SC-013)

**Decision**:
- **Latency**: `predi-eval` times each `predict` call with a monotonic clock while replaying the test
  split keystroke by keystroke, after a warm-up of 1,000 requests. It reports p50, p95, p99 and
  max. A `criterion` micro-benchmark tracks regressions in CI.
- **Memory**: `predi-eval` samples the process working set (peak and steady) using the
  `memory-stats` crate (`GetProcessMemoryInfo` on Windows) and reports peak.
- **Startup**: time from process start to the first successful prediction.
- **Privacy, static**: the core crate has no networking or filesystem dependency. CI forbids
  `std::net`, `std::fs`, `std::process` and logging macros in `predi-core` through Clippy's
  `disallowed-methods`/`disallowed-types` lints, and dependencies are audited with `cargo deny`.
- **Privacy, runtime**: on Windows, the quickstart runs `predi-eval` under Sysinternals Process
  Monitor with a filter for file writes and network events from the process. On Linux CI, the same
  replay runs under `strace -f -e trace=network,write,openat`, and a script asserts no network
  syscalls and no writes other than the report file.
- **Determinism (SC-008)**: `predi-eval --check-determinism` runs the replay twice and diffs the
  outputs.

**Rationale**: Every criterion gets a repeatable, scripted measurement (Principle IX). Static
enforcement prevents regressions between manual runs.

---

## R12. Testing approach (Principle IX, FR-036)

**Decision**:
- **Unit tests** per module: normalization, tokenizer, casing, folding, scoring, asset reader.
- **Correctness set**: `data/correctness/cases.jsonl` holds curated cases (input text → expected
  suggestions present/absent, casing, replacement span). It runs as an integration test against a
  small fixture asset and as part of `predi-eval` against the real asset.
- **Property tests** (`proptest`): for any Unicode input, the engine never panics (FR-029). Applying
  any suggestion leaves the text before the partial word byte-identical (FR-004 and SC-007).
  Output is deterministic (FR-007), and there are never more than 3 suggestions or duplicates.
- **WASM build check**: CI runs `cargo build -p predi-core --target wasm32-unknown-unknown` to keep the
  core shareable with the future Chrome client. Running the engine in a browser is out of scope here.

---

## Summary of resolved unknowns

| Technical Context item | Resolved in |
|------------------------|-------------|
| Language/Version | R1 |
| Primary dependencies | R2, R10, R11, R12 |
| Storage | R10 (static asset file; no database) |
| Testing | R11, R12 |
| Target platform | R1 (Windows x86-64 native; WASM build check) |
| Prediction approach | R2, R6, R7 |
| Data sources and licensing | R3, R8 |
| Text handling | R4, R5 |
| Context lifecycle | R9 |
| Measurement | R11 |
