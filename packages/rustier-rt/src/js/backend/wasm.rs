use std::{marker::PhantomData, rc::Rc};

#[link(wasm_import_module = "./rustier-wasm-rt.mjs")]
unsafe extern "C" {
    fn __rustier_js_new_number(value: f64) -> u32;
    fn __rustier_js_new_object() -> u32;
    fn __rustier_js_clone_ref(handle: u32) -> u32;
    fn __rustier_js_drop_ref(handle: u32);
}

/// Wasm doesn't have environments like Node.js addons so we only need this
/// handle to prevent Send/Sync auto traits on `JsEnv` and to have compatible api with
/// node's backend.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct EnvHandle(PhantomData<Rc<()>>);

impl EnvHandle {
    /// # Safety
    ///
    /// `raw` must be zero and used on the owning JavaScript thread.
    pub unsafe fn from_raw(_raw: usize) -> Self {
        Self(PhantomData)
    }

    pub fn into_raw(self) -> usize {
        0
    }

    pub fn new_number(&self, value: f64) -> ValueHandle {
        ValueHandle(unsafe { __rustier_js_new_number(value) }, PhantomData)
    }

    pub fn new_object(&self) -> ValueHandle {
        ValueHandle(unsafe { __rustier_js_new_object() }, PhantomData)
    }
}

#[derive(Debug)]
pub struct ValueHandle(u32, PhantomData<Rc<()>>);

impl ValueHandle {
    pub fn env(&self) -> EnvHandle {
        EnvHandle(PhantomData)
    }

    pub fn into_raw(self) -> u32 {
        let raw = self.0;

        std::mem::forget(self);

        raw
    }

    /// # Safety
    ///
    /// `raw` must be a live owned slot from this Wasm instance, transferred
    /// with `into_raw`. Each transferred owner must be reconstructed only once,
    /// on the owning JavaScript thread.
    pub unsafe fn from_raw(raw: u32) -> Self {
        Self(raw, PhantomData)
    }

    pub unsafe fn clone_from_raw(raw: u32) -> Self {
        Self(unsafe { __rustier_js_clone_ref(raw) }, PhantomData)
    }
}

impl Clone for ValueHandle {
    fn clone(&self) -> Self {
        Self(unsafe { __rustier_js_clone_ref(self.0) }, PhantomData)
    }
}

impl Drop for ValueHandle {
    fn drop(&mut self) {
        unsafe {
            __rustier_js_drop_ref(self.0);
        }
    }
}
