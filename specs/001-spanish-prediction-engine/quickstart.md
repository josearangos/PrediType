# Quickstart: Validate the Spanish Prediction Engine

This guide checks that the delivered engine meets the spec. Interfaces are defined in
[contracts/](contracts/) and data shapes in [data-model.md](data-model.md).

## Prerequisites

- **Data tools**: Hunspell (for `unmunch`), for example `brew install hunspell` (or the Windows equivalent).
- **Rust toolchain**: stable Rust with edition 2024 support, plus the `wasm32-unknown-unknown`
  target (`rustup target add wasm32-unknown-unknown`).
- **Data**: source data downloaded per `data/sources/manifest.toml`. The hashes are checked
  automatically.
- **Corpus**: the team-written corpus in `data/corpus/es-co-cs/`, with `splits.tsv` (at least 1,500
  messages).
- **Performance machine**: performance checks need the reference computer described in
  `docs/reference-computer.toml` (spec Assumptions: 4-core ≤ 2.5 GHz, 8 GB RAM, integrated graphics,
  SSD, Windows 10/11, mains power, default power plan).
- **Privacy machine**: the Windows privacy check needs Sysinternals Process Monitor.

## 1. Build and unit/property tests

```text
cargo test --workspace
cargo clippy --workspace -- -D warnings      # enforces no fs/net/logging in predi-core
cargo build -p predi-core --target wasm32-unknown-unknown
```

**Expected**: all tests pass, Clippy is clean, and the WASM build succeeds (shareability, Principle
VI).

## 2. Build the language asset

```text
scripts/fetch-sources.sh            # verifies the committed snapshots and extracts them
cargo run --release -p predi-tools --bin predi-build -- build \
  --manifest data/sources/manifest.toml --corpus data/corpus/es-co-cs \
  --allowlist data/allowlist-proper-nouns.txt --asset-version 1.0.0 \
  --out assets/es-CO-1.0.0.pta --tune-tau
```

**Expected**:
- Exit code 0.
- The build report shows the asset is ≤ 40 MB and lists `tau_completion` and `tau_next`.
- The build reports zero test-split leakage.

## 3. Manual smoke test

```text
cargo run --release -p predi-tools --bin predi-cli -- --asset assets/es-CO-1.0.0.pta repl
```

| Type | Expect |
|------|--------|
| `Buenos dí` | `días` present, `replace_chars` = 2 |
| `Su contrase` | `contraseña` |
| `Muchas gracias por ` | 1–3 next words, most likely first |
| `Hola,` | `leading_space: true` |
| `¿En qué puedo ` | `ayudarle` present, `ayudarte` absent |
| `Su pedido fue enviado. ` | all suggestions capitalized |
| `BUE` | all-caps completion |
| `Escríbanos a soporte@emp` | empty, status `NotApplicable` |
| *(empty line)* | up to 3 capitalized openers |

## 4. Full evaluation (reference computer)

```text
cargo run --release -p predi-tools --bin predi-eval -- \
  --asset assets/es-CO-1.0.0.pta --corpus data/corpus/es-co-cs --split test \
  --cases data/correctness/cases.jsonl --machine docs/reference-computer.toml \
  --out reports/ --check-determinism
```

**Expected**:
- Exit code 0, and a report in `reports/` that conforms to
  [eval-report.schema.json](contracts/eval-report.schema.json).
- These hard gates pass:

  | Criterion | Requirement |
  |-----------|-------------|
  | SC-005 | 0 misspellings or personal data |
  | SC-006 | 100% of correctness cases |
  | SC-007 | 100% prefix preservation |
  | SC-008 | Deterministic output |
  | SC-009 | p95 ≤ 100 ms, p99 ≤ 200 ms |
  | SC-010 | ≤ 100 MB |
  | SC-011 | Ready in ≤ 3 s |
  | SC-013 | Field-change results equal a fresh engine |

- SC-001–SC-004, SC-014, and SC-015 are reported with `provisional: true`. On the **first baseline
  run**, record the values. The project owner then amends the spec's quality targets once, within one
  week and before any release (spec, Success Criteria).

## 5. Privacy verification (SC-012)

On the Windows reference computer:

1. **Start capturing**: open Process Monitor and filter on Process Name `predi-eval.exe`, with
   Operation `WriteFile` or any `TCP`/`UDP` event.
2. **Replay**: run step 4 with `--only performance`.
3. **Check the result**: the only `WriteFile` events should target the report under `reports/`.
   There should be no network events.

On Linux CI, the equivalent automated check is `scripts/check-privacy.sh`, which runs the replay
under `strace` and asserts no network syscalls and no writes other than the report.

## 6. Pause and field-change behavior

The integration test `cargo test -p predi-core --test lifecycle` checks three things:
- After `pause()`, every `predict` returns empty with status `Paused`.
- After `resume()`, results return.
- After `field_changed()`, results are identical to a newly constructed engine.
