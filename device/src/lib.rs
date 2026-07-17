//! `podium-device` — async Rust API for driving a mobile device via Podium's test runner.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use podium_device::{DeviceBuilder, Platform, Selector};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), podium_device::PodiumError> {
//! let device = DeviceBuilder::default()
//!     .platform(Platform::Android { serial: None })
//!     .app_id("com.example.myapp")
//!     .build()
//!     .await?;
//!
//! device.launch_app("com.example.myapp", true).await?;
//! device.assert_visible(Selector::text("Welcome")).await?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod types;

mod transport;
mod device;
mod adb;
mod ios;

#[cfg(feature = "mock")]
pub mod mock;

pub use device::{DeviceBuilder, Platform, PodiumDevice};
pub use error::PodiumError;
pub use types::{Direction, Selector};

#[cfg(feature = "mock")]
pub use mock::MockTransport;
