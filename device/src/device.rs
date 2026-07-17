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

impl Default for DeviceBuilder {
    fn default() -> Self {
        Self { platform: None }
    }
}

pub struct PodiumDevice {
    pub(crate) transport: Arc<dyn Transport>,
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
            let sleep_ms = 200u64.min(deadline - now);
            tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
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
            let sleep_ms = 200u64.min(deadline - now);
            tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
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
