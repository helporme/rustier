use std::{
    cell::{BorrowError, BorrowMutError, Ref, RefCell, RefMut},
    rc::Rc,
};

use super::errors::CallResult;

pub type AccessResult<T> = CallResult<T>;

pub trait Accessor<T> {
    fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()>;
    fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()>;
}

impl<T> Accessor<T> for RefCell<T> {
    fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()> {
        let value = try_borrow(self)?;

        f(&*value)
    }

    fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()> {
        let mut value = try_borrow_mut(self)?;

        f(&mut *value)
    }
}

pub trait AccessorExt<T>: Accessor<T> {
    fn access<R, F>(&self, f: F) -> AccessResult<R>
    where
        F: FnOnce(&T) -> AccessResult<R>,
    {
        let mut f = Some(f);
        let mut result = None;

        self.access_dyn(&mut |value| {
            result = Some(f.take().unwrap()(value)?);

            Ok(())
        })?;

        Ok(result.unwrap())
    }

    fn access_mut<R, F>(&self, f: F) -> AccessResult<R>
    where
        F: FnOnce(&mut T) -> AccessResult<R>,
    {
        let mut f = Some(f);
        let mut result = None;

        self.access_mut_dyn(&mut |value| {
            result = Some(f.take().unwrap()(value)?);

            Ok(())
        })?;

        Ok(result.unwrap())
    }
}

impl<T, A: Accessor<T> + ?Sized> AccessorExt<T> for A {}

pub enum Source<T> {
    Owned(T),
    Projected(Rc<dyn Accessor<T>>),
}

pub struct SourceSlot<T>(RefCell<Source<T>>);

impl<T: 'static> SourceSlot<T> {
    pub fn owned(value: T) -> Self {
        SourceSlot(RefCell::new(Source::Owned(value)))
    }

    pub fn projected(accessor: Rc<dyn Accessor<T>>) -> Self {
        SourceSlot(RefCell::new(Source::Projected(accessor)))
    }

    pub fn replace_with_owned(&self, value: T) -> Result<(), BorrowMutError> {
        *try_borrow_mut(&self.0)? = Source::Owned(value);

        Ok(())
    }
}

impl<T> Accessor<T> for SourceSlot<T> {
    fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()> {
        let accessor = {
            let source = try_borrow(&self.0)?;

            match &*source {
                Source::Owned(value) => return f(value),
                Source::Projected(accessor) => Rc::clone(accessor),
            }
        };

        accessor.access_dyn(f)
    }

    fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()> {
        let accessor = {
            let mut source = try_borrow_mut(&self.0)?;

            match &mut *source {
                Source::Owned(value) => return f(value),
                Source::Projected(accessor) => Rc::clone(accessor),
            }
        };

        accessor.access_mut_dyn(f)
    }
}

pub struct FieldAccess<Parent, T> {
    pub parent: Rc<dyn Accessor<Parent>>,
    pub get: for<'a> fn(&'a Parent) -> AccessResult<&'a T>,
    pub get_mut: for<'a> fn(&'a mut Parent) -> AccessResult<&'a mut T>,
}

impl<Parent: 'static, T: 'static> Accessor<T> for FieldAccess<Parent, T> {
    fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()> {
        self.parent.access_dyn(&mut |parent| f((self.get)(parent)?))
    }

    fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()> {
        self.parent
            .access_mut_dyn(&mut |parent| f((self.get_mut)(parent)?))
    }
}

pub fn try_borrow<T>(value: &RefCell<T>) -> Result<Ref<'_, T>, BorrowError> {
    value.try_borrow()
}

pub fn try_borrow_mut<T>(value: &RefCell<T>) -> Result<RefMut<'_, T>, BorrowMutError> {
    value.try_borrow_mut()
}
