# podium-device Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `podium-device` crate to the workspace that exposes an async, device-agnostic Rust API for driving mobile devices, so Bombadil (and other tools) can control Android devices without implementing any transport logic.

**Architecture:** A new `device/` workspace member adds `PodiumDevice` with async methods that return `Result<_, PodiumError>`. Internally it holds an `Arc<dyn Transport>` (private trait); `DeviceBuilder::build()` selects `AdbTransport` or stub `IosTransport` based on the `Platform` enum the caller passes at init. Poll/retry logic lives in `PodiumDevice` methods; transports are thin async I/O only. `podium-core` is unchanged.

**Tech Stack:** Rust 2021 edition, tokio (async runtime), thiserror, async-trait, podium-core (internal dep)

## Global Constraints

- `podium-core` and `cli/` receive zero changes
- No `uniffi` attributes anywhere in `podium-device`
- All public methods are `async fn` returning `Result<_, PodiumError>`
- `Transport` trait and `TransportError` are private (not re-exported)
- Default visibility timeout: 10 000 ms
- `MockTransport` exposed under `podium_device::mock` behind `mock` Cargo feature
- Integration tests gated behind `integration` Cargo feature

---

## File Map

```
device/
├── Cargo.toml
└── src/
    ├── lib.rs              — public re-exports: PodiumDevice, DeviceBuilder, Platform,
    │                         Selector, Direction, PodiumError; conditionally pub mod mock
    ├── error.rs            — PodiumError (public), TransportError (private)
    ├── types.rs            — Selector, Direction (public, no uniffi)
    ├── transport.rs        — Transport trait (private async trait), TransportError
    ├── device.rs           — PodiumDevice + DeviceBuilder; poll/retry logic
    ├── adb.rs              — AdbTransport: implements Transport via tokio::process::Command
    ├── ios.rs              — IosTransport: stub, all methods return NotSupported
    └── mock.rs             — MockTransport (conditionally compiled, re-exported under mock)
```

`Cargo.toml` (workspace root) gains `"device"` in `members`.

---

### Task 1: Scaffold `podium-device` crate

**Files:**
- Create: `device/Cargo.toml`
- Create: `device/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add `"device"` to `members`

**Interfaces:**
- Produces: compilable empty crate `podium-device` in the workspace

- [ ] **Step 1: Add `device` to workspace members**

In `Cargo.toml` (root), change:
```toml
[workspace]
members = ["core", "cli"]
```
to:
```toml
[workspace]
members = ["core", "cli", "device"]
```

- [ ] **Step 2: Create `device/Cargo.toml`**

```toml
[package]
name = "podium-device"
version = "0.1.0"
edition = "2021"

[dependencies]
podium-core = { path = "../core" }
tokio = { version = "1", features = ["process", "time", "rt"] }
async-trait = "0.1"
thiserror = "2"

[dev-dependencies]
tokio = { version = "1", features = ["rt", "macros"] }

[features]
mock = []
integration = []
```

- [ ] **Step 3: Create `device/src/lib.rs`** (empty module skeleton)

```rust
pub mod error;
pub mod types;

mod transport;
mod device;
mod adb;
mod ios;

#[cfg(feature = "mock")]
pub mod mock;

pub use device::{DeviceBuilder, PodiumDevice};
pub use error::PodiumError;
pub use types::{Direction, Selector};
```

- [ ] **Step 4: Create stub files so it compiles**

Create `device/src/error.rs`:
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum TransportError {
    #[error("Element not found: {reason}")]
    ElementNotFound { reason: String },

    #[error("Operation failed: {reason}")]
    OperationFailed { reason: String },

    #[error("Not supported: {reason}")]
    NotSupported { reason: String },

    #[error("Device not found")]
    DeviceNotFound,
}

impl From<TransportError> for PodiumError {
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::ElementNotFound { reason } => PodiumError::ElementNotFound { reason },
            TransportError::OperationFailed { reason } => PodiumError::Transport { reason },
            TransportError::NotSupported { reason } => PodiumError::NotSupported { reason },
            TransportError::DeviceNotFound => PodiumError::DeviceNotFound,
        }
    }
}
```

Create `device/src/types.rs`:
```rust
#[derive(Debug, Clone)]
pub struct Selector {
    pub(crate) text: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) index: u32,
}

impl Selector {
    pub fn text(t: impl Into<String>) -> Self {
        Self { text: Some(t.into()), id: None, index: 0 }
    }

    pub fn id(id: impl Into<String>) -> Self {
        Self { text: None, id: Some(id.into()), index: 0 }
    }

    pub fn index(mut self, i: u32) -> Self {
        self.index = i;
        self
    }
}

#[derive(Debug, Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
```

Create `device/src/transport.rs`:
```rust
use crate::error::TransportError;
use crate::types::{Direction, Selector};
use async_trait::async_trait;

#[async_trait]
pub(crate) trait Transport: Send + Sync {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError>;
    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError>;
    async fn tap(&self, selector: &Selector) -> Result<(), TransportError>;
    async fn input_text(&self, text: &str) -> Result<(), TransportError>;
    async fn hide_keyboard(&self) -> Result<(), TransportError>;
    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError>;
    async fn back(&self) -> Result<(), TransportError>;
    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError>;
    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError>;
    async fn now_ms(&self) -> u64;
}
```

Create `device/src/adb.rs`:
```rust
use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;

pub(crate) struct AdbTransport {
    pub(crate) serial: Option<String>,
}

#[async_trait]
impl Transport for AdbTransport {
    async fn launch_app(&self, _app_id: &str, _clear_state: bool) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn is_visible(&self, _selector: &Selector) -> Result<bool, TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn tap(&self, _selector: &Selector) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn input_text(&self, _text: &str) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn swipe(&self, _direction: &Direction) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn back(&self) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn wait_for_idle(&self, _timeout_ms: u64) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn take_screenshot(&self, _name: &str) -> Result<(), TransportError> {
        Err(TransportError::OperationFailed { reason: "not yet implemented".into() })
    }
    async fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
}
```

Create `device/src/ios.rs`:
```rust
use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;

pub(crate) struct IosTransport;

#[async_trait]
impl Transport for IosTransport {
    async fn launch_app(&self, _app_id: &str, _clear_state: bool) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn is_visible(&self, _selector: &Selector) -> Result<bool, TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn tap(&self, _selector: &Selector) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn input_text(&self, _text: &str) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn swipe(&self, _direction: &Direction) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn back(&self) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn wait_for_idle(&self, _timeout_ms: u64) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn take_screenshot(&self, _name: &str) -> Result<(), TransportError> {
        Err(TransportError::NotSupported { reason: "iOS transport not implemented in v1".into() })
    }
    async fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
}
```

Create `device/src/device.rs`:
```rust
use crate::adb::AdbTransport;
use crate::error::PodiumError;
use crate::ios::IosTransport;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use std::sync::Arc;

pub enum Platform {
    Android { serial: Option<String> },
    Ios { udid: Option<String> },
}

pub struct DeviceBuilder {
    platform: Option<Platform>,
}

impl DeviceBuilder {
    pub fn platform(mut self, p: Platform) -> Self {
        self.platform = Some(p);
        self
    }

    pub async fn build(self) -> Result<PodiumDevice, PodiumError> {
        let transport: Arc<dyn Transport> = match self.platform {
            Some(Platform::Android { serial }) => Arc::new(AdbTransport { serial }),
            Some(Platform::Ios { .. }) => Arc::new(IosTransport),
            None => return Err(PodiumError::Transport { reason: "platform not set".into() }),
        };
        Ok(PodiumDevice { transport })
    }
}

pub struct PodiumDevice {
    transport: Arc<dyn Transport>,
}

impl PodiumDevice {
    pub fn builder() -> DeviceBuilder {
        DeviceBuilder { platform: None }
    }

    pub async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), PodiumError> {
        self.transport.launch_app(app_id, clear_state).await.map_err(Into::into)
    }

    pub async fn tap(&self, selector: Selector) -> Result<(), PodiumError> {
        self.transport.tap(&selector).await.map_err(Into::into)
    }

    pub async fn input_text(&self, text: &str) -> Result<(), PodiumError> {
        self.transport.input_text(text).await.map_err(Into::into)
    }

    pub async fn assert_visible(&self, selector: Selector) -> Result<(), PodiumError> {
        self.wait_until_visible(&selector, 10_000).await
    }

    pub async fn assert_not_visible(&self, selector: Selector) -> Result<(), PodiumError> {
        self.wait_until_not_visible(&selector, 10_000).await
    }

    pub async fn scroll_until_visible(&self, selector: Selector) -> Result<(), PodiumError> {
        self.scroll_until_visible_inner(&selector, 20).await
    }

    pub async fn swipe(&self, direction: Direction) -> Result<(), PodiumError> {
        self.transport.swipe(&direction).await.map_err(Into::into)
    }

    pub async fn back(&self) -> Result<(), PodiumError> {
        self.transport.back().await.map_err(Into::into)
    }

    pub async fn hide_keyboard(&self) -> Result<(), PodiumError> {
        self.transport.hide_keyboard().await.map_err(Into::into)
    }

    pub async fn wait_for_animation(&self) -> Result<(), PodiumError> {
        self.transport.wait_for_idle(10_000).await.map_err(Into::into)
    }

    pub async fn take_screenshot(&self, name: &str) -> Result<(), PodiumError> {
        self.transport.take_screenshot(name).await.map_err(Into::into)
    }

    async fn wait_until_visible(&self, selector: &Selector, timeout_ms: u64) -> Result<(), PodiumError> {
        let start = self.transport.now_ms().await;
        let deadline = start + timeout_ms;
        loop {
            if self.transport.is_visible(selector).await.map_err(PodiumError::from)? {
                return Ok(());
            }
            let now = self.transport.now_ms().await;
            if now >= deadline {
                return Err(PodiumError::Timeout {
                    timeout_ms,
                    reason: format!("element not visible: {:?}", selector),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(200.min(deadline - now))).await;
        }
    }

    async fn wait_until_not_visible(&self, selector: &Selector, timeout_ms: u64) -> Result<(), PodiumError> {
        let start = self.transport.now_ms().await;
        let deadline = start + timeout_ms;
        loop {
            if !self.transport.is_visible(selector).await.map_err(PodiumError::from)? {
                return Ok(());
            }
            let now = self.transport.now_ms().await;
            if now >= deadline {
                return Err(PodiumError::Timeout {
                    timeout_ms,
                    reason: format!("element still visible: {:?}", selector),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(200.min(deadline - now))).await;
        }
    }

    async fn scroll_until_visible_inner(&self, selector: &Selector, max_swipes: u32) -> Result<(), PodiumError> {
        for i in 0..max_swipes {
            if self.transport.is_visible(selector).await.map_err(PodiumError::from)? {
                return Ok(());
            }
            if i < max_swipes - 1 {
                self.transport.swipe(&Direction::Down).await.map_err(PodiumError::from)?;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
        Err(PodiumError::ElementNotFound {
            reason: format!("element not found after {} swipes: {:?}", max_swipes, selector),
        })
    }
}
```

Create `device/src/mock.rs` (empty for now — implemented in Task 3):
```rust
// populated in Task 3
```

- [ ] **Step 5: Verify it compiles**

```bash
cargo build -p podium-device
```

Expected: compiles with zero errors (warnings about unused stubs are fine).

- [ ] **Step 6: Commit**

```bash
git add device/ Cargo.toml Cargo.lock
git commit -m "feat: scaffold podium-device crate with stub transports"
```

---

### Task 2: `MockTransport` + unit tests for poll/retry logic

`MockTransport` must be available internally during this task (as a `#[cfg(test)]` struct in `device.rs` tests) so we can test `PodiumDevice`'s poll/retry loops before wiring the real transport. In Task 3 we move it to `mock.rs` for public re-export.

**Files:**
- Modify: `device/src/device.rs` — add `#[cfg(test)]` module with `MockTransport` and tests

**Interfaces:**
- Consumes: `Transport` trait from `device/src/transport.rs`, `PodiumDevice` from `device/src/device.rs`
- Produces: tested `wait_until_visible`, `wait_until_not_visible`, `scroll_until_visible_inner` logic

- [ ] **Step 1: Add test module with `MockTransport` to `device/src/device.rs`**

Append to the end of `device/src/device.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::TransportError;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct MockTransport {
        /// Simulated clock in ms — advanced by sleep calls
        time_ms: Mutex<u64>,
        /// For each selector key, list of (time_ms, is_visible) transitions
        visibility: Mutex<std::collections::HashMap<String, Vec<(u64, bool)>>>,
        /// Ordered record of calls made
        calls: Mutex<Vec<String>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                time_ms: Mutex::new(0),
                visibility: Mutex::new(std::collections::HashMap::new()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn set_visible_at(&self, key: &str, at_ms: u64, visible: bool) {
            self.visibility
                .lock().unwrap()
                .entry(key.into())
                .or_default()
                .push((at_ms, visible));
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn selector_key(s: &Selector) -> String {
            if let Some(t) = &s.text { format!("text:{t}") }
            else if let Some(id) = &s.id { format!("id:{id}") }
            else { "unknown".into() }
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("launch_app({app_id},{clear_state})"));
            Ok(())
        }
        async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
            let key = Self::selector_key(selector);
            let now = *self.time_ms.lock().unwrap();
            let vis = self.visibility.lock().unwrap();
            let visible = vis.get(&key)
                .map(|events| {
                    events.iter()
                        .filter(|(t, _)| *t <= now)
                        .last()
                        .map(|(_, v)| *v)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            Ok(visible)
        }
        async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("tap({:?})", Self::selector_key(selector)));
            Ok(())
        }
        async fn input_text(&self, text: &str) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("input_text({text})"));
            Ok(())
        }
        async fn hide_keyboard(&self) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push("hide_keyboard".into());
            Ok(())
        }
        async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("swipe({direction:?})"));
            Ok(())
        }
        async fn back(&self) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push("back".into());
            Ok(())
        }
        async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("wait_for_idle({timeout_ms})"));
            Ok(())
        }
        async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
            self.calls.lock().unwrap().push(format!("take_screenshot({name})"));
            Ok(())
        }
        async fn now_ms(&self) -> u64 {
            *self.time_ms.lock().unwrap()
        }
    }

    // PodiumDevice calls tokio::time::sleep; we need to control simulated time.
    // We do this by swapping the transport's clock forward in test helpers that
    // advance time alongside the real tokio::time::pause() + advance() approach.
    // For simplicity, tests use tokio::time::pause() + advance() so the poll loop
    // fires immediately without real wall-clock waiting.

    fn make_device(mock: Arc<MockTransport>) -> PodiumDevice {
        PodiumDevice { transport: mock as Arc<dyn Transport> }
    }

    #[tokio::test]
    async fn assert_visible_succeeds_immediately() {
        let mock = Arc::new(MockTransport::new());
        mock.set_visible_at("text:Login", 0, true);
        let device = make_device(mock.clone());
        device.assert_visible(Selector::text("Login")).await.unwrap();
    }

    #[tokio::test]
    async fn assert_visible_times_out() {
        tokio::time::pause();
        let mock = Arc::new(MockTransport::new());
        // Never becomes visible
        let device = make_device(mock.clone());
        let handle = tokio::spawn(async move {
            device.assert_visible(Selector::text("Never")).await
        });
        tokio::time::advance(std::time::Duration::from_millis(10_200)).await;
        let result = handle.await.unwrap();
        assert!(matches!(result, Err(PodiumError::Timeout { .. })));
    }

    #[tokio::test]
    async fn assert_not_visible_succeeds_immediately() {
        let mock = Arc::new(MockTransport::new());
        // Never set visible — defaults to not visible
        let device = make_device(mock.clone());
        device.assert_not_visible(Selector::text("Error")).await.unwrap();
    }

    #[tokio::test]
    async fn scroll_until_visible_finds_immediately() {
        let mock = Arc::new(MockTransport::new());
        mock.set_visible_at("text:Item", 0, true);
        let device = make_device(mock.clone());
        device.scroll_until_visible(Selector::text("Item")).await.unwrap();
        assert!(!mock.calls().iter().any(|c| c.starts_with("swipe")));
    }

    #[tokio::test]
    async fn scroll_until_visible_exceeds_max_swipes() {
        tokio::time::pause();
        let mock = Arc::new(MockTransport::new());
        // Never visible
        let device = make_device(mock.clone());
        let result = device.scroll_until_visible(Selector::text("Deep")).await;
        assert!(matches!(result, Err(PodiumError::ElementNotFound { .. })));
    }

    #[tokio::test]
    async fn ios_transport_returns_not_supported() {
        use crate::ios::IosTransport;
        let device = PodiumDevice {
            transport: Arc::new(IosTransport) as Arc<dyn Transport>,
        };
        let err = device.tap(Selector::text("Anything")).await.unwrap_err();
        assert!(matches!(err, PodiumError::NotSupported { .. }));
    }
}
```

- [ ] **Step 2: Run the tests**

```bash
cargo test -p podium-device
```

Expected: all 6 tests pass. The `assert_visible_times_out` and `scroll_until_visible_exceeds_max_swipes` tests use `tokio::time::pause()` + `advance()` so they complete instantly.

- [ ] **Step 3: Commit**

```bash
git add device/src/device.rs
git commit -m "test: add MockTransport and poll/retry unit tests for podium-device"
```

---

### Task 3: Expose `MockTransport` publicly under `podium_device::mock`

**Files:**
- Modify: `device/src/mock.rs` — implement full public `MockTransport`
- Modify: `device/src/lib.rs` — already has `#[cfg(feature = "mock")] pub mod mock;`

**Interfaces:**
- Consumes: `Transport` trait, `Selector`, `Direction`, `TransportError` from earlier tasks
- Produces: `podium_device::mock::MockTransport` (public, behind `mock` feature)

- [ ] **Step 1: Write the failing test**

Add a new test file `device/src/mock.rs` that tests the public interface compiles and records calls:

```rust
use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use std::sync::Mutex;

/// Configurable response for a single `is_visible` call.
#[derive(Clone)]
pub struct VisibilityEvent {
    pub selector_key: String,
    pub at_ms: u64,
    pub visible: bool,
}

/// Public mock transport for use by consumers (e.g. Bombadil) in their own tests.
/// Enable with `podium-device = { features = ["mock"] }`.
pub struct MockTransport {
    pub time_ms: Mutex<u64>,
    visibility: Mutex<std::collections::HashMap<String, Vec<(u64, bool)>>>,
    pub calls: Mutex<Vec<String>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            time_ms: Mutex::new(0),
            visibility: Mutex::new(std::collections::HashMap::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Register a visibility transition: `selector_key` becomes `visible` at `at_ms`.
    pub fn set_visible_at(&self, selector_key: &str, at_ms: u64, visible: bool) {
        self.visibility
            .lock().unwrap()
            .entry(selector_key.into())
            .or_default()
            .push((at_ms, visible));
    }

    /// Snapshot of all calls recorded so far.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn selector_key(s: &Selector) -> String {
        if let Some(t) = &s.text { format!("text:{t}") }
        else if let Some(id) = &s.id { format!("id:{id}") }
        else { "unknown".into() }
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("launch_app({app_id},{clear_state})"));
        Ok(())
    }
    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let key = Self::selector_key(selector);
        let now = *self.time_ms.lock().unwrap();
        let vis = self.visibility.lock().unwrap();
        let visible = vis.get(&key)
            .and_then(|events| {
                events.iter()
                    .filter(|(t, _)| *t <= now)
                    .last()
                    .map(|(_, v)| *v)
            })
            .unwrap_or(false);
        Ok(visible)
    }
    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("tap({})", Self::selector_key(selector)));
        Ok(())
    }
    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("input_text({text})"));
        Ok(())
    }
    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push("hide_keyboard".into());
        Ok(())
    }
    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("swipe({direction:?})"));
        Ok(())
    }
    async fn back(&self) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push("back".into());
        Ok(())
    }
    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("wait_for_idle({timeout_ms})"));
        Ok(())
    }
    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("take_screenshot({name})"));
        Ok(())
    }
    async fn now_ms(&self) -> u64 {
        *self.time_ms.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::PodiumDevice;
    use crate::transport::Transport;
    use std::sync::Arc;

    #[tokio::test]
    async fn mock_records_tap() {
        let mock = Arc::new(MockTransport::new());
        let device = PodiumDevice {
            transport: mock.clone() as Arc<dyn Transport>,
        };
        mock.set_visible_at("text:Btn", 0, true);
        device.tap(Selector::text("Btn")).await.unwrap();
        assert!(mock.calls().iter().any(|c| c.contains("tap(text:Btn)")));
    }

    #[tokio::test]
    async fn mock_records_launch_app() {
        let mock = Arc::new(MockTransport::new());
        let device = PodiumDevice {
            transport: mock.clone() as Arc<dyn Transport>,
        };
        device.launch_app("com.example", false).await.unwrap();
        assert!(mock.calls().iter().any(|c| c == "launch_app(com.example,false)"));
    }
}
```

- [ ] **Step 2: Run tests with `mock` feature**

```bash
cargo test -p podium-device --features mock
```

Expected: all tests pass including the two new ones in `mock.rs`.

- [ ] **Step 3: Verify feature is opt-in (no mock feature = mock module absent)**

```bash
cargo build -p podium-device 2>&1 | grep -i mock || echo "ok: mock not in default build"
```

Expected: prints `ok: mock not in default build` (no mock symbols in default build).

- [ ] **Step 4: Commit**

```bash
git add device/src/mock.rs
git commit -m "feat: add public MockTransport under podium_device::mock feature"
```

---

### Task 4: Implement `AdbTransport`

The `AdbTransport` sends single-command flows to the device via `adb push` + `am instrument`, reusing the same Espresso runner APK mechanism as the CLI. Each `PodiumDevice` method call constructs a minimal single-command `Flow`, serialises it to YAML, pushes it to `/data/local/tmp/podium/cmd.yaml`, and runs the runner. The runner returns a JSON result via `adb pull`.

**Files:**
- Modify: `device/src/adb.rs` — full implementation

**Interfaces:**
- Consumes: `podium_core::model::{Flow, Command, Selector as CoreSelector, Direction as CoreDirection}`, `podium_core::parse_flow`, `tokio::process::Command`
- Produces: fully implemented `AdbTransport` that satisfies the `Transport` trait

- [ ] **Step 1: Add `serde_json` and `serde_yaml` to `device/Cargo.toml`**

```toml
[dependencies]
podium-core = { path = "../core" }
tokio = { version = "1", features = ["process", "time", "rt"] }
async-trait = "0.1"
thiserror = "2"
serde_json = "1"
```

- [ ] **Step 2: Write the failing test**

Add an integration test skeleton gated behind the `integration` feature in a new file `device/tests/integration.rs`:

```rust
#![cfg(feature = "integration")]

use podium_device::{DeviceBuilder, Direction, Platform, Selector};

#[tokio::test]
async fn smoke_launch_and_back() {
    let device = DeviceBuilder::default()
        .platform(Platform::Android { serial: None })
        .build()
        .await
        .expect("device init failed");

    device.launch_app("dev.podium.sample", false).await.expect("launch failed");
    device.back().await.expect("back failed");
}
```

- [ ] **Step 3: Run to confirm it fails (no device needed — feature gate skips it)**

```bash
cargo test -p podium-device
```

Expected: all existing unit tests still pass; integration test not compiled.

- [ ] **Step 4: Implement `AdbTransport` in `device/src/adb.rs`**

Replace the entire contents of `device/src/adb.rs` with:

```rust
use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;

const DEVICE_CMD_DIR: &str = "/data/local/tmp/podium";
const RUNNER: &str = "dev.podium.runner.test/androidx.test.runner.AndroidJUnitRunner";

pub(crate) struct AdbTransport {
    pub(crate) serial: Option<String>,
}

impl AdbTransport {
    fn adb(&self) -> Command {
        let mut cmd = Command::new("adb");
        if let Some(s) = &self.serial {
            cmd.args(["-s", s]);
        }
        cmd
    }

    async fn adb_ok(&self, args: &[&str]) -> Result<String, TransportError> {
        let out = self.adb()
            .args(args)
            .output()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            return Err(TransportError::OperationFailed { reason: stderr });
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// Build a minimal single-command YAML flow and run it on-device via am instrument.
    /// Returns the JSON result file content.
    async fn run_command(&self, yaml: &str) -> Result<String, TransportError> {
        // 1. Push the flow YAML
        let tmp = std::env::temp_dir().join("podium-cmd.yaml");
        tokio::fs::write(&tmp, yaml)
            .await
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })?;

        self.adb_ok(&["shell", "mkdir", "-p", DEVICE_CMD_DIR]).await?;

        let dest = format!("{DEVICE_CMD_DIR}/cmd.yaml");
        self.adb_ok(&["push", tmp.to_str().unwrap(), &dest]).await?;

        // 2. Run the instrumentation
        self.adb_ok(&[
            "shell", "am", "instrument", "-w", "-r",
            "-e", "flowsDir", DEVICE_CMD_DIR,
            RUNNER,
        ]).await?;

        // 3. Pull the result JSON
        let result_dir = std::env::temp_dir().join("podium-result");
        tokio::fs::create_dir_all(&result_dir)
            .await
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })?;
        let device_results = "/sdcard/Android/data/dev.podium.runner.test/files/podium/results";
        self.adb_ok(&["pull", device_results, result_dir.to_str().unwrap()]).await?;

        // Read the first JSON result file
        let mut entries = tokio::fs::read_dir(&result_dir)
            .await
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })?;
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })? {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                return tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| TransportError::OperationFailed { reason: e.to_string() });
            }
        }
        Err(TransportError::OperationFailed { reason: "no result JSON found".into() })
    }

    /// Parse the JSON result and return Ok(()) if the flow passed, Err otherwise.
    fn check_result(json: &str) -> Result<(), TransportError> {
        let val: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| TransportError::OperationFailed { reason: e.to_string() })?;
        if val["passed"].as_bool().unwrap_or(false) {
            Ok(())
        } else {
            let msg = val["steps"]
                .as_array()
                .and_then(|steps| steps.iter().find(|s| s["status"] == "FAILED"))
                .and_then(|s| s["failure_message"].as_str())
                .unwrap_or("flow failed")
                .to_string();
            Err(TransportError::OperationFailed { reason: msg })
        }
    }

    fn selector_to_yaml(selector: &Selector) -> String {
        if let Some(text) = &selector.text {
            format!("  text: {text:?}\n  index: {}", selector.index)
        } else if let Some(id) = &selector.id {
            format!("  id: {id:?}\n  index: {}", selector.index)
        } else {
            "  index: 0".into()
        }
    }

    fn direction_str(d: &Direction) -> &'static str {
        match d {
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::Left => "left",
            Direction::Right => "right",
        }
    }
}

#[async_trait]
impl Transport for AdbTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        let yaml = format!(
            "appId: {app_id}\n---\n- launchApp:\n    clearState: {clear_state}\n"
        );
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let sel = Self::selector_to_yaml(selector);
        let yaml = format!(
            "appId: __podium_probe__\n---\n- assertVisible:\n{sel}\n    timeout: 0\n"
        );
        match Self::check_result(&self.run_command(&yaml).await?) {
            Ok(_) => Ok(true),
            Err(TransportError::OperationFailed { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        let sel = Self::selector_to_yaml(selector);
        let yaml = format!("appId: __podium_probe__\n---\n- tapOn:\n{sel}\n");
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        let yaml = format!("appId: __podium_probe__\n---\n- inputText: {text:?}\n");
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        let yaml = "appId: __podium_probe__\n---\n- hideKeyboard\n".to_string();
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        let d = Self::direction_str(direction);
        let yaml = format!("appId: __podium_probe__\n---\n- swipe: {d}\n");
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn back(&self) -> Result<(), TransportError> {
        let yaml = "appId: __podium_probe__\n---\n- back\n".to_string();
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        let yaml = format!(
            "appId: __podium_probe__\n---\n- waitForAnimationToEnd:\n    timeout: {timeout_ms}\n"
        );
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        let yaml = format!("appId: __podium_probe__\n---\n- takeScreenshot: {name:?}\n");
        Self::check_result(&self.run_command(&yaml).await?)
    }

    async fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
```

- [ ] **Step 5: Run unit tests to confirm nothing regressed**

```bash
cargo test -p podium-device
```

Expected: all unit tests pass.

- [ ] **Step 6: Commit**

```bash
git add device/src/adb.rs device/Cargo.toml device/tests/integration.rs Cargo.lock
git commit -m "feat: implement AdbTransport for podium-device"
```

---

### Task 5: Wire `DeviceBuilder` default and clean up public API

Ensure `DeviceBuilder` implements `Default` and `Platform` is exported from the crate root so consumers don't need to reach into sub-modules.

**Files:**
- Modify: `device/src/device.rs` — add `Default` impl for `DeviceBuilder`, export `Platform`
- Modify: `device/src/lib.rs` — re-export `Platform`

**Interfaces:**
- Consumes: `DeviceBuilder`, `Platform` from `device/src/device.rs`
- Produces: `podium_device::Platform`, `podium_device::DeviceBuilder` both accessible; `DeviceBuilder::default()` works

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module at the bottom of `device/src/device.rs`:

```rust
#[tokio::test]
async fn builder_requires_platform() {
    let result = DeviceBuilder::default().build().await;
    assert!(matches!(result, Err(PodiumError::Transport { .. })));
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p podium-device builder_requires_platform
```

Expected: compile error — `Default` not implemented for `DeviceBuilder`.

- [ ] **Step 3: Implement `Default` for `DeviceBuilder` in `device/src/device.rs`**

Add after the `DeviceBuilder` struct definition:

```rust
impl Default for DeviceBuilder {
    fn default() -> Self {
        Self { platform: None }
    }
}
```

- [ ] **Step 4: Re-export `Platform` from `device/src/lib.rs`**

Change:
```rust
pub use device::{DeviceBuilder, PodiumDevice};
```
to:
```rust
pub use device::{DeviceBuilder, Platform, PodiumDevice};
```

- [ ] **Step 5: Run all tests**

```bash
cargo test -p podium-device
```

Expected: all tests pass including `builder_requires_platform`.

- [ ] **Step 6: Commit**

```bash
git add device/src/device.rs device/src/lib.rs
git commit -m "feat: export Platform from crate root; add DeviceBuilder::default()"
```

---

### Task 6: Final verification and docs

**Files:**
- Modify: `device/src/lib.rs` — add crate-level doc comment
- No other file changes

**Interfaces:**
- Produces: `cargo doc` builds without warnings; `cargo test` passes; `cargo clippy` clean

- [ ] **Step 1: Add crate-level doc to `device/src/lib.rs`**

Prepend to `device/src/lib.rs`:

```rust
//! Async device-interaction API for driving mobile devices from Rust.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use podium_device::{DeviceBuilder, Platform, Selector};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), podium_device::PodiumError> {
//!     let device = DeviceBuilder::default()
//!         .platform(Platform::Android { serial: None })
//!         .build()
//!         .await?;
//!
//!     device.launch_app("com.example.app", true).await?;
//!     device.tap(Selector::text("Login")).await?;
//!     Ok(())
//! }
//! ```
```

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -p podium-device --all-features -- -D warnings
```

Fix any warnings before proceeding.

- [ ] **Step 3: Run all tests one final time**

```bash
cargo test -p podium-device --features mock
```

Expected: all tests pass.

- [ ] **Step 4: Build docs**

```bash
cargo doc -p podium-device --no-deps --open
```

Expected: doc page opens; no missing-doc warnings.

- [ ] **Step 5: Verify `podium-core` and CLI are unaffected**

```bash
cargo test -p podium-core
cargo build -p podium
```

Expected: both succeed with no changes.

- [ ] **Step 6: Commit**

```bash
git add device/src/lib.rs
git commit -m "docs: add crate-level doc comment to podium-device"
```
