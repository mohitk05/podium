use serde::{Deserialize, Serialize};

#[derive(uniffi::Record, Debug, Clone, Serialize, Deserialize)]
pub struct Flow {
    pub app_id: String,
    pub commands: Vec<Command>,
}

#[derive(uniffi::Enum, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    LaunchApp {
        #[serde(skip_serializing_if = "Option::is_none")]
        app_id: Option<String>,
        #[serde(default)]
        clear_state: bool,
    },
    TapOn {
        selector: Selector,
    },
    InputText {
        text: String,
    },
    AssertVisible {
        selector: Selector,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    AssertNotVisible {
        selector: Selector,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    ScrollUntilVisible {
        selector: Selector,
        #[serde(default = "default_max_swipes")]
        max_swipes: u32,
    },
    Back,
    WaitForAnimationToEnd {
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    Swipe {
        direction: Direction,
    },
    TakeScreenshot {
        name: String,
    },
    HideKeyboard,
}

#[derive(uniffi::Record, Debug, Clone, Serialize, Deserialize)]
pub struct Selector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub index: u32,
}

#[derive(uniffi::Enum, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

fn default_timeout() -> u64 {
    10_000
}

fn default_max_swipes() -> u32 {
    20
}

impl Selector {
    pub fn is_regex(&self) -> bool {
        self.text
            .as_ref()
            .map(|t| t.starts_with('/') && t.ends_with('/'))
            .unwrap_or(false)
    }

    pub fn text_pattern(&self) -> Option<String> {
        self.text.as_ref().map(|t| {
            if self.is_regex() {
                t[1..t.len() - 1].to_string()
            } else {
                t.clone()
            }
        })
    }
}
