#[derive(Clone)]
pub struct Counter {
    pub value: i32,
}

impl Counter {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn add(&mut self, amount: i32) {
        self.value += amount;
    }

    pub fn into_value(self) -> i32 {
        self.value
    }

    pub fn sum(&self, other: &Self) -> i32 {
        self.value + other.value
    }

    pub fn clone_of(other: &Self) -> Self {
        other.clone()
    }

    pub fn replace_with(&mut self, other: &Self) {
        self.value = other.value;
    }

    pub fn constructor(&self) -> bool {
        self.value >= 0
    }

    pub fn prototype() -> bool {
        true
    }

    fn private_method(&self) -> bool {
        true
    }

    pub(crate) fn crate_method(&self) -> bool {
        true
    }

    pub(super) fn parent_method(&self) -> bool {
        true
    }
}
