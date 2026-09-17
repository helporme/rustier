pub mod nested;

#[derive(Clone)]
pub struct Outer {
    pub nested: nested::Nested,
    pub marker: i32,
}

impl Outer {
    // Keep this method to test that the nested field takes precedence.
    pub fn nested(&self) -> i32 {
        self.nested.value
    }
}
