#!/usr/bin/env bash
set -euo pipefail

# Build the Rust core for Android targets
echo "Building Rust core for Android..."
cargo ndk \
  -t arm64-v8a \
  -t x86_64 \
  -o android/runner/src/main/jniLibs \
  build --release -p podium-core

# Generate Kotlin bindings
echo "Generating Kotlin bindings..."
cargo run -p podium-core --bin uniffi-bindgen -- generate \
  --library target/aarch64-linux-android/release/libpodium_core.so \
  --language kotlin \
  --out-dir android/runner/src/main/kotlin

# Build Android APKs
echo "Building Android APKs..."
cd android
./gradlew :runner:assembleAndroidTest :sampleapp:assembleDebug

echo "Build complete!"
echo "Runner APK: android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk"
echo "Sample APK: android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk"
