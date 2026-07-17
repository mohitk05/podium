use crate::types::Selector;
use quick_xml::{events::Event, Reader};
use std::collections::HashMap;

pub(crate) struct Bounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Bounds {
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

pub(crate) fn find_element(xml: &str, selector: &Selector) -> Option<Bounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut match_count = 0u32;
    loop {
        match reader.read_event().ok()? {
            Event::Empty(e) | Event::Start(e) => {
                let attrs: HashMap<String, String> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(a.value.as_ref()).to_string();
                        (key, val)
                    })
                    .collect();
                if matches_selector(&attrs, selector) {
                    if match_count == selector.index {
                        let bounds_str = attrs.get("bounds")?;
                        return parse_bounds(bounds_str);
                    }
                    match_count += 1;
                }
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

fn matches_selector(attrs: &HashMap<String, String>, selector: &Selector) -> bool {
    if let Some(id) = &selector.id {
        let suffix = format!(":id/{id}");
        return attrs
            .get("resource-id")
            .map(|r| r.ends_with(&suffix))
            .unwrap_or(false);
    }
    if let Some(text) = &selector.text {
        let node_text = attrs.get("text").map(String::as_str).unwrap_or("");
        if text.starts_with('/') && text.ends_with('/') && text.len() > 2 {
            let pattern = &text[1..text.len() - 1];
            return regex::Regex::new(pattern)
                .map(|re| re.is_match(node_text))
                .unwrap_or(false);
        }
        return node_text == text.as_str();
    }
    false
}

fn parse_bounds(s: &str) -> Option<Bounds> {
    // format: "[x1,y1][x2,y2]"
    let s = s.trim().strip_prefix('[')?;
    let (first, rest) = s.split_once(']')?;
    let (x1s, y1s) = first.split_once(',')?;
    let rest = rest.trim_start_matches('[');
    let (second, _) = rest.split_once(']')?;
    let (x2s, y2s) = second.split_once(',')?;
    let x1: u32 = x1s.trim().parse().ok()?;
    let y1: u32 = y1s.trim().parse().ok()?;
    let x2: u32 = x2s.trim().parse().ok()?;
    let y2: u32 = y2s.trim().parse().ok()?;
    Some(Bounds {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Selector;

    const XML_ONE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hierarchy rotation="0">
  <node index="0" text="Login" resource-id="com.example:id/login_btn"
        bounds="[100,200][300,260]" class="android.widget.Button"
        clickable="true" enabled="true" />
</hierarchy>"#;

    const XML_TWO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<hierarchy rotation="0">
  <node index="0" text="Item" resource-id="com.example:id/item_0"
        bounds="[0,0][100,50]" class="android.widget.TextView"
        clickable="true" enabled="true" />
  <node index="1" text="Item" resource-id="com.example:id/item_1"
        bounds="[0,60][100,110]" class="android.widget.TextView"
        clickable="true" enabled="true" />
</hierarchy>"#;

    #[test]
    fn find_by_text_returns_center() {
        let b = find_element(XML_ONE, &Selector::text("Login")).unwrap();
        // bounds [100,200][300,260]: center = (200, 230)
        assert_eq!(b.center(), (200, 230));
    }

    #[test]
    fn find_by_id_suffix() {
        let b = find_element(XML_ONE, &Selector::id("login_btn")).unwrap();
        assert_eq!(b.center(), (200, 230));
    }

    #[test]
    fn find_by_index_selects_nth_match() {
        let b = find_element(XML_TWO, &Selector::text("Item").index(1)).unwrap();
        // bounds [0,60][100,110]: center = (50, 85)
        assert_eq!(b.center(), (50, 85));
    }

    #[test]
    fn find_missing_returns_none() {
        assert!(find_element(XML_ONE, &Selector::text("NotHere")).is_none());
    }

    #[test]
    fn parse_bounds_basic() {
        let b = parse_bounds("[100,200][300,260]").unwrap();
        assert_eq!((b.x, b.y, b.width, b.height), (100, 200, 200, 60));
    }
}
