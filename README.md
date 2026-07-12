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

## Fresh machine setup

These steps take a machine with nothing installed to a running demo. Skip any step you've already done.

### 1. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Add the Android cross-compilation targets:

```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

### 2. Java (JDK 17+)

**macOS:**
```bash
brew install --cask temurin@17
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install -y temurin-17-jdk   # needs adoptium repo
# or: sudo apt-get install -y openjdk-17-jdk
```

Verify: `java -version` should show 17 or higher.

### 3. Android SDK + NDK + emulator

Install [Android Studio](https://developer.android.com/studio) (includes the SDK manager), or install the command-line tools only:

```bash
# macOS (Homebrew)
brew install --cask android-commandlinetools

# Linux — download from https://developer.android.com/studio#command-tools
# and unzip to ~/android-sdk/cmdline-tools/latest/
```

Then install the required SDK components:

```bash
sdkmanager "platform-tools" "platforms;android-35" "ndk;27.2.12479018"

# For running a local emulator:
sdkmanager "emulator" "system-images;android-35;google_apis;x86_64"
avdmanager create avd -n podium -k "system-images;android-35;google_apis;x86_64"
```

Set the environment variable (add to `~/.zshrc` or `~/.bashrc`):

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"   # macOS default
# export ANDROID_HOME="$HOME/android-sdk"         # Linux / custom path
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
```

Verify: `adb version` and `echo $ANDROID_HOME` both return values.

### 4. cargo-ndk

```bash
cargo install cargo-ndk --locked
```

### 5. Clone and build

```bash
git clone https://github.com/mohitk05/podium.git
cd podium

# Build Rust core → Kotlin bindings → Android APKs
bash scripts/build-runner.sh
```

### 6. Start an emulator (or connect a device)

```bash
emulator -avd podium -no-window -no-audio &
adb wait-for-device
```

Or plug in an Android device with USB debugging enabled.

### 7. Install the `podium` CLI

```bash
cargo install --path cli
```

### 8. Run the demo

```bash
# Install the sample app
adb install -r android/sampleapp/build/outputs/apk/debug/sampleapp-debug.apk

# Validate flows (no device needed)
podium validate flows/

# Run the login + smoke flows
podium test flows/login.yaml
podium test flows/smoke.yaml

# Or run everything at once
bash scripts/run-local.sh
```

### Download pre-built binaries (alternative to building from source)

Each [GitHub Release](https://github.com/mohitk05/podium/releases) ships:
- `runner-debug-androidTest.apk` — the instrumentation APK (contains the Rust core)
- `sampleapp-debug.apk` — the sample app
- `podium-macos-aarch64`, `podium-macos-x86_64`, `podium-linux-x86_64` — the CLI

```bash
# Download and use the CLI directly (macOS Apple Silicon example)
curl -Lo podium https://github.com/mohitk05/podium/releases/latest/download/podium-macos-aarch64
chmod +x podium && sudo mv podium /usr/local/bin/

# Install the APKs
adb install -r sampleapp-debug.apk
podium test flows/login.yaml --runner runner-debug-androidTest.apk
```

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
