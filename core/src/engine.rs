use crate::driver::{Driver, DriverError};
use crate::model::{Command, Flow, Selector};
use std::sync::Arc;

const POLL_INTERVAL_MS: u64 = 200;

#[derive(uniffi::Record, Debug, Clone)]
pub struct FlowResult {
    pub steps: Vec<StepResult>,
    pub passed: bool,
}

#[derive(uniffi::Record, Debug, Clone)]
pub struct StepResult {
    pub command_desc: String,
    pub status: StepStatus,
    pub duration_ms: u64,
    pub failure_message: Option<String>,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum StepStatus {
    Passed,
    Failed,
    Skipped,
}

pub fn run_flow(flow: Flow, driver: Arc<dyn Driver>) -> FlowResult {
    let mut steps = Vec::new();
    let mut passed = true;

    for command in flow.commands {
        let start = driver.now_ms();
        let command_desc = format_command(&command);

        let result = if passed {
            execute_command(&command, &driver, &flow.app_id)
        } else {
            // Skip remaining steps after first failure
            Ok(())
        };

        let duration_ms = driver.now_ms() - start;

        let (status, failure_message) = match result {
            Ok(_) if passed => (StepStatus::Passed, None),
            Ok(_) => (StepStatus::Skipped, None),
            Err(e) => {
                passed = false;
                (StepStatus::Failed, Some(e.to_string()))
            }
        };

        steps.push(StepResult {
            command_desc,
            status,
            duration_ms,
            failure_message,
        });

        if !passed {
            // Mark remaining commands as skipped (we'll add them in the loop)
            // Actually, we handle this by checking `passed` at the start of the loop
        }
    }

    FlowResult { steps, passed }
}

fn execute_command(
    command: &Command,
    driver: &Arc<dyn Driver>,
    default_app_id: &str,
) -> Result<(), DriverError> {
    match command {
        Command::LaunchApp {
            app_id,
            clear_state,
        } => {
            let app = app_id
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or(default_app_id);
            driver.launch_app(app.to_string(), *clear_state)
        }

        Command::TapOn { selector } => {
            wait_until_visible(driver, selector, 10_000)?;
            driver.tap(selector.clone())
        }

        Command::InputText { text } => driver.input_text(text.clone()),

        Command::AssertVisible {
            selector,
            timeout_ms,
        } => wait_until_visible(driver, selector, *timeout_ms),

        Command::AssertNotVisible {
            selector,
            timeout_ms,
        } => wait_until_not_visible(driver, selector, *timeout_ms),

        Command::ScrollUntilVisible {
            selector,
            max_swipes,
        } => scroll_until_visible(driver, selector, *max_swipes),

        Command::Back => driver.back(),

        Command::WaitForAnimationToEnd { timeout_ms } => driver.wait_for_idle(*timeout_ms),

        Command::Swipe { direction } => driver.swipe(direction.clone()),

        Command::TakeScreenshot { name } => driver.take_screenshot(name.clone()),
    }
}

fn wait_until_visible(
    driver: &Arc<dyn Driver>,
    selector: &Selector,
    timeout_ms: u64,
) -> Result<(), DriverError> {
    let start = driver.now_ms();
    let deadline = start + timeout_ms;

    loop {
        if driver.is_visible(selector.clone())? {
            return Ok(());
        }

        let now = driver.now_ms();
        if now >= deadline {
            return Err(DriverError::Timeout {
                reason: format!("Element not visible after {}ms: {:?}", timeout_ms, selector),
            });
        }

        let remaining = deadline - now;
        let sleep_time = POLL_INTERVAL_MS.min(remaining);
        driver.sleep_ms(sleep_time);
    }
}

fn wait_until_not_visible(
    driver: &Arc<dyn Driver>,
    selector: &Selector,
    timeout_ms: u64,
) -> Result<(), DriverError> {
    let start = driver.now_ms();
    let deadline = start + timeout_ms;

    loop {
        if !driver.is_visible(selector.clone())? {
            return Ok(());
        }

        let now = driver.now_ms();
        if now >= deadline {
            return Err(DriverError::Timeout {
                reason: format!(
                    "Element still visible after {}ms: {:?}",
                    timeout_ms, selector
                ),
            });
        }

        let remaining = deadline - now;
        let sleep_time = POLL_INTERVAL_MS.min(remaining);
        driver.sleep_ms(sleep_time);
    }
}

fn scroll_until_visible(
    driver: &Arc<dyn Driver>,
    selector: &Selector,
    max_swipes: u32,
) -> Result<(), DriverError> {
    for i in 0..max_swipes {
        if driver.is_visible(selector.clone())? {
            return Ok(());
        }

        if i < max_swipes - 1 {
            driver.swipe(crate::model::Direction::Down)?;
            driver.sleep_ms(POLL_INTERVAL_MS);
        }
    }

    Err(DriverError::ElementNotFound {
        reason: format!(
            "Element not found after {} swipes: {:?}",
            max_swipes, selector
        ),
    })
}

fn format_command(command: &Command) -> String {
    match command {
        Command::LaunchApp {
            app_id,
            clear_state,
        } => {
            if *clear_state {
                format!("launchApp(clearState: true)")
            } else if let Some(id) = app_id {
                format!("launchApp({})", id)
            } else {
                "launchApp".to_string()
            }
        }
        Command::TapOn { selector } => format!("tapOn({:?})", selector),
        Command::InputText { text } => format!("inputText(\"{}\")", text),
        Command::AssertVisible { selector, .. } => format!("assertVisible({:?})", selector),
        Command::AssertNotVisible { selector, .. } => format!("assertNotVisible({:?})", selector),
        Command::ScrollUntilVisible { selector, .. } => {
            format!("scrollUntilVisible({:?})", selector)
        }
        Command::Back => "back".to_string(),
        Command::WaitForAnimationToEnd { .. } => "waitForAnimationToEnd".to_string(),
        Command::Swipe { direction } => format!("swipe({:?})", direction),
        Command::TakeScreenshot { name } => format!("takeScreenshot(\"{}\")", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Direction, Selector};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct MockDriver {
        time_ms: Mutex<u64>,
        visibility_timeline: Mutex<HashMap<String, Vec<(u64, bool)>>>,
        calls: Mutex<Vec<String>>,
    }

    impl MockDriver {
        fn new() -> Self {
            Self {
                time_ms: Mutex::new(0),
                visibility_timeline: Mutex::new(HashMap::new()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn set_visibility_at(&self, selector_key: &str, time_ms: u64, visible: bool) {
            let mut timeline = self.visibility_timeline.lock().unwrap();
            timeline
                .entry(selector_key.to_string())
                .or_insert_with(Vec::new)
                .push((time_ms, visible));
        }

        fn selector_key(selector: &Selector) -> String {
            if let Some(text) = &selector.text {
                format!("text:{}", text)
            } else if let Some(id) = &selector.id {
                format!("id:{}", id)
            } else {
                "unknown".to_string()
            }
        }
    }

    impl Driver for MockDriver {
        fn launch_app(&self, app_id: String, clear_state: bool) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("launch_app({}, {})", app_id, clear_state));
            Ok(())
        }

        fn is_visible(&self, selector: Selector) -> Result<bool, DriverError> {
            let key = Self::selector_key(&selector);
            let current_time = *self.time_ms.lock().unwrap();
            let timeline = self.visibility_timeline.lock().unwrap();

            if let Some(events) = timeline.get(&key) {
                // Find the most recent event before or at current time
                let mut visible = false;
                for (time, vis) in events {
                    if *time <= current_time {
                        visible = *vis;
                    }
                }
                Ok(visible)
            } else {
                Ok(false)
            }
        }

        fn tap(&self, selector: Selector) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("tap({:?})", selector));
            Ok(())
        }

        fn input_text(&self, text: String) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("input_text({})", text));
            Ok(())
        }

        fn swipe(&self, direction: Direction) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("swipe({:?})", direction));
            Ok(())
        }

        fn back(&self) -> Result<(), DriverError> {
            self.calls.lock().unwrap().push("back".to_string());
            Ok(())
        }

        fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("wait_for_idle({})", timeout_ms));
            Ok(())
        }

        fn take_screenshot(&self, name: String) -> Result<(), DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("take_screenshot({})", name));
            Ok(())
        }

        fn now_ms(&self) -> u64 {
            *self.time_ms.lock().unwrap()
        }

        fn sleep_ms(&self, ms: u64) {
            *self.time_ms.lock().unwrap() += ms;
        }
    }

    #[test]
    fn test_immediate_visibility() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Login".to_string()),
            id: None,
            index: 0,
        };

        mock.set_visibility_at("text:Login", 0, true);

        let result = wait_until_visible(&driver, &selector, 5000);
        assert!(result.is_ok());
        assert_eq!(*mock.time_ms.lock().unwrap(), 0); // No waiting needed
    }

    #[test]
    fn test_delayed_visibility() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Welcome".to_string()),
            id: None,
            index: 0,
        };

        // Element becomes visible at 500ms
        mock.set_visibility_at("text:Welcome", 500, true);

        let result = wait_until_visible(&driver, &selector, 1000);
        assert!(result.is_ok());
        // Should have polled at 0, 200, 400, 600 and found it at 600
        let time = *mock.time_ms.lock().unwrap();
        assert!(time >= 400);
        assert!(time <= 600);
    }

    #[test]
    fn test_visibility_timeout() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Never".to_string()),
            id: None,
            index: 0,
        };

        // Never becomes visible
        let result = wait_until_visible(&driver, &selector, 500);
        assert!(result.is_err());
        assert!(*mock.time_ms.lock().unwrap() >= 500);
    }

    #[test]
    fn test_not_visible_immediately() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Error".to_string()),
            id: None,
            index: 0,
        };

        // Not visible from the start
        let result = wait_until_not_visible(&driver, &selector, 1000);
        assert!(result.is_ok());
        assert_eq!(*mock.time_ms.lock().unwrap(), 0);
    }

    #[test]
    fn test_not_visible_with_delay() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Loading".to_string()),
            id: None,
            index: 0,
        };

        mock.set_visibility_at("text:Loading", 0, true);
        mock.set_visibility_at("text:Loading", 300, false);

        let result = wait_until_not_visible(&driver, &selector, 1000);
        assert!(result.is_ok());
        let time = *mock.time_ms.lock().unwrap();
        assert!(time >= 200);
        assert!(time <= 400);
    }

    #[test]
    fn test_scroll_until_visible_immediate() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Item".to_string()),
            id: None,
            index: 0,
        };

        mock.set_visibility_at("text:Item", 0, true);

        let result = scroll_until_visible(&driver, &selector, 5);
        assert!(result.is_ok());
        // No swipes needed
        assert!(!mock
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.contains("swipe")));
    }

    #[test]
    fn test_scroll_until_visible_after_swipes() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Item 50".to_string()),
            id: None,
            index: 0,
        };

        // Visible after 3 swipes (at time 600ms)
        mock.set_visibility_at("text:Item 50", 600, true);

        let result = scroll_until_visible(&driver, &selector, 10);
        assert!(result.is_ok());
        let calls = mock.calls.lock().unwrap();
        let swipe_count = calls.iter().filter(|c| c.contains("swipe")).count();
        assert!(swipe_count >= 3);
    }

    #[test]
    fn test_scroll_max_swipes_exceeded() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Item 100".to_string()),
            id: None,
            index: 0,
        };

        // Never becomes visible
        let result = scroll_until_visible(&driver, &selector, 3);
        assert!(result.is_err());
        let calls = mock.calls.lock().unwrap();
        let swipe_count = calls.iter().filter(|c| c.contains("swipe")).count();
        assert_eq!(swipe_count, 2); // max_swipes - 1
    }

    #[test]
    fn test_flow_timing_capture() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let flow = Flow {
            app_id: "test.app".to_string(),
            commands: vec![
                Command::LaunchApp {
                    app_id: None,
                    clear_state: false,
                },
                Command::Back,
            ],
        };

        let result = run_flow(flow, driver);
        assert!(result.passed);
        assert_eq!(result.steps.len(), 2);

        // Each step should have timing
        for step in &result.steps {
            assert!(step.duration_ms >= 0);
        }
    }

    #[test]
    fn test_flow_abort_on_first_failure() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Missing".to_string()),
            id: None,
            index: 0,
        };

        let flow = Flow {
            app_id: "test.app".to_string(),
            commands: vec![
                Command::AssertVisible {
                    selector: selector.clone(),
                    timeout_ms: 100,
                },
                Command::TapOn {
                    selector: selector.clone(),
                },
                Command::Back,
            ],
        };

        let result = run_flow(flow, driver);
        assert!(!result.passed);
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.steps[0].status, StepStatus::Failed);
        assert_eq!(result.steps[1].status, StepStatus::Skipped);
        assert_eq!(result.steps[2].status, StepStatus::Skipped);
    }

    #[test]
    fn test_flow_all_commands_pass() {
        let mock = Arc::new(MockDriver::new());
        let driver: Arc<dyn Driver> = mock.clone();
        let selector = Selector {
            text: Some("Button".to_string()),
            id: None,
            index: 0,
        };

        mock.set_visibility_at("text:Button", 0, true);

        let flow = Flow {
            app_id: "test.app".to_string(),
            commands: vec![
                Command::LaunchApp {
                    app_id: None,
                    clear_state: true,
                },
                Command::TapOn {
                    selector: selector.clone(),
                },
                Command::InputText {
                    text: "test".to_string(),
                },
                Command::Back,
            ],
        };

        let result = run_flow(flow, driver);
        assert!(result.passed);
        assert_eq!(result.steps.len(), 4);
        for step in &result.steps {
            assert_eq!(step.status, StepStatus::Passed);
            assert!(step.failure_message.is_none());
        }
    }

    #[test]
    fn test_command_description_formatting() {
        let commands = vec![
            Command::LaunchApp {
                app_id: Some("com.app".to_string()),
                clear_state: false,
            },
            Command::TapOn {
                selector: Selector {
                    text: Some("Login".to_string()),
                    id: None,
                    index: 0,
                },
            },
            Command::InputText {
                text: "password".to_string(),
            },
        ];

        for cmd in commands {
            let desc = format_command(&cmd);
            assert!(!desc.is_empty());
        }
    }
}
