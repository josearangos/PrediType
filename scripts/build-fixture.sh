#!/usr/bin/env bash
# Rebuild the small test asset crates/predi-core/fixtures/mini.pta from its plain sources (T026).
# Run from anywhere; commit the result so predi-core tests don't depend on predi-tools.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo run --quiet --release -p predi-tools --bin predi-build -- build \
    --plain-sources crates/predi-core/fixtures/mini-sources \
    --asset-version 0.0.0-fixture \
    --min-bigram 1 --min-trigram 1 \
    --out crates/predi-core/fixtures/mini.pta
