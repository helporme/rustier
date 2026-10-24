use std::{any::Any, cell::RefCell, collections::BTreeMap, marker::PhantomData, rc::Rc};

#[derive(Default)]
struct HandleTable {
    next: u32,
    values: BTreeMap<u32, Box<dyn Any>>,
}

thread_local! {
    static HANDLES: RefCell<HandleTable> = RefCell::new(HandleTable::default());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleError {
    InvalidHandle,
    TypeMismatch,
    DuplicateMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct RustHandle {
    raw: u32,
    marker: PhantomData<Rc<()>>,
}

impl RustHandle {
    pub fn new<T: Any>(value: T) -> Self {
        HANDLES.with_borrow_mut(|table| {
            let raw = table.next;
            table.next = raw.checked_add(1).expect("Rust handle IDs exhausted");
            table.values.insert(raw, Box::new(value));

            Self::from_raw(raw)
        })
    }

    pub fn from_raw(raw: u32) -> Self {
        Self {
            raw,
            marker: PhantomData,
        }
    }

    pub fn into_raw(self) -> u32 {
        self.raw
    }

    pub fn validate<T: Any>(self) -> Result<(), HandleError> {
        HANDLES.with_borrow(|table| {
            let value = table
                .values
                .get(&self.raw)
                .ok_or(HandleError::InvalidHandle)?;

            if value.is::<T>() {
                Ok(())
            } else {
                Err(HandleError::TypeMismatch)
            }
        })
    }

    pub fn take<T: Any>(self) -> Result<T, HandleError> {
        self.validate::<T>()?;

        let value = HANDLES.with_borrow_mut(|table| table.values.remove(&self.raw).unwrap());

        Ok(*value.downcast::<T>().unwrap())
    }

    pub fn release(self) -> Result<(), HandleError> {
        let value = HANDLES.with_borrow_mut(|table| {
            table
                .values
                .remove(&self.raw)
                .ok_or(HandleError::InvalidHandle)
        })?;

        drop(value);
        Ok(())
    }
}

pub fn validate_distinct(handles: &[u32]) -> Result<(), HandleError> {
    for (index, handle) in handles.iter().enumerate() {
        if handles[..index].contains(handle) {
            return Err(HandleError::DuplicateMove);
        }
    }

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn __rustier_rust_drop(handle: u32) -> bool {
    RustHandle::from_raw(handle).release().is_ok()
}

#[cfg(test)]
mod tests {
    use super::{HandleError, RustHandle, validate_distinct};

    #[test]
    fn moves_owned_values_once() {
        let handle = RustHandle::new(vec![1_u32, 2]);
        assert_eq!(handle.take::<String>(), Err(HandleError::TypeMismatch));
        assert_eq!(handle.take::<Vec<u32>>(), Ok(vec![1, 2]));
        assert_eq!(handle.take::<Vec<u32>>(), Err(HandleError::InvalidHandle));

        let next = RustHandle::new(3_u32);
        assert_ne!(handle.into_raw(), next.into_raw());
        assert_eq!(next.take::<u32>(), Ok(3));
    }

    #[test]
    fn rejects_duplicate_moves() {
        assert_eq!(validate_distinct(&[1, 2]), Ok(()));
        assert_eq!(validate_distinct(&[1, 1]), Err(HandleError::DuplicateMove));
    }

    #[test]
    fn releases_owned_values_once() {
        let handle = RustHandle::new(String::from("owned"));
        assert_eq!(handle.release(), Ok(()));
        assert_eq!(handle.release(), Err(HandleError::InvalidHandle));
    }
}
