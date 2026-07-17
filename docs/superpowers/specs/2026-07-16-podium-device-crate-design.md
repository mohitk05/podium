# podium-device crate design

**Date:** 2026-07-16  
**Status:** Approved

## Goal

Expose Podium's device interaction engine as a standalone Rust crate (`podium-device`) so that external tools — starting with Bombadil, a PBT-based testing framework — can drive mobile devices without implementing any device interaction logic themselves.

## Workspace structure

```
podium/
├── core/          (podium-core)   — unchanged; uniffi + engine + on-device Android runner
├── cli/           (podium)        — unchanged; ADB orchestration CLI
└── device/        (podium-device) — NEW; async public API for external Rust consumers
```

`podium-core` and `cli/` receive no changes. `podium-device` depends on `podium-core` as a private implementation detail only — none of `podium-core`'s uniffi-annotated types are re-exported.

## Public API

### Entry point

```rust
let device = PodiumDevice::builder()
    .platform(Platform::Android { serial: None })
    .build()
    .await?;
```

`Platform` is the discriminant set by the caller at init time:

```rust
pub enum Platform {
    Android { serial: Option<String> },
    Ios { udid: Option<String> },   // stubbed in v1; returns NotSupported
}
```

### Interaction methods

All methods are `async` and return `Result<_, PodiumError>`. The caller (Bombadil) receives an `Err` on failure and decides how to handle it (fail the property, shrink, retry).

```rust
device.launch_app("com.example.app", true).await?;  // second arg: clear_state
device.tap(Selector::text("Login")).await?;
device.input_text("hunter2").await?;
device.assert_visible(Selector::id("home_screen")).await?;
device.assert_not_visible(Selector::text("Loading")).await?;
device.scroll_until_visible(Selector::text("Item 50")).await?;
device.swipe(Direction::Up).await?;
device.back().await?;
device.hide_keyboard().await?;
device.wait_for_animation().await?;
device.take_screenshot("after_login").await?;
```

### Selector

Builder-style, no uniffi attributes:

```rust
Selector::text("Login")           // by display text (exact or /regex/)
Selector::id("login_button")      // by resource id
Selector::text("Login").index(1)  // second match
```

### Timeouts

Visibility assertions default to 10 000 ms. Per-call timeout overrides are out of scope for v1.

## Internal architecture

`PodiumDevice` holds an `Arc<dyn Transport>`. `Transport` is a **private** async trait — not part of the public API:

```rust
trait Transport: Send + Sync {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError>;
    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError>;
    async fn tap(&self, selector: &Selector) -> Result<(), TransportError>;
    async fn input_text(&self, text: &str) -> Result<(), TransportError>;
    async fn hide_keyboard(&self) -> Result<(), TransportError>;
    async fn swipe(&self, direction: Direction) -> Result<(), TransportError>;
    async fn back(&self) -> Result<(), TransportError>;
    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError>;
    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError>;
    async fn now_ms(&self) -> u64;
}
```

`DeviceBuilder::build()` matches on `Platform` and constructs the concrete transport:

- `Platform::Android` → `AdbTransport` — drives the device via `tokio::process::Command`, using `adb push` + `am instrument` to invoke the Espresso runner APK (same mechanism as the CLI), not raw `adb shell` UIAutomator commands
- `Platform::Ios` → `IosTransport` — stub; every method returns `Err(PodiumError::NotSupported { ... })`

The poll/retry logic (wait-until-visible loop, scroll-until-visible loop) lives in `PodiumDevice` methods, mirroring the engine/driver separation in `podium-core`. The transport is a thin async I/O layer only.

`podium-core`'s synchronous `Driver` trait is **not** used inside `podium-device`. The transport is an async-native parallel interface to avoid `spawn_blocking` wrapping.

## Error handling

Single public error type:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PodiumError {
    #[error("Element not found: {reason}")]
    ElementNotFound { reason: String },

    #[error("Assertion timed out after {timeout_ms}ms: {reason}")]
    Timeout { timeout_ms: u64, reason: String },

    #[error("Transport error: {reason}")]
    Transport { reason: String },

    #[error("Not supported on this platform: {reason}")]
    NotSupported { reason: String },

    #[error("Device not found")]
    DeviceNotFound,
}
```

`TransportError` (private) maps to `PodiumError` at each `PodiumDevice` method boundary. Nothing internal leaks into the public API.

## Testing

**Unit tests** — `MockTransport` implements the private `Transport` trait, records calls, and returns configurable responses. Covers poll/retry logic in `PodiumDevice` without a real device. `MockTransport` is also re-exported publicly under `podium_device::mock` (behind a `mock` Cargo feature flag) so Bombadil can use it to test its PBT harness offline without a real device attached.

**Integration tests** — gated behind a `integration` feature flag. Require a connected device. Thin smoke tests only: launch, tap, assert. Run manually or in CI with a real device attached.

**`podium-core` tests** — unchanged.

## Out of scope for v1

- iOS transport implementation
- Per-call timeout configuration
- Async streaming of step results
- Screenshot diffing or artifact collection
