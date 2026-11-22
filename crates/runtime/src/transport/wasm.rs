//! Low-level handles for WebAssembly environments and JavaScript values.

use std::{marker::PhantomData, rc::Rc};

use super::JsValueType;

// This import-module name must match the key generated in rustier_compiler's codegen/transport/wasm.rs.
#[link(wasm_import_module = "./rustier-wasm-rt.mjs")]
unsafe extern "C" {
    fn __rustier_js_set_error(ptr: *const u8, len: usize);
    fn __rustier_js_new_number(value: f64) -> u32;
    fn __rustier_js_new_string(ptr: *const u8, len: usize) -> u32;
    fn __rustier_js_new_boolean(value: bool) -> u32;
    fn __rustier_js_undefined() -> u32;
    fn __rustier_js_type_of(handle: u32) -> JsValueType;
    fn __rustier_js_number(handle: u32) -> f64;
    fn __rustier_js_boolean(handle: u32) -> bool;
    fn __rustier_js_string_encode(handle: u32) -> u32;
    fn __rustier_js_buffer_length(handle: u32) -> f64;
    fn __rustier_js_buffer_copy(handle: u32, ptr: *mut u8, len: usize);
    fn __rustier_js_exception_message(handle: u32) -> u32;
    fn __rustier_js_clone_ref(handle: u32) -> u32;
    fn __rustier_js_drop_ref(handle: u32);
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct JsEnvHandle(PhantomData<Rc<()>>);

impl JsEnvHandle {
    /// # Safety
    ///
    /// `raw` must be zero and used on the owning JavaScript thread.
    pub unsafe fn from_raw(_raw: usize) -> Self {
        Self(PhantomData)
    }

    pub fn into_raw(self) -> usize {
        0
    }

    pub(crate) fn new_number(&self, value: f64) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_new_number(value) }, PhantomData)
    }

    pub fn new_string(&self, value: &str) -> JsValueHandle {
        JsValueHandle(
            unsafe { __rustier_js_new_string(value.as_ptr(), value.len()) },
            PhantomData,
        )
    }

    pub(crate) fn new_boolean(&self, value: bool) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_new_boolean(value) }, PhantomData)
    }

    pub(crate) fn undefined(&self) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_undefined() }, PhantomData)
    }

    pub fn set_error(&self, error: super::TransportError) {
        let message = error.as_ref();

        unsafe { __rustier_js_set_error(message.as_ptr(), message.len()) };
    }
}

#[derive(Debug)]
pub struct JsValueHandle(u32, PhantomData<Rc<()>>);

impl JsValueHandle {
    /// Borrows the value's slot in this Wasm instance without transferring it
    /// to JavaScript. The handle must stay alive while the slot is in use.
    pub fn as_raw(&self) -> u32 {
        self.0
    }

    pub fn env(&self) -> JsEnvHandle {
        JsEnvHandle(PhantomData)
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

    pub fn to_string(&self) -> Option<String> {
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

    pub(crate) fn exception_message(&self) -> Option<String> {
        Self(
            unsafe { __rustier_js_exception_message(self.0) },
            PhantomData,
        )
        .to_string()
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

impl Clone for JsValueHandle {
    fn clone(&self) -> Self {
        Self(unsafe { __rustier_js_clone_ref(self.0) }, PhantomData)
    }
}

impl Drop for JsValueHandle {
    fn drop(&mut self) {
        unsafe {
            __rustier_js_drop_ref(self.0);
        }
    }
}
