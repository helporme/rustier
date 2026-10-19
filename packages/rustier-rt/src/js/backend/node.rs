use std::{marker::PhantomData, ptr, rc::Rc};

use napi_sys::{
    Status, ValueType, napi_close_handle_scope, napi_create_double, napi_create_object,
    napi_create_reference, napi_create_string_utf8, napi_delete_reference, napi_env,
    napi_get_boolean, napi_get_null, napi_get_reference_value, napi_get_undefined,
    napi_get_value_bool, napi_get_value_double, napi_get_value_string_utf8, napi_handle_scope,
    napi_open_handle_scope, napi_ref, napi_reference_ref, napi_reference_unref, napi_status,
    napi_typeof, napi_value, napi_valuetype,
};

use super::JsValueType;

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct EnvHandle(usize, PhantomData<Rc<()>>);

impl EnvHandle {
    /// # Safety
    ///
    /// `raw` must be a live Node-API environment on the current thread.
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self(raw, PhantomData)
    }

    pub fn into_raw(self) -> usize {
        self.0
    }

    pub fn new_number(&self, value: f64) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe {
            napi_create_double(env, value, output)
        })
    }

    pub fn new_string(&self, value: &str) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe {
            napi_create_string_utf8(env, value.as_ptr().cast(), value.len() as isize, output)
        })
    }

    pub fn new_boolean(&self, value: bool) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe {
            napi_get_boolean(env, value, output)
        })
    }

    pub fn null(&self) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe { napi_get_null(env, output) })
    }

    pub fn undefined(&self) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe {
            napi_get_undefined(env, output)
        })
    }

    pub fn new_object(&self) -> ValueHandle {
        ValueHandle::new(*self, |env, output| unsafe {
            napi_create_object(env, output)
        })
    }
}

#[derive(Debug)]
pub struct ValueHandle {
    raw: napi_ref,
    env: EnvHandle,
    _marker: PhantomData<Rc<()>>,
}

impl ValueHandle {
    fn new(js: EnvHandle, create: impl FnOnce(napi_env, *mut napi_value) -> napi_status) -> Self {
        let env = js.into_raw() as napi_env;
        let _scope = HandleScope {
            env,
            raw: call(|output| unsafe { napi_open_handle_scope(env, output) }),
        };
        let value = call(|output| create(env, output));
        let reference = call(|output| unsafe { napi_create_reference(env, value, 1, output) });

        Self {
            raw: reference,
            env: js,
            _marker: PhantomData,
        }
    }

    pub fn env(&self) -> EnvHandle {
        self.env
    }

    fn with_value<T>(&self, read: impl FnOnce(napi_env, napi_value) -> T) -> T {
        let env = self.env.into_raw() as napi_env;
        let _scope = HandleScope {
            env,
            raw: call(|output| unsafe { napi_open_handle_scope(env, output) }),
        };
        let value = call(|output| unsafe { napi_get_reference_value(env, self.raw, output) });

        read(env, value)
    }

    fn read<T>(
        &self,
        expected: napi_valuetype,
        read: impl FnOnce(napi_env, napi_value) -> T,
    ) -> Option<T> {
        self.with_value(|env, value| {
            let actual = call(|output| unsafe { napi_typeof(env, value, output) });

            if actual != expected {
                return None;
            }

            Some(read(env, value))
        })
    }

    pub fn type_of(&self) -> JsValueType {
        self.with_value(|env, value| {
            match call(|output| unsafe { napi_typeof(env, value, output) }) {
                ValueType::napi_string => JsValueType::String,
                ValueType::napi_number => JsValueType::Number,
                ValueType::napi_bigint => JsValueType::Bigint,
                ValueType::napi_boolean => JsValueType::Boolean,
                ValueType::napi_symbol => JsValueType::Symbol,
                ValueType::napi_undefined => JsValueType::Undefined,
                ValueType::napi_null => JsValueType::Null,
                ValueType::napi_object | ValueType::napi_external => JsValueType::Object,
                ValueType::napi_function => JsValueType::Function,
                _ => unreachable!("unexpected Node-API value type"),
            }
        })
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.read(ValueType::napi_number, |env, value| {
            call(|output| unsafe { napi_get_value_double(env, value, output) })
        })
    }

    pub fn as_bool(&self) -> Option<bool> {
        self.read(ValueType::napi_boolean, |env, value| {
            call(|output| unsafe { napi_get_value_bool(env, value, output) })
        })
    }

    pub fn as_string(&self) -> Option<String> {
        self.read(ValueType::napi_string, |env, value| {
            let length: usize = call(|output| unsafe {
                napi_get_value_string_utf8(env, value, ptr::null_mut(), 0, output)
            });
            let capacity = length
                .checked_add(1)
                .expect("JavaScript string is too long");
            let mut bytes = vec![0; capacity];
            let written = call(|output| unsafe {
                napi_get_value_string_utf8(
                    env,
                    value,
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    output,
                )
            });

            bytes.truncate(written);

            String::from_utf8(bytes).expect("Node-API returned invalid UTF-8")
        })
    }

    pub unsafe fn from_napi_value(raw: usize, env: EnvHandle) -> Self {
        Self::new(env, |_, output| {
            unsafe {
                *output = raw as napi_value;
            }

            Status::napi_ok
        })
    }

    pub unsafe fn into_napi_value(self) -> usize {
        let env = self.env.into_raw() as napi_env;
        let value = call(|output| unsafe { napi_get_reference_value(env, self.raw, output) });

        value as usize
    }

    pub fn into_raw(self) -> usize {
        self.into_raw_parts().0
    }

    pub fn into_raw_parts(self) -> (usize, usize) {
        let parts = (self.raw as usize, self.env.into_raw());

        std::mem::forget(self);

        parts
    }

    /// # Safety
    ///
    /// `raw` must own one strong reference in the live `env`, transferred
    /// with `into_raw` or `into_raw_parts`. Each transferred owner must be
    /// reconstructed only once, on the environment's JavaScript thread.
    pub unsafe fn from_raw_parts(raw: usize, env: usize) -> Self {
        Self {
            raw: raw as napi_ref,
            env: unsafe { EnvHandle::from_raw(env) },
            _marker: PhantomData,
        }
    }
}

impl Clone for ValueHandle {
    fn clone(&self) -> Self {
        let env = self.env.into_raw() as napi_env;
        let _: u32 = call(|output| unsafe { napi_reference_ref(env, self.raw, output) });

        Self {
            raw: self.raw,
            env: self.env,
            _marker: PhantomData,
        }
    }
}

impl Drop for ValueHandle {
    fn drop(&mut self) {
        let env = self.env.into_raw() as napi_env;
        let mut count = 0;
        let status = unsafe { napi_reference_unref(env, self.raw, &mut count) };

        if status == Status::napi_ok && count == 0 {
            unsafe {
                napi_delete_reference(env, self.raw);
            }
        }
    }
}

struct HandleScope {
    env: napi_env,
    raw: napi_handle_scope,
}

impl Drop for HandleScope {
    fn drop(&mut self) {
        unsafe {
            napi_close_handle_scope(self.env, self.raw);
        }
    }
}

fn call<T: Default>(operation: impl FnOnce(*mut T) -> napi_status) -> T {
    let mut output = T::default();
    let status = operation(&mut output);

    if status != Status::napi_ok {
        panic!("Node-API operation failed with status {status}");
    }

    output
}
