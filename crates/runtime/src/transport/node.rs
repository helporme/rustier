//! Low-level handles for Node.js environments and values.

use std::{
    any::TypeId,
    cmp::Ordering,
    ffi::{CStr, c_void},
    hash::{Hash, Hasher},
    marker::PhantomData,
    ptr,
    rc::Rc,
};

use napi_sys::{
    Status, ValueType, napi_call_function, napi_close_handle_scope, napi_create_double,
    napi_create_object, napi_create_reference, napi_create_string_utf8, napi_create_symbol,
    napi_create_type_error, napi_delete_reference, napi_env, napi_get_and_clear_last_exception,
    napi_get_array_length, napi_get_boolean, napi_get_element, napi_get_global,
    napi_get_named_property, napi_get_null, napi_get_reference_value, napi_get_undefined,
    napi_get_value_bool, napi_get_value_double, napi_get_value_string_utf8, napi_handle_scope,
    napi_is_exception_pending, napi_new_instance, napi_open_handle_scope, napi_ref,
    napi_reference_ref, napi_reference_unref, napi_status, napi_strict_equals, napi_typeof,
    napi_value, napi_valuetype,
};

use super::JsValueType;

/// A non-owning handle to a Node-API environment.
#[derive(Debug, Clone, Copy)]
pub struct JsEnvHandle(usize, *mut c_void, PhantomData<Rc<()>>);

impl PartialEq for JsEnvHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for JsEnvHandle {}
impl PartialOrd for JsEnvHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for JsEnvHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl Hash for JsEnvHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[repr(C)]
struct CallbackData<T> {
    kind: TypeId,
    state: T,
}

impl JsEnvHandle {
    /// Allocates data shared by the addon's entry points until environment cleanup.
    ///
    /// # Safety
    /// `env` must be the live Node environment on its owning thread.
    pub unsafe fn allocate_static_data<T: 'static>(
        env: napi_env,
        state: T,
    ) -> Result<*mut c_void, napi_status> {
        unsafe extern "C" fn cleanup<T>(data: *mut c_void) {
            drop(unsafe { Box::from_raw(data.cast::<CallbackData<T>>()) });
        }

        let data = Box::into_raw(Box::new(CallbackData {
            kind: TypeId::of::<T>(),
            state,
        }))
        .cast();
        let status = unsafe { napi_sys::napi_add_env_cleanup_hook(env, Some(cleanup::<T>), data) };
        if status != Status::napi_ok {
            unsafe { cleanup::<T>(data) };
            return Err(status);
        }
        Ok(data)
    }

    /// Borrows the cached callback data.
    ///
    /// # Safety
    /// The borrow must end before Node starts cleaning up this environment.
    pub unsafe fn static_data<T: 'static>(&self) -> Option<&T> {
        // All CallbackData allocations start with the same repr(C) type tag.
        let kind = unsafe { self.1.cast::<TypeId>().as_ref() }?;
        (*kind == TypeId::of::<T>()).then(|| unsafe { &(*self.1.cast::<CallbackData<T>>()).state })
    }

    /// # Safety
    /// `raw` must be a live Node environment on its owning thread; `data` must
    /// be null or callback data allocated for it by `allocate_static_data`.
    /// Neither handle may be used after environment cleanup begins.
    pub unsafe fn from_raw(raw: napi_env, data: *mut c_void) -> Self {
        Self(raw as usize, data, PhantomData)
    }

    pub fn into_raw(self) -> usize {
        self.0
    }

    pub fn new_number(&self, value: f64) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe {
            napi_create_double(env, value, output)
        })
    }

    pub fn new_string(&self, value: &str) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe {
            napi_create_string_utf8(env, value.as_ptr().cast(), value.len() as isize, output)
        })
    }

    pub fn new_boolean(&self, value: bool) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe {
            napi_get_boolean(env, value, output)
        })
    }

    pub fn new_symbol(&self, description: Option<&str>) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| {
            let description = description.map_or(ptr::null_mut(), |text| {
                call(|result| unsafe {
                    napi_create_string_utf8(env, text.as_ptr().cast(), text.len() as isize, result)
                })
            });

            unsafe { napi_create_symbol(env, description, output) }
        })
    }

    pub fn null(&self) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe { napi_get_null(env, output) })
    }

    pub fn undefined(&self) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe {
            napi_get_undefined(env, output)
        })
    }

    pub fn new_object(&self) -> JsValueHandle {
        JsValueHandle::new(*self, |env, output| unsafe {
            napi_create_object(env, output)
        })
    }

    pub fn throw_error(&self, error: super::TransportError) {
        let env = self.into_raw() as napi_env;
        let message = error.as_ref();
        let message = call(|output| unsafe {
            napi_create_string_utf8(env, message.as_ptr().cast(), message.len() as isize, output)
        });
        let exception = call(|output| unsafe {
            napi_sys::napi_create_error(env, ptr::null_mut(), message, output)
        });
        let name = call(|output| unsafe {
            napi_create_string_utf8(env, c"RustError".as_ptr(), 9, output)
        });
        let status =
            unsafe { napi_sys::napi_set_named_property(env, exception, c"name".as_ptr(), name) };

        if status != Status::napi_ok {
            panic!("Could not name RustError");
        }

        let status = unsafe { napi_sys::napi_throw(env, exception) };

        if status != Status::napi_ok {
            panic!("Could not throw RustError");
        }
    }
}

#[derive(Debug)]
pub struct JsValueHandle {
    raw: napi_ref,
    env: JsEnvHandle,
    _marker: PhantomData<Rc<()>>,
}

/// A strong reference to a generated JavaScript function, bound once per addon.
pub struct NodeFunction {
    raw: napi_ref,
    env: JsEnvHandle,
}

impl NodeFunction {
    /// Roots `value` directly as a function, without a JavaScript wrapper object.
    pub fn bind(value: JsValueHandle) -> Result<Self, super::TransportError> {
        value.with_value(|env, function| {
            if call(|out| unsafe { napi_typeof(env, function, out) }) != ValueType::napi_function {
                return Err(super::TransportError::Static(
                    "Expected a JavaScript function import",
                ));
            }
            let raw = call(|out| unsafe { napi_create_reference(env, function, 1, out) });
            Ok(Self {
                raw,
                env: value.env,
            })
        })
    }

    /// Calls the bound function with `undefined` as its receiver. The raw
    /// function and argument values live only within this call's handle scope.
    pub fn call<const N: usize>(
        &self,
        args: [JsValueHandle; N],
    ) -> Result<JsValueHandle, JsValueHandle> {
        let env = self.env.0 as napi_env;
        let _scope = HandleScope {
            env,
            raw: call(|out| unsafe { napi_open_handle_scope(env, out) }),
        };
        let mut raw = [ptr::null_mut(); N];
        for (slot, arg) in raw.iter_mut().zip(&args) {
            if arg.env.0 != self.env.0 {
                return Err(property_error(
                    self.env,
                    "JsValue belongs to another JavaScript environment",
                ));
            }
            *slot = arg.value(env);
        }
        let function = call(|out| unsafe { napi_get_reference_value(env, self.raw, out) });
        let this = call(|out| unsafe { napi_get_undefined(env, out) });
        let result = checked(self.env, |out| unsafe {
            napi_call_function(env, this, function, N, raw.as_ptr(), out)
        })?;
        Ok(unsafe { JsValueHandle::from_napi_value(result as usize, self.env) })
    }
}

impl Drop for NodeFunction {
    fn drop(&mut self) {
        unsafe { napi_delete_reference(self.env.0 as napi_env, self.raw) };
    }
}

impl JsValueHandle {
    fn new(js: JsEnvHandle, create: impl FnOnce(napi_env, *mut napi_value) -> napi_status) -> Self {
        let env = js.into_raw() as napi_env;
        let _scope = HandleScope {
            env,
            raw: call(|output| unsafe { napi_open_handle_scope(env, output) }),
        };
        let value = call(|output| create(env, output));
        // Node-API references cannot hold primitives. A private wrapper keeps any
        // JavaScript value alive without changing its identity or representation.
        let wrapper = call(|output| unsafe { napi_create_object(env, output) });
        let property = napi_sys::napi_property_descriptor {
            utf8name: c"value".as_ptr(),
            name: ptr::null_mut(),
            method: None,
            getter: None,
            setter: None,
            value,
            attributes: napi_sys::PropertyAttributes::default,
            data: ptr::null_mut(),
        };
        // Define an own data property rather than invoking a possible inherited
        // setter on Object.prototype.
        let status = unsafe { napi_sys::napi_define_properties(env, wrapper, 1, &property) };
        if status != Status::napi_ok {
            panic!("Could not define the JavaScript value wrapper: Node-API status {status}");
        }
        let reference = call(|output| unsafe { napi_create_reference(env, wrapper, 1, output) });

        Self {
            raw: reference,
            env: js,
            _marker: PhantomData,
        }
    }

    pub fn env(&self) -> JsEnvHandle {
        self.env
    }

    fn with_value<T>(&self, read: impl FnOnce(napi_env, napi_value) -> T) -> T {
        let env = self.env.into_raw() as napi_env;
        let _scope = HandleScope {
            env,
            raw: call(|output| unsafe { napi_open_handle_scope(env, output) }),
        };
        let value = self.value(env);

        read(env, value)
    }

    fn value(&self, env: napi_env) -> napi_value {
        let wrapper = call(|output| unsafe { napi_get_reference_value(env, self.raw, output) });
        call(|output| unsafe { napi_get_named_property(env, wrapper, c"value".as_ptr(), output) })
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

    pub fn to_string(&self) -> Option<String> {
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

    pub fn strict_eq(&self, other: &Self) -> bool {
        if self.env != other.env {
            return false;
        }

        self.with_value(|env, value| {
            let other = other.value(env);

            call(|output| unsafe { napi_strict_equals(env, value, other, output) })
        })
    }

    pub fn soft_eq(&self, other: &Self) -> bool {
        if self.env != other.env {
            return false;
        }

        if let (Some(left), Some(right)) = (self.as_f64(), other.as_f64()) {
            return (left.is_nan() && right.is_nan()) || left.to_bits() == right.to_bits();
        }

        self.strict_eq(other)
    }

    fn reflect<'a>(
        &self,
        env: napi_env,
        target: napi_value,
        operation: &CStr,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<napi_value, Self> {
        let mut arguments = vec![target];
        arguments.extend(self.arguments(env, args)?);

        let global = checked(self.env, |output| unsafe { napi_get_global(env, output) })?;
        let reflect = checked(self.env, |output| unsafe {
            napi_get_named_property(env, global, c"Reflect".as_ptr(), output)
        })?;
        let method = checked(self.env, |output| unsafe {
            napi_get_named_property(env, reflect, operation.as_ptr(), output)
        })?;

        checked(self.env, |output| unsafe {
            napi_call_function(
                env,
                reflect,
                method,
                arguments.len(),
                arguments.as_ptr(),
                output,
            )
        })
    }

    pub fn get(&self, key: &Self) -> Result<Self, Self> {
        self.with_value(|env, target| {
            let result = self.reflect(env, target, c"get", [key])?;

            Ok(unsafe { Self::from_napi_value(result as usize, self.env) })
        })
    }

    fn reflect_bool<'a>(
        &self,
        operation: &CStr,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<bool, Self> {
        self.with_value(|env, target| {
            let result = self.reflect(env, target, operation, args)?;

            checked(self.env, |output| unsafe {
                napi_get_value_bool(env, result, output)
            })
        })
    }

    pub fn set(&self, key: &Self, value: &Self) -> Result<bool, Self> {
        self.reflect_bool(c"set", [key, value])
    }

    pub fn has(&self, key: &Self) -> Result<bool, Self> {
        self.reflect_bool(c"has", [key])
    }

    pub fn delete(&self, key: &Self) -> Result<bool, Self> {
        self.reflect_bool(c"deleteProperty", [key])
    }

    pub fn define_property(&self, key: &Self, descriptor: &Self) -> Result<bool, Self> {
        self.reflect_bool(c"defineProperty", [key, descriptor])
    }

    pub fn get_own_property_descriptor(&self, key: &Self) -> Result<Self, Self> {
        self.with_value(|env, target| {
            let result = self.reflect(env, target, c"getOwnPropertyDescriptor", [key])?;

            Ok(unsafe { Self::from_napi_value(result as usize, self.env) })
        })
    }

    pub fn own_keys(&self) -> Result<Vec<Self>, Self> {
        self.with_value(|env, target| {
            let keys = self.reflect(env, target, c"ownKeys", [])?;
            let length = checked(self.env, |output| unsafe {
                napi_get_array_length(env, keys, output)
            })?;

            (0..length)
                .map(|index| {
                    let key = checked(self.env, |output| unsafe {
                        napi_get_element(env, keys, index, output)
                    })?;

                    Ok(unsafe { Self::from_napi_value(key as usize, self.env) })
                })
                .collect()
        })
    }

    fn arguments<'a>(
        &self,
        env: napi_env,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Vec<napi_value>, Self> {
        args.into_iter()
            .map(|value| {
                if value.env != self.env {
                    return Err(property_error(
                        self.env,
                        "JsValue belongs to another JavaScript environment",
                    ));
                }

                Ok(value.value(env))
            })
            .collect()
    }

    fn invoke(
        &self,
        env: napi_env,
        function: napi_value,
        this: napi_value,
        args: &[napi_value],
    ) -> Result<Self, Self> {
        let result = checked(self.env, |output| unsafe {
            napi_call_function(env, this, function, args.len(), args.as_ptr(), output)
        })?;

        Ok(unsafe { Self::from_napi_value(result as usize, self.env) })
    }

    fn apply_with<'a>(
        &self,
        this: Option<&Self>,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, Self> {
        if this.is_some_and(|value| value.env != self.env) {
            return Err(property_error(
                self.env,
                "JsValue belongs to another JavaScript environment",
            ));
        }

        self.with_value(|env, function| {
            let args = self.arguments(env, args)?;
            let this = match this {
                Some(value) => value.value(env),
                None => checked(self.env, |output| unsafe {
                    napi_get_undefined(env, output)
                })?,
            };

            self.invoke(env, function, this, &args)
        })
    }

    pub fn apply<'a>(&self, args: impl IntoIterator<Item = &'a Self>) -> Result<Self, Self> {
        self.apply_with(None, args)
    }

    pub fn apply_as<'a>(
        &self,
        this: &Self,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, Self> {
        self.apply_with(Some(this), args)
    }

    pub fn call<'a>(
        &self,
        method: &Self,
        args: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, Self> {
        self.with_value(|env, target| {
            let args = self.arguments(env, args)?;
            let function = self.reflect(env, target, c"get", [method])?;

            self.invoke(env, function, target, &args)
        })
    }

    pub fn construct<'a>(&self, args: impl IntoIterator<Item = &'a Self>) -> Result<Self, Self> {
        self.with_value(|env, constructor| {
            let args = self.arguments(env, args)?;
            let result = checked(self.env, |output| unsafe {
                napi_new_instance(env, constructor, args.len(), args.as_ptr(), output)
            })?;

            Ok(unsafe { Self::from_napi_value(result as usize, self.env) })
        })
    }

    /// # Safety
    ///
    /// `raw` must be a valid `napi_value` belonging to `env` in an open handle
    /// scope on the current JavaScript thread.
    pub unsafe fn from_napi_value(raw: usize, env: JsEnvHandle) -> Self {
        Self::new(env, |_, output| {
            unsafe {
                *output = raw as napi_value;
            }

            Status::napi_ok
        })
    }

    /// # Safety
    ///
    /// A handle scope for this environment must be open on the current thread.
    /// The returned value must not be used after that scope closes.
    pub unsafe fn into_napi_value(self) -> usize {
        let env = self.env.into_raw() as napi_env;
        let value = self.value(env);

        value as usize
    }

    pub fn into_raw(self) -> usize {
        self.into_raw_parts().0
    }

    pub fn into_raw_parts(self) -> (usize, JsEnvHandle) {
        let parts = (self.raw as usize, self.env);

        std::mem::forget(self);

        parts
    }

    /// # Safety
    ///
    /// `raw` must come from [`Self::into_raw`] or [`Self::into_raw_parts`], with
    /// the matching, live `env`. Each transferred owner may be reconstructed
    /// only once, on the environment's JavaScript thread.
    pub unsafe fn from_raw_parts(raw: usize, env: JsEnvHandle) -> Self {
        Self {
            raw: raw as napi_ref,
            env,
            _marker: PhantomData,
        }
    }
}

impl Clone for JsValueHandle {
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

impl Drop for JsValueHandle {
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

fn property_error(js: JsEnvHandle, message: &str) -> JsValueHandle {
    JsValueHandle::new(js, |env, output| {
        let message = call(|result| unsafe {
            napi_create_string_utf8(env, message.as_ptr().cast(), message.len() as isize, result)
        });

        unsafe { napi_create_type_error(env, ptr::null_mut(), message, output) }
    })
}

fn checked<T: Default>(
    js: JsEnvHandle,
    operation: impl FnOnce(*mut T) -> napi_status,
) -> Result<T, JsValueHandle> {
    let mut output = T::default();
    let status = operation(&mut output);

    if status == Status::napi_ok {
        return Ok(output);
    }

    let env = js.into_raw() as napi_env;
    let pending = call(|result| unsafe { napi_is_exception_pending(env, result) });

    if pending {
        let exception = call(|result| unsafe { napi_get_and_clear_last_exception(env, result) });

        return Err(unsafe { JsValueHandle::from_napi_value(exception as usize, js) });
    }

    Err(property_error(
        js,
        &format!("Node-API operation failed with status {status}"),
    ))
}

fn call<T: Default>(operation: impl FnOnce(*mut T) -> napi_status) -> T {
    let mut output = T::default();
    let status = operation(&mut output);

    if status != Status::napi_ok {
        panic!("Node-API operation failed with status {status}");
    }

    output
}
