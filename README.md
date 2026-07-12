# Podium

On-device Maestro-flow interpreter embedded inside an Android instrumentation APK.

## Problem

Appium-based cloud testing is slow because the test interpreter runs on the client machine and every command — find element, tap, assert — is a separate HTTP round-trip over the WAN to the device farm. Maestro is fast because its decision loop runs adjacent to the device, but Maestro can't run natively on most cloud farms.

**Thesis:** If the flow interpreter is embedded *inside* an Android instrumentation APK (the artifact type every cloud farm — BrowserStack, Sauce Labs, Firebase Test Lab, AWS Device Farm — accepts via their native Espresso endpoints), then execution is Maestro-speed or faster, because element matching and action dispatch happen in-process on the device with zero network hops, and the same artifact runs locally and on any cloud farm.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  flows/*.yaml  ──parse──►  podium validate (Rust, host)             │
│                                                                     │
│  podium test ──────────────────────────────────────────────────────►│
│       │  adb install + push flows                                   │
│       │  adb shell am instrument                                    │
│       │  stream PODIUM| lines from logcat                           │
│       ▼                                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Instrumentation APK (on-device)                            │   │
│  │                                                             │   │
│  │  FlowRunner.kt ──► parse_flow() ──► run_flow()              │   │
│  │                           ▲               │                 │   │
│  │                           │         ┌─────┴──────────────┐  │   │
│  │                    podium-core.so    │  engine.rs (Rust)  │  │   │
│  │                    (UniFFI)          │  wait / retry loop │  │   │
│  │                                     │  scroll / timeout  │  │   │
│  │                                     └────────┬───────────┘  │   │
│  │                                              │ Driver trait  │   │
│  │                                     ┌────────▼───────────┐  │   │
│  │                                     │ UiAutomatorDriver  │  │   │
│  │                                     │   (Kotlin)         │  │   │
│  │                                     │   UIAutomator2     │  │   │
│  │                                     └────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

**Key design invariant:** All timing/retry semantics live in Rust (`engine.rs`). The Kotlin driver is a thin adapter — no polling, no sleeps, no retry logic. This makes the engine unit-testable against a `MockDriver` on the host, and makes a future iOS port a matter of writing a Swift `Driver` over XCUITest.

## Quick start (pre-built binary)

The release binary embeds the runner and sample APKs — the only prerequisite is `adb`.

### 1. Install `adb`

**macOS:**
```bash
brew install --cask android-platform-tools
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install -y adb
```

Verify: `adb version` returns a value.

### 2. Download `podium`

```bash
# macOS (Apple Silicon)
curl -Lo podium https://github.com/mohitk05/podium/releases/latest/download/podium-macos-aarch64

# macOS (Intel)
curl -Lo podium https://github.com/mohitk05/podium/releases/latest/download/podium-macos-x86_64

# Linux (x86-64)
curl -Lo podium https://github.com/mohitk05/podium/releases/latest/download/podium-linux-x86_64
```

```bash
chmod +x podium && sudo mv podium /usr/local/bin/
```

### 3. Connect a device or start an emulator

Plug in an Android device with USB debugging enabled, or start an emulator:

```bash
emulator -avd <avd-name> -no-window -no-audio &
adb wait-for-device
```

### 4. Run the flows

```bash
podium test flows/login.yaml
podium test flows/smoke.yaml
```

`podium test` installs the runner and sample APKs automatically from the embedded copies on first run. No APK files to download or manage.

---

## Building from source

Required: Rust, JDK 17, Android SDK (platform-tools + NDK 27), `cargo-ndk`.

### 1. Rust + Android targets

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
```

### 2. Java (JDK 17+)

**macOS:** `brew install --cask temurin@17`

**Linux:** `sudo apt-get install -y openjdk-17-jdk`

### 3. Android SDK + NDK

Install [Android Studio](https://developer.android.com/studio) or the command-line tools, then:

```bash
sdkmanager "platform-tools" "platforms;android-35" "ndk;27.2.12479018"
```

Set `ANDROID_HOME` (add to `~/.zshrc` or `~/.bashrc`):

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"   # macOS default
export PATH="$ANDROID_HOME/platform-tools:$PATH"
```

### 4. Clone, build, run

```bash
git clone https://github.com/mohitk05/podium.git
cd podium

# Build Rust core → Kotlin bindings → Android APKs
bash scripts/build-runner.sh

# Install the CLI
cargo install --path cli

# Start an emulator (or plug in a device)
emulator -avd podium -no-window -no-audio &
adb wait-for-device

# Run the flows
podium test flows/login.yaml
podium test flows/smoke.yaml

# Or run everything at once
bash scripts/run-local.sh
```

## Running on cloud device farms

Podium's runner APK is a standard Espresso instrumentation APK. Every cloud farm that accepts Espresso APKs via their native endpoint works without any Podium-specific plugin.

### BrowserStack App Automate

Requires: `BROWSERSTACK_USERNAME` and `BROWSERSTACK_ACCESS_KEY` (find them under [Account → Settings](https://www.browserstack.com/accounts/settings)).

```bash
export BROWSERSTACK_USERNAME=your_username
export BROWSERSTACK_ACCESS_KEY=your_access_key

# Optional overrides — defaults point to the locally-built APKs
# export APP_APK=path/to/sampleapp-debug.apk
# export RUNNER_APK=path/to/runner-debug-androidTest.apk
# export DEVICES="Samsung Galaxy S23-13.0"   # default: Google Pixel 7-13.0

bash scripts/upload-browserstack.sh
```

The script:
1. Uploads the app APK and gets an `app_url`.
2. Uploads the runner APK as the Espresso test suite and gets a `test_suite_url`.
3. Triggers a build targeting the specified device(s) with `class: dev.podium.runner.FlowRunner`.
4. Prints a build ID and a direct link to the App Automate dashboard.

To run against multiple devices in parallel, set `DEVICES` to a comma-separated list:

```bash
DEVICES="Google Pixel 7-13.0,Samsung Galaxy S23-13.0" bash scripts/upload-browserstack.sh
```

### Sauce Labs

Requires: `SAUCE_USERNAME` and `SAUCE_ACCESS_KEY` (find them under User Settings → Access Key).

Sauce Labs uses the same two-step upload-then-trigger pattern via their [Espresso endpoint](https://docs.saucelabs.com/mobile-apps/automated-testing/espresso-xcuitest/).

```bash
export SAUCE_USERNAME=your_username
export SAUCE_ACCESS_KEY=your_access_key
export SAUCE_REGION=us-west-1   # or eu-central-1

APP_APK=android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk
RUNNER_APK=android/runner/build/outputs/apk/androidTest/debug/runner-debug-androidTest.apk

# 1. Upload app APK
APP_ID=$(curl -s -u "$SAUCE_USERNAME:$SAUCE_ACCESS_KEY" \
  -X POST "https://api.$SAUCE_REGION.saucelabs.com/v1/storage/upload" \
  -F "payload=@$APP_APK" -F "name=sampleapp-debug.apk" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['item']['id'])")

# 2. Upload runner APK (Espresso test suite)
RUNNER_ID=$(curl -s -u "$SAUCE_USERNAME:$SAUCE_ACCESS_KEY" \
  -X POST "https://api.$SAUCE_REGION.saucelabs.com/v1/storage/upload" \
  -F "payload=@$RUNNER_APK" -F "name=runner-debug-androidTest.apk" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['item']['id'])")

# 3. Trigger a run
curl -s -u "$SAUCE_USERNAME:$SAUCE_ACCESS_KEY" \
  -X POST "https://api.$SAUCE_REGION.saucelabs.com/v1/rdc/jobs" \
  -H "Content-Type: application/json" \
  -d "{
    \"kind\": \"espresso\",
    \"app\": \"storage:$APP_ID\",
    \"testApp\": \"storage:$RUNNER_ID\",
    \"devices\": [{\"name\": \"Google Pixel 7 GoogleAPI Emulator\", \"platformVersion\": \"13\"}],
    \"testOptions\": {\"class\": \"dev.podium.runner.FlowRunner\"}
  }" | python3 -m json.tool
```

Replace the `devices` entry with any device from the [Sauce Labs real device catalog](https://app.saucelabs.com/live/web-testing/virtual).

### How it works on cloud farms

The runner APK IS the Espresso test suite — there is no separate test framework. When the farm invokes `am instrument`, the runner:
1. Reads the flow YAML files pushed to the device (or uses flows baked in at build time).
2. Executes each step via UIAutomator2 entirely on-device with zero network round-trips.
3. Reports results through the standard instrumentation output that every Espresso-compatible farm already understands.

The only constraint is **ABI coverage**: real devices require the `arm64-v8a` native library in the runner APK. The release build includes both `arm64-v8a` and `x86_64` ABIs, so it runs on both real devices and x86 cloud emulators.

## Command Reference

### `podium validate <flows>`

Parse flows locally without a device. Catches YAML errors instantly.

```bash
podium validate flows/
podium validate flows/login.yaml
podium validate flows/smoke.yaml --env BASE_URL=https://example.com
```

### `podium test <flows>`

Run flows on a connected device.

```bash
podium test flows/ --runner <path/to/runner.apk>
podium test flows/login.yaml --app sampleapp.apk --runner runner.apk
podium test flows/ --serial emulator-5554 --out ./results
podium test flows/ --env USERNAME=alice --env PASSWORD=secret
```

Options: `--app`, `--runner`, `--serial`, `--env KEY=VALUE` (repeatable), `--out` (default `./podium-out`).

### `podium report <dir>`

Pretty-print results from a previous run.

```bash
podium report podium-out/results/
```

## Flow YAML Reference

Flows use a Maestro-compatible two-document format:

```yaml
appId: com.example.myapp
---
- launchApp:
    clearState: true          # pm clear + relaunch
- tapOn:
    id: "username"            # resource-id suffix match
- inputText: "alice"
- tapOn: "Log in"             # shorthand: string → text selector
- assertVisible: "Welcome"
- assertVisible:
    text: "Welcome"
    timeout: 5000             # ms, default 10000
- assertNotVisible: "Loading"
- scrollUntilVisible: "Item 50"
- scrollUntilVisible:
    element: "Item 50"
    maxSwipes: 30
- swipe:
    direction: UP             # UP | DOWN | LEFT | RIGHT
- back                        # press back button
- waitForAnimationToEnd:
    timeout: 3000
- takeScreenshot: "after-login"
```

**Selectors:**
- String shorthand: `"Log in"` → `By.text("Log in")`
- `id:` → `By.res(Pattern.compile(".*:id/<id>"))` (suffix match)
- Regex text: `"/Item \\d+/"` → `By.text(Pattern.compile(...))`

**Environment substitution:** `${VAR}` in YAML is replaced from `--env VAR=value` or `-e env.VAR=value`.

## Known Limitations

- Android only. No iOS (the Rust core is portable; a Swift `Driver` over XCUITest would work — see "Porting to iOS" below).
- 10 commands: `launchApp`, `tapOn`, `inputText`, `assertVisible`, `assertNotVisible`, `scrollUntilVisible`, `swipe`, `back`, `waitForAnimationToEnd`, `takeScreenshot`. No JavaScript, no `runFlow` composition.
- No parallel flow execution.
- Cloud upload scripts are documented templates — actual runs require credentials you supply separately (see `scripts/upload-browserstack.sh`).
- `inputText` types via `UiObject2.text = value` (replaces field content); does not simulate per-keystroke events.

## Porting to iOS

The Rust engine is already isolated from Android APIs. To port:
1. Compile `podium-core` as a static library for iOS targets (`aarch64-apple-ios`, `x86_64-apple-ios-sim`).
2. Generate Swift bindings: `cargo run --bin uniffi-bindgen generate --language swift`.
3. Implement `Driver` in Swift over `XCUITest` — `isVisible` becomes `XCUIElement.exists`, `tap` becomes `element.tap()`, etc.
4. The engine, retry semantics, and timing capture require zero changes.

## Repository Layout

```
podium/
├── core/          Rust crate: parser, engine, Driver trait (pure Rust, zero Android deps)
├── cli/           Rust binary: podium validate / test / report
├── android/
│   ├── runner/    Instrumentation APK: FlowRunner + UiAutomatorDriver
│   └── sampleapp/ Sample app under test (dev.podium.sample)
├── flows/         Sample YAML flows
├── scripts/       build-runner.sh, run-local.sh, bench-local.sh
├── BENCHMARK.md   Measured timings on local emulator
└── PLAN.md        Original implementation plan
```
