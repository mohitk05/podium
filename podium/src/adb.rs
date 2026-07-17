use crate::error::TransportError;
use crate::hierarchy::find_element;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use tokio::process::Command;
use tonic::transport::Channel;

pub(crate) mod proto {
    include!(concat!(env!("OUT_DIR"), "/maestro_android.rs"));
}

use proto::maestro_driver_client::MaestroDriverClient;
use proto::{
    CheckWindowUpdatingRequest, InputTextRequest, LaunchAppRequest, ScreenshotRequest,
    TapRequest, ViewHierarchyRequest,
};

const MAESTRO_RUNNER: &str =
    "dev.mobile.maestro.test/androidx.test.runner.AndroidJUnitRunner";
const STARTUP_TIMEOUT_MS: u64 = 30_000;
const STARTUP_POLL_MS: u64 = 200;

pub(crate) struct AdbTransport {
    serial: Option<String>,
    #[allow(dead_code)]
    port: u16,
    client: MaestroDriverClient<Channel>,
}

impl AdbTransport {
    pub(crate) async fn connect(
        serial: Option<String>,
        port: u16,
    ) -> Result<Self, TransportError> {
        // 1. Forward the port
        adb_cmd(&serial, &["forward", &format!("tcp:{port}"), &format!("tcp:{port}")])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("adb forward: {e}"),
            })?;

        // 2. Start on-device gRPC server in background — don't await exit
        adb_cmd(
            &serial,
            &[
                "shell",
                "am",
                "instrument",
                "-w",
                "-e",
                "class",
                "dev.mobile.maestro.MaestroDriverService#grpcServer",
                "-e",
                "port",
                &port.to_string(),
                MAESTRO_RUNNER,
            ],
        )
        .spawn()
        .map_err(|e| TransportError::OperationFailed {
            reason: format!("am instrument spawn: {e}"),
        })?;

        // 3. Wait for gRPC server to accept TCP connections
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(STARTUP_TIMEOUT_MS);
        loop {
            if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .is_ok()
            {
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err(TransportError::OperationFailed {
                    reason: format!(
                        "Maestro gRPC server did not start on port {port} within {STARTUP_TIMEOUT_MS}ms"
                    ),
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(STARTUP_POLL_MS)).await;
        }

        // 4. Open tonic channel
        let endpoint = format!("http://127.0.0.1:{port}");
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("channel: {e}"),
            })?
            .connect()
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("connect: {e}"),
            })?;

        Ok(Self {
            serial,
            port,
            client: MaestroDriverClient::new(channel),
        })
    }

    fn adb_shell(&self, args: &[&str]) -> Command {
        let mut cmd = adb_cmd(&self.serial, &["shell"]);
        cmd.args(args);
        cmd
    }

    async fn view_hierarchy(&self) -> Result<String, TransportError> {
        let mut client = self.client.clone();
        let resp = client
            .view_hierarchy(ViewHierarchyRequest {})
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("viewHierarchy: {e}"),
            })?;
        Ok(resp.into_inner().hierarchy)
    }
}

fn adb_cmd(serial: &Option<String>, args: &[&str]) -> Command {
    let mut cmd = Command::new("adb");
    if let Some(s) = serial {
        cmd.args(["-s", s.as_str()]);
    }
    cmd.args(args);
    cmd
}

#[async_trait]
impl Transport for AdbTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        if clear_state {
            self.adb_shell(&["pm", "clear", app_id])
                .status()
                .await
                .map_err(|e| TransportError::OperationFailed {
                    reason: format!("pm clear: {e}"),
                })?;
        }
        let mut client = self.client.clone();
        client
            .launch_app(LaunchAppRequest {
                package_name: app_id.to_string(),
                arguments: vec![],
            })
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("launchApp: {e}"),
            })?;
        Ok(())
    }

    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let xml = self.view_hierarchy().await?;
        Ok(find_element(&xml, selector).is_some())
    }

    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        let xml = self.view_hierarchy().await?;
        let bounds =
            find_element(&xml, selector).ok_or_else(|| TransportError::ElementNotFound {
                reason: format!("tap: element not found: {selector:?}"),
            })?;
        let (cx, cy) = bounds.center();
        let mut client = self.client.clone();
        client
            .tap(TapRequest { x: cx, y: cy })
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("tap: {e}"),
            })?;
        Ok(())
    }

    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        let mut client = self.client.clone();
        client
            .input_text(InputTextRequest {
                text: text.to_string(),
            })
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("inputText: {e}"),
            })?;
        Ok(())
    }

    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        self.adb_shell(&["input", "keyevent", "KEYCODE_BACK"])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("hide_keyboard: {e}"),
            })?;
        Ok(())
    }

    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        let args: &[&str] = match direction {
            Direction::Up => &["input", "swipe", "540", "1400", "540", "400", "300"],
            Direction::Down => &["input", "swipe", "540", "400", "540", "1400", "300"],
            Direction::Left => &["input", "swipe", "900", "800", "180", "800", "300"],
            Direction::Right => &["input", "swipe", "180", "800", "900", "800", "300"],
        };
        self.adb_shell(args)
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("swipe: {e}"),
            })?;
        Ok(())
    }

    async fn back(&self) -> Result<(), TransportError> {
        self.adb_shell(&["input", "keyevent", "KEYCODE_BACK"])
            .status()
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("back: {e}"),
            })?;
        Ok(())
    }

    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            let mut client = self.client.clone();
            let updating = client
                .is_window_updating(CheckWindowUpdatingRequest {
                    app_id: String::new(),
                })
                .await
                .map(|r| r.into_inner().is_window_updating)
                .unwrap_or(false);
            if !updating {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(()); // best-effort
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        let mut client = self.client.clone();
        let resp = client
            .screenshot(ScreenshotRequest {})
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("screenshot: {e}"),
            })?;
        let bytes = resp.into_inner().bytes;
        let path = format!("{name}.png");
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| TransportError::OperationFailed {
                reason: format!("write screenshot: {e}"),
            })?;
        Ok(())
    }
}
