//! Shared projection runtime preamble emitted into generated glue files.

pub const PROJECTION_PREAMBLE: &str = r#"
#[allow(unused_imports)]
use std::cell::RefCell;
#[allow(unused_imports)]
use std::rc::{Rc, Weak};

// rust-curse projection shared utils

trait __cursed_Projection<T> {
    fn with(&self, f: &mut dyn FnMut(&T));
    fn with_mut(&self, f: &mut dyn FnMut(&mut T));
}

impl<T> __cursed_Projection<T> for RefCell<T> {
    fn with(&self, f: &mut dyn FnMut(&T)) {
        f(&*self.borrow());
    }
    fn with_mut(&self, f: &mut dyn FnMut(&mut T)) {
        f(&mut *self.borrow_mut());
    }
}

#[allow(dead_code)]
struct __cursed_Projector<Parent, T> {
    parent: Rc<RefCell<Rc<dyn __cursed_Projection<Parent>>>>,
    project: fn(&Parent) -> &T,
    project_mut: fn(&mut Parent) -> &mut T,
}

impl<Parent: 'static, T: 'static> __cursed_Projection<T> for __cursed_Projector<Parent, T> {
    fn with(&self, f: &mut dyn FnMut(&T)) {
        let parent = self.parent.borrow().clone();
        parent.with(&mut |p| f((self.project)(p)));
    }
    fn with_mut(&self, f: &mut dyn FnMut(&mut T)) {
        let parent = self.parent.borrow().clone();
        parent.with_mut(&mut |p| f((self.project_mut)(p)));
    }
}
"#;
