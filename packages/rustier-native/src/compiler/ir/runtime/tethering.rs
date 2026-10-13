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
//! However, it is impractical for rustier to use this design.
//! It would require generating a "binding struct" with the same fields as the
//! source struct, but with all non-copy fields wrapped in `Arc<Mutex<T>>`.
//! We cannot pass the binding struct directly to the source code, so we would
//! need to convert it back to the source struct. This conversion breaks, for
//! example, mutable borrowing through `&mut` references. Other libraries use
//! macros to transform the source struct in place, but rustier should work
//! with unannotated Rust code.
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
//! unchanged. The runtime IR describes tethering structs and fields; this
//! module renders their Rust implementation.

use crate::compiler::{
    db::Db,
    ir::{
        interface::InterClass,
        runtime::RuntimeType,
        rust::{FieldQualified, TypeKind},
    },
};

#[salsa::interned(debug)]
pub struct RuntimeTethering<'db> {
    pub source: InterClass<'db>,
}

#[salsa::tracked]
impl<'db> RuntimeTethering<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<RuntimeTetheredField<'db>> {
        self.source(db)
            .fields(db)
            .iter()
            .map(|field| RuntimeTetheredField::new(db, field.src(db)))
            .collect()
    }
}

#[salsa::interned(debug)]
pub struct RuntimeTetheredField<'db> {
    pub src: FieldQualified<'db>,
}

impl<'db> RuntimeTetheredField<'db> {
    pub fn parent(self, db: &'db dyn Db) -> RuntimeTethering<'db> {
        let class: InterClass<'db> = match self.src(db).target(db).kind(db) {
            TypeKind::Struct(struct_) => struct_.interface(db).into(),
            TypeKind::Enum(enum_) => enum_.interface(db).into(),
            _ => unreachable!("fields belong to classes"),
        };

        RuntimeTethering::new(db, class)
    }

    pub fn kind(self, db: &'db dyn Db) -> RuntimeTetheredFieldKind<'db> {
        match self.src(db)._type(db).tethering(db) {
            Some(tethering) => RuntimeTetheredFieldKind::Tethered(tethering),
            None => RuntimeTetheredFieldKind::Cloned,
        }
    }

    pub fn value_type(self, db: &'db dyn Db) -> RuntimeType<'db> {
        match self.kind(db) {
            RuntimeTetheredFieldKind::Cloned => RuntimeType::Source(self.src(db)._type(db)),
            RuntimeTetheredFieldKind::Tethered(tethering) => RuntimeType::Tethered(tethering),
        }
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.src(db).is_readonly(db)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum RuntimeTetheredFieldKind<'db> {
    /// Returned value is cloned on get (e.g primitive, string)
    Cloned,
    /// Returned value is tethered (e.g struct, enum)
    Tethered(RuntimeTethering<'db>),
}
