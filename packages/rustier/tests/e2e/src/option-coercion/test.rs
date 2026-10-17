#[derive(Clone)]
pub struct Preferences {
    pub number: Option<i32>,
    pub enabled: Option<bool>,
    pub label: Option<String>,
    pub nested: Option<Option<i32>>,
}

impl Preferences {
    pub fn new(number: Option<i32>) -> Self {
        Self {
            number,
            enabled: None,
            label: None,
            nested: None,
        }
    }

    pub fn from_number(number: Option<i32>) -> Self {
        Self::new(number)
    }

    pub fn replace_number(&mut self, number: Option<i32>) -> Option<i32> {
        std::mem::replace(&mut self.number, number)
    }

    pub fn number_or(&self, fallback: i32) -> i32 {
        self.number.unwrap_or(fallback)
    }

    pub fn replace_nested(&mut self, nested: Option<Option<i32>>) {
        self.nested = nested;
    }

    pub fn nested_or(&self, fallback: i32) -> i32 {
        self.nested.flatten().unwrap_or(fallback)
    }
}

#[derive(Clone)]
pub struct Override(pub Option<i32>);

#[derive(Clone)]
pub enum Setting {
    Named { value: Option<i32> },
    Tuple(Option<i32>),
}
