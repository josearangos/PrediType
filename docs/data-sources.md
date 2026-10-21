# Data Sources and Licenses

Record for task T008: license verification for the bundled language data (spec FR-028, research R3).

**Verified**: 2026-10-16, from the publishers' own pages and the license files inside the
downloaded packages.

> This is a good-faith engineering review, not legal advice. Have someone qualified approve it
> before the first public distribution of an asset.

## Summary

| Source | Role | License chosen | Redistribution in the product | Status |
|--------|------|----------------|-------------------------------|--------|
| Tatoeba Spanish sentences | `general`: n-gram counts | CC BY 2.0 FR | Allowed with attribution and an indication of changes | ✅ Approved for use |
| RLA-ES `es_CO` Hunspell dictionary v2.9 | `spelling`: canonical spellings and validation | MPL 2.0 (via "MPL 1.1 or later") | Allowed. The derived word list stays under MPL, with a notice and a link to the source | ✅ Approved for use |
| Team-written Colombian customer-service set | `domain` | Project-owned | N/A | Drafted, pending human review (T013) |

## 1. Tatoeba: Spanish sentences

- **Publisher**: Tatoeba Association, <https://tatoeba.org>
- **Download page**: <https://tatoeba.org/en/downloads>
- **File**: <https://downloads.tatoeba.org/exports/per_language/spa/spa_sentences.tsv.bz2>
  - **Format**: TSV `id<TAB>lang<TAB>text`, 442,544 sentences.
  - **Snapshot retrieved**: 2026-10-16. Upstream last-modified 2026-10-10.
  - **SHA-256**: `d1f7aa997368571e8bbba341ab60f9145fefe14970f21065bd97be8e03c8e6fb`
- **License**: the downloads page states "These files are released under CC BY 2.0 FR"
  (<https://creativecommons.org/licenses/by/2.0/fr/>).
  - A CC0 subset exists, but for Spanish it's only about 2 KB, so it isn't useful.
- **Obligations**:
  - Credit Tatoeba and link the license.
  - State that the data was transformed: only aggregated n-gram statistics are bundled, not
    sentences.
- **Note on content**: Tatoeba Spanish is multi-regional and uses *tú* heavily. The register filter
  (FR-022) and the domain weighting handle this. Some sentences contain fictional names such as
  "Tom", which the personal-data filter (research R8) removes.
- **Reproducibility**: Tatoeba regenerates its exports weekly, so this URL won't keep serving the
  same file.
  - Keep the snapshot above as the canonical input. Commit it to git (it is small) and pin
    its SHA-256 in `data/sources/manifest.toml` (T009).
  - `scripts/fetch-sources.sh` must verify that hash instead of trusting the live URL.

## 2. RLA-ES `es_CO` Hunspell dictionary

- **Publisher**: RLA-ES project (coordinator Santiago Bosio), <https://github.com/sbosio/rla-es>
- **File**: <https://github.com/sbosio/rla-es/releases/download/v2.9/es_CO.oxt>, release v2.9 (asset
  dated 2025-01-01).
  - **SHA-256**: `d47ff4cf5cab98577d9a1913e226e31bc8ade9334c15fd389b97ddc0341a2901`
  - **Contents used**: `es_CO.aff` and `es_CO.dic`. The `.oxt` file is a ZIP archive. Expand it
    with Hunspell `unmunch` into a plain word list.
- **License** (from `LICENSE.md` in the package): "El proyecto y los diccionarios se distribuyen
  bajo un triple esquema de licencias disjuntas: GNU GPL versión 3 o posterior, GNU LGPL versión 3
  o posterior o MPL versión 1.1 o posterior. Puede seleccionar libremente bajo cuál de estas
  licencias realizará el uso."
  - **Chosen**: MPL 2.0, permitted by "MPL 1.1 o posterior". It's file-level copyleft, which is
    compatible with shipping alongside proprietary or other-licensed code.
- **Obligations**:
  - Keep the MPL notice for the derived word-list data.
  - Tell recipients where the source form is available (the upstream release URL above).
  - Don't mix the thesaurus, which is LGPL 2.1. We don't use it.

## 3. Team-written Colombian customer-service set

- **Location**: `data/corpus/es-co-cs/messages.tsv`. The guidelines are in
  `data/corpus/es-co-cs/README.md`.
- **Ownership**: project-owned original text. It contains no real customer data, and placeholders
  replace personal data.
- **Status**: the first draft was prepared with AI assistance on 2026-10-16. It **must be reviewed
  by a person** for natural Colombian phrasing and absence of personal data before
  `predi-build split` freezes the splits (T013, T014).

## 4. Project-written lists

These are written by the PrediType team and are project-owned. They are build inputs, and only
their effect (flags, casing, exclusions) reaches the asset.

| File | Purpose |
|------|---------|
| `data/allowlist-proper-nouns.txt` | Places, institutions, brands and acronyms that keep their capitalization (FR-014, FR-019). No person names. |
| `data/given-names.txt` | Common given names for the build-time personal-data scan (FR-028). Never added to the asset. |
| `data/lexicon/register-flags.txt` | *Tú*-form and other-variant words (FR-022) |
| `data/lexicon/domain-words.txt` | Correctly spelled customer-service words missing from the `es_CO` dictionary, mostly *usted* imperatives with enclitic pronouns (`permítame`, `infórmenos`) |

## How the sources are used in a build

1. `scripts/fetch-sources.sh` verifies each snapshot's SHA-256 against
   `data/sources/manifest.toml`, decompresses the Tatoeba TSV, and expands the dictionary with
   Hunspell `unmunch`. `unmunch` misses some affix combinations (for example the plural
   `disponibles`), so the script also adds every word of the Tatoeba and corpus text that Hunspell
   accepts with the same `es_CO` dictionary. The resulting `data/sources/raw/es_CO.words` is the
   spelling reference.
2. `predi-build build` reads the Tatoeba sentences, and only the `build` split of the team corpus;
   the `dev` split is used only for τ tuning, and the `test` split never. Tatoeba sentences
   identical to a test message are excluded, and any test message in the build input fails the
   build (exit code 3).
3. Placeholders, numbers, emails and URLs never become words. Words seen capitalized mid-sentence
   but never lowercase are dropped as likely names unless allowlisted. Words not in the spelling
   reference or the domain list are dropped. The finished vocabulary is scanned for personal data,
   and any match fails the build.
4. Only words, their flags, and pruned, quantized n-gram statistics are stored. No sentence is
   stored. The build report (`assets/*.build-report.txt`) records sizes, pruning, τ values and
   dropped-token counts.

## Shipping obligations

Every distribution of an asset (and of a product that bundles one) must include
`THIRD_PARTY_DATA.md`, which carries:

- for Tatoeba: the credit, the CC BY 2.0 FR license link, and the statement of changes;
- for RLA-ES: the MPL 2.0 notice, the license link, and where the source form is available.

## Rejected sources

| Source | Reason |
|--------|--------|
| Spanish Wikipedia | Encyclopedic register, and CC BY-SA share-alike on derived data |
| OpenSubtitles | Unclear redistribution rights |
| Europarl | Formal parliamentary register, and Spain-centric |
| RLA-ES thesaurus | Not needed, and under a different license (LGPL 2.1) |
