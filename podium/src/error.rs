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
#[allow(dead_code)]
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
