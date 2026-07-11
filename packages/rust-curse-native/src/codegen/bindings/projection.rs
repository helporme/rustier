// todo: fix messed up terminology of projection / accessor
use rustdoc_types::ItemEnum;

use crate::{
    codegen::{db::Db, fragment::FragmentPrint, rustdoc::mod_::all_mod_ids},
    frag,
};

pub const PROJECTION_RUNTIME: &str = indoc::indoc! {r#"
    #[allow(non_snake_case, dead_code)]
    mod __curse {
        use std::cell::RefCell;
        use std::rc::Rc;

        pub trait Accessor<T> {
            fn access(&self, f: &mut dyn FnMut(&T));
            fn access_mut(&self, f: &mut dyn FnMut(&mut T));
        }

        impl<T> Accessor<T> for RefCell<T> {
            fn access(&self, f: &mut dyn FnMut(&T)) {
                f(&*self.borrow());
            }
            fn access_mut(&self, f: &mut dyn FnMut(&mut T)) {
                f(&mut *self.borrow_mut());
            }
        }

        pub enum Backing<T> {
            Owned(T),
            Projected(Rc<dyn Accessor<T>>),
        }

        pub struct Slot<T>(RefCell<Backing<T>>);

        impl<T: 'static> Slot<T> {
            pub fn owned(value: T) -> Self {
                Slot(RefCell::new(Backing::Owned(value)))
            }
            pub fn projected(projection: Rc<dyn Accessor<T>>) -> Self {
                Slot(RefCell::new(Backing::Projected(projection)))
            }
            pub fn replace_with_owned(&self, value: T) {
                *self.0.borrow_mut() = Backing::Owned(value);
            }
        }

        impl<T> Accessor<T> for Slot<T> {
            fn access(&self, f: &mut dyn FnMut(&T)) {
                let projection = {
                    let target = self.0.borrow();

                    match &*target {
                        Backing::Owned(value) => {
                            f(value);
                            return;
                        }
                        Backing::Projected(access) => Rc::clone(access),
                    }
                };
                projection.access(f);
            }

            fn access_mut(&self, f: &mut dyn FnMut(&mut T)) {
                let projection = {
                    let mut target = self.0.borrow_mut();

                    match &mut *target {
                        Backing::Owned(value) => {
                            f(value);
                            return;
                        }
                        Backing::Projected(access) => Rc::clone(access),
                    }
                };
                projection.access_mut(f);
            }
        }

        pub struct Projection<Parent, T> {
            pub parent: Rc<dyn Accessor<Parent>>,
            pub project: fn(&Parent) -> &T,
            pub project_mut: fn(&mut Parent) -> &mut T,
        }

        impl<Parent: 'static, T: 'static> Accessor<T> for Projection<Parent, T> {
            fn access(&self, f: &mut dyn FnMut(&T)) {
                self.parent.access(&mut |p| f((self.project)(p)))
            }
            fn access_mut(&self, f: &mut dyn FnMut(&mut T)) {
                self.parent.access_mut(&mut |p| f((self.project_mut)(p)))
            }
        }
    }
    use __curse::Accessor as _;
"#};

/// Returns true if the projection preamble is needed for any struct in the
/// crate.
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

/// Returns the fully qualified name of the slot type for the given inner type.
pub fn compile_slot_type_name(inner_type_name: &str) -> String {
    frag!(__curse::Slot<{ inner_type_name }>).print()
}

/// Compiles a statement that accesses a value behind an Accessor (runtime)
/// trait.
pub fn compile_ref_access_stmt(accessor_expr: &str, out_var: &str, in_stmt: &str) -> String {
    frag!(
        {accessor_expr}.access(&mut |{out_var}|~{{ {in_stmt} }})
    )
    .print()
}

/// Compiles a statement that accesses a mut value behind an Accessor (runtime)
/// trait.
pub fn compile_mut_access_stmt(accessor_expr: &str, out_var: &str, in_stmt: &str) -> String {
    frag!(
        {accessor_expr}.access_mut(&mut |{out_var}|~{{ {in_stmt} }})
    )
    .print()
}
