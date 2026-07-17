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
}
