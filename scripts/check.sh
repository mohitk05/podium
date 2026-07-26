#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0

run() {
    local label="$1"
    shift
    printf "%-30s" "$label"
    if "$@" &>/dev/null; then
        echo "OK"
        PASS=$((PASS + 1))
    else
        echo "FAIL"
        FAIL=$((FAIL + 1))
        # Re-run without suppression so the error is visible
        "$@" || true
    fi
}

run "cargo fmt --check"   cargo fmt --all -- --check
run "cargo test"          cargo test --workspace
run "cargo clippy"        cargo clippy --workspace -- -D warnings

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
