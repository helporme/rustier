pub mod nested;

#[derive(Clone)]
pub struct Outer {
    pub nested: nested::Nested,
    pub marker: i32,
}

impl Outer {
    // The method hides the field accessor, leaving Nested referenced only by Coerce.
    pub fn nested(&self) -> i32 {
        self.nested.value
    }
}
