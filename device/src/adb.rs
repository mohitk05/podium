use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use tokio::process::Command;

const DEVICE_DIR: &str = "/data/local/tmp/podium/cmd";
const RUNNER: &str = "dev.podium.runner.test/androidx.test.runner.AndroidJUnitRunner";
const RESULTS_DIR: &str = "/sdcard/Android/data/dev.podium.runner.test/files/podium/results";

pub(crate) struct AdbTransport {
    pub(crate) serial: Option<String>,
    pub(crate) app_id: String,
}

impl AdbTransport {
    fn adb(&self) -> Command {
        let mut cmd = Command::new("adb");
        if let Some(s) = &self.serial {
            cmd.args(["-s", s]);
        }
        cmd
    }

    async fn run_flow(&self, flow_name: &str, yaml: &str) -> Result<(), TransportError> {
        let flow_file = format!("{DEVICE_DIR}/{flow_name}.yaml");

        // Push the YAML to the device
        let local = self.write_temp_yaml(flow_name, yaml).await?;
        let push_ok = self.adb()
            .args(["push", &local, &flow_file])
            .status().await
            .map(|s| s.success())
            .unwrap_or(false);
        if !push_ok {
            return Err(TransportError::OperationFailed { reason: format!("adb push failed for {flow_name}") });
        }

        // Run the single flow via am instrument
        let output = self.adb()
            .args([
                "shell", "am", "instrument", "-w", "-r",
                "-e", "flowsDir", DEVICE_DIR,
                "-e", "flowFilter", flow_name,
                RUNNER,
            ])
            .output().await
            .map_err(|e| TransportError::OperationFailed { reason: format!("am instrument failed: {e}") })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("INSTRUMENTATION_STATUS_CODE: -2") || stdout.contains("FAILURES!!!") {
            let reason = self.pull_failure_reason(flow_name).await
                .unwrap_or_else(|| "flow failed".into());
            return Err(TransportError::OperationFailed { reason });
        }

        Ok(())
    }

    async fn run_flow_for_bool(&self, flow_name: &str, yaml: &str) -> Result<bool, TransportError> {
        match self.run_flow(flow_name, yaml).await {
            Ok(()) => Ok(true),
            Err(TransportError::OperationFailed { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn write_temp_yaml(&self, name: &str, yaml: &str) -> Result<String, TransportError> {
        let path = std::env::temp_dir().join(format!("podium-{name}.yaml"));
        tokio::fs::write(&path, yaml).await
            .map_err(|e| TransportError::OperationFailed { reason: format!("write temp yaml: {e}") })?;
        Ok(path.to_string_lossy().into_owned())
    }

    async fn pull_failure_reason(&self, flow_name: &str) -> Option<String> {
        let device_path = format!("{RESULTS_DIR}/{flow_name}.json");
        let out = self.adb()
            .args(["shell", "cat", &device_path])
            .output().await.ok()?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        json["steps"].as_array()?.iter().find_map(|s| {
            if s["status"].as_str() == Some("FAILED") {
                s["failure_message"].as_str().map(|m| m.to_string())
            } else {
                None
            }
        })
    }

    fn selector_yaml(s: &Selector) -> String {
        if let Some(text) = &s.text {
            if s.index > 0 {
                format!("    text: {text:?}\n    index: {}", s.index)
            } else {
                format!("    text: {text:?}")
            }
        } else if let Some(id) = &s.id {
            if s.index > 0 {
                format!("    id: {id:?}\n    index: {}", s.index)
            } else {
                format!("    id: {id:?}")
            }
        } else {
            format!("    index: {}", s.index)
        }
    }

    fn flow_header(&self) -> String {
        format!("appId: {}\n---\n", self.app_id)
    }
}

#[async_trait]
impl Transport for AdbTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        let yaml = format!(
            "{}- launchApp:\n    appId: {app_id:?}\n    clearState: {clear_state}\n",
            self.flow_header()
        );
        self.run_flow("launch_app", &yaml).await
    }

    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let sel = Self::selector_yaml(selector);
        let yaml = format!(
            "{}- assertVisible:\n{sel}\n    timeout: 0\n",
            self.flow_header()
        );
        self.run_flow_for_bool("is_visible", &yaml).await
    }

    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        let sel = Self::selector_yaml(selector);
        let yaml = format!("{}- tapOn:\n{sel}\n", self.flow_header());
        self.run_flow("tap", &yaml).await
    }

    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        let yaml = format!("{}- inputText: {text:?}\n", self.flow_header());
        self.run_flow("input_text", &yaml).await
    }

    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        let yaml = format!("{}- hideKeyboard\n", self.flow_header());
        self.run_flow("hide_keyboard", &yaml).await
    }

    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        let dir = match direction {
            Direction::Up => "up",
            Direction::Down => "down",
            Direction::Left => "left",
            Direction::Right => "right",
        };
        let yaml = format!("{}- swipe: {dir:?}\n", self.flow_header());
        self.run_flow("swipe", &yaml).await
    }

    async fn back(&self) -> Result<(), TransportError> {
        let yaml = format!("{}- back\n", self.flow_header());
        self.run_flow("back", &yaml).await
    }

    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        let yaml = format!(
            "{}- waitForAnimationToEnd:\n    timeout: {timeout_ms}\n",
            self.flow_header()
        );
        self.run_flow("wait_for_idle", &yaml).await
    }

    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        let yaml = format!("{}- takeScreenshot: {name:?}\n", self.flow_header());
        self.run_flow("take_screenshot", &yaml).await
    }
}
