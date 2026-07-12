use crate::model::{Direction, Selector};
use thiserror::Error;

#[derive(uniffi::Error, Debug, Error)]
pub enum DriverError {
    #[error("Element not found: {reason}")]
    ElementNotFound { reason: String },

    #[error("Operation failed: {reason}")]
    OperationFailed { reason: String },

    #[error("Timeout: {reason}")]
    Timeout { reason: String },
}

/// Driver trait implemented by platform-specific drivers (UIAutomator, XCUITest)
/// All timing and retry semantics live in the engine, not in the driver.
#[uniffi::export(with_foreign)]
pub trait Driver: Send + Sync {
    /// Launch an app by package/bundle ID
    fn launch_app(&self, app_id: String, clear_state: bool) -> Result<(), DriverError>;

    /// Check if an element matching the selector is currently visible
    /// NO waiting - returns immediately
    fn is_visible(&self, selector: Selector) -> Result<bool, DriverError>;

    /// Tap on an element matching the selector
    /// Assumes element is already visible (engine ensures this)
    fn tap(&self, selector: Selector) -> Result<(), DriverError>;

    /// Input text into the currently focused field
    fn input_text(&self, text: String) -> Result<(), DriverError>;

    /// Dismiss the soft keyboard if it is visible
    fn hide_keyboard(&self) -> Result<(), DriverError>;

    /// Swipe in the given direction
    fn swipe(&self, direction: Direction) -> Result<(), DriverError>;

    /// Press the back button
    fn back(&self) -> Result<(), DriverError>;

    /// Wait for UI to become idle (no animations)
    fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), DriverError>;

    /// Take a screenshot and save with the given name
    fn take_screenshot(&self, name: String) -> Result<(), DriverError>;

    /// Get current time in milliseconds (for timing measurements)
    fn now_ms(&self) -> u64;

    /// Sleep for the given duration in milliseconds
    fn sleep_ms(&self, ms: u64);
}

// Re-export for engine
pub use DriverError as Error;
