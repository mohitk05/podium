//! Integration tests — require Maestro driver APK installed and a connected device.
//! Run: PODIUM_SERIAL=<serial> PODIUM_APP_ID=<pkg> cargo test -p podium --features integration -- --ignored

#[cfg(feature = "integration")]
mod adb {
    use podium::{DeviceBuilder, Platform, Selector};

    fn serial() -> Option<String> {
        std::env::var("PODIUM_SERIAL").ok()
    }

    fn app_id() -> String {
        std::env::var("PODIUM_APP_ID").unwrap_or_else(|_| "dev.podium.sample".into())
    }

    #[tokio::test]
    #[ignore = "requires connected device with Maestro driver APK"]
    async fn launch_app_smoke() {
        let device = DeviceBuilder::default()
            .platform(Platform::Android { serial: serial() })
            .app_id(app_id())
            .build()
            .await
            .expect("build device");
        device.launch_app(&app_id(), false).await.expect("launch_app");
    }

    #[tokio::test]
    #[ignore = "requires connected device with Maestro driver APK"]
    async fn tap_and_assert_visible() {
        let device = DeviceBuilder::default()
            .platform(Platform::Android { serial: serial() })
            .app_id(app_id())
            .build()
            .await
            .expect("build device");
        device.launch_app(&app_id(), true).await.expect("launch_app");
        device.assert_visible(Selector::text("Welcome")).await.expect("welcome visible");
    }
}
