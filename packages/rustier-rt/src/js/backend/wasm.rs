//! Low-level handles for WebAssembly environments and JavaScript values.

use std::{marker::PhantomData, rc::Rc};

use super::JsValueType;

#[link(wasm_import_module = "./rustier-wasm-rt.mjs")]
unsafe extern "C" {
    fn __rustier_js_new_number(value: f64) -> u32;
    fn __rustier_js_new_string(ptr: *const u8, len: usize) -> u32;
    fn __rustier_js_new_boolean(value: bool) -> u32;
    fn __rustier_js_null() -> u32;
    fn __rustier_js_undefined() -> u32;
    fn __rustier_js_new_object() -> u32;
    fn __rustier_js_type_of(handle: u32) -> JsValueType;
    fn __rustier_js_number(handle: u32) -> f64;
    fn __rustier_js_boolean(handle: u32) -> bool;
    fn __rustier_js_string_encode(handle: u32) -> u32;
    fn __rustier_js_buffer_length(handle: u32) -> f64;
    fn __rustier_js_buffer_copy(handle: u32, ptr: *mut u8, len: usize);
    fn __rustier_js_get(handle: u32, key: *const u8, len: usize, failed: *mut u32) -> u32;
    fn __rustier_js_set(
        handle: u32,
        key: *const u8,
        len: usize,
        value: u32,
        failed: *mut u32,
    ) -> u32;
    fn __rustier_js_clone_ref(handle: u32) -> u32;
    fn __rustier_js_drop_ref(handle: u32);
}

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

    pub fn new_string(&self, value: &str) -> ValueHandle {
        ValueHandle(
            unsafe { __rustier_js_new_string(value.as_ptr(), value.len()) },
            PhantomData,
        )
    }

    pub fn new_boolean(&self, value: bool) -> ValueHandle {
        ValueHandle(unsafe { __rustier_js_new_boolean(value) }, PhantomData)
    }

    pub fn null(&self) -> ValueHandle {
        ValueHandle(unsafe { __rustier_js_null() }, PhantomData)
    }

    pub fn undefined(&self) -> ValueHandle {
        ValueHandle(unsafe { __rustier_js_undefined() }, PhantomData)
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

    pub fn type_of(&self) -> JsValueType {
        unsafe { __rustier_js_type_of(self.0) }
    }

    pub fn as_f64(&self) -> Option<f64> {
        (self.type_of() == JsValueType::Number).then(|| unsafe { __rustier_js_number(self.0) })
    }

    pub fn as_bool(&self) -> Option<bool> {
        (self.type_of() == JsValueType::Boolean).then(|| unsafe { __rustier_js_boolean(self.0) })
    }

    pub fn as_string(&self) -> Option<String> {
        if self.type_of() != JsValueType::String {
            return None;
        }

        let encoded = Self(unsafe { __rustier_js_string_encode(self.0) }, PhantomData);
        let length = unsafe { __rustier_js_buffer_length(encoded.0) } as usize;
        let mut bytes = vec![0; length];

        unsafe {
            __rustier_js_buffer_copy(encoded.0, bytes.as_mut_ptr(), bytes.len());
        }

        drop(encoded);

        Some(String::from_utf8(bytes).expect("JavaScript encoder returned invalid UTF-8"))
    }

    pub fn get(&self, key: &str) -> Result<Self, Self> {
        let mut failed = 0;
        let raw = unsafe { __rustier_js_get(self.0, key.as_ptr(), key.len(), &mut failed) };
        let value = Self(raw, PhantomData);

        if failed == 0 { Ok(value) } else { Err(value) }
    }

    pub fn set(&self, key: &str, value: &Self) -> Result<bool, Self> {
        let mut failed = 0;
        let raw =
            unsafe { __rustier_js_set(self.0, key.as_ptr(), key.len(), value.0, &mut failed) };

        if failed == 0 {
            Ok(raw != 0)
        } else {
            Err(Self(raw, PhantomData))
        }
    }

    pub fn into_raw(self) -> u32 {
        let raw = self.0;

        std::mem::forget(self);

        raw
    }

    /// # Safety
    ///
    /// `raw` must be a live handle transferred by [`Self::into_raw`] in this
    /// Wasm instance. Each transferred owner may be reconstructed only once,
    /// on the owning JavaScript thread.
    pub unsafe fn from_raw(raw: u32) -> Self {
        Self(raw, PhantomData)
    }

    /// # Safety
    ///
    /// `raw` must refer to a live value in this Wasm instance for the duration
    /// of the call. The call must be made on its owning JavaScript thread.
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
