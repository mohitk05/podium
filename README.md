# Podium

Async Rust library for driving Android (and eventually iOS) devices in tests. Wraps Maestro's on-device gRPC server behind a clean `async/await` API — no YAML, no separate CLI process.

## How it works

`DeviceBuilder::build()` connects to a running Android emulator or device via ADB:

1. Installs the Maestro driver APKs (`maestro-app.apk` + `maestro-server.apk`) if not already present.
2. Forwards the gRPC port over ADB (`tcp:7001`).
3. Starts the on-device gRPC server via `am instrument` (skipped if already running).
4. Opens a `tonic` channel to `127.0.0.1:7001`.

All subsequent calls (`tap`, `assert_visible`, `scroll_until_visible`, …) go through that channel.

## Quick start

Add to `Cargo.toml`:

```toml
[dependencies]
podium-driver = "0.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use podium::{DeviceBuilder, Platform, Selector};

#[tokio::main]
async fn main() -> Result<(), podium::PodiumError> {
    let device = DeviceBuilder::default()
        .platform(Platform::Android { serial: None })
        .app_id("com.example.myapp")
        .build()
        .await?;

    device.launch_app("com.example.myapp", true).await?;
    device.assert_visible(Selector::text("Welcome")).await?;
    device.tap(Selector::text("Log in")).await?;
    device.input_text("alice").await?;
    device.take_screenshot("after-login").await?;
    Ok(())
}
```

`serial: None` picks the only connected device/emulator. Pass `serial: Some("emulator-5554".into())` to target a specific one.

## API

### `DeviceBuilder`

```rust
DeviceBuilder::default()
    .platform(Platform::Android { serial: Option<String> })
    .app_id("com.example.app")   // used for pm clear in launch_app
    .build()
    .await?
```

### `PodiumDevice` methods

| Method | Description |
|---|---|
| `launch_app(app_id, clear_state)` | Launch app. If `clear_state` is true, runs `pm clear` first. |
| `tap(selector)` | Tap the center of the matching element. |
| `input_text(text)` | Type text into the focused field. |
| `assert_visible(selector)` | Poll until element is visible (10 s timeout). |
| `assert_not_visible(selector)` | Poll until element is gone (10 s timeout). |
| `scroll_until_visible(selector)` | Swipe down up to 20 times until element appears. |
| `swipe(direction)` | Single swipe. `Direction`: `Up`, `Down`, `Left`, `Right`. |
| `back()` | Press the back button (`KEYCODE_BACK`). |
| `hide_keyboard()` | Dismiss the soft keyboard (`KEYCODE_BACK`). |
| `wait_for_animation()` | Wait for the window to stop updating (up to 10 s). |
| `take_screenshot(name)` | Write `<name>.png` to the current directory. |

### `Selector`

```rust
Selector::text("Network & internet")   // exact text match
Selector::text("/Item \\d+/")          // regex (wrap in slashes)
Selector::id("login_btn")              // resource-id suffix match
Selector::text("Item").index(1)        // second match
```

## Features

| Feature | Description |
|---|---|
| `mock` | Exports `MockTransport` for unit-testing code that drives a `PodiumDevice`. |
| `integration` | Enables the integration test suite in `tests/integration.rs`. |

## Running integration tests

Requires a running Android emulator:

```bash
emulator -avd <avd-name> &
adb wait-for-device

cargo test -p podium --features integration -- --ignored --nocapture --test-threads=1
```

`PODIUM_SERIAL=emulator-5554` targets a specific device.

## Repository layout

```
podium/
├── src/
│   ├── lib.rs          public API surface
│   ├── device.rs       PodiumDevice + DeviceBuilder, retry/poll logic
│   ├── adb.rs          AdbTransport: APK install, port-forward, gRPC calls
│   ├── hierarchy.rs    XML view-hierarchy parser (quick-xml)
│   ├── transport.rs    Transport trait
│   ├── types.rs        Selector, Direction, Bounds
│   ├── mock.rs         MockTransport (feature = "mock")
│   ├── ios.rs          IosTransport stub (not yet implemented)
│   ├── maestro-app.apk    bundled Maestro driver v2.6.1
│   └── maestro-server.apk bundled Maestro server v2.6.1
├── proto/
│   └── maestro_android.proto
├── tests/
│   └── integration.rs
└── Cargo.toml
```

## Publishing a new version

The Maestro driver APKs are downloaded at build time from this repo's GitHub releases. When bumping the crate version:

1. Update `version` in `podium/Cargo.toml`.
2. Create a GitHub release tagged `v<version>` (e.g. `v0.1.0`).
3. Upload `maestro-app-2.6.1.apk` and `maestro-server-2.6.1.apk` as release assets (copy from the previous release if the Maestro version hasn't changed).
4. `cargo publish -p podium`

The APK asset names encode the Maestro version (`MAESTRO_VERSION` in `build.rs`) so they survive across Podium releases without re-upload as long as the Maestro version is unchanged.

## Known limitations

- Android only. `IosTransport` exists as a stub but returns `NotSupported` for all calls.
- `input_text` replaces the focused field's content via the gRPC `InputText` call; it does not simulate per-keystroke events.
- `hide_keyboard` sends `KEYCODE_BACK`; it only works while the keyboard is shown.
- Swipe coordinates are hardcoded for a 1080×2400 screen — correct for most emulators, may need adjustment for small or unusual resolutions.
