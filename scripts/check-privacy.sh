#!/usr/bin/env bash
# Runtime privacy check for Linux CI (T049, SC-012, research R11).
#
# Runs `predi-eval --only performance` (a full keystroke replay) under strace and fails on:
#   - any network syscall;
#   - any file opened for writing, created or renamed outside the report directory.
#
#   scripts/check-privacy.sh                 uses assets/es-CO-1.1.0.pta if present, else the fixture
#   ASSET=path scripts/check-privacy.sh      checks a specific asset
#
# The corpus is split in a temporary copy, so the repository's splits are never touched.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v strace >/dev/null 2>&1 || { echo "check-privacy: strace is required (Linux only)" >&2; exit 2; }

ASSET="${ASSET:-}"
if [[ -z "$ASSET" ]]; then
    if [[ -f assets/es-CO-1.1.0.pta ]]; then
        ASSET=assets/es-CO-1.1.0.pta
    else
        ASSET=crates/predi-core/fixtures/mini.pta
    fi
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/corpus" "$WORK/reports"
cp data/corpus/es-co-cs/messages.tsv "$WORK/corpus/"
if [[ -f data/corpus/es-co-cs/splits.tsv ]]; then
    cp data/corpus/es-co-cs/splits.tsv "$WORK/corpus/"
fi

cargo build --release --quiet -p predi-tools
target/release/predi-build split --corpus "$WORK/corpus" >/dev/null

REPORTS="$(cd "$WORK/reports" && pwd -P)"
LOG="$WORK/strace.log"
strace -f -qq -o "$LOG" \
    -e trace=network,write,openat,open,creat,rename,renameat,renameat2 \
    target/release/predi-eval \
    --asset "$ASSET" \
    --corpus "$WORK/corpus" \
    --machine docs/reference-computer.toml \
    --out "$REPORTS" \
    --only performance >/dev/null || true   # the exit code reflects gates, not privacy

fail=0

# 1. No network syscalls at all.
if grep -E '(^|[[:space:]])(socket|connect|bind|listen|accept4?|sendto|sendmsg|sendmmsg|recvfrom|recvmsg|getsockopt|setsockopt|getpeername|getsockname|socketpair|shutdown)\(' "$LOG"; then
    echo "check-privacy: FAIL: network syscalls above" >&2
    fail=1
fi

# 2. Writable opens, creations and renames only inside the report directory.
while IFS= read -r line; do
    path="$(sed -E 's/.*"([^"]*)".*/\1/' <<<"$line")"
    case "$path" in
        "$REPORTS"/*) ;;
        *) echo "check-privacy: FAIL: write outside reports/: $line" >&2; fail=1 ;;
    esac
done < <(grep -E '(openat|open)\(.*O_(WRONLY|RDWR|CREAT|TRUNC|APPEND)' "$LOG" | grep -v ' = -1 ' || true)

if grep -E '(^|[[:space:]])(creat|rename|renameat|renameat2)\(' "$LOG" | grep -v "$REPORTS"; then
    echo "check-privacy: FAIL: file creation or rename outside reports/" >&2
    fail=1
fi

# The run must actually have produced its report.
ls "$REPORTS"/eval-*.json >/dev/null 2>&1 || { echo "check-privacy: FAIL: no report written" >&2; fail=1; }

if [[ $fail -ne 0 ]]; then
    exit 1
fi
echo "check-privacy: PASS ($(grep -c . "$LOG") syscalls traced; asset $ASSET)"
