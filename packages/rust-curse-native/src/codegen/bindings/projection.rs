use rustdoc_types::ItemEnum;

use crate::codegen::{db::Db, rustdoc::mod_::all_mod_ids};

pub const PROJECTION_RUNTIME: &str = indoc::indoc! {r#"
    #[allow(non_snake_case, dead_code)]
    mod __curse {
        use std::cell::RefCell;
        use std::rc::Rc;

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
                parent.with(&mut |p| f((self.project)(p)))
            }
            fn with_mut(&self, f: &mut dyn FnMut(&mut T)) {
                let parent = self.parent.borrow().clone();
                parent.with_mut(&mut |p| f((self.project_mut)(p)))
            }
        }
    }
"#};

#[salsa::tracked]
pub fn is_projection_preamble_needed(db: &dyn Db) -> bool {
    for id in all_mod_ids(db) {
        let crate_ = id.crate_body(db);

        if crate_
            .index
            .values()
            .any(|item| matches!(item.inner, ItemEnum::Struct(_)))
        {
            return true;
        }
    }

    false
}
