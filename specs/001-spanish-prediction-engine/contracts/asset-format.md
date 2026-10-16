# Contract: Language Asset Format (`.pta`), format version 1

The asset is a single little-endian binary file, produced only by `predi-build` and read only by
`predi-core`.

| Offset | Size | Field |
|--------|------|-------|
| 0 | 8 | Magic `PREDITYP` (ASCII) |
| 8 | 2 | Format version (`1`) |
| 10 | 2 | Reserved (0) |
| 12 | 16 | Asset version string, semver, NUL-padded (for example `1.0.0`) |
| 28 | 4 | `tau_completion` (f32) |
| 32 | 4 | `tau_next` (f32) |
| 36 | 4 | Backoff α (f32) |
| 40 | 32 | SHA-256 of the source manifest |
| 72 | 32 | SHA-256 of the payload (bytes from offset 128 to end of file) |
| 104 | 24 | Section table: 6 × u32 offsets (lexicon FST, spellings, casing flags, n-gram tables, top-next lists, reserved) |
| 128 | … | Payload sections, each 8-byte aligned |

## Rules

- **Reader**: refuses the asset if the magic number differs, the format version is greater than it
  supports, or the payload hash doesn't match.
- **Contents**: there are no strings other than vocabulary words. Every word passes the
  personal-data filter (research R8).
- **Versioning**: a change to sources, filters, pruning, weighting, or τ bumps the asset version.
  A layout change bumps the format version.
- **Size target**: ≤ 40 MB, and ≤ 70 MB working set once loaded (research R10).
- **Distribution**: the asset is distributed with attribution and license texts for each source
  (`THIRD_PARTY_DATA.md`).
