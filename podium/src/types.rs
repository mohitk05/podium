#[derive(Debug, Clone)]
pub struct Selector {
    pub(crate) text: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) index: u32,
}

impl Selector {
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: Some(t.into()),
            id: None,
            index: 0,
        }
    }

    pub fn id(id: impl Into<String>) -> Self {
        Self {
            text: None,
            id: Some(id.into()),
            index: 0,
        }
    }

    pub fn index(mut self, i: u32) -> Self {
        self.index = i;
        self
    }
}

#[derive(Debug, Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
