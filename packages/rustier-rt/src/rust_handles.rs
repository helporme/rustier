// Rustier runtime handles to rust memory, referenced from js code.

use std::rc::Rc;

pub fn into_handle<T>(value: Rc<T>) -> usize {
    Rc::into_raw(value) as usize
}

/// Creates an owned `Rc` without consuming the reference represented by the handle.
///
/// # Safety
///
/// `handle` must originate from `into_handle` for the same `T`. Its allocation
/// must have a live strong reference throughout this call. The caller must
/// obey `Rc`'s single-threaded ownership rules, including preventing concurrent
/// refcount access or release.
pub unsafe fn clone_from_handle<T>(handle: usize) -> Rc<T> {
    let ptr = handle as *const T;

    unsafe {
        Rc::increment_strong_count(ptr);
        Rc::from_raw(ptr)
    }
}

/// Releases one strong reference represented by the handle.
///
/// # Safety
///
/// `handle` must originate from `into_handle` for the same `T` and represent
/// an unreleased strong reference. This call consumes that reference; it must
/// not be released again. The caller must obey `Rc`'s single-threaded ownership
/// rules, including preventing concurrent refcount access or release.
pub unsafe fn drop_handle<T>(handle: usize) {
    unsafe {
        Rc::decrement_strong_count(handle as *const T);
    }
}
