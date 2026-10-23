# PrediType

PrediType helps Colombian customer-service agents type Spanish faster. While an agent writes, it
offers up to three suggestions: a completion of the word being typed, the next word, or a short
continuation. The agent decides whether to accept one. Nothing typed is stored, learned from or
sent anywhere.

This repository holds the shared **Spanish prediction engine** (feature
[`001-spanish-prediction-engine`](specs/001-spanish-prediction-engine/spec.md)): a small Rust
library that runs offline on a modest Windows CPU and also compiles to WebAssembly, plus the
developer tools to build its language data and evaluate it. The Windows widget and the Chrome
extension come in later features.

## Workspace layout

| Path | What it is |
|------|------------|
| `crates/predi-core/` | The engine (`Engine::from_bytes`, `predict`, `pause`, `resume`, `field_changed`). No I/O, no logging, `#![forbid(unsafe_code)]`. API: [contracts/engine-api.md](specs/001-spanish-prediction-engine/contracts/engine-api.md) |
| `crates/predi-tools/` | Developer tools: `predi-build` (corpus splits, language asset), `predi-cli` (manual testing), `predi-eval` (evaluation report). [contracts/cli.md](specs/001-spanish-prediction-engine/contracts/cli.md) |
| `data/sources/` | Source manifest and the pinned Tatoeba and RLA-ES snapshots |
| `data/corpus/es-co-cs/` | Team-written Colombian customer-service corpus and its frozen `splits.tsv` |
| `data/correctness/cases.jsonl` | Curated correctness cases |
| `data/lexicon/`, `data/allowlist-proper-nouns.txt`, `data/given-names.txt` | Register flags, domain words, approved capitalized words, personal-data scan list |
| `assets/` | Built language assets (`es-CO-<version>.pta`) and build reports |
| `docs/` | Data sources and licenses, locale profile, reference computer, privacy verification |
| `scripts/` | `fetch-sources.sh`, `build-fixture.sh`, `check-privacy.sh` |
| `reports/` | Evaluation reports (only the baseline is committed) |
| `specs/` | Spec Kit feature specs, plans, contracts and tasks |

## Getting started

Prerequisites: Rust 1.99 (installed automatically from `rust-toolchain.toml`, including the
`wasm32-unknown-unknown` target) and Hunspell (`brew install hunspell` or the Windows equivalent).

```text
# 1. Build and test
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p predi-core --target wasm32-unknown-unknown

# 2. Verify and extract the pinned sources (checks SHA-256 against the manifest)
scripts/fetch-sources.sh

# 3. Freeze the corpus splits (once, after the corpus review)
cargo run --release -p predi-tools --bin predi-build -- split --corpus data/corpus/es-co-cs

# 4. Build the language asset, tuning the confidence cutoffs on the dev split
cargo run --release -p predi-tools --bin predi-build -- build --tune-tau \
  --asset-version 1.1.0 --out assets/es-CO-1.1.0.pta

# 5. Try it
cargo run --release -p predi-tools --bin predi-cli -- --asset assets/es-CO-1.1.0.pta predict "Buenos dí"
cargo run --release -p predi-tools --bin predi-cli -- --asset assets/es-CO-1.1.0.pta repl

# 6. Evaluate (on the reference computer for performance numbers)
cargo run --release -p predi-tools --bin predi-eval -- --asset assets/es-CO-1.1.0.pta \
  --machine docs/reference-computer.toml --out reports/ --check-determinism
```

The full validation guide is
[quickstart.md](specs/001-spanish-prediction-engine/quickstart.md). After changing the fixture
sources or the engine's ranking, rebuild the test asset with `scripts/build-fixture.sh` and update
`crates/predi-core/fixtures/correctness.digest` (the failing test prints the new value).

## How it works

An interpolated trigram model with Stupid Backoff scoring, an accent-folded prefix index, and
precomputed next-word lists, all in one static, versioned binary asset (about 10 MB). A
suggestion is shown only when its share of the candidate score reaches a cutoff tuned on the dev
split. Casing, accents, `ñ`, `¿`/`¡` and the formal *usted* register follow
[docs/locale-profile.md](docs/locale-profile.md). Design decisions are in
[research.md](specs/001-spanish-prediction-engine/research.md).

## Known limitations

- **One locale**: Colombian Spanish with formal *usted*. Other variants are not claimed as
  supported.
- **No offensive-word filter**, by decision: vocabulary is whatever the approved sources contain.
- **Not a spell checker**: only missing accents and `ñ` are tolerated in what the agent types.
- **No client integration yet**: there is no Windows widget, Chrome extension, UI or transport in
  this feature.
- The quality targets (SC-001–SC-004, SC-014, SC-015) are provisional until the first baseline on
  the reference computer.

## Licenses and attribution

The language asset is derived from Tatoeba (CC BY 2.0 FR) and the RLA-ES `es_CO` dictionary (MPL
2.0). [THIRD_PARTY_DATA.md](THIRD_PARTY_DATA.md) must be distributed with every copy of an asset.
Details: [docs/data-sources.md](docs/data-sources.md).
