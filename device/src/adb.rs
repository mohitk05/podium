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
