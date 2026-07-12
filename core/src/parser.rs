use crate::model::{Command, Direction, Flow, Selector};
use serde_yaml::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Missing required field 'appId' in flow header")]
    MissingAppId,

    #[error("Invalid flow format: expected two YAML documents separated by '---'")]
    InvalidFormat,

    #[error("Unknown command '{0}' at position {1}")]
    UnknownCommand(String, usize),

    #[error("Invalid command format for '{0}': {1}")]
    InvalidCommandFormat(String, String),
}

pub fn parse_flow(yaml: &str, env: &HashMap<String, String>) -> Result<Flow, ParseError> {
    let yaml = substitute_env(yaml, env);

    // Split on --- to get header and commands
    let parts: Vec<&str> = yaml.split("\n---\n").collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidFormat);
    }

    // Parse header
    let header: Value = serde_yaml::from_str(parts[0])?;
    let app_id = header
        .get("appId")
        .and_then(|v| v.as_str())
        .ok_or(ParseError::MissingAppId)?
        .to_string();

    // Parse commands
    let commands_yaml: Vec<Value> = serde_yaml::from_str(parts[1])?;
    let mut commands = Vec::new();

    for (idx, cmd_value) in commands_yaml.iter().enumerate() {
        let cmd = parse_command(cmd_value, idx)?;
        commands.push(cmd);
    }

    Ok(Flow { app_id, commands })
}

fn substitute_env(yaml: &str, env: &HashMap<String, String>) -> String {
    let mut result = yaml.to_string();
    for (key, value) in env {
        let pattern = format!("${{{}}}", key);
        result = result.replace(&pattern, value);
    }
    result
}

fn parse_command(value: &Value, position: usize) -> Result<Command, ParseError> {
    // Support bare string commands: `- back` (no colon required)
    if let Some(s) = value.as_str() {
        return match s {
            "back" => Ok(Command::Back),
            _ => Err(ParseError::UnknownCommand(s.to_string(), position)),
        };
    }

    let obj = value.as_mapping().ok_or_else(|| {
        ParseError::InvalidCommandFormat("unknown".to_string(), "expected mapping".to_string())
    })?;

    if obj.len() != 1 {
        return Err(ParseError::InvalidCommandFormat(
            "unknown".to_string(),
            "expected single command key".to_string(),
        ));
    }

    let (cmd_name, cmd_value) = obj.iter().next().unwrap();
    let cmd_name_str = cmd_name.as_str().ok_or_else(|| {
        ParseError::InvalidCommandFormat(
            "unknown".to_string(),
            "command name must be string".to_string(),
        )
    })?;

    match cmd_name_str {
        "launchApp" => parse_launch_app(cmd_value),
        "tapOn" => parse_tap_on(cmd_value),
        "inputText" => parse_input_text(cmd_value),
        "assertVisible" => parse_assert_visible(cmd_value),
        "assertNotVisible" => parse_assert_not_visible(cmd_value),
        "scrollUntilVisible" => parse_scroll_until_visible(cmd_value),
        "back" => Ok(Command::Back),
        "waitForAnimationToEnd" => parse_wait_for_animation(cmd_value),
        "swipe" => parse_swipe(cmd_value),
        "takeScreenshot" => parse_take_screenshot(cmd_value),
        _ => Err(ParseError::UnknownCommand(
            cmd_name_str.to_string(),
            position,
        )),
    }
}

fn parse_launch_app(value: &Value) -> Result<Command, ParseError> {
    if value.is_null() {
        return Ok(Command::LaunchApp {
            app_id: None,
            clear_state: false,
        });
    }

    let app_id = value
        .get("appId")
        .and_then(|v| v.as_str())
        .map(String::from);
    let clear_state = value
        .get("clearState")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(Command::LaunchApp {
        app_id,
        clear_state,
    })
}

fn parse_tap_on(value: &Value) -> Result<Command, ParseError> {
    let selector = parse_selector(value)?;
    Ok(Command::TapOn { selector })
}

fn parse_input_text(value: &Value) -> Result<Command, ParseError> {
    let text = value
        .as_str()
        .ok_or_else(|| {
            ParseError::InvalidCommandFormat("inputText".to_string(), "expected string".to_string())
        })?
        .to_string();
    Ok(Command::InputText { text })
}

fn parse_assert_visible(value: &Value) -> Result<Command, ParseError> {
    let selector = parse_selector(value)?;
    let timeout_ms = value
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(10_000);
    Ok(Command::AssertVisible {
        selector,
        timeout_ms,
    })
}

fn parse_assert_not_visible(value: &Value) -> Result<Command, ParseError> {
    let selector = parse_selector(value)?;
    let timeout_ms = value
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(10_000);
    Ok(Command::AssertNotVisible {
        selector,
        timeout_ms,
    })
}

fn parse_scroll_until_visible(value: &Value) -> Result<Command, ParseError> {
    let selector = if let Some(element) = value.get("element") {
        parse_selector(element)?
    } else {
        parse_selector(value)?
    };

    let max_swipes = value
        .get("maxSwipes")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(20);

    Ok(Command::ScrollUntilVisible {
        selector,
        max_swipes,
    })
}

fn parse_wait_for_animation(value: &Value) -> Result<Command, ParseError> {
    let timeout_ms = if value.is_null() {
        10_000
    } else {
        value
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000)
    };
    Ok(Command::WaitForAnimationToEnd { timeout_ms })
}

fn parse_swipe(value: &Value) -> Result<Command, ParseError> {
    let direction_str = value
        .as_str()
        .or_else(|| value.get("direction").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            ParseError::InvalidCommandFormat("swipe".to_string(), "expected direction".to_string())
        })?;

    let direction = match direction_str.to_lowercase().as_str() {
        "up" => Direction::Up,
        "down" => Direction::Down,
        "left" => Direction::Left,
        "right" => Direction::Right,
        _ => {
            return Err(ParseError::InvalidCommandFormat(
                "swipe".to_string(),
                format!("unknown direction '{}'", direction_str),
            ))
        }
    };

    Ok(Command::Swipe { direction })
}

fn parse_take_screenshot(value: &Value) -> Result<Command, ParseError> {
    let name = value
        .as_str()
        .ok_or_else(|| {
            ParseError::InvalidCommandFormat(
                "takeScreenshot".to_string(),
                "expected string".to_string(),
            )
        })?
        .to_string();
    Ok(Command::TakeScreenshot { name })
}

fn parse_selector(value: &Value) -> Result<Selector, ParseError> {
    // Shorthand: just a string means text selector
    if let Some(text) = value.as_str() {
        return Ok(Selector {
            text: Some(text.to_string()),
            id: None,
            index: 0,
        });
    }

    // Full form: object with text, id, index
    let text = value.get("text").and_then(|v| v.as_str()).map(String::from);
    let id = value.get("id").and_then(|v| v.as_str()).map(String::from);
    let index = value
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);

    Ok(Selector { text, id, index })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_flow() {
        let yaml = r#"
appId: dev.podium.sample
---
- launchApp:
- tapOn: "Log in"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        assert_eq!(flow.app_id, "dev.podium.sample");
        assert_eq!(flow.commands.len(), 2);
    }

    #[test]
    fn test_missing_app_id() {
        let yaml = r#"
name: test
---
- tapOn: "button"
"#;
        let result = parse_flow(yaml, &HashMap::new());
        assert!(matches!(result, Err(ParseError::MissingAppId)));
    }

    #[test]
    fn test_invalid_format_no_separator() {
        let yaml = r#"
appId: dev.podium.sample
- tapOn: "button"
"#;
        let result = parse_flow(yaml, &HashMap::new());
        assert!(matches!(result, Err(ParseError::InvalidFormat)));
    }

    #[test]
    fn test_unknown_command() {
        let yaml = r#"
appId: dev.podium.sample
---
- unknownCommand: "test"
"#;
        let result = parse_flow(yaml, &HashMap::new());
        assert!(matches!(result, Err(ParseError::UnknownCommand(_, _))));
    }

    #[test]
    fn test_tap_on_shorthand() {
        let yaml = r#"
appId: dev.podium.sample
---
- tapOn: "Log in"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::TapOn { selector } => {
                assert_eq!(selector.text.as_ref().unwrap(), "Log in");
                assert!(selector.id.is_none());
            }
            _ => panic!("Expected TapOn command"),
        }
    }

    #[test]
    fn test_tap_on_with_id() {
        let yaml = r#"
appId: dev.podium.sample
---
- tapOn:
    id: "username"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::TapOn { selector } => {
                assert_eq!(selector.id.as_ref().unwrap(), "username");
                assert!(selector.text.is_none());
            }
            _ => panic!("Expected TapOn command"),
        }
    }

    #[test]
    fn test_input_text() {
        let yaml = r#"
appId: dev.podium.sample
---
- inputText: "podium"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::InputText { text } => {
                assert_eq!(text, "podium");
            }
            _ => panic!("Expected InputText command"),
        }
    }

    #[test]
    fn test_assert_visible_shorthand() {
        let yaml = r#"
appId: dev.podium.sample
---
- assertVisible: "Welcome"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::AssertVisible {
                selector,
                timeout_ms,
            } => {
                assert_eq!(selector.text.as_ref().unwrap(), "Welcome");
                assert_eq!(*timeout_ms, 10_000);
            }
            _ => panic!("Expected AssertVisible command"),
        }
    }

    #[test]
    fn test_scroll_until_visible_with_element() {
        let yaml = r#"
appId: dev.podium.sample
---
- scrollUntilVisible:
    element: "Item 50"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::ScrollUntilVisible {
                selector,
                max_swipes,
            } => {
                assert_eq!(selector.text.as_ref().unwrap(), "Item 50");
                assert_eq!(*max_swipes, 20);
            }
            _ => panic!("Expected ScrollUntilVisible command"),
        }
    }

    #[test]
    fn test_regex_text_selector() {
        let selector = Selector {
            text: Some("/Item \\d+/".to_string()),
            id: None,
            index: 0,
        };
        assert!(selector.is_regex());
        assert_eq!(selector.text_pattern().unwrap(), "Item \\d+");
    }

    #[test]
    fn test_non_regex_text_selector() {
        let selector = Selector {
            text: Some("Item 50".to_string()),
            id: None,
            index: 0,
        };
        assert!(!selector.is_regex());
        assert_eq!(selector.text_pattern().unwrap(), "Item 50");
    }

    #[test]
    fn test_env_substitution() {
        let yaml = r#"
appId: ${APP_ID}
---
- inputText: "${USERNAME}"
"#;
        let mut env = HashMap::new();
        env.insert("APP_ID".to_string(), "dev.podium.sample".to_string());
        env.insert("USERNAME".to_string(), "podium".to_string());

        let flow = parse_flow(yaml, &env).unwrap();
        assert_eq!(flow.app_id, "dev.podium.sample");
        match &flow.commands[0] {
            Command::InputText { text } => {
                assert_eq!(text, "podium");
            }
            _ => panic!("Expected InputText command"),
        }
    }

    #[test]
    fn test_launch_app_with_clear_state() {
        let yaml = r#"
appId: dev.podium.sample
---
- launchApp:
    clearState: true
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::LaunchApp {
                app_id,
                clear_state,
            } => {
                assert!(app_id.is_none());
                assert!(*clear_state);
            }
            _ => panic!("Expected LaunchApp command"),
        }
    }

    #[test]
    fn test_swipe_shorthand() {
        let yaml = r#"
appId: dev.podium.sample
---
- swipe: "down"
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::Swipe { direction } => {
                assert!(matches!(direction, Direction::Down));
            }
            _ => panic!("Expected Swipe command"),
        }
    }

    #[test]
    fn test_timeout_defaults() {
        let yaml = r#"
appId: dev.podium.sample
---
- assertVisible: "test"
- waitForAnimationToEnd:
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::AssertVisible { timeout_ms, .. } => {
                assert_eq!(*timeout_ms, 10_000);
            }
            _ => panic!("Expected AssertVisible command"),
        }
        match &flow.commands[1] {
            Command::WaitForAnimationToEnd { timeout_ms } => {
                assert_eq!(*timeout_ms, 10_000);
            }
            _ => panic!("Expected WaitForAnimationToEnd command"),
        }
    }

    #[test]
    fn test_selector_with_index() {
        let yaml = r#"
appId: dev.podium.sample
---
- tapOn:
    text: "Button"
    index: 2
"#;
        let flow = parse_flow(yaml, &HashMap::new()).unwrap();
        match &flow.commands[0] {
            Command::TapOn { selector } => {
                assert_eq!(selector.text.as_ref().unwrap(), "Button");
                assert_eq!(selector.index, 2);
            }
            _ => panic!("Expected TapOn command"),
        }
    }
}
