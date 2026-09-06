//! Tethering is a mechanism for simulating a JavaScript-like object model for
//! Rust language constructs, like structs or enums.
//!
//! ## Idea
//!
//! Without loss of generality, we will consider the following example:
//!
//! ```
//! pub struct Point {
//!     pub x: f64,
//!     pub y: f64,
//! }
//!
//! pub struct Line {
//!     pub start: Point,
//!     pub end: Point,
//! }
//! ```
//!
//! If we mentally translate these two structs to JavaScript classes, we can
//! expect the following code to work:
//! ```js
//! const line = new Line(new Point(1, 1), new Point(2, -3));
//!
//! const start = line.start;
//! start.x = 10;
//! console.log(line.start.x, line.start.y); // 10, 1
//!
//! line.start = new Point(-20, 13);
//! console.log(line.start.x, line.start.y); // -20, 13
//! console.log(start.x, start.y); // 10, 1
//! ```
//!
//! It works since JavaScript is a garbage-collected language, and each `new
//! Point()` allocates a new object in the heap, so `Line.start` and `Line.end`
//! are holding a pointer to the `Point` object in the heap.
//!
//! In Rust, we could represent something similar using `Rc` and `RwLock` or
//! `Arc` and `Mutex`:
//! ```text
//! use std::sync::{Arc, Mutex};
//!
//! struct Line {
//!     start: Arc<Mutex<Point>>,
//!     end: Arc<Mutex<Point>>,
//! }
//! ```
//! This ignores the fact that we can deadlock ourselves.
//!
//! However, it is not reasonable for rust-curse to use this design.
//! It implies that we need to generate a "binding struct" that contains all the
//! same fields as the source struct and wrap all non-copy fields in
//! `Arc<Mutex<T>>`. It's enough to say that it's impossible to do for private
//! fields.
//!
//! Instead, we introduce the concept that we **clone** values on
//! move. With this constraint, we can expose fields that hold struct (like
//! `line.start` or `line.end`) to JavaScript using _projection_.
//!
//! Projection is nothing more than a closure that will return for a given
//! struct a (mutable) reference to its field value, like `|line| &line.start`
//! is a projection from a `Line` to `Point`.
//!
//! A tethered struct is compiled specifically for a source struct and can be
//! initialized from:
//!  - an owned value, so `TetheringPoint` will be created like
//!    `TetheringPoint::new_owned(Point::new(1,2))`.
//!  - a projection, so `TetheringPoint` will be created like
//!    `TetheringPoint::from_projection(Projection::new(line, |line|
//!    &line.start))`.
//!
//! This is not real code, but it illustrates the idea.
//!
//! Now, for each struct we generate a tethering struct with internal structure
//! like this:
//! ```text
//! struct TetheringPoint {
//!     src: ProjectionOrOwned<Point>,
//! }
//! // impl of TetheringPoint omitted for brevity
//!
//! struct TetheringLine {
//!     src: ProjectionOrOwned<Line>,
//! }
//!
//! impl TetheringLine {
//!     pub fn get_start(&self) -> TetheringPoint {
//!         // access field via reference projection or just directly in case it's owned value
//!         self.src.access(|src| {
//!             TetheringPoint::from_projection(Projection::new(src, |src| &src.start))
//!         })
//!     }
//!
//!     pub fn set_start(&mut self, point: TetheringPoint) {
//!         self.src.access_mut(|src| {
//!             point.src.access(|point| {
//!                 src.start = point.into_owned();
//!             });
//!         });
//!     }
//!
//!     // same for end
//! }
//! ```
//!
//! By using projections, we can generate code that emulates JavaScript-like
//! field access for every line of the JavaScript snippet shown at the beginning
//! of this module documentation except the last one.
//!
//! All projections created and returned from the previous pseudo-code example
//! are projecting from the same source struct, so doing this:
//! ```js
//! const start = line.start;
//! line.start = new Point(-20, 13);
//! ```
//! will update the struct stored in `start` variable to point to `(-20, 13)`,
//! since both `line.start` and `start` are practically accessing the same
//! memory. To avoid this, the setter first checks whether any previously
//! returned `TetheringPoint` value is still held
//! by JavaScript, and if so, we replace the underlying projection of the
//! `TetheringPoint` with the owned value of the `Line.start` value before
//! overwriting it.
//!
//! In practice, the code differs and has more nuances, but the idea is
//! unchanged. Tethering structs are an internal mechanism used to support
//! code generation, so they do not and should not depend on IR.

use crate::compiler::{
    codegen::{
        fragment::{Fragment, FragmentOnce, doubles, frag, fragment, spaced},
        shim::Platform,
    },
    db::Db,
    ir::rust::ValueType,
};

/// Compiles the platform-specific supporting code used by generated tethering
/// structs.
pub fn compile_tethering_prelude(platform: Platform) -> impl FragmentOnce {
    let access_result = platform.result_type("T");
    let borrow_error = platform.error(frag!(
        { doubles("cannot access a tethered value while it is mutably borrowed") }.to_string()
    ));
    let borrow_mut_error = platform.error(frag!(
        { doubles("cannot mutably access a tethered value while it is borrowed") }.to_string()
    ));

    let borrow_support = frag!(
        use super::*;@
        use std::cell::RefCell;@
        use std::rc::Rc;@@

        pub type AccessResult<T> = ~{access_result};@@

        /// Borrows a tethered value or returns a platform-specific aliasing error.@
        pub fn try_borrow<T>(value: &RefCell<T>) -> AccessResult<std::cell::Ref<'_, T>>~{{
            value.try_borrow().map_err(|_|~{borrow_error})
        }}@@

        /// Mutably borrows a tethered value or returns a platform-specific aliasing error.@
        pub fn try_borrow_mut<T>(value: &RefCell<T>) -> AccessResult<std::cell::RefMut<'_, T>>~{{
            value.try_borrow_mut().map_err(|_|~{borrow_mut_error})
        }}@@

        /// Provides object-safe access to a potentially projected value.@
        pub trait Accessor<T>~{{
            /// Invokes `f` with shared access to the value.@
            fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()>;@
            /// Invokes `f` with exclusive access to the value.@
            fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()>;
        }}@@

        impl<T> Accessor<T> for RefCell<T>~{{
            fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()>~{{
                let value = try_borrow(self)?;@
                f(&*value)
            }}@
            fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()>~{{
                let mut value = try_borrow_mut(self)?;@
                f(&mut *value)
            }}
        }}
    );

    let accessor_ext = frag!(
        /// Adds allocation-free one-shot callbacks to every accessor.@
        pub trait AccessorExt<T>: Accessor<T>~{{
            /// Invokes `f` exactly once when shared access succeeds.@
            fn access<R, F>(&self, f: F) -> AccessResult<R>@
            where@
                F: FnOnce(&T) -> AccessResult<R>,@
            ~{{
                let mut f = Some(f);@
                let mut result = None;@
                self.access_dyn(&mut |value|~{{
                    result = Some(f.take().unwrap()(value)?);@
                    Ok(())
                }})?;@
                Ok(result.unwrap())
            }}@@

            /// Invokes `f` exactly once when exclusive access succeeds.@
            fn access_mut<R, F>(&self, f: F) -> AccessResult<R>@
            where@
                F: FnOnce(&mut T) -> AccessResult<R>,@
            ~{{
                let mut f = Some(f);@
                let mut result = None;@
                self.access_mut_dyn(&mut |value|~{{
                    result = Some(f.take().unwrap()(value)?);@
                    Ok(())
                }})?;@
                Ok(result.unwrap())
            }}
        }}@@

        impl<T, A: Accessor<T> + ?Sized> AccessorExt<T> for A~{{}}
    );

    let source = frag!(
        pub enum Source<T>~{{
            Owned(T),@
            Projected(Rc<dyn Accessor<T>>),
        }}@@

        pub struct SourceSlot<T>(RefCell<Source<T>>);@@

        impl<T: 'static> SourceSlot<T>~{{
            pub fn owned(value: T) -> Self~{{
                SourceSlot(RefCell::new(Source::Owned(value)))
            }}@
            pub fn projected(accessor: Rc<dyn Accessor<T>>) -> Self~{{
                SourceSlot(RefCell::new(Source::Projected(accessor)))
            }}@
            pub fn replace_with_owned(&self, value: T) -> AccessResult<()>~{{
                *try_borrow_mut(&self.0)? = Source::Owned(value);@
                Ok(())
            }}
        }}
    );

    let source_accessor = frag!(
        impl<T> Accessor<T> for SourceSlot<T>~{{
            fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()>~{{
                let accessor = ~{{
                    let source = try_borrow(&self.0)?;@@

                    match &*source~{{
                        Source::Owned(value) => return f(value),@
                        Source::Projected(accessor) => Rc::clone(accessor),
                    }}
                }};@
                accessor.access_dyn(f)
            }}@@

            fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()>~{{
                let accessor = ~{{
                    let mut source = try_borrow_mut(&self.0)?;@@

                    match &mut *source~{{
                        Source::Owned(value) => return f(value),@
                        Source::Projected(accessor) => Rc::clone(accessor),
                    }}
                }};@
                accessor.access_mut_dyn(f)
            }}
        }}
    );

    let field_access = frag!(
        pub struct FieldAccess<Parent, T>~{{
            pub parent: Rc<dyn Accessor<Parent>>,@
            pub get: for<'a> fn(&'a Parent) -> AccessResult<&'a T>,@
            pub get_mut: for<'a> fn(&'a mut Parent) -> AccessResult<&'a mut T>,
        }}@@

        impl<Parent: 'static, T: 'static> Accessor<T> for FieldAccess<Parent, T>~{{
            fn access_dyn(&self, f: &mut dyn FnMut(&T) -> AccessResult<()>) -> AccessResult<()>~{{
                self.parent.access_dyn(&mut |parent| f((self.get)(parent)?))
            }}@
            fn access_mut_dyn(&self, f: &mut dyn FnMut(&mut T) -> AccessResult<()>) -> AccessResult<()>~{{
                self.parent.access_mut_dyn(&mut |parent| f((self.get_mut)(parent)?))
            }}
        }}
    );

    // todo: box it?
    frag!(
        #[allow(non_snake_case, dead_code)]@
        mod __tethering~{{
            {borrow_support}@@
            {accessor_ext}@@
            {source}@@
            {source_accessor}@@
            {field_access}
        }}@
        use __tethering::{Accessor as _, AccessorExt as _};
    )
}

impl<'db> ValueType<'db> {
    pub fn tethering_struct_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        frag!(__tethering__{self.render_mangled_name(db)})
    }
}

/// A tethering struct. See the module-level documentation for more details.
pub struct TetheringStruct<'a, Fields> {
    /// Source type for which the tethering struct is generated.
    pub source_type: ValueType<'a>,
    /// Source fields exposed by the tethering struct.
    pub fields: Fields,
}

/// A tethered field. See the module-level documentation for more details.
#[derive(Clone)]
pub enum TetheredField<'a, GetRefExpr, GetMutExpr> {
    /// A field that holds a primitive-like value that is cloned when read.
    Cloned(ClonedField<'a, GetRefExpr, GetMutExpr>),
    /// A field that holds a value represented by another tethering struct.
    TetheringStruct(TetheringStructField<'a, GetRefExpr, GetMutExpr>),
}

pub trait FieldExpr {
    fn render(&self, source: &str) -> impl FragmentOnce;
}

#[derive(Clone)]
pub struct ClonedField<'a, GetRefExpr, GetMutExpr> {
    pub name: &'a str,
    /// Expression returning a fallible shared reference to the source value.
    pub get_ref_expr: GetRefExpr,
    /// Expression returning a fallible mutable reference to the source value.
    pub get_mut_expr: GetMutExpr,
    /// The type of this field on source type.
    pub source_type: ValueType<'a>,
}

#[derive(Clone)]
pub struct TetheringStructField<'a, GetRefExpr, GetMutExpr> {
    // The field name.
    pub name: &'a str,
    /// Expression returning a fallible shared reference to the source value.
    pub get_ref_expr: GetRefExpr,
    /// Expression returning a fallible mutable reference to the source value.
    pub get_mut_expr: GetMutExpr,
    /// The type of this field on source type.
    pub source_type: ValueType<'a>,
    /// Whether to generate a setter for this value.
    pub with_setter: bool,
}

/// Renders a [`TetheringStruct`]. See the module-level documentation for more
/// details.
pub fn render_tethering_struct<'db, Fields, GetRefExpr, GetMutExpr>(
    db: &'db dyn Db,
    struct_: TetheringStruct<'db, Fields>,
) -> impl FragmentOnce + 'db
where
    Fields: IntoIterator<Item = TetheredField<'db, GetRefExpr, GetMutExpr>> + 'db,
    Fields::IntoIter: Clone,
    GetRefExpr: FieldExpr,
    GetMutExpr: FieldExpr,
{
    fragment(move |p| {
        let fields = struct_.fields.into_iter();

        let cache_field_definitions = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let TetheredField::TetheringStruct(field) = field {
                        let cache_field_name = cache_field_name(field.name);

                        frag!(
                            {&cache_field_name}: std::cell::RefCell<Option<std::rc::Weak<{field.source_type.tethering_struct_name(db)}>>>,@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let owned_cache_initializers = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let TetheredField::TetheringStruct(field) = field {
                        let cache_field_name = cache_field_name(field.name);
                        frag!(
                            {&cache_field_name}: std::cell::RefCell::new(None),@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let projected_cache_initializers = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let TetheredField::TetheringStruct(field) = field {
                        let cache_field_name = cache_field_name(field.name);
                        frag!(
                            {&cache_field_name}: std::cell::RefCell::new(None),@
                        )
                        .apply(p);
                    }
                }
            })
        };

        let field_methods = spaced(fields.map(|field| {
            fragment(move |p| {
                render_tethered_field_getter(&field, db).apply(p);

                let with_setter = match &field {
                    TetheredField::Cloned(_) => true,
                    TetheredField::TetheringStruct(field) => field.with_setter,
                };

                if with_setter {
                    frag!(@@{render_tethered_field_setter(&field, db)}).apply(p);
                }
            })
        }));

        let name = || struct_.source_type.tethering_struct_name(db);
        let source_type = || struct_.source_type.render_fqn(db);

        frag!(
            struct ~{name()}~{{
                src: __tethering::SourceSlot<{source_type()}>,@
                {cache_field_definitions}
            }}@@

            impl __tethering::Accessor<{source_type()}> for ~{name()}~{{
                fn access_dyn(
                    &self,@
                    f: &mut dyn FnMut(&{source_type()}) -> __tethering::AccessResult<()>,@
                ) -> __tethering::AccessResult<()>~{{
                    self.src.access_dyn(f)
                }}@@

                fn access_mut_dyn(
                    &self,@
                    f: &mut dyn FnMut(&mut ~{source_type()}) -> __tethering::AccessResult<()>,@
                ) -> __tethering::AccessResult<()>~{{
                    self.src.access_mut_dyn(f)
                }}
            }}@@

            impl ~{name()}~{{
                pub fn new_owned(input: ~{source_type()})~ -> Self ~{{
                    {name()}~{{
                        src: __tethering::SourceSlot::owned(input),@
                        {owned_cache_initializers}
                    }}
                }}@@

                pub fn new_projected(accessor: std::rc::Rc<dyn __tethering::Accessor<{source_type()}>>) -> Self~{{
                    {name()}~{{
                        src: __tethering::SourceSlot::projected(accessor),@
                        {projected_cache_initializers}
                    }}
                }}@@

                {field_methods}
            }}
        )
        .apply(p);
    })
}

/// Renders a call to a generated tethered field getter.
///
/// Expression is of:
///  - Rc of tethering struct type in case field holds tetherable source type
///  - A source type otherwise
///
/// The expression must be inserted into a scope that returns the
/// platform-specific result type because it uses `?`.
pub fn render_tethering_struct_get_field_expr<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    field_name: &'a str,
) -> impl FragmentOnce + 'a {
    frag!({tethered_struct_expr}.get_{field_name}()?)
}

/// Renders a call to a generated tethered field setter.
///
/// Value should be of:
///  - Rc of tethering struct type in case field holds tetherable source type
///  - A source type otherwise
///
/// The statement must be inserted into a scope that returns the
/// platform-specific result type because it uses `?`.
pub fn render_tethering_struct_set_field_stmt<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    field_name: &'a str,
    value: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!({tethered_struct_expr}.set_{field_name}({value})?;)
}

fn cache_field_name(name: &str) -> impl Fragment + '_ {
    frag!(__c__ { name })
}

impl<'a, GetRefExpr, GetMutExpr> ClonedField<'a, GetRefExpr, GetMutExpr> {
    fn render_getter(&self, db: &'a dyn Db) -> impl FragmentOnce + '_
    where
        GetRefExpr: FieldExpr,
    {
        fragment(move |p| {
            let get_ref_expr = self.get_ref_expr.render("src");

            frag!(
                pub fn get_{self.name}(&self) -> __tethering::AccessResult<{self.source_type.render_fqn(db)}>~{{
                    self.access(|src| Ok({get_ref_expr}?.clone()))
                }}
            )
            .apply(p);
        })
    }

    fn render_setter(&self, db: &'a dyn Db) -> impl FragmentOnce + '_
    where
        GetMutExpr: FieldExpr,
    {
        fragment(move |p| {
            let get_mut_expr = self.get_mut_expr.render("src");

            frag!(
                pub fn set_{self.name}(&self, value: ~{self.source_type.render_fqn(db)}) -> __tethering::AccessResult<()>~{{
                    self.access_mut(|src|~{{
                        *{get_mut_expr}? = value;@
                        Ok(())
                    }})
                }}
            )
            .apply(p);
        })
    }
}

impl<'a, GetRefExpr, GetMutExpr> TetheringStructField<'a, GetRefExpr, GetMutExpr> {
    fn render_getter(&'a self, db: &'a dyn Db) -> impl FragmentOnce + 'a
    where
        GetRefExpr: FieldExpr,
        GetMutExpr: FieldExpr,
    {
        fragment(move |p| {
            let cache_field_name = cache_field_name(self.name);
            let validate_get_ref_expr = self.get_ref_expr.render("parent");
            let get_ref_expr = self.get_ref_expr.render("parent");
            let get_mut_expr = self.get_mut_expr.render("parent");

            let tethered_name = || self.source_type.tethering_struct_name(db);
            let source_type = || self.source_type.render_fqn(db);

            frag!(
                pub fn get_{self.name}(
                    self: &std::rc::Rc<Self>,@
                ) -> __tethering::AccessResult<std::rc::Rc<{tethered_name()}>>~{{
                    // Try to access the field first so we can return early if it's invalid
                    self.access(|parent|~{{
                        {validate_get_ref_expr}?;@
                        Ok(())
                    }})?;@@

                    if let Some(tethered)~ = __tethering::try_borrow(&self.{&cache_field_name})?.as_ref().and_then(std::rc::Weak::upgrade)~{{
                        return Ok(tethered);@
                    }}@@

                    let field_access: std::rc::Rc<dyn __tethering::Accessor<{source_type()}>> = std::rc::Rc::new(__tethering::FieldAccess~{{
                        parent: self.clone(),@
                        get: |parent|~{get_ref_expr},@
                        get_mut: |parent|~{get_mut_expr},@
                    }});@@

                    let tethered = std::rc::Rc::new({tethered_name()}::new_projected(field_access));@@

                    *__tethering::try_borrow_mut(&self.{&cache_field_name})? = Some(std::rc::Rc::downgrade(&tethered));@@

                    Ok(tethered)
                }}
            )
            .apply(p);
        })
    }

    fn render_setter(&'a self, db: &'a dyn Db) -> impl FragmentOnce + 'a
    where
        GetMutExpr: FieldExpr,
    {
        fragment(move |p| {
            let cache_field_name = cache_field_name(self.name);
            let validate_get_mut_expr = self.get_mut_expr.render("src");
            let get_mut_expr = self.get_mut_expr.render("src");
            let source_type = self.source_type.render_fqn(db);

            frag!(
                pub fn set_{self.name}(&self, value: ~{source_type}) -> __tethering::AccessResult<()>~{{
                    // Try to access the field first so we can return early if it's invalid
                    self.access_mut(|src|~{{
                        {validate_get_mut_expr}?;@
                        Ok(())
                    }})?;@@

                    let cached_value = __tethering::try_borrow_mut(&self.{&cache_field_name})?
                        .take()
                        .and_then(|tethered| tethered.upgrade());@@

                    let old_value = self.access_mut(|src|~{{
                        Ok(std::mem::replace(
                            {get_mut_expr}?,
                            value,
                        ))
                    }})?;@@

                    if let Some(cached_value)~= cached_value~{{
                        cached_value.src.replace_with_owned(old_value)?;
                    }}@
                    Ok(())
                }}
            )
            .apply(p);
        })
    }
}

fn render_tethered_field_getter<'field, 'a, GetRefExpr, GetMutExpr>(
    field: &'field TetheredField<'a, GetRefExpr, GetMutExpr>,
    db: &'a dyn Db,
) -> impl FragmentOnce + 'field
where
    GetRefExpr: FieldExpr,
    GetMutExpr: FieldExpr,
{
    fragment(move |p| match field {
        TetheredField::Cloned(field) => field.render_getter(db).apply(p),
        TetheredField::TetheringStruct(field) => field.render_getter(db).apply(p),
    })
}

fn render_tethered_field_setter<'field, 'a, GetRefExpr, GetMutExpr>(
    field: &'field TetheredField<'a, GetRefExpr, GetMutExpr>,
    db: &'a dyn Db,
) -> impl FragmentOnce + 'field
where
    GetMutExpr: FieldExpr,
{
    fragment(move |p| match field {
        TetheredField::Cloned(field) => field.render_setter(db).apply(p),
        TetheredField::TetheringStruct(field) => field.render_setter(db).apply(p),
    })
}

/// A type of [`TetheringStruct`] that is used for casting.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, salsa::Update)]
pub struct TetheringStructType<'db> {
    // A source type which tethering struct is storing.
    pub source_type: ValueType<'db>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        compiler::ir::rust::{Member, Rust},
    };

    fn test_value_type<'db>(db: &'db dyn Db, name: &str) -> ValueType<'db> {
        let crate_ = Rust::get(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered");

        crate_
            .root(db)
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Struct(struct_) if struct_.name(db) == name => {
                    Some(ValueType::Struct(*struct_))
                }
                Member::Enum(enum_) if enum_.name(db) == name => Some(ValueType::Enum(*enum_)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("test value type `{name}` should be a root member"))
    }

    #[derive(Clone, Copy)]
    struct TestFieldExpr {
        field_name: &'static str,
        is_mutable: bool,
    }

    impl TestFieldExpr {
        fn shared(field_name: &'static str) -> Self {
            Self {
                field_name,
                is_mutable: false,
            }
        }

        fn mutable(field_name: &'static str) -> Self {
            Self {
                field_name,
                is_mutable: true,
            }
        }
    }

    impl FieldExpr for TestFieldExpr {
        fn render(&self, source: &str) -> impl FragmentOnce {
            let Self {
                field_name,
                is_mutable,
            } = *self;

            fragment(move |p| {
                if is_mutable {
                    frag!(Ok(&mut ~{source}.{field_name})).apply(p);
                } else {
                    frag!(Ok(&{source}.{field_name})).apply(p);
                }
            })
        }
    }

    #[test]
    fn compiles_tethered_struct_empty() {
        assert_rust_snap! {
            source: {
                pub struct Empty;
            },
            expression: |db, _platform, p| {
                let db: &dyn Db = &*db;
                let fields: [TetheredField<'_, TestFieldExpr, TestFieldExpr>; 0] = [];

                render_tethering_struct(
                    db,
                    TetheringStruct {
                        source_type: test_value_type(db, "Empty"),
                        fields,
                    },
                )
                .apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_cloned_field() {
        assert_rust_snap! {
            source: {
                pub struct Parent {
                    pub count: u32,
                }
            },
            expression: |db, _platform, p| {
                let db: &dyn Db = &*db;
                let fields: [TetheredField<'_, TestFieldExpr, TestFieldExpr>; 1] =
                    [TetheredField::Cloned(ClonedField {
                        name: "count",
                        get_ref_expr: TestFieldExpr::shared("count"),
                        get_mut_expr: TestFieldExpr::mutable("count"),
                        source_type: ValueType::u32,
                    })];

                render_tethering_struct(
                    db,
                    TetheringStruct {
                        source_type: test_value_type(db, "Parent"),
                        fields,
                    },
                )
                .apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_tethered_struct_field() {
        assert_rust_snap! {
            source: {
                pub struct Child;
                pub struct Parent {
                    pub child: Child,
                }
            },
            expression: |db, _platform, p| {
                let db: &dyn Db = &*db;
                let fields: [TetheredField<'_, TestFieldExpr, TestFieldExpr>; 1] =
                    [TetheredField::TetheringStruct(TetheringStructField {
                        name: "child",
                        get_ref_expr: TestFieldExpr::shared("child"),
                        get_mut_expr: TestFieldExpr::mutable("child"),
                        source_type: test_value_type(db, "Child"),
                        with_setter: true,
                    })];

                render_tethering_struct(
                    db,
                    TetheringStruct {
                        source_type: test_value_type(db, "Parent"),
                        fields,
                    },
                )
                .apply(p);
            },
        }
    }

    #[test]
    fn compiles_tethered_struct_mixed() {
        assert_rust_snap! {
            source: {
                pub struct Child;
                pub struct ReadonlyChild;
                pub struct Parent {
                    pub count: u32,
                    pub child: Child,
                    pub readonly_child: ReadonlyChild,
                }
            },
            expression: |db, _platform, p| {
                let db: &dyn Db = &*db;
                let fields: [TetheredField<'_, TestFieldExpr, TestFieldExpr>; 3] = [
                    TetheredField::Cloned(ClonedField {
                        name: "count",
                        get_ref_expr: TestFieldExpr::shared("count"),
                        get_mut_expr: TestFieldExpr::mutable("count"),
                        source_type: ValueType::u32,
                    }),
                    TetheredField::TetheringStruct(TetheringStructField {
                        name: "child",
                        get_ref_expr: TestFieldExpr::shared("child"),
                        get_mut_expr: TestFieldExpr::mutable("child"),
                        source_type: test_value_type(db, "Child"),
                        with_setter: true,
                    }),
                    TetheredField::TetheringStruct(TetheringStructField {
                        name: "readonly_child",
                        get_ref_expr: TestFieldExpr::shared("readonly_child"),
                        get_mut_expr: TestFieldExpr::mutable("readonly_child"),
                        source_type: test_value_type(db, "ReadonlyChild"),
                        with_setter: false,
                    }),
                ];

                render_tethering_struct(
                    db,
                    TetheringStruct {
                        source_type: test_value_type(db, "Parent"),
                        fields,
                    },
                )
                .apply(p);
            },
        }
    }
}
