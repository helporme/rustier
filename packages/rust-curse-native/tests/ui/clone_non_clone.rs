//@ api: wasm, node

pub struct NotClone;

impl NotClone {
    pub fn consume(self) {}
}

pub fn moved(value: NotClone) {
    drop(value);
}

pub struct Wrapper<T>(pub T);

impl<T: Clone> Clone for Wrapper<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub fn moved_generic(value: Wrapper<NotClone>) {
    drop(value);
}
