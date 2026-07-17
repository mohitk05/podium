# AdbTransport: Maestro gRPC redesign

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the YAML-per-call `AdbTransport` with a gRPC client that talks to Maestro's on-device APK (`dev.mobile.maestro.test`). The host side speaks the Maestro proto over a TCP connection forwarded via `adb forward`. Element finding (selector → coordinates) is implemented in Rust on the host using the XML view hierarchy returned by the `viewHierarchy` RPC.

**Architecture:** `AdbTransport::connect()` installs the Maestro driver APK (if not present), runs `adb forward tcp:PORT tcp:PORT`, starts `am instrument` in the background to launch the on-device gRPC server, waits for the port to accept connections, then opens a tonic gRPC channel. Each `Transport` method maps to one or more gRPC calls. Element finding (for `tap`, `is_visible`) calls `viewHierarchy`, parses the returned UIAutomator XML with `quick-xml`, matches the selector, extracts `bounds="[x1,y1][x2,y2]"`, and derives the center point for `tap(x, y)`. `AdbTransport` continues implementing the existing `Transport` trait — the `PodiumDevice` orchestration layer, `MockTransport`, and all unit tests are unchanged.

**Tech Stack:** Rust 2021, tonic 0.13 (gRPC client), prost 0.13 (generated proto types), quick-xml 0.37 (XML parsing), tokio (process + net + time), build.rs with tonic-build

**Reference:** Maestro proto at `https://raw.githubusercontent.com/mobile-dev-inc/maestro/main/maestro-proto/src/main/proto/maestro_android.proto`

## Global Constraints

- Only `device/` changes — `core/`, `cli/`, and the Android runner are untouched
- No uniffi attributes anywhere in `podium-device`
- `Transport` trait signature is unchanged — all existing unit tests must continue to pass
- `MockTransport` and `IosTransport` are unchanged
- `am instrument` is started with `-e class 'dev.mobile.maestro.MaestroDriverService#grpcServer'` to launch the gRPC server (not the default test runner mode)
- Fixed host port: `7001` (configurable via `AdbTransport` field, default `7001`)
- `adb forward` maps `tcp:7001` on host to `tcp:7001` on device
- Selector matching priority: `id` field matches `resource-id` attribute (suffix: `.*:id/<value>`); `text` field matches `text` attribute (exact) or regex if wrapped in `/…/`; `index` selects the Nth match (0-based)
- `swipe`, `back`, `hide_keyboard` use `adb shell input` — not in the Maestro proto
- Bounds format from UIAutomator XML: `[x1,y1][x2,y2]` — center = `((x1+x2)/2, (y1+y2)/2)`
- `DeviceBuilder::build()` for Android now calls `AdbTransport::connect().await` which does the full setup; failures surface as `PodiumError::Transport`
- Integration tests remain gated behind `integration` feature, all `#[ignore]`

---

## File Map

```
device/
├── Cargo.toml                 — add tonic, prost, quick-xml; add build-dependencies
├── build.rs                   — NEW: tonic_build::compile_protos("proto/maestro_android.proto")
├── proto/
│   └── maestro_android.proto  — NEW: copy of Maestro proto (vendored)
└── src/
    ├── adb.rs                 — REPLACE: gRPC client + XML element finder
    ├── device.rs              — MODIFY: DeviceBuilder::build() calls AdbTransport::connect()
    └── (all other files unchanged)
```

---

### Task 1: Add dependencies, vendor proto, write build.rs

Wire tonic codegen so the proto types compile before any Rust code references them.

**Files:**
- Modify: `device/Cargo.toml`
- Create: `device/build.rs`
- Create: `device/proto/maestro_android.proto`

**Interfaces:**
- Produces: `maestro_android` module available via `include!(concat!(env!("OUT_DIR"), "/maestro_android.rs"))` in `adb.rs`

- [ ] **Step 1: Update `device/Cargo.toml`**

Replace the entire file:

```toml
[package]
name = "podium-device"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
quick-xml   = "0.37"
serde_json  = "1"
thiserror   = "2"
tokio       = { version = "1", features = ["process", "time", "rt", "fs", "net"] }
tonic       = "0.13"
prost       = "0.13"

[build-dependencies]
tonic-build = "0.13"

[dev-dependencies]
tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros", "time", "test-util"] }

[features]
mock        = []
integration = []
```

Note: `podium-core` dependency is removed — `AdbTransport` no longer uses it.

- [ ] **Step 2: Create `device/build.rs`**

```rust
fn main() {
    tonic_build::compile_protos("proto/maestro_android.proto")
        .expect("tonic_build failed");
}
```

- [ ] **Step 3: Create `device/proto/maestro_android.proto`**

Vendor the Maestro proto exactly as published (copy verbatim from the upstream source):

```proto
syntax = "proto3";

package maestro_android;

service MaestroDriver {
  rpc deviceInfo(DeviceInfoRequest) returns (DeviceInfo) {}
  rpc viewHierarchy(ViewHierarchyRequest) returns (ViewHierarchyResponse) {}
  rpc screenshot(ScreenshotRequest) returns (ScreenshotResponse) {}
  rpc tap(TapRequest) returns (TapResponse) {}
  rpc inputText(InputTextRequest) returns (InputTextResponse) {}
  rpc eraseAllText(EraseAllTextRequest) returns (EraseAllTextResponse) {}
  rpc setLocation(SetLocationRequest) returns (SetLocationResponse) {}
  rpc isWindowUpdating(CheckWindowUpdatingRequest) returns (CheckWindowUpdatingResponse) {}
  rpc launchApp(LaunchAppRequest) returns (LaunchAppResponse) {}
  rpc addMedia(stream AddMediaRequest) returns (AddMediaResponse) {}
  rpc enableMockLocationProviders(EmptyRequest) returns (EmptyResponse) {}
  rpc disableLocationUpdates(EmptyRequest) returns (EmptyResponse) {}
}

message EmptyRequest {}
message EmptyResponse {}

message LaunchAppRequest {
  string packageName = 1;
  repeated ArgumentValue arguments = 2;
}

message ArgumentValue {
  string key = 1;
  string value = 2;
  string type = 3;
}

message LaunchAppResponse {}

message DeviceInfoRequest {}

message DeviceInfo {
  uint32 widthPixels = 1;
  uint32 heightPixels = 2;
}

message ScreenshotRequest {}

message ScreenshotResponse {
  bytes bytes = 1;
}

message ViewHierarchyRequest {}

message ViewHierarchyResponse {
  string hierarchy = 1;
}

message TapRequest {
  uint32 x = 1;
  uint32 y = 2;
}

message TapResponse {}

message InputTextRequest {
  string text = 1;
}

message InputTextResponse {}

message EraseAllTextRequest {
  uint32 charactersToErase = 1;
}

message EraseAllTextResponse {}

message SetLocationRequest {
  double latitude = 1;
  double longitude = 2;
}

message SetLocationResponse {}

message CheckWindowUpdatingRequest {
  string appId = 1;
}

message CheckWindowUpdatingResponse {
  bool isWindowUpdating = 1;
}

message AddMediaRequest {
  Payload payload = 1;
  string media_name = 2;
  string media_ext = 3;
}

message AddMediaResponse {}

message Payload {
  bytes data = 1;
}
```

- [ ] **Step 4: Verify codegen compiles**

```bash
cargo build -p podium-device
```

Expected: compiles (tonic generates `maestro_android.rs` in `OUT_DIR`). The existing `adb.rs` still has the old implementation — that is expected at this stage.

- [ ] **Step 5: Commit**

```bash
git add device/Cargo.toml device/build.rs device/proto/ Cargo.lock
git commit -m "build: vendor maestro_android proto and wire tonic codegen in podium-device"
```

---

### Task 2: XML view hierarchy parser

Implement the element-finding logic that maps a `Selector` to `(center_x, center_y)` coordinates by parsing the UIAutomator XML returned by the `viewHierarchy` gRPC call.

**Files:**
- Create: `device/src/hierarchy.rs`
- Modify: `device/src/lib.rs` — add `mod hierarchy;`

**Interfaces:**
- Produces:
  ```rust
  // in device/src/hierarchy.rs
  pub(crate) struct Bounds { pub x: u32, pub y: u32, pub width: u32, pub height: u32 }
  impl Bounds {
      pub fn center(&self) -> (u32, u32) { (self.x + self.width/2, self.y + self.height/2) }
  }
  pub(crate) fn find_element(xml: &str, selector: &Selector) -> Option<Bounds>
  ```

- [ ] **Step 1: Write the failing tests**

Create `device/src/hierarchy.rs`:

```rust
use crate::types::Selector;

pub(crate) struct Bounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Bounds {
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

pub(crate) fn find_element(xml: &str, selector: &Selector) -> Option<Bounds> {
    todo!()
}

fn parse_bounds(s: &str) -> Option<Bounds> {
    todo!()
}

fn matches_selector(attrs: &std::collections::HashMap<String, String>, selector: &Selector) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Selector;

    // Minimal UIAutomator XML with one node
    const XML_ONE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hierarchy rotation="0">
  <node index="0" text="Login" resource-id="com.example:id/login_btn"
        bounds="[100,200][300,260]" class="android.widget.Button"
        clickable="true" enabled="true" />
</hierarchy>"#;

    // XML with two nodes sharing the same text
    const XML_TWO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hierarchy rotation="0">
  <node index="0" text="Item" resource-id="com.example:id/item_0"
        bounds="[0,0][100,50]" class="android.widget.TextView"
        clickable="true" enabled="true" />
  <node index="1" text="Item" resource-id="com.example:id/item_1"
        bounds="[0,60][100,110]" class="android.widget.TextView"
        clickable="true" enabled="true" />
</hierarchy>"#;

    #[test]
    fn find_by_text_returns_center() {
        let b = find_element(XML_ONE, &Selector::text("Login")).unwrap();
        // bounds [100,200][300,260]: center = (200, 230)
        assert_eq!(b.center(), (200, 230));
    }

    #[test]
    fn find_by_id_suffix() {
        // id selector matches resource-id suffix after ":id/"
        let b = find_element(XML_ONE, &Selector::id("login_btn")).unwrap();
        assert_eq!(b.center(), (200, 230));
    }

    #[test]
    fn find_by_index_selects_nth_match() {
        // index 1 → second "Item" node
        let b = find_element(XML_TWO, &Selector::text("Item").index(1)).unwrap();
        // bounds [0,60][100,110]: center = (50, 85)
        assert_eq!(b.center(), (50, 85));
    }

    #[test]
    fn find_missing_returns_none() {
        assert!(find_element(XML_ONE, &Selector::text("NotHere")).is_none());
    }

    #[test]
    fn parse_bounds_basic() {
        let b = parse_bounds("[100,200][300,260]").unwrap();
        assert_eq!((b.x, b.y, b.width, b.height), (100, 200, 200, 60));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p podium-device hierarchy
```

Expected: compile error from `todo!()`.

- [ ] **Step 3: Implement `parse_bounds`**

```rust
fn parse_bounds(s: &str) -> Option<Bounds> {
    // format: "[x1,y1][x2,y2]"
    let s = s.trim();
    let s = s.strip_prefix('[')?;
    let (coords, _) = s.split_once(']')?;
    let (x1s, y1s) = coords.split_once(',')?;
    let rest = s.get(coords.len() + 1..)?.trim_start_matches('[');
    let (coords2, _) = rest.split_once(']')?;
    let (x2s, y2s) = coords2.split_once(',')?;
    let x1: u32 = x1s.trim().parse().ok()?;
    let y1: u32 = y1s.trim().parse().ok()?;
    let x2: u32 = x2s.trim().parse().ok()?;
    let y2: u32 = y2s.trim().parse().ok()?;
    Some(Bounds { x: x1, y: y1, width: x2 - x1, height: y2 - y1 })
}
```

- [ ] **Step 4: Implement `matches_selector`**

```rust
fn matches_selector(attrs: &std::collections::HashMap<String, String>, selector: &Selector) -> bool {
    if let Some(id) = &selector.id {
        // resource-id ends with ":id/<id>"
        let suffix = format!(":id/{id}");
        return attrs.get("resource-id").map(|r| r.ends_with(&suffix)).unwrap_or(false);
    }
    if let Some(text) = &selector.text {
        let node_text = attrs.get("text").map(String::as_str).unwrap_or("");
        if text.starts_with('/') && text.ends_with('/') {
            // regex match
            let pattern = &text[1..text.len() - 1];
            return regex::Regex::new(pattern).map(|re| re.is_match(node_text)).unwrap_or(false);
        }
        return node_text == text.as_str();
    }
    false
}
```

- [ ] **Step 5: Implement `find_element` using `quick-xml`**

Add `use quick_xml::{events::Event, Reader};` at the top of `hierarchy.rs`, then:

```rust
pub(crate) fn find_element(xml: &str, selector: &Selector) -> Option<Bounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut match_count = 0u32;
    loop {
        match reader.read_event().ok()? {
            Event::Empty(e) | Event::Start(e) => {
                let attrs: std::collections::HashMap<String, String> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(a.value.as_ref()).to_string();
                        (key, val)
                    })
                    .collect();
                if matches_selector(&attrs, selector) {
                    if match_count == selector.index {
                        let bounds_str = attrs.get("bounds")?;
                        return parse_bounds(bounds_str);
                    }
                    match_count += 1;
                }
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}
```

- [ ] **Step 6: Add `regex` to `device/Cargo.toml` dependencies**

```toml
regex = "1"
```

- [ ] **Step 7: Add `mod hierarchy;` to `device/src/lib.rs`**

Add `mod hierarchy;` alongside the other private module declarations.

- [ ] **Step 8: Run tests**

```bash
cargo test -p podium-device hierarchy
```

Expected: all 5 hierarchy tests pass.

- [ ] **Step 9: Run full test suite to confirm nothing regressed**

```bash
cargo test -p podium-device --features mock
```

Expected: all existing tests still pass.

- [ ] **Step 10: Commit**

```bash
git add device/src/hierarchy.rs device/src/lib.rs device/Cargo.toml Cargo.lock
git commit -m "feat: add XML view hierarchy parser for UIAutomator element finding"
```

---

### Task 3: Implement `AdbTransport` as a gRPC client

Replace the entire `device/src/adb.rs` with the new implementation. `AdbTransport` connects to Maestro's on-device gRPC server and implements the `Transport` trait.

**Files:**
- Replace: `device/src/adb.rs`
- Modify: `device/src/device.rs` — `DeviceBuilder::build()` now calls `AdbTransport::connect().await`

**Interfaces:**
- Consumes:
  - `maestro_android::maestro_driver_client::MaestroDriverClient` (tonic generated)
  - `maestro_android::{LaunchAppRequest, TapRequest, InputTextRequest, ViewHierarchyRequest}` etc.
  - `hierarchy::find_element` from Task 2
  - `tokio::process::Command` for `adb forward` and `am instrument`
  - `tokio::net::TcpStream` for the readiness probe
- Produces: `AdbTransport` with a `connect(serial, port) -> Result<Self, TransportError>` async constructor and full `Transport` impl

- [ ] **Step 1: Write the failing integration test skeleton**

In `device/tests/integration.rs`, replace contents:

```rust
//! Integration tests — require Maestro driver APK installed and a connected device.
//! Run: PODIUM_SERIAL=<serial> PODIUM_APP_ID=<pkg> cargo test -p podium-device --features integration -- --ignored

#[cfg(feature = "integration")]
mod adb {
    use podium_device::{DeviceBuilder, Platform, Selector};

    fn serial() -> Option<String> { std::env::var("PODIUM_SERIAL").ok() }
    fn app_id() -> String { std::env::var("PODIUM_APP_ID").unwrap_or_else(|_| "dev.podium.sample".into()) }

    #[tokio::test]
    #[ignore = "requires connected device with Maestro driver APK"]
    async fn launch_app_smoke() {
        let device = DeviceBuilder::default()
            .platform(Platform::Android { serial: serial() })
            .app_id(app_id())
            .build().await.expect("build device");
        device.launch_app(&app_id(), false).await.expect("launch_app");
    }

    #[tokio::test]
    #[ignore = "requires connected device with Maestro driver APK"]
    async fn tap_and_assert_visible() {
        let device = DeviceBuilder::default()
            .platform(Platform::Android { serial: serial() })
            .app_id(app_id())
            .build().await.expect("build device");
        device.launch_app(&app_id(), true).await.expect("launch_app");
        device.assert_visible(Selector::text("Welcome")).await.expect("welcome visible");
    }
}
```

- [ ] **Step 2: Implement `AdbTransport` in `device/src/adb.rs`**

Replace the entire file:

```rust
use crate::error::TransportError;
use crate::hierarchy::find_element;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use tokio::process::Command;
use tonic::transport::Channel;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/maestro_android.rs"));
}

use proto::maestro_driver_client::MaestroDriverClient;
use proto::{
    InputTextRequest, LaunchAppRequest, TapRequest, ViewHierarchyRequest,
};

const MAESTRO_RUNNER: &str =
    "dev.mobile.maestro.test/androidx.test.runner.AndroidJUnitRunner";
const STARTUP_TIMEOUT_MS: u64 = 30_000;
const STARTUP_POLL_MS: u64 = 200;

pub(crate) struct AdbTransport {
    serial: Option<String>,
    port: u16,
    client: MaestroDriverClient<Channel>,
}

impl AdbTransport {
    /// Set up `adb forward`, start `am instrument` in background, wait for gRPC server,
    /// then return a connected `AdbTransport`.
    pub(crate) async fn connect(
        serial: Option<String>,
        port: u16,
    ) -> Result<Self, TransportError> {
        let adb = |args: &[&str]| {
            let mut cmd = Command::new("adb");
            if let Some(s) = &serial {
                cmd.arg("-s").arg(s);
            }
            cmd.args(args);
            cmd
        };

        // 1. Forward the port
        adb(&["forward", &format!("tcp:{port}"), &format!("tcp:{port}")])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("adb forward: {e}") })?;

        // 2. Start the on-device gRPC server (background — don't await exit)
        adb(&[
            "shell", "am", "instrument", "-w",
            "-e", "class", "dev.mobile.maestro.MaestroDriverService#grpcServer",
            "-e", "port", &port.to_string(),
            MAESTRO_RUNNER,
        ])
        .spawn()
        .map_err(|e| TransportError::OperationFailed { reason: format!("am instrument spawn: {e}") })?;

        // 3. Wait for the TCP port to accept connections
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_millis(STARTUP_TIMEOUT_MS);
        loop {
            if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .is_ok()
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(TransportError::OperationFailed {
                    reason: format!(
                        "Maestro gRPC server did not start on port {port} within {STARTUP_TIMEOUT_MS}ms"
                    ),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(STARTUP_POLL_MS)).await;
        }

        // 4. Open the tonic channel
        let endpoint = format!("http://127.0.0.1:{port}");
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| TransportError::OperationFailed { reason: format!("channel: {e}") })?
            .connect()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("connect: {e}") })?;

        Ok(Self { serial, port, client: MaestroDriverClient::new(channel) })
    }

    fn adb_shell(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("adb");
        if let Some(s) = &self.serial {
            cmd.arg("-s").arg(s);
        }
        cmd.arg("shell");
        cmd.args(args);
        cmd
    }

    async fn view_hierarchy(&self) -> Result<String, TransportError> {
        let mut client = self.client.clone();
        let resp = client
            .view_hierarchy(ViewHierarchyRequest {})
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("viewHierarchy: {e}") })?;
        Ok(resp.into_inner().hierarchy)
    }
}

#[async_trait]
impl Transport for AdbTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        if clear_state {
            self.adb_shell(&["pm", "clear", app_id])
                .status()
                .await
                .map_err(|e| TransportError::OperationFailed { reason: format!("pm clear: {e}") })?;
        }
        let mut client = self.client.clone();
        client
            .launch_app(LaunchAppRequest {
                package_name: app_id.to_string(),
                arguments: vec![],
            })
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("launchApp: {e}") })?;
        Ok(())
    }

    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let xml = self.view_hierarchy().await?;
        Ok(find_element(&xml, selector).is_some())
    }

    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        let xml = self.view_hierarchy().await?;
        let bounds = find_element(&xml, selector).ok_or_else(|| TransportError::ElementNotFound {
            reason: format!("tap: element not found: {:?}", selector),
        })?;
        let (cx, cy) = bounds.center();
        let mut client = self.client.clone();
        client
            .tap(TapRequest { x: cx, y: cy })
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("tap: {e}") })?;
        Ok(())
    }

    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        let mut client = self.client.clone();
        client
            .input_text(InputTextRequest { text: text.to_string() })
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("inputText: {e}") })?;
        Ok(())
    }

    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        self.adb_shell(&["input", "keyevent", "KEYCODE_BACK"])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("hide_keyboard: {e}") })?;
        Ok(())
    }

    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        // Get screen dimensions for swipe coordinates
        let xml = self.view_hierarchy().await?;
        // Use adb shell input swipe — not in Maestro proto
        let swipe_args: &[&str] = match direction {
            Direction::Up    => &["input", "swipe", "540", "1400", "540", "400",  "300"],
            Direction::Down  => &["input", "swipe", "540", "400",  "540", "1400", "300"],
            Direction::Left  => &["input", "swipe", "900", "800",  "180", "800",  "300"],
            Direction::Right => &["input", "swipe", "180", "800",  "900", "800",  "300"],
        };
        self.adb_shell(swipe_args)
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("swipe: {e}") })?;
        Ok(())
    }

    async fn back(&self) -> Result<(), TransportError> {
        self.adb_shell(&["input", "keyevent", "KEYCODE_BACK"])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("back: {e}") })?;
        Ok(())
    }

    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        // Poll isWindowUpdating until false or timeout
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_millis(timeout_ms);
        loop {
            let mut client = self.client.clone();
            let updating = client
                .is_window_updating(proto::CheckWindowUpdatingRequest {
                    app_id: String::new(),
                })
                .await
                .map(|r| r.into_inner().is_window_updating)
                .unwrap_or(false);
            if !updating {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(()); // best-effort
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        let mut client = self.client.clone();
        let resp = client
            .screenshot(proto::ScreenshotRequest {})
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("screenshot: {e}") })?;
        let bytes = resp.into_inner().bytes;
        let path = format!("{name}.png");
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| TransportError::OperationFailed { reason: format!("write screenshot: {e}") })?;
        Ok(())
    }
}
```

- [ ] **Step 3: Update `DeviceBuilder::build()` in `device/src/device.rs`**

The `Android` arm of the `match` in `build()` must call `AdbTransport::connect()`. Change:

```rust
Some(Platform::Android { serial }) => Arc::new(AdbTransport { serial, app_id }),
```

to:

```rust
Some(Platform::Android { serial }) => {
    let t = crate::adb::AdbTransport::connect(serial, 7001)
        .await
        .map_err(|e| PodiumError::Transport { reason: e.to_string() })?;
    Arc::new(t)
}
```

Also remove the `app_id` field from `AdbTransport` construction — `launch_app` takes `app_id` as a parameter now. Remove the `app_id: String` field from `DeviceBuilder` and the `.app_id()` builder method if it was only there for the old `AdbTransport`. Check that nothing else in `device.rs` references `app_id` on the builder before removing.

Actually: keep `app_id` on `DeviceBuilder` and `PodiumDevice` if integration tests pass it — the integration test uses `.app_id(app_id())`. The `app_id` field on `AdbTransport` is removed; the value passes through `device.launch_app(app_id, ...)` directly instead. So `DeviceBuilder` may keep `.app_id()` for the integration test convenience, but it no longer flows into `AdbTransport`.

- [ ] **Step 4: Run unit tests**

```bash
cargo test -p podium-device --features mock
```

Expected: all 7+ unit tests pass. (The `AdbTransport::connect()` is never called from unit tests — they use `MockTransport`.)

- [ ] **Step 5: Commit**

```bash
git add device/src/adb.rs device/src/device.rs device/tests/integration.rs Cargo.lock
git commit -m "feat: rewrite AdbTransport as Maestro gRPC client over adb forward"
```

---

### Task 4: Final verification

**Files:** no new files; verification only.

- [ ] **Step 1: Full test suite**

```bash
cargo test -p podium-device --features mock
```

Expected: all tests pass.

- [ ] **Step 2: Clippy**

```bash
cargo clippy -p podium-device --all-features -- -D warnings
```

Fix any warnings before proceeding.

- [ ] **Step 3: Core and CLI unaffected**

```bash
cargo test -p podium-core
cargo build -p podium
```

Expected: both pass with zero changes.

- [ ] **Step 4: Doc build**

```bash
cargo doc -p podium-device --no-deps
```

Expected: no errors.

- [ ] **Step 5: Commit if any clippy fixes were needed**

```bash
git add -p
git commit -m "fix: clippy warnings in podium-device after gRPC redesign"
```

Only commit if there were fixes; skip otherwise.
