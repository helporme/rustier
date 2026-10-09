//@ api: wasm, node

pub struct Wrapper<T> {
    pub value: T,
}

impl<T: Clone> Clone for Wrapper<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

impl<T> Wrapper<T> {
    pub fn into_value(self) -> T {
        self.value
    }
}

pub fn concrete(value: Wrapper<String>) -> Wrapper<String> {
    value
}

pub fn bounded<T: Clone>(value: Wrapper<T>) -> Wrapper<T> {
    value
}

pub fn unbounded<T>(value: Wrapper<T>) -> Wrapper<T> {
    value
}
