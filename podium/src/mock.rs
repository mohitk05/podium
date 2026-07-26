use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{Direction, Selector};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

/// In-memory transport for use in Bombadil tests.
///
/// Control visibility via `set_visible`; inspect interactions via `calls`.
pub struct MockTransport {
    visibility: Mutex<HashMap<String, bool>>,
    calls: Mutex<Vec<String>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            visibility: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Mark a selector key visible or not. Key format: `"text:Login"` or `"id:btn_ok"`.
    pub fn set_visible(&self, key: &str, visible: bool) {
        self.visibility.lock().unwrap().insert(key.into(), visible);
    }

    /// Return a snapshot of all transport calls recorded so far.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn selector_key(s: &Selector) -> String {
        if let Some(t) = &s.text {
            format!("text:{t}")
        } else if let Some(id) = &s.id {
            format!("id:{id}")
        } else {
            "unknown".into()
        }
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn launch_app(&self, app_id: &str, clear_state: bool) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("launch_app({app_id},{clear_state})"));
        Ok(())
    }

    async fn is_visible(&self, selector: &Selector) -> Result<bool, TransportError> {
        let key = Self::selector_key(selector);
        let visible = self
            .visibility
            .lock()
            .unwrap()
            .get(&key)
            .copied()
            .unwrap_or(false);
        Ok(visible)
    }

    async fn tap(&self, selector: &Selector) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("tap({})", Self::selector_key(selector)));
        Ok(())
    }

    async fn input_text(&self, text: &str) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("input_text({text})"));
        Ok(())
    }

    async fn hide_keyboard(&self) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push("hide_keyboard".into());
        Ok(())
    }

    async fn swipe(&self, direction: &Direction) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("swipe({direction:?})"));
        Ok(())
    }

    async fn back(&self) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push("back".into());
        Ok(())
    }

    async fn wait_for_idle(&self, timeout_ms: u64) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("wait_for_idle({timeout_ms})"));
        Ok(())
    }

    async fn take_screenshot(&self, name: &str) -> Result<(), TransportError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("take_screenshot({name})"));
        Ok(())
    }

    async fn view_hierarchy(&self) -> Result<String, TransportError> {
        self.calls.lock().unwrap().push("view_hierarchy".into());
        Ok("<hierarchy />".into())
    }

    async fn tap_at(&self, x: u32, y: u32) -> Result<(), TransportError> {
        self.calls.lock().unwrap().push(format!("tap_at({x},{y})"));
        Ok(())
    }

    async fn foreground_package(&self) -> Result<Option<String>, TransportError> {
        Ok(None)
    }
}
