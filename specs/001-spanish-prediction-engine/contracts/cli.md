# Contract: Developer Tools (`predi-tools`)

These are developer and evaluation tools only. They are not shipped to agents and are not the
client protocol, which is chosen in the client features. No tool reads typed text except from the
files or arguments given to it.

## `predi-build`: corpus splits and language asset

`predi-build` has two subcommands.

### `predi-build split`: assign the corpus splits (run once)

```text
predi-build split --corpus data/corpus/es-co-cs
```

- Assigns `test`/`dev`/`build` by SHA-256 rank of each message's NFC-normalized text, and writes
  `splits.tsv`. It never reassigns a hash that is already listed.
- Exit codes: 0 on success, 2 if the minimums aren't met (≥ 500 test, ≥ 250 dev, ≥ 5,000 test word
  positions).

### `predi-build build`: build the language asset

```text
predi-build build --manifest data/sources/manifest.toml \
                  --corpus data/corpus/es-co-cs \
                  --allowlist data/allowlist-proper-nouns.txt \
                  --asset-version 1.0.0 \
                  --out assets/es-CO-1.0.0.pta
                  [--tune-tau]            # tune tau on the dev split (research R6)

predi-build build --plain-sources crates/predi-core/fixtures/mini-sources \
                  --asset-version 0.0.0-fixture \
                  --out crates/predi-core/fixtures/mini.pta   # test fixtures only
```

- **Inputs**: it reads only the `build` split for counts, and the `dev` split for `--tune-tau`. It
  fails if any `test` message hash appears in the build input.
- **Outputs**: it writes the asset plus `assets/es-CO-1.0.0.build-report.txt`, which lists sizes,
  pruning, dropped-token counts, and τ values.
- **Exit codes**: 0 on success, 2 on invalid input, 3 if the test split leaks into build.

## `predi-cli`: manual prediction

```text
predi-cli --asset assets/es-CO-1.0.0.pta predict "Muchas gracias por "
predi-cli --asset assets/es-CO-1.0.0.pta repl
```

- **`predict`**: prints the `PredictionResult` as JSON on stdout.
- **`repl`**: reads one line at a time from stdin and prints the result. It writes nothing to disk
  and keeps no history.

## `predi-eval`: repeatable evaluation

```text
predi-eval --asset assets/es-CO-1.0.0.pta \
           --corpus data/corpus/es-co-cs --split test \
           --cases data/correctness/cases.jsonl \
           --machine docs/reference-computer.toml \
           --out reports/
           [--check-determinism] [--only quality|correctness|performance]
```

- **Output**: it writes `reports/eval-<timestamp>.json`, which conforms to
  [eval-report.schema.json](eval-report.schema.json), and prints a pass/fail table.
- **Exit codes**: 0 if all hard gates pass. 1 if any hard gate fails. Provisional criteria are
  reported but never fail the run.
