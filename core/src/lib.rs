pub mod driver;
pub mod engine;
pub mod model;
pub mod parser;

use std::collections::HashMap;
use std::sync::Arc;

#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum PodiumError {
    #[error("Parse error: {details}")]
    ParseError { details: String },
}

uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn core_version() -> String {
    format!("podium-core {}", env!("CARGO_PKG_VERSION"))
}

#[uniffi::export]
pub fn parse_flow(yaml: String, env: HashMap<String, String>) -> Result<model::Flow, PodiumError> {
    parser::parse_flow(&yaml, &env).map_err(|e| PodiumError::ParseError {
        details: e.to_string(),
    })
}

#[uniffi::export]
pub fn run_flow(flow: model::Flow, driver: Arc<dyn driver::Driver>) -> engine::FlowResult {
    engine::run_flow(flow, driver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_version() {
        let version = core_version();
        assert!(version.starts_with("podium-core"));
    }
}
