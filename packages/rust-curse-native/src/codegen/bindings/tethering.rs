//! Tethering is a mechanism of simulating a js-like object model for Rust
//! language constructs, like structs or enums.
//!
//! ## Idea
//!
//! Without loss of generality, we will consider the following example:
//!
//! ```no_run
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
//! If we mentally translate this two structs to javascript classes, we can
//! expect the following code to work:
//! ```no_run
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
//! It works since JavaScript is GC-collected language, and each `new Point()`
//! allocates a new object in the heap, so `Line.start` and `Line.end` are
//! holding a pointer to the `Point` object in the heap.
//!
//! In rust we could represent something simillar using `Rc` and `RwLock` or
//! `Arc` and `Mutex`:
//! ```no_run
//! use std::sync::{Arc, Mutex};
//!
//! struct Line {
//!     start: Arc<Mutex<Point>>,
//!     end: Arc<Mutex<Point>>,
//! }
//! ```
//! Avoid the fact that we can deadlock ourself.
//!
//! However, it's not reasonable approach for rust-curse to design it this way.
//! It implies that we need to generate a "binding struct" that contains all the
//! same fields as source one and wrap all non-copy fields to `Arc<Mutex<T>>`.
//! It's enough to say that it's impossible to do for private fields.
//!
//! Instead, we introducing The grand concept that we **clone** values on
//! move. With this constraint, we can expose fields that hold struct (like
//! `line.start` or `line.end`) to JavaScript using _projection_.
//!
//! Projection is nothing more than a closure that will return for a given
//! struct a (mutable) reference to it's field value, like `|line| &line.start`
//! is a projection from a `Line` to `Point`.
//!
//! Teathered struct is a struct that is compiled specifically for a source
//! struct and can be initalized from:
//!  - an owned value, so `TetheredPoint` will be crated like
//!    `TetheredPoint::new_owned(Point::new(1,2))`.
//!  - a projection, so `TetheredPoint` will be crated like
//!    `TetheredPoint::from_projection(Projection::new(line, |line|
//!    &line.start))`.
//!
//! It's not a real code, but fine example.
//!
//! Now, for each struct we generate a tethered struct with internal structure
//! like this:
//! ```no_run
//! struct TetheredPoint {
//!     src: ProjectionOrOwned<Point>,
//! }
//! // impl of TetheredPoint ommited for brevity
//!
//! struct TetheredLine {
//!     src: ProjectionOrOwned<Line>,
//! }
//!
//! impl TetheredLine {
//!     pub fn get_start(&self) -> TetheredPoint {
//!         // access field via reference projection or just directly in case it's owned value
//!         self.src.access(|src| {
//!             TetheredPoint::from_projection(Projection::new(src, |src| &src.start))
//!         })
//!     }
//!
//!     pub fn set_start(&mut self, point: TetheredPoint) {
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
//! By utilizing projections, we can generate code that emulate JS-like field
//! access just fine for all lines of code of javascript snippet shown in the
//! beggining of this unreasonably long module doc, except the last one.
//!
//! All projections created and returned from the previous pseudo-code example
//! are projecting from the same source struct, so doing this:
//! ```no_run
//! const start = line.start;
//! line.start = new Point(-20, 13);
//! ```
//! will update the struct stored in `start` variable to point to `(-20, 13)`,
//! since both `line.start` and `start` are practically accessing the same
//! memory. To circumvent it, in setter, when setting a value, we first
//! check if there is any previously returned `TetheredPoint` value holded
//! by JavaScript, and if so, we replace the underlying projection of the
//! `TetheredPoint` with the owned value of the `Line.start` value before
//! overwriting it.
//!
//! In practice the code differs and have much more nuances, but the idea is
//! unchanged. Tethered structs is a internal mechanism used to support
//! codegen, and because of that, it doesn't have nor should have any dependency
//! on IR.

use crate::{
    codegen::fragment::{Fragment, FragmentOnce, fragment, spaced},
    frag,
};

/// Supporting code
pub const TETHERING_PRELUDE: &str = indoc::indoc! {r#"
    #[allow(non_snake_case, dead_code)]
    mod __tethering {
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
            pub fn replace_with_owned(&self, value: T) {
                *self.0.borrow_mut() = Source::Owned(value);
            }
        }

        impl<T> Accessor<T> for SourceSlot<T> {
            fn access(&self, f: &mut dyn FnMut(&T)) {
                let accessor = {
                    let source = self.0.borrow();

                    match &*source {
                        Source::Owned(value) => {
                            f(value);
                            return;
                        }
                        Source::Projected(accessor) => Rc::clone(accessor),
                    }
                };
                accessor.access(f);
            }

            fn access_mut(&self, f: &mut dyn FnMut(&mut T)) {
                let accessor = {
                    let mut source = self.0.borrow_mut();

                    match &mut *source {
                        Source::Owned(value) => {
                            f(value);
                            return;
                        }
                        Source::Projected(accessor) => Rc::clone(accessor),
                    }
                };
                accessor.access_mut(f);
            }
        }

        pub struct FieldAccess<Parent, T> {
            pub parent: Rc<dyn Accessor<Parent>>,
            pub get: fn(&Parent) -> &T,
            pub get_mut: fn(&mut Parent) -> &mut T,
        }

        impl<Parent: 'static, T: 'static> Accessor<T> for FieldAccess<Parent, T> {
            fn access(&self, f: &mut dyn FnMut(&T)) {
                self.parent.access(&mut |parent| f((self.get)(parent)))
            }
            fn access_mut(&self, f: &mut dyn FnMut(&mut T)) {
                self.parent
                    .access_mut(&mut |parent| f((self.get_mut)(parent)))
            }
        }
    }
    use __tethering::Accessor as _;
"#};

/// A tethered struct. See the module-level documentation for more details.
pub struct TetheredStruct<'a, FQN, Fields>
where
    FQN: Fragment,
{
    /// Fully qualified source type represented by the tethered struct.
    pub for_: FQN,
    /// Mangled name of the generated tethered struct.
    pub name: &'a str,
    /// Source fields exposed by the tethered struct.
    pub fields: Fields,
}

/// A tethered field. See the module-level documentation for more details.
#[derive(Clone, Copy)]
pub enum TetheredField<'a, FQN, GetRefExpr, GetMutExpr>
where
    FQN: Fragment,
{
    /// A field that holds a primitive-like value that is cloned when read.
    Cloned(ClonedField<'a, FQN, GetRefExpr, GetMutExpr>),
    /// A field that holds a value represented by another tethered struct.
    TetheredStruct(TetheredStructField<'a, FQN, GetRefExpr, GetMutExpr>),
}

#[derive(Clone, Copy)]
pub struct ClonedField<'a, FQN, GetRefExpr, GetMutExpr>
where
    FQN: Fragment,
{
    /// Name of the generated field accessor.
    pub name: &'a str,
    /// Expression to access a source value through a shared reference.
    pub get_ref_expr: GetRefExpr,
    /// Expression to access a source value through a mutable reference.
    pub get_mut_expr: GetMutExpr,
    /// Source type read from the field and accepted by its setter.
    pub type_: FQN,
}

#[derive(Clone, Copy)]
pub struct TetheredStructField<'a, FQN, GetRefExpr, GetMutExpr>
where
    FQN: Fragment,
{
    /// Name of the generated field accessor.
    pub name: &'a str,
    /// Expression to access a source value through a shared reference.
    pub get_ref_expr: GetRefExpr,
    /// Expression to access a source value through a mutable reference.
    pub get_mut_expr: GetMutExpr,
    /// Source type represented by the projected tether.
    pub type_: FQN,
    /// Generated tethered type representing the projected value.
    pub tethered_type: FQN,
    /// Whether to generate a setter for this value.
    pub with_setter: bool,
}

/// Compiles a source-backed struct with field accessors and cached projected
/// tethers.
///
/// ```
/// use rust_curse::codegen::{
///     bindings::tethering::{
///         ClonedField, TetheredField, TetheredStruct, compile_tethered_struct,
///     },
///     fragment::{FragmentPrint, frag},
/// };
///
/// let compiled = compile_tethered_struct(TetheredStruct {
///     for_: "crate::Example",
///     name: "__tethered_example",
///     fields: [TetheredField::Cloned(ClonedField {
///         name: "value",
///         get_ref_expr: |source: &'static str| frag!({ source }.value),
///         get_mut_expr: |source: &'static str| frag!({ source }.value),
///         type_: "i32",
///     })],
/// })
/// .print();
///
/// assert!(compiled.contains("struct __tethered_example"));
/// assert!(compiled.contains("pub fn get_value"));
/// assert!(compiled.contains("pub fn set_value"));
/// ```
pub fn compile_tethered_struct<
    'a,
    StructFQN,
    Fields,
    FieldFQN,
    GetRefExpr,
    RefExpr,
    GetMutExpr,
    MutExpr,
>(
    struct_: TetheredStruct<'a, StructFQN, Fields>,
) -> impl FragmentOnce + 'a
where
    StructFQN: Fragment + 'a,
    Fields: IntoIterator<Item = TetheredField<'a, FieldFQN, GetRefExpr, GetMutExpr>> + 'a,
    Fields::IntoIter: Clone,
    FieldFQN: Fragment + 'a,
    GetRefExpr: Fn(&'static str) -> RefExpr,
    RefExpr: FragmentOnce,
    GetMutExpr: Fn(&'static str) -> MutExpr,
    MutExpr: FragmentOnce,
{
    fragment(move |p| {
        let fields = struct_.fields.into_iter();
        let source_type = &struct_.for_;
        let name = struct_.name;

        let cache_field_definitions = {
            let fields = fields.clone();

            fragment(move |p| {
                for field in fields {
                    if let TetheredField::TetheredStruct(field) = field {
                        let cache_field_name = cache_field_name(field.name);

                        frag!(
                            {&cache_field_name}: std::cell::RefCell<Option<std::rc::Weak<{&field.tethered_type}>>>,@
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
                    if let TetheredField::TetheredStruct(field) = field {
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
                    if let TetheredField::TetheredStruct(field) = field {
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
                compile_tethered_field_getter(&field).apply(p);

                let with_setter = match &field {
                    TetheredField::Cloned(_) => true,
                    TetheredField::TetheredStruct(field) => field.with_setter,
                };

                if with_setter {
                    frag!(@@{compile_tethered_field_setter(&field)}).apply(p);
                }
            })
        }));

        frag!(
            struct ~{name}~{{
                src: __tethering::SourceSlot<{source_type}>,@
                {cache_field_definitions}
            }}@@

            impl __tethering::Accessor<{source_type}> for ~{name}~{{
                fn access(&self, f: &mut dyn FnMut(&{source_type}))~{{
                    self.src.access(f);
                }}@@

                fn access_mut(&self, f: &mut dyn FnMut(&mut~{source_type}))~{{
                    self.src.access_mut(f);
                }}
            }}@@

            impl ~{name}~{{
                pub fn new_owned(input: ~{source_type})~ -> Self ~{{
                    {name}~{{
                        src: __tethering::SourceSlot::owned(input),@
                        {owned_cache_initializers}
                    }}
                }}@@

                pub fn new_projected(accessor: std::rc::Rc<dyn __tethering::Accessor<{source_type}>>) -> Self~{{
                    {name}~{{
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

/// Compiles a call to a generated tethered field getter.
///
/// Expression is of:
///  - Rc of tethered struct type in case field holds tetherable source type
///  - A source type otherwise
///
/// ```
/// use rust_curse::codegen::{
///     bindings::tethering::compile_tethered_struct_get_field_expr, fragment::FragmentPrint,
/// };
///
/// assert_eq!(
///     compile_tethered_struct_get_field_expr("value", "name").print(),
///     "value.get_name()",
/// );
/// ```
pub fn compile_tethered_struct_get_field_expr<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    field_name: &'a str,
) -> impl FragmentOnce + 'a {
    frag!({tethered_struct_expr}.get_{field_name}())
}

/// Compiles a call to a generated tethered field setter.
///
/// Value should be of:
///  - Rc of tethered struct type in case field holds tetherable source type
///  - A source type otherwise
///
/// ```
/// use rust_curse::codegen::{
///     bindings::tethering::compile_tethered_struct_set_field_stmt, fragment::FragmentPrint,
/// };
///
/// let compiled = compile_tethered_struct_set_field_stmt("value", "name", "input").print();
/// assert!(compiled.starts_with("value.set_name("));
/// assert!(compiled.ends_with(");"));
/// ```
pub fn compile_tethered_struct_set_field_stmt<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    field_name: &'a str,
    value: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!({tethered_struct_expr}.set_{field_name}({value});)
}

/// Compiles a statement that accesses the complete source value behind a
/// tethered field by shared reference.
///
/// ```
/// use rust_curse::codegen::{
///     bindings::tethering::compile_tethered_struct_ref_access_stmt, fragment::FragmentPrint,
/// };
///
/// let compiled =
///     compile_tethered_struct_ref_access_stmt("value", "source", "consume(source);").print();
/// assert!(compiled.starts_with("value.access("));
/// ```
pub fn compile_tethered_struct_ref_access_stmt<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    out_var: &'a str,
    in_stmt: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!(
        {tethered_struct_expr}.access(&mut |{out_var}|~{{ {in_stmt} }});
    )
}

/// Compiles a statement that accesses the complete source value behind a
/// tethered field by mutable reference.
///
/// ```
/// use rust_curse::codegen::{
///     bindings::tethering::compile_tethered_struct_mut_access_stmt, fragment::FragmentPrint,
/// };
///
/// let compiled =
///     compile_tethered_struct_mut_access_stmt("value", "source", "mutate(source);").print();
/// assert!(compiled.starts_with("value.access_mut("));
/// ```
pub fn compile_tethered_struct_mut_access_stmt<'a>(
    tethered_struct_expr: impl FragmentOnce + 'a,
    out_var: &'a str,
    in_stmt: impl FragmentOnce + 'a,
) -> impl FragmentOnce + 'a {
    frag!(
        {tethered_struct_expr}.access_mut(&mut |{out_var}|~{{ {in_stmt} }});
    )
}

fn cache_field_name(name: &str) -> impl Fragment + '_ {
    frag!(__c__ { name })
}

impl<'a, FQN, GetRefExpr, GetMutExpr> ClonedField<'a, FQN, GetRefExpr, GetMutExpr>
where
    FQN: Fragment,
{
    fn compile_getter<RefExpr>(&self) -> impl FragmentOnce + '_
    where
        GetRefExpr: Fn(&'static str) -> RefExpr,
        RefExpr: FragmentOnce,
    {
        fragment(move |p| {
            let get_ref_expr = (self.get_ref_expr)("src");

            frag!(
                pub fn get_{self.name}(&self) -> ~{&self.type_}~{{
                    let mut result = std::mem::MaybeUninit::uninit();@
                    self.access(&mut |src|~{{
                        result.write({get_ref_expr}.clone());
                    }});@
                    unsafe~{{
                        result.assume_init()
                    }}
                }}
            )
            .apply(p);
        })
    }

    fn compile_setter<MutExpr>(&self) -> impl FragmentOnce + '_
    where
        GetMutExpr: Fn(&'static str) -> MutExpr,
        MutExpr: FragmentOnce,
    {
        fragment(move |p| {
            let get_mut_expr = (self.get_mut_expr)("src");

            frag!(
                pub fn set_{self.name}(&self, value: ~{&self.type_})~{{
                    let mut value = Some(value);@
                    self.access_mut(&mut |src|~{{
                        {get_mut_expr} = value.take().unwrap();
                    }});
                }}
            )
            .apply(p);
        })
    }
}

impl<'a, FQN, GetRefExpr, GetMutExpr> TetheredStructField<'a, FQN, GetRefExpr, GetMutExpr>
where
    FQN: Fragment,
{
    fn compile_getter<RefExpr, MutExpr>(&self) -> impl FragmentOnce + '_
    where
        GetRefExpr: Fn(&'static str) -> RefExpr,
        RefExpr: FragmentOnce,
        GetMutExpr: Fn(&'static str) -> MutExpr,
        MutExpr: FragmentOnce,
    {
        fragment(move |p| {
            let cache_field_name = cache_field_name(self.name);
            let get_ref_expr = (self.get_ref_expr)("parent");
            let get_mut_expr = (self.get_mut_expr)("parent");

            frag!(
                pub fn get_{self.name}(self: &std::rc::Rc<Self>) -> std::rc::Rc<{&self.tethered_type}>~{{
                    if let Some(tethered)~ = self.{&cache_field_name}.borrow().as_ref().and_then(std::rc::Weak::upgrade)~{{
                        return tethered;@
                    }}@@

                    let field_access: std::rc::Rc<dyn __tethering::Accessor<{&self.type_}>> = std::rc::Rc::new(__tethering::FieldAccess~{{
                        parent: self.clone(),
                        get: |parent| &{get_ref_expr},
                        get_mut: |parent| &mut {get_mut_expr},
                    }});@@

                    let tethered = std::rc::Rc::new({&self.tethered_type}::new_projected(field_access));@@

                    *self.{&cache_field_name}.borrow_mut() = Some(std::rc::Rc::downgrade(&tethered));

                    tethered
                }}
            )
            .apply(p);
        })
    }

    fn compile_setter<MutExpr>(&self) -> impl FragmentOnce + '_
    where
        GetMutExpr: Fn(&'static str) -> MutExpr,
        MutExpr: FragmentOnce,
    {
        fragment(move |p| {
            let cache_field_name = cache_field_name(self.name);
            let get_mut_expr = (self.get_mut_expr)("src");

            frag!(
                pub fn set_{self.name}(&self, value: ~{&self.type_})~{{
                    let cached_value = self.{&cache_field_name}
                        .borrow_mut()
                        .take()
                        .and_then(|tethered| tethered.upgrade());@@

                    let mut old_value = None;@
                    let mut value = Some(value);@@

                    self.access_mut(&mut |src|~{{
                        old_value = Some(std::mem::replace(
                            &mut {get_mut_expr},
                            value.take().unwrap(),
                        ));
                    }});@@

                    if let Some(cached_value)~= cached_value~{{
                        cached_value.src.replace_with_owned(old_value.unwrap());
                    }}
                }}
            )
            .apply(p);
        })
    }
}

fn compile_tethered_field_getter<'field, 'a, FQN, GetRefExpr, RefExpr, GetMutExpr, MutExpr>(
    field: &'field TetheredField<'a, FQN, GetRefExpr, GetMutExpr>,
) -> impl FragmentOnce + 'field
where
    FQN: Fragment,
    GetRefExpr: Fn(&'static str) -> RefExpr,
    RefExpr: FragmentOnce,
    GetMutExpr: Fn(&'static str) -> MutExpr,
    MutExpr: FragmentOnce,
{
    fragment(move |p| match field {
        TetheredField::Cloned(field) => field.compile_getter().apply(p),
        TetheredField::TetheredStruct(field) => field.compile_getter().apply(p),
    })
}

fn compile_tethered_field_setter<'field, 'a, FQN, GetRefExpr, GetMutExpr, MutExpr>(
    field: &'field TetheredField<'a, FQN, GetRefExpr, GetMutExpr>,
) -> impl FragmentOnce + 'field
where
    FQN: Fragment,
    GetMutExpr: Fn(&'static str) -> MutExpr,
    MutExpr: FragmentOnce,
{
    fragment(move |p| match field {
        TetheredField::Cloned(field) => field.compile_setter().apply(p),
        TetheredField::TetheredStruct(field) => field.compile_setter().apply(p),
    })
}
