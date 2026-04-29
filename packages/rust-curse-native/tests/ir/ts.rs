// ── functions ────────────────────────────────────────────────────────────────

#[test]
fn fn_primitives() {
    assert_ts_snap! {
        /// Exercises every primitive type that maps to `number` or `string` or `boolean`.
        pub fn f(
            a: i8, b: i16, c: i32,
            d: u8, e: u16, f: u32,
            g: f32, h: f64,
            i: bool,
            j: char, k: String, l: &str,
        ) {}
    };
}

#[test]
fn fn_bigint_types() {
    assert_ts_snap! {
        /// 64-bit and pointer-sized integers map to `bigint`.
        pub fn f(a: i64, b: u64, c: isize, d: usize) {}
    };
}

#[test]
fn fn_borrow_modes_primitive() {
    assert_ts_snap! {
        /// Borrow mode does not affect the TypeScript type for primitives.
        pub fn f(by_ref: &i32, by_mut_ref: &mut i32, by_value: i32) {}
    };
}

#[test]
fn fn_borrow_modes_struct() {
    assert_ts_snap! {
        /// By-value nominal types are wrapped in `Copy<>`, borrows are not.
        pub struct Point {
            /// X coordinate.
            pub x: f64,
            /// Y coordinate.
            pub y: f64,
        }
        pub fn translate(origin: &Point, target: &mut Point, offset: Point) {}
    };
}

#[test]
fn fn_return_void() {
    assert_ts_snap! {
        /// Unit return type maps to `void`.
        pub fn f() {}
    };
}

#[test]
fn fn_returns_struct() {
    assert_ts_snap! {
        /// Returning a user-defined struct surfaces the class type as the return type.
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
        /// Returns the origin point `(0, 0)`.
        pub fn make_origin() -> Point { Point { x: 0.0, y: 0.0 } }
    };
}

// ── structs ──────────────────────────────────────────────────────────────────

#[test]
fn struct_plain() {
    assert_ts_snap! {
        /// A point in 2D space. All fields are public, so it is copy-constructible.
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
    };
}

#[test]
fn struct_opaque() {
    assert_ts_snap! {
        /// A handle with a hidden internal state and no explicit constructor.
        pub struct Handle {
            id: u32,
        }
    };
}

#[test]
fn struct_opaque_ctor() {
    assert_ts_snap! {
        /// A counter whose constructor is the canonical entry point.
        pub struct Counter {
            count: u32,
        }
        impl Counter {
            /// Creates a new counter starting at `start`.
            pub fn new(start: u32) -> Self { Self { count: start } }
        }
    };
}

#[test]
fn struct_tuple() {
    assert_ts_snap! {
        /// A pair of values stored as a tuple struct.
        pub struct Pair(pub i32, pub String);
    };
}

#[test]
fn struct_with_struct_field() {
    assert_ts_snap! {
        /// A point in 2D space.
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
        /// A circle defined by its center and radius.
        pub struct Circle {
            /// The center point.
            pub center: Point,
            /// The radius in the same units as `center`.
            pub radius: f64,
        }
    };
}

// ── modules ──────────────────────────────────────────────────────────────────

#[test]
fn module_nested() {
    assert_ts_snap! {
        pub mod math {
            /// Returns the square of `x`.
            pub fn square(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn module_cross_import() {
    assert_ts_snap! {
        pub mod types {
            /// An authentication token.
            pub struct Token {
                /// The raw token string.
                pub value: String,
            }
        }
        pub mod ops {
            /// Parses a raw string into a Token.
            pub fn parse(s: String) -> crate::types::Token { todo!() }
        }
    };
}

#[test]
fn unit_struct() {
    assert_ts_snap! {
        pub struct Marker;
    };
}

#[test]
fn empty_struct_plain() {
    assert_ts_snap! {
        pub struct Empty {}
    };
}

#[test]
fn fn_smart_pointer_inlining() {
    assert_ts_snap! {
        /// Smart pointers are erased, so every parameter maps to `number`.
        pub fn f(
            a: Box<i32>,
            b: std::sync::Arc<i32>,
            c: std::rc::Rc<i32>,
            d: std::cell::Cell<i32>,
            e: std::cell::RefCell<i32>,
        ) {}
    };
}

#[test]
fn struct_pub_crate_field_forces_ref() {
    assert_ts_snap! {
        /// A `pub(crate)` field downgrades the boundary to ref (opaque class).
        pub struct Gated {
            pub x: i32,
            pub(crate) y: i32,
        }
    };
}

#[test]
fn struct_mixed_pub_priv_fields() {
    assert_ts_snap! {
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    };
}

#[test]
fn module_deep_nested() {
    assert_ts_snap! {
        pub mod a {
            pub mod b {
                pub mod c {
                    pub fn deep() {}
                }
            }
        }
    };
}

#[test]
fn module_duplicate_struct_names() {
    assert_ts_snap! {
        pub mod a {
            pub struct Thing { pub x: i32 }
        }
        pub mod b {
            pub struct Thing { pub y: i32 }
        }
    };
}

#[test]
fn module_re_export_of_struct() {
    assert_ts_snap! {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner;
    };
}

#[test]
fn module_re_export_renamed_struct() {
    assert_ts_snap! {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner as Renamed;
    };
}

#[test]
fn module_re_export_glob() {
    assert_ts_snap! {
        pub mod inner {
            pub struct S { pub x: i32 }
            pub fn f() -> i32 { 0 }
        }
        pub use inner::*;
    };
}

#[test]
fn ctor_new_returning_struct_name() {
    assert_ts_snap! {
        /// `new() -> Token` is recognised as an explicit constructor. The `raw` param name
        /// differs from the field name `value` so the snapshot proves the ctor is picked
        /// up rather than synthesised from the struct's fields.
        pub struct Token {
            value: String,
        }
        impl Token {
            pub fn new(raw: String) -> Token { Token { value: raw } }
        }
    };
}

#[test]
fn ctor_new_with_self_arg_is_implicit() {
    assert_ts_snap! {
        /// `new(&self)` is not treated as an explicit ctor.
        pub struct Builder {
            x: i32,
        }
        impl Builder {
            pub fn new(&self) -> Self { Self { x: self.x } }
        }
    };
}

#[test]
fn ctor_multiple_impls_finds_new() {
    assert_ts_snap! {
        /// The `seed` param name differs from the field `count` so the snapshot proves
        /// the ctor from the second impl block is picked up.
        pub struct Counter {
            count: u32,
        }
        impl Counter {
            pub fn increment(&mut self) { self.count += 1; }
        }
        impl Counter {
            pub fn new(seed: u32) -> Self { Counter { count: seed } }
        }
    };
}

#[test]
fn ctor_new_in_trait_impl_is_explicit() {
    assert_ts_snap! {
        /// `new()` in a trait impl is recognised as an explicit constructor. The `seed`
        /// param name differs from the field `x` so the snapshot proves the trait-impl
        /// ctor is picked up rather than synthesised from the struct's fields.
        pub trait Factory {
            fn new(seed: u32) -> Self;
        }
        pub struct Foo {
            x: i32,
        }
        impl Factory for Foo {
            fn new(seed: u32) -> Self { Foo { x: seed as i32 } }
        }
    };
}
