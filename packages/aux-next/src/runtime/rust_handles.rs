use std::{
    any::Any,
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

type Value = Option<Box<dyn Any>>;

#[derive(Clone)]
pub struct RustHandle(Rc<RwLock<Value>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleError {
    Moved,
    TypeMismatch,
    BorrowConflict,
}

impl fmt::Display for HandleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Moved => "Passed Rust value was moved elsewhere",
            Self::TypeMismatch => "Rust handle has an incompatible type",
            Self::BorrowConflict => "Rust value is already borrowed incompatibly",
        })
    }
}

impl std::error::Error for HandleError {}

impl RustHandle {
    pub fn new<T: Any>(value: T) -> Self {
        Self(Rc::new(RwLock::new(Some(Box::new(value)))))
    }

    pub fn into_raw(self) -> usize {
        Rc::into_raw(self.0) as usize
    }

    pub unsafe fn clone_from_raw(raw: usize) -> Self {
        let ptr = raw as *const RwLock<Value>;

        unsafe {
            Rc::increment_strong_count(ptr);
            Self(Rc::from_raw(ptr))
        }
    }

    pub unsafe fn drop_raw(raw: usize) {
        unsafe {
            Rc::decrement_strong_count(raw as *const RwLock<Value>);
        }
    }

    pub fn take<T: Any>(&self) -> Result<T, HandleError> {
        let mut guard = self
            .0
            .try_write()
            .map_err(|_| HandleError::BorrowConflict)?;
        let value = guard.as_ref().ok_or(HandleError::Moved)?;

        if !value.is::<T>() {
            return Err(HandleError::TypeMismatch);
        }

        let Some(value) = guard.take() else {
            unreachable!()
        };
        let Ok(value) = value.downcast::<T>() else {
            unreachable!()
        };

        Ok(*value)
    }

    pub fn borrow<T: Any>(&self) -> Result<HandleRef<'_, T>, HandleError> {
        let guard = self.0.try_read().map_err(|_| HandleError::BorrowConflict)?;
        let value = guard.as_ref().ok_or(HandleError::Moved)?;

        if !value.is::<T>() {
            return Err(HandleError::TypeMismatch);
        }

        Ok(HandleRef {
            guard,
            marker: PhantomData,
        })
    }

    pub fn borrow_mut<T: Any>(&self) -> Result<HandleMut<'_, T>, HandleError> {
        let guard = self
            .0
            .try_write()
            .map_err(|_| HandleError::BorrowConflict)?;
        let value = guard.as_ref().ok_or(HandleError::Moved)?;

        if !value.is::<T>() {
            return Err(HandleError::TypeMismatch);
        }

        Ok(HandleMut {
            guard,
            marker: PhantomData,
        })
    }
}

pub struct HandleRef<'a, T> {
    guard: RwLockReadGuard<'a, Value>,
    marker: PhantomData<T>,
}

impl<T: Any> Deref for HandleRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        let Some(value) = self.guard.as_ref().and_then(|value| value.downcast_ref()) else {
            unreachable!()
        };

        value
    }
}

pub struct HandleMut<'a, T> {
    guard: RwLockWriteGuard<'a, Value>,
    marker: PhantomData<T>,
}

impl<T: Any> Deref for HandleMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        let Some(value) = self.guard.as_ref().and_then(|value| value.downcast_ref()) else {
            unreachable!()
        };

        value
    }
}

impl<T: Any> DerefMut for HandleMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        let Some(value) = self.guard.as_mut().and_then(|value| value.downcast_mut()) else {
            unreachable!()
        };

        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn moves_owned_values_once() {
        let handle = RustHandle::new(vec![1_u32, 2]);
        let alias = handle.clone();

        assert_eq!(handle.take::<String>(), Err(HandleError::TypeMismatch));
        assert_eq!(handle.take::<Vec<u32>>(), Ok(vec![1, 2]));
        assert_eq!(alias.take::<Vec<u32>>(), Err(HandleError::Moved));
        assert!(matches!(
            alias.borrow::<Vec<u32>>(),
            Err(HandleError::Moved)
        ));
        assert!(matches!(
            alias.borrow_mut::<Vec<u32>>(),
            Err(HandleError::Moved)
        ));
    }

    #[test]
    fn enforces_borrows_without_blocking() {
        let handle = RustHandle::new(42_u32);
        let first = handle.borrow::<u32>().unwrap();
        let second = handle.borrow::<u32>().unwrap();

        assert_eq!(*first, 42);
        assert_eq!(*second, 42);
        assert_eq!(handle.take::<u32>(), Err(HandleError::BorrowConflict));
        assert!(matches!(
            handle.borrow_mut::<u32>(),
            Err(HandleError::BorrowConflict)
        ));
        drop((first, second));

        let mut mutable = handle.borrow_mut::<u32>().unwrap();
        *mutable = 43;
        assert!(matches!(
            handle.borrow::<u32>(),
            Err(HandleError::BorrowConflict)
        ));
        drop(mutable);
        assert_eq!(handle.take::<u32>(), Ok(43));
    }

    #[test]
    fn releases_raw_ownership_after_the_last_reference() {
        struct Tracked(Rc<Cell<u32>>);

        impl Drop for Tracked {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let raw = RustHandle::new(Tracked(drops.clone())).into_raw();
        let borrowed = unsafe { RustHandle::clone_from_raw(raw) };

        unsafe { RustHandle::drop_raw(raw) };
        assert_eq!(drops.get(), 0);
        drop(borrowed);
        assert_eq!(drops.get(), 1);

        let raw = RustHandle::new(Tracked(drops.clone())).into_raw();
        let moved = unsafe { RustHandle::clone_from_raw(raw) };
        drop(moved.take::<Tracked>().unwrap());
        assert_eq!(drops.get(), 2);
        drop(moved);
        unsafe { RustHandle::drop_raw(raw) };
        assert_eq!(drops.get(), 2);
    }
}
