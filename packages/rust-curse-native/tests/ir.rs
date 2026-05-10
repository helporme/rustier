#[macro_use]
#[path = "ir/common.rs"]
mod common;

#[path = "ir/negative.rs"]
mod negative;

#[test]
fn fn_simple_add() {
    assert_snap! {
        pub fn add(a: i32, b: i32) -> i32 { a + b }
    };
}

#[test]
fn fn_two_same_primitive_params() {
    assert_snap! {
        pub fn two_i32(a: i32, b: i32) {}
    };
}

#[test]
fn fn_two_share_primitive_across_fns() {
    assert_snap! {
        pub fn f(x: i32) {}
        pub fn g(y: i32) {}
    };
}

#[test]
fn fn_all_primitive_types() {
    assert_snap! {
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
    assert_snap! {
        pub fn f(a: i64, b: u64, c: isize, d: usize) {}
    };
}

#[test]
fn fn_borrow_mode_mut_only() {
    assert_snap! {
        pub fn takes(x: &mut i32) { *x += 1; }
    };
}

#[test]
fn fn_borrow_modes_primitive() {
    assert_snap! {
        pub fn f(by_ref: &i32, by_mut_ref: &mut i32, by_value: i32) {}
    };
}

#[test]
fn fn_borrow_modes_struct() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub fn translate(origin: &Point, target: &mut Point, offset: Point) {}
    };
}

#[test]
fn fn_return_void() {
    assert_snap! {
        pub fn f() {}
    };
}

#[test]
fn fn_returns_struct() {
    assert_snap! {
        /// A point in 2D space.
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

#[test]
fn struct_plain() {
    assert_snap! {
        /// A point in 2D space.
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
    };
}

#[test]
fn struct_used_by_value() {
    assert_snap! {
        pub struct Point {
            pub x: i32,
        }
        pub fn use_it(p: Point) {}
    };
}

#[test]
fn struct_opaque() {
    assert_snap! {
        /// A handle with a hidden internal state.
        pub struct Handle {
            id: u32,
        }
    };
}

#[test]
fn struct_unit() {
    assert_snap! {
        pub struct Marker;
    };
}

#[test]
fn struct_empty() {
    assert_snap! {
        pub struct Empty {}
    };
}

#[test]
fn struct_tuple() {
    assert_snap! {
        /// A pair of values stored as a tuple struct.
        pub struct Pair(pub i32, pub String);
    };
}

#[test]
fn struct_mixed_pub_priv_fields() {
    assert_snap! {
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    };
}

#[test]
fn struct_with_struct_field() {
    assert_snap! {
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

#[test]
fn struct_pub_crate_field_forces_ref() {
    assert_snap! {
        pub struct Gated {
            pub x: i32,
            pub(crate) y: i32,
        }
    };
}

#[test]
fn struct_intern_across_uses() {
    assert_snap! {
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        pub struct Wrapper {
            pub p: Point,
        }
        pub fn take(p: Point) {}
        pub fn produce() -> Point { Point { x: 0, y: 0 } }
    };
}

#[test]
fn module_nested_fn() {
    assert_snap! {
        pub mod math {
            /// Returns the square of `x`.
            pub fn square(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn module_two_fns() {
    assert_snap! {
        pub mod math {
            pub fn square(x: f64) -> f64 { x * x }
            pub fn square2(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn module_root_two_fns() {
    assert_snap! {
        pub fn square(x: f64) -> f64 { x * x }
        pub fn square2(x: f64) -> f64 { x * x }
    };
}

#[test]
fn module_root_two_structs() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub struct Point2 {
            pub x: f64,
            pub y: f64,
        }
    };
}

#[test]
fn module_cross_import() {
    assert_snap! {
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
fn module_deep_nested() {
    assert_snap! {
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
fn module_only_submodule_exports() {
    assert_snap! {
        pub mod inner {
            pub fn f() {}
        }
    };
}

#[test]
fn module_duplicate_struct_names() {
    assert_snap! {
        pub mod a {
            pub struct Thing { pub x: i32 }
        }
        pub mod b {
            pub struct Thing { pub y: i32 }
        }
    };
}

#[test]
fn module_empty_pruned() {
    assert_snap! {
        pub mod kept {
            pub fn stays() {}
        }
        pub mod pruned {}
    };
}

#[test]
fn module_nested_tree() {
    assert_snap! {
        pub mod child {
            pub fn leaf() {}
        }
        pub mod empty {}
    };
}

#[test]
fn re_export_struct() {
    assert_snap! {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner;
    };
}

#[test]
fn re_export_struct_renamed() {
    assert_snap! {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner as Renamed;
    };
}

#[test]
fn re_export_glob() {
    assert_snap! {
        pub mod inner {
            pub struct S { pub x: i32 }
            pub fn f() -> i32 { 0 }
        }
        pub use inner::*;
    };
}

#[test]
fn re_export_glob_transitive() {
    assert_snap! {
        pub mod deeper {
            pub struct X { pub x: i32 }
        }
        pub mod inner {
            pub use super::deeper::X;
        }
        pub use inner::*;
    };
}

#[test]
fn re_export_chain() {
    assert_snap! {
        pub mod inner {
            pub struct Inner { pub x: i32 }
        }
        pub mod middle {
            pub use super::inner::Inner;
        }
        pub use middle::Inner;
    };
}

#[test]
fn re_export_primitive_skipped() {
    assert_snap! {
        pub use i32 as MyI32;
    };
}

#[test]
fn re_export_renamed_swap() {
    assert_snap! {
        pub mod inner {
            pub struct Inner { pub x: i32 }
            pub struct Outer { pub y: i32 }
        }
        pub use inner::{Inner as Outer, Outer as Inner};
    };
}

#[test]
fn ctor_explicit_new() {
    assert_snap! {
        pub struct Opaque {
            x: i32,
        }
        impl Opaque {
            pub fn new(x: i32) -> Self { Self { x } }
        }
    };
}

#[test]
fn ctor_new_returning_struct_name() {
    assert_snap! {
        pub struct Token {
            value: String,
        }
        impl Token {
            pub fn new(raw: String) -> Token { Token { value: raw } }
        }
    };
}

#[test]
fn ctor_new_with_self_is_implicit() {
    assert_snap! {
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
    assert_snap! {
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
fn ctor_new_in_trait_impl() {
    assert_snap! {
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

#[test]
fn ctor_multiple_new_across_traits() {
    assert_snap! {
        pub mod zeta {
            pub trait Factory { fn new(from_zeta: i32) -> Self; }
        }
        pub mod alpha {
            pub trait Factory { fn new(from_alpha: u64) -> Self; }
        }
        pub struct Foo { x: i32 }
        impl zeta::Factory for Foo { fn new(from_zeta: i32) -> Self { Foo { x: from_zeta } } }
        impl alpha::Factory for Foo { fn new(from_alpha: u64) -> Self { Foo { x: from_alpha as i32 } } }
    };
}
