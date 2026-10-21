#!/usr/bin/env bash
# Verify the pinned source snapshots against data/sources/manifest.toml and extract them into
# data/sources/raw/ (task T009, research R3).
#
#   scripts/fetch-sources.sh            verify hashes, then extract
#   scripts/fetch-sources.sh --refresh  download fresh copies from upstream, print their hashes
#
# A refresh changes the canonical inputs: update the manifest and bump the asset version.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/data/sources/manifest.toml"
SNAPSHOTS="$ROOT/data/sources/snapshots"
RAW="$ROOT/data/sources/raw"

die() { echo "fetch-sources: $*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is not installed. $2"
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

# Print "field" of the [[source]] entry named $1 (simple, flat TOML only).
manifest_field() {
    awk -v want="$1" -v field="$2" '
        /^\[\[source\]\]/ { name = "" }
        /^name *=/ { gsub(/^name *= *"|"$/, ""); name = $0 }
        $0 ~ "^" field " *=" && name == want {
            sub("^" field " *= *\"", ""); sub("\"$", ""); print; exit
        }
    ' "$MANIFEST"
}

REFRESH=0
if [[ "${1:-}" == "--refresh" ]]; then
    REFRESH=1
elif [[ $# -gt 0 ]]; then
    die "unknown argument '$1' (expected --refresh or nothing)"
fi

need hunspell "Install Hunspell (for example 'brew install hunspell' or 'apt-get install hunspell')."
need unmunch "Install Hunspell (for example 'brew install hunspell' or 'apt-get install hunspell-tools')."
need bunzip2 "Install bzip2."
need unzip "Install unzip."

mkdir -p "$SNAPSHOTS" "$RAW"

if [[ $REFRESH -eq 1 ]]; then
    need curl "Install curl."
    for src in tatoeba-spa rla-es-es_CO; do
        url="$(manifest_field "$src" url)"
        snap="$ROOT/$(manifest_field "$src" snapshot)"
        echo "Downloading $src from $url"
        curl -fL --retry 3 -o "$snap" "$url"
        echo "  new sha256: $(sha256_of "$snap")"
    done
    echo
    echo "Update the sha256 values in data/sources/manifest.toml and docs/data-sources.md,"
    echo "then bump the asset version before building."
    exit 0
fi

for src in tatoeba-spa rla-es-es_CO; do
    snap="$ROOT/$(manifest_field "$src" snapshot)"
    want="$(manifest_field "$src" sha256)"
    [[ -f "$snap" ]] || die "snapshot missing: $snap"
    got="$(sha256_of "$snap")"
    [[ "$got" == "$want" ]] || die "hash mismatch for $src: expected $want, got $got"
    echo "ok  $src  $got"
done

# Tatoeba: decompress the per-language TSV (id<TAB>lang<TAB>text).
bunzip2 -kc "$SNAPSHOTS/spa_sentences.tsv.bz2" > "$RAW/spa_sentences.tsv"
echo "extracted $(wc -l < "$RAW/spa_sentences.tsv" | tr -d ' ') Tatoeba sentences"

# RLA-ES: unzip the .oxt (a ZIP) and expand the Hunspell dictionary into a plain word list.
unzip -o -q -j "$SNAPSHOTS/es_CO.oxt" es_CO.aff es_CO.dic -d "$RAW"
# unmunch leaves some continuation flags ("word/S|"); strip them and de-duplicate.
unmunch "$RAW/es_CO.dic" "$RAW/es_CO.aff" 2>/dev/null \
    | sed 's#/.*##' \
    | LC_ALL=C sort -u \
    > "$RAW/es_CO.words"
echo "expanded $(wc -l < "$RAW/es_CO.words" | tr -d ' ') dictionary words"

# unmunch misses some affix combinations (for example plurals such as "disponibles"). Add every
# word of the sources that Hunspell itself accepts with the same dictionary.
CORPUS="$ROOT/data/corpus/es-co-cs/messages.tsv"
cut -f3 "$RAW/spa_sentences.tsv" | cat - <(cut -f2 "$CORPUS") \
    | perl -CSD -ne 'for (/(\p{L}+)/g) { print "$_\n"; print lc($_), "\n" }' \
    | LC_ALL=C sort -u \
    | hunspell -d "$RAW/es_CO" -G 2>/dev/null \
    | cat - "$RAW/es_CO.words" \
    | LC_ALL=C sort -u > "$RAW/es_CO.words.tmp"
mv "$RAW/es_CO.words.tmp" "$RAW/es_CO.words"
echo "with Hunspell-accepted source words: $(wc -l < "$RAW/es_CO.words" | tr -d ' ') words"
