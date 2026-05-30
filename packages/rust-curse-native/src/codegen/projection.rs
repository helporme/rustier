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

    pub struct StaleRef {
        pub enum_path: &'static str,
        pub expected_variant: &'static str,
        pub current_variant: &'static str,
        pub field_name: &'static str,
    }

    impl StaleRef {
        pub fn message(&self) -> String {
            format!(
                "stale reference: this \"{enum_path}.{field}\" was obtained while the active \
variant of \"{enum_path}\" was \"{expected}\", but the variant has since changed to \
\"{current}\", so the \"{field}\" field no longer exists.\n\nThis usually happens when an \
\"&mut\" method on the enum, an \"&mut {enum_path}\" function parameter, or an assignment \
that replaced the enum, ran between when this reference was obtained and when it was \
accessed.\n\nTo fix:\n  - Store the original \"{enum_path}\" and access fields of it.\n  - \
Store the copied value of the \"{enum_path}.{field}\".",
                enum_path = self.enum_path,
                field = self.field_name,
                expected = self.expected_variant,
                current = self.current_variant,
            )
        }
    }

    pub trait Projection<T> {
        fn with(&self, f: &mut dyn FnMut(&T)) -> Result<(), StaleRef>;
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) -> Result<(), StaleRef>;
    }

    impl<T> Projection<T> for RefCell<T> {
        fn with(&self, f: &mut dyn FnMut(&T)) -> Result<(), StaleRef> {
            f(&*self.borrow());
            Ok(())
        }
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) -> Result<(), StaleRef> {
            f(&mut *self.borrow_mut());
            Ok(())
        }
    }

    pub struct Projector<Parent, T> {
        pub parent: Rc<RefCell<Rc<dyn Projection<Parent>>>>,
        pub project: fn(&Parent) -> &T,
        pub project_mut: fn(&mut Parent) -> &mut T,
    }

    impl<Parent: 'static, T: 'static> Projection<T> for Projector<Parent, T> {
        fn with(&self, f: &mut dyn FnMut(&T)) -> Result<(), StaleRef> {
            let parent = self.parent.borrow().clone();
            parent.with(&mut |p| f((self.project)(p)))
        }
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) -> Result<(), StaleRef> {
            let parent = self.parent.borrow().clone();
            parent.with_mut(&mut |p| f((self.project_mut)(p)))
        }
    }

    pub struct VariantProjector<Parent, T> {
        pub parent: Rc<RefCell<Rc<dyn Projection<Parent>>>>,
        pub project: fn(&Parent) -> Option<&T>,
        pub project_mut: fn(&mut Parent) -> Option<&mut T>,
        pub current_tag: fn(&Parent) -> &'static str,
        pub expected_tag: &'static str,
        pub enum_path: &'static str,
        pub field_name: &'static str,
    }

    impl<Parent: 'static, T: 'static> Projection<T> for VariantProjector<Parent, T> {
        fn with(&self, f: &mut dyn FnMut(&T)) -> Result<(), StaleRef> {
            let parent = self.parent.borrow().clone();
            let mut staled: Option<&'static str> = None;
            parent.with(&mut |p| {
                let tag = (self.current_tag)(p);
                match (self.project)(p) {
                    Some(t) => f(t),
                    None => staled = Some(tag),
                }
            })?;
            match staled {
                None => Ok(()),
                Some(current_variant) => Err(StaleRef {
                    enum_path: self.enum_path,
                    expected_variant: self.expected_tag,
                    current_variant,
                    field_name: self.field_name,
                }),
            }
        }
        fn with_mut(&self, f: &mut dyn FnMut(&mut T)) -> Result<(), StaleRef> {
            let parent = self.parent.borrow().clone();
            let mut staled: Option<&'static str> = None;
            parent.with_mut(&mut |p| {
                let tag = (self.current_tag)(p);
                match (self.project_mut)(p) {
                    Some(t) => f(t),
                    None => staled = Some(tag),
                }
            })?;
            match staled {
                None => Ok(()),
                Some(current_variant) => Err(StaleRef {
                    enum_path: self.enum_path,
                    expected_variant: self.expected_tag,
                    current_variant,
                    field_name: self.field_name,
                }),
            }
        }
    }

    pub enum Coerced<T> {
        Projected(Rc<dyn Projection<T>>),
        Owned(T),
    }

    impl<T: 'static> Coerced<T> {
        pub fn with(&self, f: &mut dyn FnMut(&T)) -> Result<(), StaleRef> {
            match self {
                Self::Projected(p) => p.with(f),
                Self::Owned(v) => {
                    f(v);
                    Ok(())
                }
            }
        }
        pub fn with_mut(&mut self, f: &mut dyn FnMut(&mut T)) -> Result<(), StaleRef> {
            match self {
                Self::Projected(p) => p.with_mut(f),
                Self::Owned(v) => {
                    f(v);
                    Ok(())
                }
            }
        }
        pub fn into_owned(self) -> Result<T, StaleRef>
        where
            T: Clone,
        {
            match self {
                Self::Projected(p) => {
                    let mut out = None;
                    p.with(&mut |v| out = Some(v.clone()))?;
                    Ok(out.expect("non-stale projection ran the closure"))
                }
                Self::Owned(v) => Ok(v),
            }
        }
    }
}
"#;
