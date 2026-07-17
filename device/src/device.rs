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
    app_id: Option<String>,
}

impl DeviceBuilder {
    pub fn platform(mut self, p: Platform) -> Self {
        self.platform = Some(p);
        self
    }

    pub fn app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    pub async fn build(self) -> Result<PodiumDevice, PodiumError> {
        let app_id = self.app_id.unwrap_or_default();
        let transport: Arc<dyn Transport> = match self.platform {
            Some(Platform::Android { serial }) => Arc::new(AdbTransport { serial, app_id }),
            Some(Platform::Ios { .. }) => Arc::new(IosTransport),
            None => return Err(PodiumError::Transport { reason: "platform not set".into() }),
        };
        Ok(PodiumDevice { transport })
    }
}

impl Default for DeviceBuilder {
    fn default() -> Self {
        Self { platform: None, app_id: None }
    }
}

pub struct PodiumDevice {
    pub(crate) transport: Arc<dyn Transport>,
}

impl PodiumDevice {
    pub fn builder() -> DeviceBuilder {
        DeviceBuilder { platform: None, app_id: None }
    }

    #[cfg(feature = "mock")]
    pub fn from_mock(mock: std::sync::Arc<crate::mock::MockTransport>) -> Self {
        PodiumDevice { transport: mock as Arc<dyn Transport> }
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
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            if self.transport.is_visible(selector).await.map_err(PodiumError::from)? {
                return Ok(());
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(PodiumError::Timeout {
                    timeout_ms,
                    reason: format!("element not visible: {:?}", selector),
                });
            }
            let remaining = deadline - now;
            let sleep = remaining.min(std::time::Duration::from_millis(200));
            tokio::time::sleep(sleep).await;
        }
    }

    async fn wait_until_not_visible(&self, selector: &Selector, timeout_ms: u64) -> Result<(), PodiumError> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            if !self.transport.is_visible(selector).await.map_err(PodiumError::from)? {
                return Ok(());
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(PodiumError::Timeout {
                    timeout_ms,
                    reason: format!("element still visible: {:?}", selector),
                });
            }
            let remaining = deadline - now;
            let sleep = remaining.min(std::time::Duration::from_millis(200));
            tokio::time::sleep(sleep).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::TransportError;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockTransport {
        visibility: Mutex<HashMap<String, bool>>,
        calls: Mutex<Vec<String>>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                visibility: Mutex::new(HashMap::new()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn set_visible(&self, key: &str, visible: bool) {
            self.visibility.lock().unwrap().insert(key.into(), visible);
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
            let visible = self.visibility.lock().unwrap().get(&key).copied().unwrap_or(false);
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
    }

    fn make_device(mock: Arc<MockTransport>) -> PodiumDevice {
        PodiumDevice { transport: mock as Arc<dyn Transport> }
    }

    #[tokio::test]
    async fn assert_visible_succeeds_immediately() {
        let mock = Arc::new(MockTransport::new());
        mock.set_visible("text:Login", true);
        let device = make_device(mock.clone());
        device.assert_visible(Selector::text("Login")).await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn assert_visible_times_out() {
        let mock = Arc::new(MockTransport::new());
        let device = make_device(mock.clone());
        let result = device.assert_visible(Selector::text("Never")).await;
        assert!(matches!(result, Err(PodiumError::Timeout { .. })));
    }

    #[tokio::test]
    async fn assert_not_visible_succeeds_immediately() {
        let mock = Arc::new(MockTransport::new());
        let device = make_device(mock.clone());
        device.assert_not_visible(Selector::text("Error")).await.unwrap();
    }

    #[tokio::test]
    async fn scroll_until_visible_finds_immediately() {
        let mock = Arc::new(MockTransport::new());
        mock.set_visible("text:Item", true);
        let device = make_device(mock.clone());
        device.scroll_until_visible(Selector::text("Item")).await.unwrap();
        assert!(!mock.calls().iter().any(|c| c.starts_with("swipe")));
    }

    #[tokio::test(start_paused = true)]
    async fn scroll_until_visible_exceeds_max_swipes() {
        let mock = Arc::new(MockTransport::new());
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

    #[tokio::test]
    async fn builder_requires_platform() {
        let result = DeviceBuilder::default().build().await;
        assert!(matches!(result, Err(PodiumError::Transport { .. })));
    }
}
