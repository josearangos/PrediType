# Correctness test set

`cases.jsonl` holds one curated case per line, following
`specs/001-spanish-prediction-engine/contracts/correctness-case.schema.json` (FR-036).

- `cargo test -p predi-core --test correctness` runs it against the small fixture asset, natively
  and on WASM.
- `predi-eval` runs it against the real asset (SC-006).

Case ids are stable: don't rename or reuse them.
