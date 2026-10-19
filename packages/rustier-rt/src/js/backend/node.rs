use std::{marker::PhantomData, rc::Rc};

use napi_sys::{
    Status, napi_close_handle_scope, napi_create_double, napi_create_object, napi_create_reference,
    napi_delete_reference, napi_env, napi_get_reference_value, napi_handle_scope,
    napi_open_handle_scope, napi_ref, napi_reference_ref, napi_reference_unref, napi_status,
    napi_value,
};

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
