#!/usr/bin/env bash
# Upload and run Podium flows on BrowserStack App Automate (Espresso endpoint).
# Requires: BROWSERSTACK_USERNAME and BROWSERSTACK_ACCESS_KEY env vars.
# See: https://www.browserstack.com/docs/app-automate/espresso/getting-started
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

APP_APK="${APP_APK:-$ROOT/android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk}"
RUNNER_APK="${RUNNER_APK:-$ROOT/android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk}"
DEVICES="${DEVICES:-Google Pixel 7-13.0}"  # comma-separated list

: "${BROWSERSTACK_USERNAME:?Set BROWSERSTACK_USERNAME}"
: "${BROWSERSTACK_ACCESS_KEY:?Set BROWSERSTACK_ACCESS_KEY}"
AUTH="$BROWSERSTACK_USERNAME:$BROWSERSTACK_ACCESS_KEY"

echo "=== BrowserStack Upload ==="

# 1. Upload app APK
echo "Uploading app APK..."
APP_URL=$(curl -s -u "$AUTH" \
  -X POST "https://api-cloud.browserstack.com/app-automate/upload" \
  -F "file=@$APP_APK" | python3 -c "import sys,json; print(json.load(sys.stdin)['app_url'])")
echo "  app_url: $APP_URL"

# 2. Upload runner (test suite) APK
# The runner APK with baked-in flows IS the Espresso test suite.
# It must contain the arm64 .so (arm64-v8a ABI) for real devices.
echo "Uploading runner APK (test suite)..."
TEST_URL=$(curl -s -u "$AUTH" \
  -X POST "https://api-cloud.browserstack.com/app-automate/espresso/test-suite" \
  -F "file=@$RUNNER_APK" | python3 -c "import sys,json; print(json.load(sys.stdin)['test_suite_url'])")
echo "  test_suite_url: $TEST_URL"

# 3. Trigger a build
echo "Triggering build..."
BUILD=$(curl -s -u "$AUTH" \
  -X POST "https://api-cloud.browserstack.com/app-automate/espresso/v2/build" \
  -H "Content-Type: application/json" \
  -d "{
    \"app\": \"$APP_URL\",
    \"testSuite\": \"$TEST_URL\",
    \"devices\": [\"$DEVICES\"],
    \"class\": [\"dev.podium.runner.FlowRunner\"],
    \"logs\": true,
    \"video\": true,
    \"networkLogs\": false
  }")
echo "$BUILD" | python3 -m json.tool
BUILD_ID=$(echo "$BUILD" | python3 -c "import sys,json; print(json.load(sys.stdin)['build_id'])")
echo
echo "Build ID: $BUILD_ID"
echo "Track at: https://app-automate.browserstack.com/builds/$BUILD_ID"
echo
echo "To pull timing results once the build completes:"
echo "  curl -u '$BROWSERSTACK_USERNAME:<key>' \\"
echo "    https://api-cloud.browserstack.com/app-automate/espresso/v2/builds/$BUILD_ID"
