//! Integration tests — require a running Android emulator (no physical device needed).
//!
//! Run all:
//!   cargo test -p podium --features integration -- --ignored --nocapture --test-threads=1
//!
//! Run one:
//!   cargo test -p podium --features integration -- --ignored tap_into_search --nocapture
//!
//! Optional env vars:
//!   PODIUM_SERIAL   — adb serial (defaults to the only connected device/emulator)

#[cfg(feature = "integration")]
mod emulator {
    use podium::{DeviceBuilder, Direction, Platform, PodiumDevice, Selector};

    const SETTINGS_PKG: &str = "com.android.settings";

    // Each test gets a fresh PodiumDevice so that the tonic channel is always owned by
    // the current test's Tokio runtime. Dropping a Device across runtime boundaries
    // invalidates the channel. The gRPC server stays up (port-already-up check skips
    // respawn), so connect() is fast for subsequent tests.
    async fn device() -> PodiumDevice {
        DeviceBuilder::default()
            .platform(Platform::Android {
                serial: std::env::var("PODIUM_SERIAL").ok(),
            })
            .app_id(SETTINGS_PKG)
            .build()
            .await
            .expect("connect to emulator — is one running? (`emulator -avd <name>`)")
    }

    // ── driver installation ───────────────────────────────────────────────────

    /// Verifies AdbTransport::connect auto-installs the Maestro driver APKs.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn driver_apk_auto_installed() {
        // connect() installs; if Ok both APKs are present.
        let _d = device().await;
    }

    // ── launch ────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn launch_settings_cold() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.assert_visible(Selector::text("Search Settings"))
            .await
            .expect("Settings root visible");
    }

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn launch_settings_warm() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, false).await.expect("launch_app");
        d.assert_visible(Selector::text("Search Settings"))
            .await
            .expect("Settings root visible");
    }

    // ── visibility ───────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn assert_not_visible_nonexistent_element() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.assert_not_visible(Selector::text("zz_podium_sentinel_zz"))
            .await
            .expect("sentinel must not be visible");
    }

    // ── tap + navigate ────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn tap_network_and_internet() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.assert_visible(Selector::text("Network & internet"))
            .await
            .expect("row visible");
        d.tap(Selector::text("Network & internet"))
            .await
            .expect("tap");
        // On the sub-screen "Internet" is the first list item
        d.assert_visible(Selector::text("Internet"))
            .await
            .expect("sub-screen open");
    }

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn tap_then_back_returns_to_root() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.assert_visible(Selector::text("Network & internet"))
            .await
            .expect("root loaded");
        d.tap(Selector::text("Network & internet"))
            .await
            .expect("tap");
        // Wait for sub-screen to open before pressing back
        d.assert_visible(Selector::text("Internet"))
            .await
            .expect("network sub-screen open");
        d.back().await.expect("back");
        // Allow the back transition and accessibility service to settle
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        // "Connected devices" only appears on the root Settings screen, not on Network sub-screen
        d.assert_visible(Selector::text("Connected devices"))
            .await
            .expect("back at root");
    }

    // ── scroll ────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn scroll_to_about_phone() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");

        // "About emulated device" on AOSP emulators; "About phone" on branded devices.
        // Try both names, re-launching to reset scroll position between attempts.
        let r = d
            .scroll_until_visible(Selector::text("About emulated device"))
            .await;
        if r.is_ok() {
            return;
        }
        d.launch_app(SETTINGS_PKG, true).await.expect("relaunch");
        d.scroll_until_visible(Selector::text("About phone"))
            .await
            .expect("About phone / About emulated device reachable by scrolling");
    }

    // ── swipe ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn swipe_down_stays_in_settings() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.swipe(Direction::Down).await.expect("swipe down");
        d.assert_visible(Selector::text("Search Settings"))
            .await
            .expect("Settings still visible");
    }

    // ── screenshot ────────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn screenshot_writes_file() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");

        let name = "podium_test_screenshot";
        d.take_screenshot(name).await.expect("take_screenshot");

        let png = format!("{name}.png");
        let path = std::path::Path::new(&png);
        assert!(path.exists(), "{png} not found");
        assert!(path.metadata().unwrap().len() > 0, "{png} is empty");
        std::fs::remove_file(path).ok();
    }

    // ── wait_for_animation ────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn wait_for_animation_after_tap() {
        let d = device().await;
        d.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        d.assert_visible(Selector::text("Network & internet"))
            .await
            .expect("root loaded");
        d.tap(Selector::text("Network & internet"))
            .await
            .expect("tap");
        d.wait_for_animation().await.expect("wait_for_animation");
    }

    // ── full e2e flow ─────────────────────────────────────────────────────────

    /// Cold launch → navigate → screenshot → back → scroll to bottom.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn full_settings_flow() {
        let d = device().await;

        d.launch_app(SETTINGS_PKG, true).await.expect("launch");
        d.assert_visible(Selector::text("Search Settings"))
            .await
            .expect("root visible");

        d.tap(Selector::text("Network & internet"))
            .await
            .expect("tap network");
        d.wait_for_animation().await.expect("settle");
        // On the sub-screen the title is in the toolbar (content-desc), not text; assert a list item
        d.assert_visible(Selector::text("Internet"))
            .await
            .expect("network screen open");

        d.take_screenshot("podium_e2e_network")
            .await
            .expect("screenshot");
        std::fs::remove_file("podium_e2e_network.png").ok();

        d.back().await.expect("back");
        d.assert_visible(Selector::text("Search Settings"))
            .await
            .expect("back at root");

        let r = d
            .scroll_until_visible(Selector::text("About emulated device"))
            .await;
        if r.is_err() {
            d.launch_app(SETTINGS_PKG, true)
                .await
                .expect("relaunch for scroll");
            d.scroll_until_visible(Selector::text("About phone"))
                .await
                .expect("scrolled to bottom");
        }
    }
}
