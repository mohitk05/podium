#!/usr/bin/env bash
# Upload and run Podium flows on Sauce Labs Real Device Cloud (Espresso endpoint).
# Requires: SAUCE_USERNAME and SAUCE_ACCESS_KEY env vars.
# See: https://docs.saucelabs.com/mobile-apps/automated-testing/espresso-xcuitest/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

APP_APK="${APP_APK:-$ROOT/android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk}"
RUNNER_APK="${RUNNER_APK:-$ROOT/android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk}"
SAUCE_REGION="${SAUCE_REGION:-us-west-1}"  # or eu-central-1
DEVICE_NAME="${DEVICE_NAME:-Google Pixel 7 GoogleAPI Emulator}"
PLATFORM_VERSION="${PLATFORM_VERSION:-13}"

: "${SAUCE_USERNAME:?Set SAUCE_USERNAME}"
: "${SAUCE_ACCESS_KEY:?Set SAUCE_ACCESS_KEY}"
AUTH="$SAUCE_USERNAME:$SAUCE_ACCESS_KEY"
API="https://api.$SAUCE_REGION.saucelabs.com"

echo "=== Sauce Labs Upload ==="

# 1. Upload app APK
echo "Uploading app APK..."
APP_ID=$(curl -s -u "$AUTH" \
  -X POST "$API/v1/storage/upload" \
  -F "payload=@$APP_APK" \
  -F "name=sampleapp-debug.apk" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['item']['id'])")
echo "  app id: $APP_ID"

# 2. Upload runner (test suite) APK
echo "Uploading runner APK (test suite)..."
RUNNER_ID=$(curl -s -u "$AUTH" \
  -X POST "$API/v1/storage/upload" \
  -F "payload=@$RUNNER_APK" \
  -F "name=runner-debug-androidTest.apk" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['item']['id'])")
echo "  runner id: $RUNNER_ID"

# 3. Trigger a build
echo "Triggering build..."
BUILD=$(curl -s -u "$AUTH" \
  -X POST "$API/v1/rdc/jobs" \
  -H "Content-Type: application/json" \
  -d "{
    \"kind\": \"espresso\",
    \"app\": \"storage:$APP_ID\",
    \"testApp\": \"storage:$RUNNER_ID\",
    \"devices\": [{\"name\": \"$DEVICE_NAME\", \"platformVersion\": \"$PLATFORM_VERSION\"}],
    \"testOptions\": {\"class\": \"dev.podium.runner.FlowRunner\"}
  }")
echo "$BUILD" | python3 -m json.tool
JOB_ID=$(echo "$BUILD" | python3 -c "import sys,json; print(json.load(sys.stdin)['job_id'])")
echo
echo "Job ID: $JOB_ID"
echo "Track at: https://app.saucelabs.com/tests/$JOB_ID"
echo
echo "To pull results once the job completes:"
echo "  curl -u '$SAUCE_USERNAME:<key>' \\"
echo "    $API/v1/rdc/jobs/$JOB_ID"
