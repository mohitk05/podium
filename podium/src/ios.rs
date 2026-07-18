use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;

pub(crate) struct IosTransport;

#[async_trait]
impl Transport for IosTransport {
    async fn launch_app(&self, _app_id: &str, _clear_state: bool) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn is_visible(&self, _selector: &Selector) -> Result<bool, TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn tap(&self, _selector: &Selector) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn input_text(&self, _text: &str) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn swipe(&self, _direction: &Direction) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn back(&self) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn wait_for_idle(&self, _timeout_ms: u64) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
    async fn take_screenshot(&self, _name: &str) -> Result<(), TransportError> {
        Err(TransportError::NotSupported {
            reason: "iOS transport not implemented in v1".into(),
        })
    }
}
