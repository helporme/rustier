//! Shared projection runtime preamble emitted into generated glue files.

pub const PROJECTION_PREAMBLE: &str = r#"
#[allow(unused_imports)]
use std::cell::RefCell;
#[allow(unused_imports)]
use std::rc::{Rc, Weak};
// Brought into scope so trait-object method lookup (`Rc<dyn __cursed::Projection<T>>`
// .with / .with_mut) resolves in generated wrappers and glue.
#[allow(unused_imports)]
use __cursed::Projection as _;

#[allow(non_snake_case, dead_code)]
mod __cursed {
    use std::cell::RefCell;
    use std::rc::Rc;

    // rust-curse projection shared utils

    pub trait Projection<T> {
        fn with(&self, f: &mut dyn FnMut(&T));
        fn with_mut(&self, f: &mut dyn FnMut(&mut T));
    }

    impl<T> Projection<T> for RefCell<T> {
        fn with(&self, f: &mut dyn FnMut(&T)) {
            f(&*self.borrow());
        }
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) {
            f(&mut *self.borrow_mut());
        }
    }

    pub struct Projector<Parent, T> {
        pub parent: Rc<RefCell<Rc<dyn Projection<Parent>>>>,
        pub project: fn(&Parent) -> &T,
        pub project_mut: fn(&mut Parent) -> &mut T,
    }

    impl<Parent: 'static, T: 'static> Projection<T> for Projector<Parent, T> {
        fn with(&self, f: &mut dyn FnMut(&T)) {
            let parent = self.parent.borrow().clone();
            parent.with(&mut |p| f((self.project)(p)));
        }
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) {
            let parent = self.parent.borrow().clone();
            parent.with_mut(&mut |p| f((self.project_mut)(p)));
        }
    }

    pub enum Coerced<T> {
        Projected(Rc<dyn Projection<T>>),
        Owned(T),
    }

    impl<T: 'static> Coerced<T> {
        pub fn with(&self, f: &mut dyn FnMut(&T)) {
            match self {
                Self::Projected(p) => p.with(f),
                Self::Owned(v) => f(v),
            }
        }
        pub fn with_mut(&mut self, f: &mut dyn FnMut(&mut T)) {
            match self {
                Self::Projected(p) => p.with_mut(f),
                Self::Owned(v) => f(v),
            }
        }
        pub fn into_owned(self) -> T
        where
            T: Clone,
        {
            match self {
                Self::Projected(p) => {
                    let mut out = None;
                    p.with(&mut |v| out = Some(v.clone()));
                    out.unwrap()
                }
                Self::Owned(v) => v,
            }
        }
    }
}
"#;
