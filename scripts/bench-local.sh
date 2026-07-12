#!/usr/bin/env bash
# Run flows/bench.yaml 5 times, extract timings, print mean/median/p95.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."
RUNNER_APK="$ROOT/android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk"
BENCH_FLOW="$ROOT/flows/bench.yaml"
RUNS=5
OUT_DIR="$ROOT/podium-bench-out"

echo "=== Podium Benchmark ==="
echo "Flow:  $BENCH_FLOW"
echo "Runs:  $RUNS"
echo "Runner: $RUNNER_APK"
echo

if [[ ! -f "$RUNNER_APK" ]]; then
    echo "Runner APK not found. Run scripts/build-runner.sh first."
    exit 1
fi

mkdir -p "$OUT_DIR"

# Collect total wall times (ms) across runs
TOTALS=()

for i in $(seq 1 $RUNS); do
    RUN_OUT="$OUT_DIR/run-$i"
    mkdir -p "$RUN_OUT"
    echo "--- Run $i/$RUNS ---"
    START_MS=$(python3 -c "import time; print(int(time.time()*1000))")
    cargo run --manifest-path "$ROOT/Cargo.toml" -p podium --quiet -- \
        test "$BENCH_FLOW" \
        --runner "$RUNNER_APK" \
        --out "$RUN_OUT" 2>&1 | grep -E "passed|FAILED|All flows|One or more"
    END_MS=$(python3 -c "import time; print(int(time.time()*1000))")
    WALL_MS=$(( END_MS - START_MS ))
    TOTALS+=("$WALL_MS")
    echo "  wall time: ${WALL_MS}ms"
    echo
done

echo "=== Results ==="

# Compute mean
SUM=0
for t in "${TOTALS[@]}"; do SUM=$((SUM + t)); done
MEAN=$(( SUM / RUNS ))

# Sort for median / p95
IFS=$'\n' SORTED=($(sort -n <<<"${TOTALS[*]}")); unset IFS
MEDIAN=${SORTED[$(( RUNS / 2 ))]}
P95_IDX=$(( (RUNS * 95 + 99) / 100 - 1 ))
[[ $P95_IDX -ge $RUNS ]] && P95_IDX=$(( RUNS - 1 ))
P95=${SORTED[$P95_IDX]}

echo "Run wall times (ms): ${TOTALS[*]}"
echo "Mean   : ${MEAN}ms"
echo "Median : ${MEDIAN}ms"
echo "p95    : ${P95}ms"
echo

# Per-command means from last run's result JSON
LAST_JSON="$OUT_DIR/run-$RUNS/results/bench.json"
if [[ -f "$LAST_JSON" ]]; then
    echo "--- Per-step timings (last run) ---"
    python3 - "$LAST_JSON" <<'PYEOF'
import json, sys
data = json.load(open(sys.argv[1]))
steps = data.get("steps", [])
print(f"{'Step':<5} {'Command':<60} {'ms':>6} {'Status'}")
print("-" * 85)
for i, s in enumerate(steps, 1):
    cmd = s["command"][:58]
    ms = s["duration_ms"]
    st = s["status"]
    print(f"{i:<5} {cmd:<60} {ms:>6}ms {st}")
print()
total = sum(s["duration_ms"] for s in steps)
print(f"Total step time: {total}ms  (excludes overhead)")
PYEOF
fi

# Append summary to BENCHMARK.md
BENCH_MD="$ROOT/BENCHMARK.md"
TIMESTAMP=$(date -u '+%Y-%m-%d %H:%M UTC')
cat >> "$BENCH_MD" <<MDEOF

## Run: $TIMESTAMP

**Device:** $(adb shell getprop ro.product.model 2>/dev/null | tr -d '\r' || echo "unknown")
**Android:** $(adb shell getprop ro.build.version.release 2>/dev/null | tr -d '\r' || echo "unknown")
**Flow:** flows/bench.yaml (50 steps)
**Runs:** $RUNS

| Metric | Value |
|--------|-------|
| Mean   | ${MEAN}ms |
| Median | ${MEDIAN}ms |
| p95    | ${P95}ms |
| Run times | $(IFS=', '; echo "${TOTALS[*]}") ms |

MDEOF

echo "Results appended to $BENCH_MD"
