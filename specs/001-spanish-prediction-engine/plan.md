# Implementation Plan: Spanish Prediction Engine

**Branch**: `001-spanish-prediction-engine` (spec directory; the git branch is still `main`) |
**Date**: 2026-10-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-spanish-prediction-engine/spec.md`

## Summary

Build PrediType's shared, offline Spanish prediction engine as a Rust library (`predi-core`). Given
the text before the caret, it returns up to 3 ranked word completions, next words, or short
continuations, using Colombian Spanish and the formal *usted* register.

**Approach**: the baseline is an interpolated trigram model with Stupid Backoff scoring, plus an
accent-folded prefix index, precomputed into one static, versioned binary asset.
- **Data**: the asset is built from open-licensed Spanish (Tatoeba), the `es_CO` Hunspell
  dictionary, and a team-written Colombian customer-service corpus.
- **Confidence cutoff**: tuned on a dev split so weak guesses are suppressed.
- **No retained text**: the engine is stateless with respect to text, which satisfies the privacy
  and context-lifecycle rules by construction.
- **Tools**: a separate tools crate builds the asset, provides a manual CLI, and runs the repeatable
  evaluation that produces evidence for every success criterion.

## Technical Context

**Language/Version**: Rust stable, edition 2024 (local toolchain 1.99.1)

**Primary Dependencies**:
- `predi-core`: `fst` (prefix/lexicon maps), `unicode-normalization`. Nothing else at runtime.
- `predi-tools`: `clap`, `serde`, `serde_json`, `toml`, `sha2`, `memory-stats`.
- Development: `proptest`, `criterion`.

**Storage**: No database. One static, read-only asset file (`assets/es-CO-<version>.pta`) loaded as
bytes. No user text is ever written anywhere.

**Testing**:
- `cargo test`: unit tests, integration tests (correctness set, lifecycle) and `proptest` property
  tests.
- `criterion` micro-benchmarks.
- `predi-eval` end-to-end evaluation.
- Process Monitor (Windows) or `strace` (CI) privacy checks.

**Target Platform**:
- Windows 10/11 x86-64, native, CPU only.
- The core must also compile to `wasm32-unknown-unknown` for the future Chrome client.

**Project Type**: Library, plus developer command-line tools (Cargo workspace)

**Performance Goals**: p95 ≤ 100 ms and p99 ≤ 200 ms per request on the reference computer. The
internal design target is p95 ≤ 10 ms. Ready in ≤ 3 s.

**Constraints**:
- Memory: ≤ 100 MB working set (design target ≤ 70 MB); asset ≤ 40 MB.
- Offline: no network, no filesystem access in the core, no logging of text.
- Deterministic output; input capped at 1,000 characters.

**Scale/Scope**:
- One concurrent active field.
- Vocabulary of about 50–100 k words.
- Evaluation corpus of at least 1,500 messages (at least 500 test).
- One locale (es-CO, *usted*).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| # | Principle | How this plan complies | Pre | Post |
|---|-----------|------------------------|-----|------|
| I | Prediction only; user in control | The API only returns suggestions, never inserts. Each suggestion carries an exact replacement span, and application stays with the client (engine API G3, client obligations). | ✅ | ✅ |
| II | Spanish-first | es-CO lexicon with accents and `ñ`, casing rules (R4, R5), Colombian test corpus, `usted` locale. No claim of other languages. | ✅ | ✅ |
| III | Local-first, offline | Local asset only. No network dependency, and Clippy bans `std::net` in the core (R11). No RAG, LLM, or cloud. | ✅ | ✅ |
| IV | No database, no persistent text | A static asset is not a database (R10). The engine holds no text (R9). No logging. Assets contain no conversations (R8). | ✅ | ✅ |
| V | Lightweight performance | Precomputed lookups. Measured p95, p99, memory, and startup on the agreed reference laptop (R11). Requests run off the typing path (G6, client obligations). | ✅ | ✅ |
| VI | Two clients, clear boundaries | One shared core that compiles native and WASM. The client protocol and UI are deferred to client features. | ✅ | ✅ |
| VII | Safe input handling | `pause` stops suggestions. The engine captures nothing. The not-applicable rules cover non-word tokens. Sensitive-field detection is a client responsibility (spec, out of scope). | ✅ | ✅ |
| VIII | Predictable, non-intrusive | The confidence cutoff suppresses weak suggestions (R6). Results are deterministic, and the replacement span is explicit. | ✅ | ✅ |
| IX | Testability and evidence | Correctness set, property tests, repeatable `predi-eval` report with hardware and versions. Provisional quality targets are clearly flagged. | ✅ | ✅ |
| X | Spec-driven, simple | Two crates, minimal dependencies, no services. The spec was updated through clarify. | ✅ | ✅ |

**Gate result**: PASS. There are no violations, so Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-spanish-prediction-engine/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R12
├── data-model.md        # Phase 1: runtime and static entities
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   ├── engine-api.md                  # predi-core public API v1 + guarantees
│   ├── asset-format.md                # .pta binary layout v1
│   ├── cli.md                         # predi-build / predi-cli / predi-eval
│   ├── correctness-case.schema.json   # curated test case format
│   └── eval-report.schema.json        # evaluation report format
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks; not created here)
```

### Source Code (repository root)

```text
Cargo.toml                      # workspace
clippy.toml                     # disallowed fs/net/process/log in predi-core
deny.toml                       # dependency/license audit

crates/
├── predi-core/                 # the engine library (no I/O)
│   ├── src/
│   │   ├── lib.rs              # Engine, public types (contracts/engine-api.md)
│   │   ├── normalize.rs        # NFC, folding (R4)
│   │   ├── tokenize.rs         # word boundaries, not-applicable detection (R4)
│   │   ├── casing.rs           # output casing rules (R5)
│   │   ├── asset.rs            # .pta reader + validation (R10)
│   │   ├── model.rs            # backoff scoring, candidate generation (R2)
│   │   ├── rank.rs             # confidence, threshold, dedupe, continuation (R6, R7)
│   │   └── state.rs            # pause/resume flag (R9)
│   ├── tests/
│   │   ├── correctness.rs      # runs data/correctness/cases.jsonl on fixture asset
│   │   ├── lifecycle.rs        # pause/resume/field_changed
│   │   └── properties.rs       # proptest: no panic, prefix preservation, determinism
│   ├── benches/predict.rs      # criterion
│   └── fixtures/mini.pta       # tiny asset for tests (built by predi-build)
└── predi-tools/                # developer tools (not shipped to agents)
    └── src/
        ├── bin/predi-build.rs
        ├── bin/predi-cli.rs
        ├── bin/predi-eval.rs
        ├── build/              # source readers, PII filter (R8), counting, pruning, tau tuning
        └── eval/               # replay, metrics, memory/latency, report writer

data/
├── sources/manifest.toml       # external sources, licenses, hashes (downloads git-ignored)
├── corpus/es-co-cs/            # team-written messages + splits.tsv
├── correctness/cases.jsonl
└── allowlist-proper-nouns.txt

assets/                         # built .pta + build reports (versioned releases)
docs/
├── locale-profile.md           # es-CO, usted (FR-022)
├── reference-computer.toml     # agreed hardware (spec Assumptions)
└── data-sources.md             # attribution + licenses (FR-028)
scripts/check-privacy.sh        # strace-based CI privacy check (R11)
reports/                        # evaluation outputs (git-ignored except baseline)
```

**Structure Decision**: A Cargo workspace with two crates.
- `predi-core` holds everything that will ship inside both clients. It has no I/O and is
  WASM-compatible.
- `predi-tools` holds build, manual-test, and evaluation tooling.

Data, docs, and assets live at the repository root so future client features can reuse them. Client
code (Windows, Chrome) will be added later as sibling directories (for example `clients/windows/`,
`clients/chrome/`) in their own features.

## Phase Outputs

- **Phase 0**: [research.md](research.md). Every Technical Context item is resolved, with no
  remaining NEEDS CLARIFICATION.
- **Phase 1**: [data-model.md](data-model.md), [contracts/](contracts/), and
  [quickstart.md](quickstart.md).
- **Post-design Constitution Check**: PASS (table above).

## Risks and Follow-ups

| Risk | Mitigation |
|------|------------|
| Writing at least 1,500 quality Colombian messages takes effort | Start with the category coverage list (data-model) and track progress. The test split is filled first so evaluation can begin early. |
| Source licenses differ from what research assumes | The first task verifies Tatoeba and RLA-ES licenses before any asset ships (R3). |
| Vulgar words from general text surface, because no blocklist is used by decision | They're expected to be rare given the domain weighting and cutoff. If evaluation shows them, a filter is proposed through a spec amendment. |
| Provisional quality targets are missed by the baseline | By design, values are recorded and the targets are amended once with evidence (spec, Success Criteria). |
| The team's Rust experience | Two crates, a small API, and extensive tests and property checks. |

## Complexity Tracking

No constitution violations; nothing to justify.
