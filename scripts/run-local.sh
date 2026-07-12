#!/usr/bin/env bash
# One-command local demo: build everything and run all sample flows.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

echo "=== Podium local demo ==="
echo

# 1. Check prerequisites
for cmd in adb cargo java; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: '$cmd' not found. See README prerequisites."
        exit 1
    fi
done

if ! adb devices | grep -q "device$"; then
    echo "ERROR: No Android device/emulator connected."
    echo "  Start one: emulator -avd <avd-name> -no-window -no-audio &"
    echo "  Then:      adb wait-for-device"
    exit 1
fi

# 2. Build Rust core + Android APKs
echo "Building..."
bash "$SCRIPT_DIR/build-runner.sh"
echo

# 3. Install sample app
echo "Installing sample app..."
adb install -r "$ROOT/android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk"
echo

# 4. Validate flows
echo "Validating flows..."
cargo run --manifest-path "$ROOT/Cargo.toml" -p podium --quiet -- validate "$ROOT/flows/"
echo

# 5. Run login and smoke flows
RUNNER_APK="$ROOT/android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk"
echo "Running login + smoke flows..."
cargo run --manifest-path "$ROOT/Cargo.toml" -p podium --quiet -- \
    test "$ROOT/flows/login.yaml" \
    --runner "$RUNNER_APK" \
    --out "$ROOT/podium-demo-out"
echo
cargo run --manifest-path "$ROOT/Cargo.toml" -p podium --quiet -- \
    test "$ROOT/flows/smoke.yaml" \
    --runner "$RUNNER_APK" \
    --out "$ROOT/podium-demo-out"
echo

# 6. Print report
echo "Results:"
cargo run --manifest-path "$ROOT/Cargo.toml" -p podium --quiet -- \
    report "$ROOT/podium-demo-out/results/"
