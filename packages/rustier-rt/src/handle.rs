use std::{marker::PhantomData, mem::ManuallyDrop, rc::Rc};

// Each live handle owns one strong reference to an allocation from Rc::into_raw.
// Private fields prevent safe callers from forging pointers or ownership.
// PhantomData preserves Rc<T>'s ownership and !Send/!Sync semantics.
pub struct Handle<T> {
    ptr: *const T,
    owner: PhantomData<Rc<T>>,
}

impl<T> Handle<T> {
    pub fn into_rc(self) -> Rc<T> {
        let handle = ManuallyDrop::new(self);

        // SAFETY: The handle owns a live reference from Rc::into_raw.
        // ManuallyDrop prevents its destructor from releasing that reference,
        // so reconstructing the Rc transfers ownership without a double release.
        unsafe { Rc::from_raw(handle.ptr) }
    }

    pub fn clone_rc(&self) -> Rc<T> {
        // SAFETY: Borrowing self keeps its strong reference alive. Incrementing
        // the count creates a separate reference for the reconstructed Rc,
        // leaving the handle's reference untouched.
        unsafe {
            Rc::increment_strong_count(self.ptr);

            Rc::from_raw(self.ptr)
        }
    }
}

impl<T> From<Rc<T>> for Handle<T> {
    fn from(value: Rc<T>) -> Self {
        Self {
            ptr: Rc::into_raw(value),
            owner: PhantomData,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        self.clone_rc().into()
    }
}

impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        // SAFETY: The handle still owns its reference from Rc::into_raw.
        // Reconstructing and dropping it releases exactly that reference.
        unsafe {
            drop(Rc::from_raw(self.ptr));
        }
    }
}
