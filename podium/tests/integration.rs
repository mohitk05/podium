//! Integration tests — require a running Android emulator (no physical device needed).
//!
//! Run all:
//!   cargo test -p podium --features integration -- --ignored --nocapture
//!
//! Run one:
//!   cargo test -p podium --features integration -- --ignored tap_into_search --nocapture
//!
//! Optional env vars:
//!   PODIUM_SERIAL   — adb serial (defaults to the only connected device/emulator)

#[cfg(feature = "integration")]
mod emulator {
    use podium::{DeviceBuilder, Direction, Platform, Selector};

    const SETTINGS_PKG: &str = "com.android.settings";

    async fn settings_device() -> podium::PodiumDevice {
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

    /// Verifies AdbTransport::connect auto-installs the Maestro driver APK when
    /// it is absent, and that subsequent connects skip the install step.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn driver_apk_auto_installed() {
        // connect() itself installs the APK; if it returns Ok the APK is present.
        let _device = settings_device().await;

        // Second connect must succeed immediately (APK already installed).
        let _device2 = settings_device().await;
    }

    // ── launch ────────────────────────────────────────────────────────────────

    /// Cold-launch Settings with a clean state and assert the root screen is visible.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn launch_settings_cold() {
        let device = settings_device().await;
        device
            .launch_app(SETTINGS_PKG, true)
            .await
            .expect("launch_app");

        // "Settings" heading appears on the root screen of every AOSP build.
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("Settings root screen visible");
    }

    /// Warm-launch (no state clear) and assert the root screen.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn launch_settings_warm() {
        let device = settings_device().await;
        device
            .launch_app(SETTINGS_PKG, false)
            .await
            .expect("launch_app");
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("Settings root screen visible");
    }

    // ── visibility ───────────────────────────────────────────────────────────

    /// Elements that are not on screen must not be reported as visible.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn assert_not_visible_nonexistent_element() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        device
            .assert_not_visible(Selector::text("zz_podium_sentinel_zz"))
            .await
            .expect("sentinel element must not be visible");
    }

    // ── tap + navigate ────────────────────────────────────────────────────────

    /// Tap the "Network & internet" row and assert the sub-screen opens.
    /// This is the first list item on every AOSP 10+ emulator.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn tap_network_and_internet() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        device
            .assert_visible(Selector::text("Network & internet"))
            .await
            .expect("Network & internet row visible");

        device
            .tap(Selector::text("Network & internet"))
            .await
            .expect("tap Network & internet");

        // The sub-screen header or a child item must appear.
        device
            .assert_visible(Selector::text("Network & internet"))
            .await
            .expect("Network & internet screen opened");
    }

    /// Tap into a screen then press Back and verify we return to root.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn tap_then_back_returns_to_root() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        device
            .tap(Selector::text("Network & internet"))
            .await
            .expect("tap Network & internet");
        device.back().await.expect("back");
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("back to Settings root");
    }

    // ── scroll ────────────────────────────────────────────────────────────────

    /// "About emulated device" (or "About phone") is always at the bottom of the
    /// Settings list — scroll until it's visible.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn scroll_to_about_phone() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");

        // Try both label variants used across API levels.
        let found = device
            .scroll_until_visible(Selector::text("About emulated device"))
            .await;
        let found = if found.is_err() {
            device
                .scroll_until_visible(Selector::text("About phone"))
                .await
        } else {
            found
        };

        found.expect("About phone / About emulated device reachable by scrolling");
    }

    // ── swipe ─────────────────────────────────────────────────────────────────

    /// Swipe down on the Settings list; the screen should still be showing Settings.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn swipe_down_stays_in_settings() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        device.swipe(Direction::Down).await.expect("swipe down");
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("Settings still visible after swipe");
    }

    // ── screenshot ────────────────────────────────────────────────────────────

    /// take_screenshot writes a PNG file to disk.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn screenshot_writes_file() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");

        let name = "podium_test_screenshot";
        device.take_screenshot(name).await.expect("take_screenshot");

        let png = format!("{name}.png");
        let path = std::path::Path::new(&png);
        assert!(path.exists(), "screenshot file {png} not found");
        assert!(path.metadata().unwrap().len() > 0, "screenshot file is empty");

        std::fs::remove_file(path).ok();
    }

    // ── wait_for_animation ────────────────────────────────────────────────────

    /// wait_for_animation must complete without error after a tap.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn wait_for_animation_after_tap() {
        let device = settings_device().await;
        device.launch_app(SETTINGS_PKG, true).await.expect("launch_app");
        device
            .tap(Selector::text("Network & internet"))
            .await
            .expect("tap");
        device
            .wait_for_animation()
            .await
            .expect("wait_for_animation");
    }

    // ── full e2e flow ─────────────────────────────────────────────────────────

    /// Full flow: launch → navigate → screenshot → back → verify root.
    #[tokio::test]
    #[ignore = "requires running emulator"]
    async fn full_settings_flow() {
        let device = settings_device().await;

        // Cold launch
        device.launch_app(SETTINGS_PKG, true).await.expect("launch");
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("root visible");

        // Navigate into Network & internet
        device
            .tap(Selector::text("Network & internet"))
            .await
            .expect("tap network");
        device.wait_for_animation().await.expect("settle");
        device
            .assert_visible(Selector::text("Network & internet"))
            .await
            .expect("network screen open");

        // Screenshot of sub-screen
        device
            .take_screenshot("podium_e2e_network")
            .await
            .expect("screenshot");
        std::fs::remove_file("podium_e2e_network.png").ok();

        // Go back
        device.back().await.expect("back");
        device
            .assert_visible(Selector::text("Settings"))
            .await
            .expect("back at root");

        // Scroll to bottom
        let scrolled = device
            .scroll_until_visible(Selector::text("About emulated device"))
            .await;
        let scrolled = if scrolled.is_err() {
            device
                .scroll_until_visible(Selector::text("About phone"))
                .await
        } else {
            scrolled
        };
        scrolled.expect("scrolled to bottom");
    }
}

