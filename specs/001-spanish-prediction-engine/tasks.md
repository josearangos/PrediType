---

description: "Task list for the Spanish Prediction Engine"
---

# Tasks: Spanish Prediction Engine

**Input**: Design documents from `/specs/001-spanish-prediction-engine/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md),
[data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)

**Tests**: These are included. The spec requires a curated correctness set and repeatable evaluation
(FR-036, FR-037), and constitution Principle IX requires automated tests.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested
independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story the task belongs to (US1–US6, from spec.md)
- Paths follow plan.md: a Cargo workspace with `crates/predi-core` (engine, no I/O) and
  `crates/predi-tools` (build, CLI, eval), plus `data/`, `docs/`, `assets/`, `scripts/` at the
  repository root.
- 🧑 marks tasks that need a human decision or review before they can be called done.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Workspace, toolchain, linting, and directory layout

- [X] T001 Create the Cargo workspace:
  - `Cargo.toml`: members `crates/predi-core` and `crates/predi-tools`, `edition = "2024"`,
    `resolver = "3"`, release profile `lto = "thin"`.
  - `rust-toolchain.toml`: pinned to stable 1.99 with the `wasm32-unknown-unknown` target.
  - `.gitignore`: ignore `target/`, `data/sources/raw/`, and `reports/*`, but keep
    `reports/baseline-eval.json`.
  - `.gitattributes`: `data/sources/snapshots/** filter=lfs diff=lfs merge=lfs -text`, so the pinned
    source snapshots are stored with Git LFS.
- [X] T002 Create `crates/predi-core/Cargo.toml`:
  - Runtime dependencies are only `fst`, `unicode-normalization`, and `sha2`. `sha2` is pure Rust,
    WASM-compatible, and needed for the asset checksum (R10). Add `sha2` to the plan.md Technical
    Context dependency list.
  - Dev-dependencies: `proptest`, `criterion`, `serde`, `serde_json`.
  - `crates/predi-core/src/lib.rs`: declare modules `types`, `normalize`, `tokenize`, `casing`,
    `asset`, `model`, `rank`, `state`, and add `#![forbid(unsafe_code)]`.
- [X] T003 [P] Create `crates/predi-tools/Cargo.toml`:
  - Binaries `predi-build`, `predi-cli`, `predi-eval` (in `src/bin/`).
  - Dependencies: `predi-core` (path), `clap` (derive), `serde`, `serde_json`, `toml`, `sha2`,
    `memory-stats`.
  - Module directories `src/build/` and `src/eval/`, each with a `mod.rs`.
- [X] T004 [P] Enforce the no-I/O, no-logging rule in the core (FR-023, FR-027, research R11):
  - Create `clippy.toml` with `disallowed-methods`, `disallowed-types`, and `disallowed-macros`
    covering `std::fs::*`, `std::net::*`, `std::process::*`, `println!`, `eprintln!`, `print!`,
    `eprint!`, and `dbg!`.
  - Add `#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro, clippy::disallowed_methods, clippy::disallowed_types, clippy::disallowed_macros)]`
    to `crates/predi-core/src/lib.rs`.
  - **Done 2026-10-16**: `clippy.toml` lives in `crates/predi-core/` so the bans apply only to the
    engine crate (predi-tools legitimately reads files and prints); plan.md updated.
- [X] T005 [P] Create `deny.toml`:
  - License allowlist: MIT, Apache-2.0, BSD-2/3-Clause, Unicode-3.0, Zlib, MPL-2.0.
  - Ban networking crates (`reqwest`, `hyper`, `ureq`, `tokio`) from the workspace.
- [X] T006 [P] Create the CI workflow `.github/workflows/ci.yml` with these jobs:
  - `cargo fmt --check`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo test --workspace`
  - `cargo build -p predi-core --target wasm32-unknown-unknown`
  - A WASM test job: `wasm-pack test --node crates/predi-core` (or `cargo test` with the
    `wasm-bindgen-test-runner`), running the correctness cases and comparing them with native output
    (SC-008, FR-007). The cases and fixture are embedded with `include_str!`/`include_bytes!` so the
    test needs no filesystem.
  - `cargo deny check`
  - A `windows-latest` job running `cargo build --workspace` and `cargo test --workspace` (FR-032,
    constitution §V). This verifies that the code builds and passes tests on the target OS. It
    doesn't replace measurement on the reference computer (T065).
- [X] T007 [P] Create the directory scaffolding with short `README.md` files explaining each purpose:
  `data/sources/`, `data/corpus/es-co-cs/`, `data/correctness/`, `data/lexicon/`, `assets/`,
  `docs/`, `scripts/`, `reports/`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Data sources, corpus, core types, text handling, asset format, builder skeleton,
engine skeleton, and test harness

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Data and documentation

- [x] T008 🧑 Verify the licenses of the Tatoeba Spanish sentences (expected CC BY 2.0 FR) and the
  RLA-ES `es_CO` Hunspell dictionary (expected GPL/LGPL/MPL 1.1 tri-license, MPL used) for
  redistribution in the product (FR-028, research R3).
  - Record the source URL, license, retrieval date, and attribution text in `docs/data-sources.md`
    and `THIRD_PARTY_DATA.md`.
  - Stop and escalate if either license doesn't allow redistribution.
  - **Done 2026-10-16**: both allow redistribution (Tatoeba CC BY 2.0 FR; RLA-ES `es_CO` v2.9 under
    MPL 2.0 via "MPL 1.1 or later"). See `docs/data-sources.md` and `THIRD_PARTY_DATA.md`. A legal
    sign-off is still recommended before the first public release.
- [X] T009 Create `data/sources/manifest.toml`, with fields `name`, `url`, `license`, `retrieved`,
  `sha256`, and `role` (`general` | `spelling` | `domain`) per entry.
  - Commit the pinned snapshots `data/sources/snapshots/spa_sentences.tsv.bz2` and
    `data/sources/snapshots/es_CO.oxt` through Git LFS. Their SHA-256 values are recorded in
    `docs/data-sources.md`. Tatoeba regenerates its export weekly, so the live URL can't be the
    canonical input.
  - Create `scripts/fetch-sources.sh`:
    - By default it verifies each snapshot's `sha256` against the manifest, then extracts it into
      `data/sources/raw/`: decompress the Tatoeba TSV, unzip `es_CO.aff` and `es_CO.dic` from the
      `.oxt` file, and expand the dictionary with Hunspell `unmunch es_CO.dic es_CO.aff` into
      `data/sources/raw/es_CO.words` (UTF-8, one word per line).
    - It downloads from the upstream URLs only with `--refresh`, which also prints the new hashes.
      A refresh requires updating the manifest and bumping the asset version.
    - It fails with a clear message if `unmunch` or `git lfs` isn't installed, or if a hash doesn't
      match.
  - **Done 2026-10-16**: snapshots placed under `data/sources/snapshots/` with `.gitattributes` LFS
    rules; hashes verified. 2026-10-21: Git LFS dropped; the snapshots (7.5 MB) are committed
    directly. `fetch-sources.sh` also
    adds source words Hunspell accepts, because `unmunch` misses some plurals; it needs `hunspell`.
- [X] T010 [P] Write `docs/locale-profile.md` for Colombian Spanish with formal *usted* (FR-022):
  - register rules;
  - a vocabulary-differences table (`celular`/`móvil`, `computador`/`ordenador`, and similar);
  - customer-service phrases (`con mucho gusto`, `a la orden`).

  Also create `data/lexicon/register-flags.txt` (one word per line) listing the *tú*-form and
  other-variant words to flag, for example `ayudarte`, `te`, `tu`, `puedes`, `móvil`, `ordenador`,
  and `vosotros`.
- [X] T011 [P] Create `docs/reference-computer.toml` with the keys `model`, `cpu`, `cores`,
  `base_clock_ghz`, `ram_gb`, `os_build`, `power_plan`, and `is_reference`, matching
  `contracts/eval-report.schema.json` `machine`. Prefill it with the spec's reference description:
  4 cores, ≤ 2.5 GHz, 8 GB RAM, Windows 10/11, mains power, default plan.
- [X] T012 [P] Create `data/allowlist-proper-nouns.txt` with Colombian places, institutions, and
  acronyms that must keep their capitalization (FR-019), for example `Colombia`, `Bogotá`,
  `Medellín`, `Cali`, `Barranquilla`, `PSE`, `DIAN`, `NIT`. Add a header comment explaining the
  rule: no person names.

  Also create `data/given-names.txt` (one name per line), a list of common Colombian given names
  used by the build-time personal-data scan (FR-028). Record its source and license in
  `docs/data-sources.md`, or note that the team wrote it.
- [X] T013 🧑 Write the corpus guidelines in `data/corpus/es-co-cs/README.md`:
  - Placeholders instead of personal data: `{NOMBRE}`, `{NUMERO_PEDIDO}`, `{TELEFONO}`, `{CORREO}`,
    `{FECHA}`, `{VALOR}`.
  - The *usted* register.
  - The category list `saludo`, `disculpa`, `pregunta`, `pedido`, `envio`, `facturacion`, `cuenta`,
    `cierre`, `otro`.

  Then draft `data/corpus/es-co-cs/messages.tsv` (columns `category<TAB>text`, UTF-8):
  - At least 1,500 messages, with at least 120 per category except `otro`.
  - Include accents, `ñ`, `¿`, `¡`, and mixed punctuation.
  - It needs human review: no real customer data, natural Colombian phrasing.
  - **Draft done 2026-10-16** (AI-assisted): 1,629 messages, 16,557 words, 174–194 per category, no
    duplicates, digits, or *tú* forms. **Human review done 2026-10-21.**
- [X] T014a Implement the split assignment in `crates/predi-tools/src/build/splits.rs` and the
  `predi-build split` subcommand, with unit tests that run on a temporary copy of the corpus. This
  task only writes and tests the code; it does **not** freeze the real splits:
  - Hash each message's `text` field only (not the category), after NFC normalization.
  - Assign each message to a split by the **rank** of its SHA-256 hash (sorted ascending): the first
    ⌈n/3⌉ are `test`, the next ⌈n/6⌉ are `dev`, the rest `build`. Using rank instead of a random
    ratio gives exact counts. With the current 1,629 messages, that's 543 test messages (about
    5,560 words) and 272 dev messages.
  - Write `data/corpus/es-co-cs/splits.tsv` as `message_sha256<TAB>split`.
  - Never reassign a hash that's already listed: the split is "assigned once and immutable".
  - Fail unless there are at least 500 test messages, at least 250 dev messages, and at least 5,000
    word positions in test.
- [X] T014b 🧑 Run `predi-build split` once on the real corpus and commit
  `data/corpus/es-co-cs/splits.tsv`. Do this **only after** the T013 human review, because editing
  a message after freezing changes its split. It blocks T058, T064, and T065, but not US1–US4.

### Core types and text handling (`predi-core`)

- [X] T015 [P] Define the public types in `crates/predi-core/src/types.rs`, per data-model.md:
  - `PredictionRequest { request_id: u64, text_before_caret: &str }`.
  - `Suggestion { text, kind, replace_chars, replace_utf16, leading_space, rank, confidence }`, with
    these doc-comment constraints:
    - "`replace_chars` … never greater than the partial word's length";
    - "`rank` 1–3, contiguous";
    - "`text` never begins or ends with whitespace";
    - "`leading_space` always false for `Completion`".
  - `SuggestionKind { Completion, NextWord, Continuation }`.
  - `Status { Ok, Paused, NotApplicable, LowConfidence }`.
  - `PredictionResult { request_id, suggestions (0–3), status }`.
  - `EngineInfo` and `AssetError`. `AssetError`'s variants are `Truncated`, `BadMagic`,
    `UnsupportedFormat { found, supported }`, `BadChecksum`, and `Malformed(&'static str)`, and they
    carry no input text.
- [X] T016 [P] Implement `crates/predi-core/src/normalize.rs` with unit tests in the same file:
  - NFC normalization.
  - `fold()`: lowercase, strip combining diacritics, `ñ`→`n`, `ü`→`u`.
  - `matches_exact_diacritics(typed, canonical)`.
  - `restores_diacritics(typed, canonical)`: true when they fold equal but differ only by added
    accents, `ñ`, or `ü`.
- [X] T017 [P] Implement `crates/predi-core/src/tokenize.rs` with unit tests:
  - Truncate the input to the last 1,000 Unicode scalar values at a word boundary (FR-031).
  - Word characters are letters, plus a hyphen or apostrophe only between letters (research R4).
  - Normalize to NFC only for lookups. Report the partial word's start offset and length against
    the **original** input string, so replacement counts match the client's text even when it
    arrives in decomposed form (NFD) (FR-003).
  - Classify the ending as `Empty`, `InsideWord { partial, start }`, or `Boundary`. Report the
    previous words of the current message, the last boundary characters (for sentence-start
    detection), and whether a leading space is needed, per the FR-041 table (`¿ ¡ ( «`, an opening
    straight quote, whitespace, and line breaks need none).
  - Return `NotApplicable` when the current token contains a digit, `@`, `/`, `\`, `_`, `=`, `{`,
    `}`, `<`, `>`, or a `.` or `:` between two letters or digits, is longer than 30 characters, or
    the partial word contains a letter outside Latin plus áéíóúüñ (FR-012, FR-029, FR-040). Emoji
    and other symbols are boundaries.
- [X] T018 Implement the asset reader in `crates/predi-core/src/asset.rs`, following
  `contracts/asset-format.md`:
  - Check the magic `PREDITYP`, format version `1`, the reserved bytes, the asset version string,
    the τ values and α, the source-manifest hash, the payload SHA-256 (offset 128 to end of file),
    and the 6-entry section table with 8-byte alignment.
  - Load the lexicon section: the folded-key FST mapping to word ids, canonical spellings, and the
    word-flags byte per word id. Bits 0–1 hold the casing (`Lower`, `Capitalized`, `Acronym`),
    bit 2 holds `register_other`, and bits 3–7 are reserved and must be 0.
  - Reject the asset with the matching `AssetError` (FR-038, FR-039).
  - The reader works on `&[u8]` only, with no filesystem access.
- [X] T019 Implement the asset writer in `crates/predi-tools/src/build/writer.rs`, mirroring the
  layout in `contracts/asset-format.md`. Add a round-trip test in
  `crates/predi-tools/tests/asset_roundtrip.rs`: a written asset must read back identically
  through `predi_core` (depends on T018).
- [X] T020 [P] Implement the source readers in `crates/predi-tools/src/build/sources.rs`:
  - Tatoeba `sentences.csv`, keeping only Spanish (`spa`) rows.
  - The `es_CO.words` list.
  - `messages.tsv`, filtered to the `build` split using `splits.tsv`.
  - Return an error with exit code 3 if any `test` hash appears in the build input (contracts/cli.md).
- [X] T021 [P] Implement the personal-data filter in `crates/predi-tools/src/build/pii.rs` (research
  R8):
  - Strip `{PLACEHOLDER}` tokens.
  - Drop tokens with digits, `@`, or URL-like form.
  - Drop tokens that appear only capitalized mid-sentence and aren't in
    `data/allowlist-proper-nouns.txt`.
  - Collect dropped tokens with counts for the build report.
  - After the asset is assembled, scan its vocabulary and phrases for email and phone patterns,
    digit runs, and entries with `Capitalized` casing that are on `data/given-names.txt` but not in
    the allowlist, and fail the build if any match (FR-028, SC-005). Lowercase words that are also
    names (`luz`, `paz`) pass.
- [X] T022 Implement unigram counting and lexicon building in `crates/predi-tools/src/build/lexicon.rs`
  (depends on T020, T021):
  - Tokenize with `predi_core::tokenize`.
  - Count with a domain weight parameter.
  - Pick the canonical spelling: the most frequent form that is present in `es_CO.words`, or present
    in `data/lexicon/domain-words.txt`.
  - Set the word flags: casing bits `Capitalized` if allowlisted, `Acronym` if all-caps in the
    allowlist; the `register_other` bit from `data/lexicon/register-flags.txt`.
  - Build the folded-key FST.
- [X] T023 Implement the `predi-build build` subcommand in `crates/predi-tools/src/bin/predi-build.rs`:
  - Arguments per `contracts/cli.md`: `--manifest`, `--corpus`, `--allowlist`, `--asset-version`,
    `--out`, plus a placeholder `--tune-tau` flag.
  - Use default τ values (`tau_completion = 0.15`, `tau_next = 0.15`) and α = 0.4 until US5 tunes
    them.
  - Write the asset and `<out>.build-report.txt` (sizes, pruning, dropped-token counts, τ).
  - Exit codes: 0, 2 (invalid input), 3 (test leakage).
  - Add a `--plain-sources <dir>` mode for test fixtures. It reads `sentences.txt` (one sentence per
    line) and `words.txt` (one word per line) from the directory, needs no manifest and no splits,
    and writes a source-manifest hash of all zeros. Only `scripts/build-fixture.sh` uses it.

### Engine skeleton and test harness

- [X] T024 Implement `crates/predi-core/src/state.rs` (an `AtomicBool` paused flag) and the `Engine`
  skeleton in `crates/predi-core/src/lib.rs`:
  - `from_bytes`, `predict`, `pause`, `resume`, `field_changed` (a no-op), and `info`, per
    `contracts/engine-api.md`.
  - `predict` returns `Paused` when paused, `NotApplicable` from the tokenizer, and otherwise an
    empty `LowConfidence` result.
  - Add a compile-time `Send + Sync` assertion.
- [X] T025 Implement `crates/predi-core/src/rank.rs`:
  - Confidence is each candidate's share of the total candidate score (R6).
  - Apply the per-kind cutoff (`tau_completion` / `tau_next`).
  - Drop duplicates by `fold()` of the final text (FR-013).
  - Keep at most 3, with contiguous ranks 1–3.
  - Break ties deterministically: score descending, then canonical text ascending (FR-007).
  - Set status `LowConfidence` when nothing qualifies.
- [X] T026 Create the small test sources in `crates/predi-core/fixtures/mini-sources/`, about 300
  Colombian customer-service sentences plus a word list.
  - Create `scripts/build-fixture.sh`, which runs `predi-build build --plain-sources
    crates/predi-core/fixtures/mini-sources` to produce `crates/predi-core/fixtures/mini.pta`.
  - Commit the built fixture, so `predi-core` tests don't depend on `predi-tools` at test time.
- [X] T027 Implement the correctness test runner in `crates/predi-core/tests/correctness.rs`:
  - Parse each line of `data/correctness/cases.jsonl` according to
    `contracts/correctness-case.schema.json`. Embed the file and `fixtures/mini.pta` with
    `include_str!`/`include_bytes!`, so the same runner also works on WASM (T006).
  - Run each case against `fixtures/mini.pta` and assert every `expect` key.
  - Report failing case ids.
  - Seed `data/correctness/cases.jsonl` with `empty` and `not_applicable` cases, for example the
    email `Escríbanos a soporte@emp`, `Pedido 12345`, and `https://ej`.
- [X] T028 [P] Implement `predi-cli` in `crates/predi-tools/src/bin/predi-cli.rs`:
  - `predict "<text>"` prints the `PredictionResult` as JSON.
  - `repl` reads stdin line by line, prints results, writes nothing to disk, and keeps no history
    (contracts/cli.md).

**Checkpoint**: The workspace builds, the asset round-trips, the engine loads the fixture, and the
correctness runner is green on the seed cases.

---

## Phase 3: User Story 1: Complete a partially typed word (Priority: P1) 🎯 MVP

**Goal**: When the text ends inside a word, return up to 3 correctly spelled completions of it.

**Independent Test**: Run `cargo test -p predi-core --test correctness` (partial/accents/enye
cases), then run `predi-cli predict "Buenos dí"` and see `días` with `replace_chars` 2.

### Tests for User Story 1

- [X] T029 [P] [US1] Add completion cases to `data/correctness/cases.jsonl`:
  - `Buenos dí` → contains `días`, `replace_chars` 2.
  - `Su contrase` → contains `contraseña`.
  - `Le pedimos una disculpa por las molest` → contains `molestias`.
  - `Gracias por su comunicacion` → contains `comunicación`, `replace_chars` 12 (accent restoration
    allowed).
  - `an` → may contain `año`.
  - `Muchas gracias d` → not_contains `de` (adds fewer than 2 characters).
  - `gracias` → every suggestion `kind` is `Completion`, and `gracias` is not suggested.
  - `ayud` → not_contains `ayudarte`.
  - `ayudart` → contains `ayudarte`.
  - **Done 2026-10-16, with one change**: `ayudart` → `ayudarte` adds only 1 character, so FR-008
    (completions add ≥ 2 characters unless they restore accents) forbids it. This contradicts the
    FR-022 example. The case uses `Esto es para vosot` → `vosotros` instead. Needs a spec decision.
- [X] T030 [P] [US1] Write the property tests in `crates/predi-core/tests/properties.rs` (proptest,
  arbitrary Unicode input up to 2,000 characters, run against `fixtures/mini.pta`). For every input:
  - no panic (FR-029);
  - at most 3 suggestions with contiguous ranks;
  - no fold-duplicates;
  - applying any suggestion per the data-model application rule leaves all text before the partial
    word byte-identical (FR-004);
  - repeated calls give identical results (FR-007);
  - `text` has no leading or trailing whitespace;
  - when the input ends inside a word, every suggestion is a `Completion` (G10);
  - **deletion**: for any input and any number of characters removed from its end, the result for
    the shortened text equals that of a freshly constructed engine, even after the engine has
    served the longer text (constitution §IX);
  - **decomposed input**: for inputs converted to NFD (for example `Buenos di` + U+0301), applying
    any suggestion by `replace_chars`/`replace_utf16` against the original string yields correct,
    uncorrupted text (FR-003, SC-007).

### Implementation for User Story 1

- [X] T031 [US1] Implement prefix candidate lookup in `crates/predi-core/src/model.rs`:
  - Do an FST range query on the folded partial word.
  - Collect candidates in deterministic order, capped at 5,000 visited keys.
  - Score by unigram weight.
  - Add the exact-diacritic bonus when `matches_exact_diacritics` holds (R4).
- [X] T032 [US1] Apply the completion eligibility rules in `crates/predi-core/src/model.rs`
  (FR-008, FR-016):
  - Keep a candidate if it adds at least 2 characters, or if `restores_diacritics(partial,
    canonical)`.
  - Exclude the identical word.
  - Apply the register filter (FR-022): exclude flagged words while any unflagged candidate still
    matches the folded prefix.
- [X] T033 [US1] Wire the `InsideWord` path into `Engine::predict` in `crates/predi-core/src/lib.rs`:
  - Compute `replace_chars` (scalar values of the partial word) and `replace_utf16`.
  - Set `leading_space = false` and `kind = Completion`.
  - Pass candidates through `rank.rs`.
  - Return status `Ok` or `LowConfidence`.
- [X] T034 [US1] Run `scripts/build-fixture.sh` to rebuild `crates/predi-core/fixtures/mini.pta`,
  making sure the fixture sources contain the T029 words. Get T029 and T030 passing.

**Checkpoint**: Word completion works through `predi-cli` and the tests, with no context model yet.

---

## Phase 4: User Story 2: Predict the next word after a completed word (Priority: P1)

**Goal**: At a word boundary, return up to 3 likely next words using trigram, bigram, and unigram
context. Completions also benefit from context.

**Independent Test**: Run the correctness boundary cases, then run
`predi-cli predict "Muchas gracias por "` and get 1–3 plausible words.

### Tests for User Story 2

- [X] T035 [P] [US2] Add boundary cases to `data/correctness/cases.jsonl`:
  - `Muchas gracias por ` → non-empty, `leading_space` false.
  - `Hola,` → `leading_space` true.
  - `Hola, ` → `leading_space` false.
  - `¿En qué puedo ` → contains `ayudarle`, not_contains `ayudarte`.
  - Empty input → status `Ok` or `LowConfidence`, at most 3 suggestions, all capitalized.
  - `¿` → `leading_space` false.
  - FR-041 leading-space cases. `leading_space` true: `Listo;`, `Gracias.`, `(ver anexo)`,
    `«Hola»`. `leading_space` false: `(`, `«`, `"` (at the start), `Hola\n`.
  - Deletion sequence (`area: deletion`): `Buenos días` (the complete word, no suggestion is
    `días`) → `Buenos dí` (contains `días`) → `Buenos d` → `Buenos ` (next-word results) →
    `Bueno` (completion results only). None of them suggests text that was only present before
    the deletion.

### Implementation for User Story 2

- [X] T036 [US2] Implement n-gram counting in `crates/predi-tools/src/build/ngrams.rs`:
  - Count bigrams and trigrams with a message-start token `<s>` and sentence-end handling.
  - Apply the domain interpolation weight to counts from the team set.
  - Prune by minimum count, configurable, with the target asset ≤ 40 MB.
  - Quantize Stupid Backoff scores to `u16`.
  - Precompute "up to 32 ranked next-word ids per frequent unigram or bigram context".
  - Write the sections via `writer.rs`.
- [X] T037 [US2] Extend `crates/predi-core/src/asset.rs` to read the n-gram tables (sorted packed
  keys with binary search) and the top-next lists (depends on T036 for the layout).
- [X] T038 [US2] Implement Stupid Backoff next-word scoring in `crates/predi-core/src/model.rs`:
  - Take the context from the last 2 words of the current message, resetting after sentence-ending
    punctuation to `<s>`.
  - Back off trigram → bigram → unigram with α from the header.
  - Candidates come from the top-next lists.
- [X] T039 [US2] Use context for completions in `crates/predi-core/src/model.rs` (R2): filter the
  context candidates by the folded prefix first, then fill from the T031 prefix lookup when fewer
  than 3 survive.
- [X] T040 [US2] Wire the `Boundary` and `Empty` paths in `crates/predi-core/src/lib.rs`:
  - `kind = NextWord`, `replace_chars = 0`.
  - Take `leading_space` from the tokenizer (T017), which applies FR-041. Don't recompute it here.
  - Exclude register-flagged words (FR-022).
  - Empty input uses the `<s>` context, with openers subject to the cutoff (FR-011).
- [X] T041 [US2] Rebuild `crates/predi-core/fixtures/mini.pta` with n-grams via
  `scripts/build-fixture.sh`, and get T035, T029, and T030 passing.

**Checkpoint**: Completion and next-word prediction work together, and US1 tests still pass.

---

## Phase 5: User Story 3: Respect Spanish orthography, capitalization, and punctuation (Priority: P2)

**Goal**: Suggestions match sentence position and the agent's casing, keep canonical spelling, and
never touch completed words.

**Independent Test**: All `casing`, `accents`, `enye`, `opening_punct`, and `punctuation` cases in
the correctness set pass.

### Tests for User Story 3

- [X] T042 [P] [US3] Add casing cases to `data/correctness/cases.jsonl`:
  - `Su pedido fue enviado. ` → `all_capitalized`.
  - `¿` → `all_capitalized`.
  - `BUE` → `all_uppercase`.
  - `Gracias, le` → `all_lowercase`.
  - `Hola.\n` → `all_capitalized`.
  - `Lo esperamos en Bog` → contains `Bogotá`.
  - `Hola dias ` → no suggestion has `replace_chars` > 0 (FR-021).
  - `Gracias por esperar… ` → `all_capitalized`.
  - `Gracias por esperar... ` → `all_capitalized`.
  - `Gracias, ¿c` → `all_lowercase`; `Listo. ¿C` → `all_capitalized` (FR-017).
  - `Tenga en cuenta lo siguiente: ` → `all_lowercase`; `Estimado cliente:\n` → `all_capitalized`.
  - `Le dijimos "gracias` → `all_lowercase` (quotes follow the surrounding sentence).
  - `Hola, bUe` → `all_lowercase`; `Hola, BUe` → `all_capitalized` (FR-018).
  - `Su 2do` → empty, status `NotApplicable` (FR-040).

### Implementation for User Story 3

- [X] T043 [US3] Implement `crates/predi-core/src/casing.rs` with the priority rules from research
  R5, and unit tests for each rule:
  1. Lexicon casing, unless the partial word is all-caps.
  2. All-caps partial word with at least 2 letters → output all-caps.
  3. Sentence start (message start, or after `.`, `?`, `!`, `…`, `...`, or a line break, optionally
     followed by `¿`/`¡` and whitespace) → capitalize the first word. A mid-sentence `¿`/`¡` does
     not start a sentence; after a colon, lowercase unless a line break follows; quotes and
     parentheses are transparent (FR-017).
  4. Partial word with a capital first letter → keep the initial capital. Other mixed case is
     ignored (FR-018).
  5. Otherwise lowercase.
- [X] T044 [US3] Apply casing in the output pipeline in `crates/predi-core/src/lib.rs` before
  `rank.rs` deduplication, so duplicates are detected on the final text. Continuations case only
  their first word.
- [X] T045 [US3] Add spelling validation to `crates/predi-tools/src/build/lexicon.rs`:
  - Each canonical form must be in `es_CO.words` or `data/lexicon/domain-words.txt`.
  - List non-dictionary words in the build report and exclude them from the lexicon.
  - Create `data/lexicon/domain-words.txt` (one word per line) for legitimate domain terms.

**Checkpoint**: Orthography cases pass, and US1 and US2 cases still pass.

---

## Phase 6: User Story 4: Transient, private context (Priority: P2)

**Goal**: Show that no text is retained, written, or transmitted, that pause works, that a field
change equals a fresh engine, and that a missing or broken asset fails safely.

**Independent Test**: `cargo test -p predi-core --test lifecycle --test asset_errors` passes, and
`scripts/check-privacy.sh` reports no network or unexpected writes.

### Tests for User Story 4

- [X] T046 [P] [US4] Write `crates/predi-core/tests/lifecycle.rs`:
  - After `pause()`, every `predict` returns empty with status `Paused` (FR-025).
  - After `resume()`, results return.
  - After `field_changed()`, results for 200 sample inputs equal those of a freshly constructed
    engine (FR-024, SC-013).
  - A fixed request gives an identical result before and after 10,000 random requests (FR-026).
- [X] T047 [P] [US4] Write `crates/predi-core/tests/asset_errors.rs`. Each of these inputs makes
  `from_bytes` return the matching `AssetError`, and the error's `Display` and `Debug` output
  contains no request text (FR-038, FR-039):
  - empty bytes;
  - a header truncated at 50 bytes;
  - wrong magic;
  - format version 2;
  - one flipped payload byte (bad checksum);
  - a section offset beyond the end of the file.

### Implementation for User Story 4

- [X] T048 [US4] Harden `crates/predi-core/src/lib.rs` and `crates/predi-core/src/tokenize.rs`:
  - Make sure no code path formats request text into errors, panics, or `Debug` output. Implement
    `Debug` for `PredictionRequest` manually, with the text redacted.
  - Replace any `unwrap` or `expect` on input-dependent values with total handling.
- [X] T049 [US4] Write `scripts/check-privacy.sh` for Linux:
  - Run `predi-eval --only performance` under
    `strace -f -e trace=network,write,openat,creat,rename`.
  - Fail on any network syscall, or on a write-mode open outside `reports/`.
  - Add it as a CI job in `.github/workflows/ci.yml`.
  - **Done 2026-10-16**: written and added to CI; not yet executed (no `strace` on the macOS dev
    machine). Its first real run is the CI `privacy` job.
- [X] T050 [P] [US4] Write `docs/privacy-verification.md` describing the Windows Process Monitor
  procedure from quickstart §5: filter, expected events, and how to record the result in the
  evaluation notes.

**Checkpoint**: Privacy and lifecycle guarantees are tested, and asset failures are safe.

---

## Phase 7: User Story 5: Repeatable quality and performance evaluation (Priority: P2)

**Goal**: One command replays the test split and reports every success criterion with pass/fail,
hardware, and versions. The τ values are tuned on the dev split.

**Independent Test**: Run `predi-eval --check-determinism` twice on the same machine. Quality
metrics should be identical, and the report should validate against
`contracts/eval-report.schema.json`.

### Tests for User Story 5

- [X] T051 [P] [US5] Write `crates/predi-tools/tests/eval_report.rs`:
  - A generated report validates against `contracts/eval-report.schema.json`. Check required keys,
    criterion ids matching `^SC-0(0[1-9]|1[0-5])$`, and the `comparison` enum.
  - `detail` values are only numbers, booleans, or case-id lists, and no string anywhere in the
    report contains corpus text (FR-037). Check this by searching the report for every test-split
    message.
  - Provisional criteria never cause a non-zero exit code.

### Implementation for User Story 5

- [X] T052 [P] [US5] Implement the report types and JSON writer in
  `crates/predi-tools/src/eval/report.rs`, following `contracts/eval-report.schema.json`.
  - Mark SC-001–SC-004, SC-014, and SC-015 as `provisional: true`.
  - Write to `reports/eval-<UTC timestamp>.json`.
- [X] T053 [US5] Implement the keystroke replay in `crates/predi-tools/src/eval/replay.rs`:
  - For each test message, issue a request at every character position.
  - Classify each position as a partial word (with typed-letter count), a boundary, or
    not-applicable.
  - Record the intended word or text at that position and the returned result.
- [X] T054 [US5] Implement the quality and correctness metrics in
  `crates/predi-tools/src/eval/metrics.rs`:

  | Criterion | What it measures |
  |-----------|------------------|
  | SC-001 | Partial-word points (spec "Prediction points"): intended word in top 3, with empty results counted as misses |
  | SC-002 | Boundary points: actual next word in top 3 |
  | SC-003 | Simulated keystroke savings per the spec formula: characters not typed minus one per acceptance; inserted leading space saved, trailing space typed, accent-only replacement saves nothing |
  | SC-004 | First rank given that a correct suggestion is present |
  | SC-014 | First-rank precision among non-empty results |
  | SC-005 | Suggestions not in `es_CO.words` or the domain list, or matching the personal-data patterns |
  | SC-007 | Prefix preservation for every applied suggestion |
  | SC-015 | Share of shown continuations that match the following corpus text in full, plus the count shown (provisional) |
- [X] T055 [US5] Implement the performance measurement in `crates/predi-tools/src/eval/perf.rs`:
  - Run a 1,000-request warm-up, then time each `predict` with `std::time::Instant` and report p50,
    p95, p99, and max (SC-009).
  - Sample the working set with `memory-stats` before, during, and after the replay and report the
    peak (SC-010).
  - Measure startup as the time from process start to the first successful prediction (SC-011).
- [X] T056 [US5] Add the remaining checks in `crates/predi-tools/src/eval/checks.rs`:
  - SC-006: run `data/correctness/cases.jsonl` against the real asset.
  - SC-008: with `--check-determinism`, replay twice and diff all results.
  - SC-013: compare results before and after `field_changed()` on a sample of 1,000 requests with a
    fresh engine.
- [X] T057 [US5] Implement `crates/predi-tools/src/bin/predi-eval.rs`:
  - Arguments per `contracts/cli.md`: `--asset`, `--corpus`, `--split`, `--cases`, `--machine`,
    `--out`, `--check-determinism`, `--only`.
  - Print a pass/fail table.
  - Exit 0 if all hard gates pass and 1 otherwise. Provisional criteria never fail the run.
- [X] T058 [US5] Implement τ tuning in `crates/predi-tools/src/build/tune.rs` and wire up
  `predi-build --tune-tau`:
  - Replay the `dev` split only (never `test`) to find the smallest `tau_completion` and the
    smallest `tau_next` that each give first-rank precision of at least 0.55 among non-empty
    results (R6).
  - Write both values to the asset header and the build report.
  - **Done 2026-10-16**: implemented and exercised on a scratch split of the draft corpus. The real
    tuning run happens in T064, after T014b.
- [X] T059 [P] [US5] Write the criterion benchmark in `crates/predi-core/benches/predict.rs`,
  covering empty input, a short partial word, a long partial word, a boundary after 1,000
  characters, and a not-applicable token, against `fixtures/mini.pta`.

**Checkpoint**: The evaluation runs end to end and produces a schema-valid report, and τ is tuned
on dev.

---

## Phase 8: User Story 6: Brief multi-word continuations (Priority: P3)

**Goal**: Optionally offer a continuation of 2–4 words for strongly implied phrases.

**Independent Test**: Continuation cases pass, no continuation exceeds 4 words or crosses
sentence-ending punctuation, and the continuation rate is reported by `predi-eval`.

### Tests for User Story 6

- [X] T060 [P] [US6] Add continuation cases to `data/correctness/cases.jsonl`:
  - `Quedamos atentos a ` → `max_words` 4.
  - `¿En qué ` → `max_words` 4, may contain `puedo ayudarle`.
  - `gracias` (inside a word) → no `Continuation` kind.

  Add a property to `crates/predi-core/tests/properties.rs`: every `Continuation` has 2–4 words and
  contains no `.`, `?`, `!`, or `…`.

### Implementation for User Story 6

- [X] T061 [US6] Implement greedy continuation building in `crates/predi-core/src/model.rs` (R7):
  - Start from the top next word and extend with the most likely next word, up to 4 words.
  - Stop when the conditional score falls below `tau_next` or a sentence end is reached.
  - Offer the continuation only at boundaries, with at least 2 words and an overall score of at
    least `tau_next`.
- [X] T062 [US6] Integrate continuations in `crates/predi-core/src/rank.rs`. A continuation takes its
  first word's slot when that word is already a candidate (no duplicates, FR-013), and counts toward
  the limit of 3.
- [X] T063 [US6] Confirm `predi-eval` reports SC-015 (defined in T054) with real continuations: the
  full-match rate as a provisional criterion, plus the number of continuations shown.

**Checkpoint**: All six user stories work and are tested.

---

## Phase 9: Polish and Cross-Cutting Concerns

**Purpose**: Build the real asset, collect baseline evidence, document, and validate

- [ ] T064 Run `scripts/fetch-sources.sh`, then `predi-build build --tune-tau --asset-version 1.0.0`,
  to produce `assets/es-CO-1.0.0.pta` and `assets/es-CO-1.0.0.build-report.txt`.
  - Confirm the asset is ≤ 40 MB and the test-leakage check passed.
  - Adjust the pruning thresholds if needed.
- [ ] T065 🧑 Run `predi-eval --check-determinism` on the reference computer.
  - Save the output as `reports/baseline-eval.json`.
  - All hard gates (SC-005–SC-013) must pass. Record the provisional values.
- [ ] T066 🧑 The project owner amends the provisional quality targets SC-001–SC-004, SC-014, and
  SC-015 once in `specs/001-spanish-prediction-engine/spec.md`, within one week of T065 and before
  any release, citing `reports/baseline-eval.json`. Add a dated
  note under Success Criteria, and mark the criteria as hard gates from then on.
- [X] T067 [P] Write `README.md`:
  - project purpose, workspace layout, and how to fetch sources, build the asset, run the CLI, and
    run the evaluation;
  - known limitations: Colombian *usted* only, no offensive-word filter, not a spell checker, and no
    client integration yet.
- [X] T068 [P] Finalize `docs/data-sources.md` and `THIRD_PARTY_DATA.md` with the attribution and
  license texts that must ship with the asset (FR-028).
- [ ] T069 Run the full `quickstart.md` validation, steps 1–6. This includes the Windows Process
  Monitor privacy check (SC-012), with the outcome recorded in `docs/privacy-verification.md`.
- [ ] T070 Confirm CI is green: fmt, Clippy (no-I/O lints), tests, the WASM build of `predi-core`,
  the correctness set run on the WASM build with results identical to native (SC-008, FR-007),
  `cargo deny`, and `scripts/check-privacy.sh`.

---

## Dependencies & Execution Order

### Phase Dependencies

| Phase | Depends on |
|-------|------------|
| Setup (T001–T007) | Nothing |
| Foundational (T008–T028) | Setup. It blocks all stories. |
| US1 (T029–T034) | Foundational |
| US2 (T035–T041) | Foundational. It enhances US1 completions (T039) but US1 stays independently testable. |
| US3 (T042–T045) | Foundational. Its casing applies to US1 and US2 output. |
| US4 (T046–T050) | Foundational. Independent of the other stories. T049 needs `predi-eval` (T057). |
| US5 (T051–T059) | Foundational. Meaningful metrics need US1 and US2. T058 (τ tuning) needs T053 and T054. |
| US6 (T060–T063) | US2 (needs the n-gram model) |
| Polish (T064–T070) | All desired stories |

### Key Task Dependencies

- T018 → T019. T020 and T021 → T022 → T023. T023 → T026 → T027.
- T036 → T037 → T038 → T039 and T040.
- T053 → T054, T055, T056 → T057. T053 and T054 → T058.
- T008 (licenses) must be done before T064 (building a distributable asset).
- T013 (reviewed) → T014b (freeze splits). T014b must be done before T058, T064, and T065.
- T014a (split code) is part of Phase 2 and doesn't wait for the corpus review.

### Parallel Opportunities

- **Setup**: T003–T007 can run in parallel after T001.
- **Foundational**:
  - T010, T011, T012 (docs and data) run in parallel.
  - T015, T016, T017 (core modules) run in parallel.
  - T020 and T021 (builder modules) run in parallel.
  - T028 runs in parallel with the T024–T027 chain.
- **Within stories**: test tasks marked [P] can be written in parallel with each other. Correctness
  cases across stories all edit `data/correctness/cases.jsonl`, so don't edit it from two stories
  at once.
- **Across stories**: after Foundational, US4 and the US5 tooling (T051–T057) can proceed in
  parallel with US1, US2, and US3.

---

## Parallel Example: Foundational core modules

```bash
Task: "Define public types in crates/predi-core/src/types.rs (T015)"
Task: "Implement normalize.rs with fold/restores_diacritics (T016)"
Task: "Implement tokenize.rs with truncation and NotApplicable detection (T017)"
```

## Parallel Example: User Story 4

```bash
Task: "Write crates/predi-core/tests/lifecycle.rs (T046)"
Task: "Write crates/predi-core/tests/asset_errors.rs (T047)"
Task: "Write docs/privacy-verification.md (T050)"
```

## Parallel Example: User Story 5

```bash
Task: "Report types and writer in crates/predi-tools/src/eval/report.rs (T052)"
Task: "Criterion benchmark in crates/predi-core/benches/predict.rs (T059)"
Task: "Schema test in crates/predi-tools/tests/eval_report.rs (T051)"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Phase 1 (Setup) and Phase 2 (Foundational). Start T013, the corpus writing, early,
   because it's the longest human task.
2. Complete Phase 3 (US1). Word completion then works through `predi-cli`.
3. **Stop and validate** with the correctness and property tests.

### Incremental Delivery

1. **US2** (next words) → the core value is complete.
2. **US3** (orthography) → suggestions are acceptable to insert.
3. **US4** (privacy evidence) and **US5** (evaluation and τ tuning) → release evidence.
4. **US6** (continuations) → optional extra savings.
5. **Polish** → real asset, baseline report, one-time target amendment, docs.

### Constitution Gates (release-blocking)

- Text corruption (SC-007), swallowed input, captured-text leaks (SC-012), or interference with
  typing block the release (Principle IX).
- Performance (SC-009–SC-011) is measured on the reference computer only (Principle V).

---

## Notes

- [P] = different files, no dependency on an incomplete task. [USx] = traceability to a spec user
  story.
- 🧑 tasks need a human decision or review: licenses, corpus content, reference-computer runs, and
  the target amendment.
- Commit after each task or logical group, and keep `fixtures/mini.pta` rebuilt whenever its
  sources change.
- `checklists/engine-review.md` items remain reviewer-owned. `/speckit-implement` reads them but
  doesn't change them.
