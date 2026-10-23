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
| 104 | 24 | Section table: 6 × u32 offsets (lexicon FST, spellings, word flags, n-gram tables, top-next lists, reserved) |
| 128 | … | Payload sections, each 8-byte aligned |

## Word flags

One byte per word id:

| Bits | Meaning |
|------|---------|
| 0–1 | Casing: `0` Lower, `1` Capitalized, `2` Acronym (`3` invalid) |
| 2 | `register_other`: a *tú*-form or other-variant word, filtered per FR-022 |
| 3–7 | Reserved. Must be 0; the reader rejects the asset otherwise (`Malformed`) |

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

## Payload sections (format version 1)

Integers are little-endian. Every section starts 8-byte aligned, and padding bytes are 0. A
section ends where the next non-zero section offset begins (the last one at end of file).

**Ids**: word ids are `0 … n_words − 1`, assigned in (folded key, canonical spelling) order, so ids
sharing a folded key are contiguous. Id `n_words` is the sentence-start marker `<s>` and
`n_words + 1` the sentence-end marker `</s>`. N-gram keys pack ids into 21 bits each
(`pack2(a, b) = a << 21 | b`, `pack3(a, b, c) = a << 42 | b << 21 | c`).

**Scores**: probabilities are stored as `q = round(−log2(p) · 1024)` in a `u16`. The reader
dequantizes with the stored table: `p = frac[q mod 1024] · 2^−(q div 1024)`. The power of two is
built from its bit pattern, so runtime scoring needs no platform math functions (engine API G4).

| Section | Contents |
|---------|----------|
| 0 Lexicon FST | `u64` FST length, then an `fst` map: folded key (lowercase, no diacritics) → `first_id << 8 \| count` |
| 1 Spellings | `u32 n_words`; `u32 offsets[n_words + 1]` (first 0, strictly increasing, char boundaries); the UTF-8 spellings |
| 2 Word flags | `u8 flags[n_words]` (see Word flags) |
| 3 N-gram tables | `u32 n_unigram` (= `n_words + 2`), `u32 n_bigram`, `u32 n_trigram`, `u32 0`; `f64 frac[1024]`; `u16 unigram[n_unigram]`; pad; `u64 bigram_keys[n_bigram]` (strictly increasing); `u16 bigram_q[n_bigram]`; pad; `u64 trigram_keys[n_trigram]` (strictly increasing); `u16 trigram_q[n_trigram]`; pad |
| 4 Top-next lists | `u32 pool_len`, `u32 global_len`, `u32 n_bictx`, `u32 prefix_fst_len`; `u32 pool[pool_len]` (word ids; the global list is `pool[0 … global_len]`); `u32 uni_offsets[n_unigram + 1]` (list after one-id context `a` is `pool[uni_offsets[a] … uni_offsets[a + 1]]`); pad; `u64 bictx_keys[n_bictx]` (strictly increasing `pack2` contexts); `u32 bictx_offsets[n_bictx + 1]`; pad; the prefix FST (`prefix_fst_len` bytes): folded prefix of 1–3 characters → `pool_offset << 8 \| len`; pad |
| 5 Reserved | Offset must be 0 |

- **Conditional probabilities**: a bigram stores `P(b | a)`, a trigram `P(c | a b)`, each divided by
  the context's total count before pruning. Unigrams store `P(w)`; `<s>` stores `q = 65535`.
- **Lists**: each list holds at most 32 ids, ranked by probability then spelling. Lists never
  contain `<s>`, `</s>` or `register_other` words.
- **Reader checks** (all give `Malformed` unless noted): table sizes, offsets and lengths within the
  section (`Truncated`), sorted keys, ids in range, valid UTF-8 spellings, reserved flag bits,
  τ ∈ [0, 1], α ∈ (0, 1], and each FST's own checksum.
