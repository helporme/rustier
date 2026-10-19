use std::{marker::PhantomData, rc::Rc};

/// A particular JavaScript environment, for example a web worker.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnv(usize, PhantomData<Rc<()>>); // for WASM the underlying address is always 0

impl JsEnv {
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw, PhantomData)
    }
}
