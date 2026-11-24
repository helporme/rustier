//! Low-level handles for WebAssembly environments and JavaScript values.

use std::{marker::PhantomData, rc::Rc};

use super::JsValueType;

// This import-module name must match the key generated in rustier_compiler's codegen/transport/wasm.rs.
#[link(wasm_import_module = "./rustier-wasm-rt.mjs")]
unsafe extern "C" {
    fn __rustier_js_set_error(ptr: *const u8, len: usize);
    fn __rustier_js_new_number(value: f64) -> u32;
    fn __rustier_js_new_i64(value: i64) -> u32;
    fn __rustier_js_new_u64(value: u64) -> u32;
    fn __rustier_js_i64(handle: u32, invalid: *mut u8) -> i64;
    fn __rustier_js_u64(handle: u32, invalid: *mut u8) -> u64;
    fn __rustier_js_new_i128(low: u64, high: i64) -> u32;
    fn __rustier_js_new_u128(low: u64, high: u64) -> u32;
    fn __rustier_js_bigint_two_words(handle: u32, words: *mut [u64; 2]) -> i32;
    fn __rustier_js_new_string(ptr: *const u8, len: usize) -> u32;
    fn __rustier_js_new_boolean(value: bool) -> u32;
    fn __rustier_js_new_symbol(ptr: *const u8, len: usize, has_description: bool) -> u32;
    fn __rustier_js_null() -> u32;
    fn __rustier_js_undefined() -> u32;
    fn __rustier_js_new_object() -> u32;
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

    pub(crate) fn new_i64(&self, value: i64) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_new_i64(value) }, PhantomData)
    }

    pub(crate) fn new_u64(&self, value: u64) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_new_u64(value) }, PhantomData)
    }

    pub fn new_i128(&self, value: i128) -> JsValueHandle {
        let raw = unsafe { __rustier_js_new_i128(value as u64, (value >> 64) as i64) };
        JsValueHandle(raw, PhantomData)
    }

    pub fn new_u128(&self, value: u128) -> JsValueHandle {
        let raw = unsafe { __rustier_js_new_u128(value as u64, (value >> 64) as u64) };
        JsValueHandle(raw, PhantomData)
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

    pub(crate) fn new_symbol(&self, description: Option<&str>) -> JsValueHandle {
        let text = description.unwrap_or_default();
        JsValueHandle(
            unsafe { __rustier_js_new_symbol(text.as_ptr(), text.len(), description.is_some()) },
            PhantomData,
        )
    }

    pub(crate) fn null(&self) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_null() }, PhantomData)
    }

    pub(crate) fn undefined(&self) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_undefined() }, PhantomData)
    }

    pub(crate) fn new_object(&self) -> JsValueHandle {
        JsValueHandle(unsafe { __rustier_js_new_object() }, PhantomData)
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

    pub(crate) fn as_i64(&self) -> Option<i64> {
        let mut invalid = 0;
        let value = unsafe { __rustier_js_i64(self.0, &mut invalid) };
        (invalid == 0).then_some(value)
    }

    pub(crate) fn as_u64(&self) -> Option<u64> {
        let mut invalid = 0;
        let value = unsafe { __rustier_js_u64(self.0, &mut invalid) };
        (invalid == 0).then_some(value)
    }

    pub(super) fn bigint_parts(&self) -> Option<(bool, u128)> {
        let mut words = [0u64; 2];
        let sign = unsafe { __rustier_js_bigint_two_words(self.0, &mut words) };
        (sign >= 0).then(|| {
            (
                sign != 0,
                (u128::from(words[1]) << 64) | u128::from(words[0]),
            )
        })
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
