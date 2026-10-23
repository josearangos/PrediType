# External language sources

- `manifest.toml`: every external source with its URL, license, retrieval date, SHA-256 and role
  (`general` | `spelling` | `domain`). See `docs/data-sources.md` for the license review.
- `snapshots/`: the pinned upstream files, committed to git (7.5 MB). They are the canonical build
  inputs, because upstream exports change over time.
- `raw/` (git-ignored): files extracted from the snapshots by `scripts/fetch-sources.sh`.
