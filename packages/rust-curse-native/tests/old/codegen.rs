#[macro_use]
#[path = "codegen/common.rs"]
mod common;

#[path = "codegen/negative.rs"]
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
        #[derive(Clone)]
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
        #[derive(Clone)]
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
        /// A circle defined by its center and radius.
        #[derive(Clone)]
        pub struct Circle {
            /// The center point.
            pub center: Point,
            /// The radius in the same units as `center`.
            pub radius: f64,
        }
    };
}

#[test]
fn struct_clone_derive() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Cloneable {
            pub x: i32,
        }
        pub struct NotCloneable {
            pub x: i32,
        }
    };
}

#[test]
fn struct_clone_manual_impl() {
    assert_snap! {
        pub struct Manual {
            pub x: i32,
        }
        impl Clone for Manual {
            fn clone(&self) -> Self { Manual { x: self.x } }
        }
    };
}

#[test]
fn struct_intern_across_uses() {
    assert_snap! {
        #[derive(Clone)]
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
fn struct_field_projection_nested() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        #[derive(Clone)]
        pub struct Line {
            pub start: Point,
            pub end: Point,
        }
    };
}

#[test]
fn struct_field_projection_deep() {
    assert_snap! {
        #[derive(Clone)]
        pub struct C {
            pub x: u32,
        }
        #[derive(Clone)]
        pub struct B {
            pub c: C,
        }
        #[derive(Clone)]
        pub struct A {
            pub b: B,
        }
    };
}

#[test]
fn warn_non_clonable_struct_as_field() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub struct Circle {
            pub center: Point,
            pub radius: f64,
        }
    };
}

#[test]
fn warn_non_clonable_struct_as_byvalue_param() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub fn take(p: Point) {}
    };
}

#[test]
fn nonclonable_byref_no_warn() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub fn inspect(p: &Point) {}
    };
}

#[test]
fn struct_string_field() {
    assert_snap! {
        pub struct Token {
            pub value: String,
        }
    };
}

#[test]
fn fn_struct_ref_with_string_param() {
    assert_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub fn label(p: &Point, name: String) {}
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

#[test]
fn warn_unsupported_type_on_fn_param() {
    assert_snap! {
        pub fn f(v: Vec<i32>) {}
    };
}

#[test]
fn warn_unsupported_type_on_fn_return() {
    assert_snap! {
        pub fn f() -> Option<i32> { None }
    };
}

#[test]
fn enum_all_unit_unsupported() {
    assert_snap! {
        pub enum E { A, B, C }
    };
}

#[test]
fn enum_data_two_struct_variants() {
    assert_snap! {
        #[derive(Clone)]
        pub enum ProfileBuildOutput {
            Wasm { wasm_abs_path: String },
            Node { node_path: String },
            Skipped,
        }
    };
}

#[test]
fn enum_data_with_methods_and_ctor() {
    assert_snap! {
        #[derive(Clone)]
        pub enum E {
            On { level: f64 },
            Off,
        }

        impl E {
            pub fn new(level: f64) -> Self { Self::On { level } }
            pub fn describe(&self) -> String { String::new() }
        }
    };
}

#[test]
fn enum_data_type_escape() {
    assert_snap! {
        #[derive(Clone)]
        pub enum Event {
            Click { r#type: String, x: i32 },
        }
    };
}

#[test]
fn enum_data_shared_field_name() {
    assert_snap! {
        #[derive(Clone)]
        pub enum Value {
            A { data: f64 },
            B { data: f64 },
            C,
        }
    };
}

#[test]
fn enum_tuple_variant_unsupported() {
    assert_snap! {
        pub enum E {
            A(f64),
            B { x: f64 },
        }
    };
}

#[test]
fn enum_data_non_clonable_ref_only() {
    assert_snap! {
        pub enum E {
            On { level: f64 },
            Off,
        }
    };
}

#[test]
fn warn_unsupported_type_on_struct_field() {
    assert_snap! {
        pub struct S {
            pub items: Vec<i32>,
        }
    };
}

#[test]
fn warn_unsupported_import_external_crate() {
    assert_snap! {
        pub use std::collections::HashMap;
    };
}

#[test]
fn warn_unsupported_import_glob() {
    assert_snap! {
        pub use std::collections::*;
    };
}

#[test]
fn warn_unsupported_ctor_returning_result() {
    assert_snap! {
        pub struct S {
            x: i32,
        }
        impl S {
            pub fn new() -> Result<Self, String> { Ok(Self { x: 0 }) }
        }
    };
}

#[test]
fn warn_mixed_some_items_skipped() {
    assert_snap! {
        pub fn good(a: i32, b: i32) -> i32 { a + b }
        pub fn bad(v: Vec<i32>) {}
        pub use std::mem::swap;
    };
}

#[test]
fn of_object_only() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
    };
}

#[test]
fn of_and_new() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn new(x: i32, y: i32) -> Self { Self { x, y } }
        }
    };
}

#[test]
fn of_unavailable_private_field() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    };
}

#[test]
fn of_nested_object() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        #[derive(Clone)]
        pub struct Line {
            pub start: Point,
            pub end: Point,
        }
        #[derive(Clone)]
        pub struct Triangle {
            pub a: Line,
            pub b: Point,
        }
    };
}

#[test]
fn fn_accepts_object_literal() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        #[derive(Clone)]
        pub struct Line {
            pub start: Point,
            pub end: Point,
        }
        #[derive(Clone)]
        pub struct Triangle {
            pub a: Line,
            pub b: Point,
        }
        pub fn triangle_area(t: Triangle) -> f64 { 0.0 }
    };
}

#[test]
fn impl_static_method() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn make_default() -> i32 { 42 }
        }
    };
}

#[test]
fn impl_static_returns_self() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        impl Point {
            pub fn make(x: f64, y: f64) -> Self {
                Self { x, y }
            }
        }
    };
}

#[test]
fn impl_static_method_named_new() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Box {
            pub width: i32,
        }
        impl Box {
            pub fn new(width: i32) -> i32 { width * 2 }
        }
    };
}

#[test]
fn impl_instance_method_ref() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn distance(&self, other: &Self) -> i32 {
                let dx = self.x - other.x;
                let dy = self.y - other.y;
                dx * dx + dy * dy
            }
        }
    };
}

#[test]
fn impl_instance_method_mut() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Counter {
            pub count: i32,
        }
        impl Counter {
            pub fn add(&mut self, by: i32) {
                self.count += by;
            }
        }
    };
}

#[test]
fn impl_instance_returns_self() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        impl Point {
            pub fn move_towards(&self, t: &Self, s: f64) -> Self {
                Self { x: self.x + (t.x - self.x) * s, y: self.y + (t.y - self.y) * s }
            }
        }
    };
}

#[test]
fn impl_self_by_value() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Counter {
            pub count: i32,
        }
        impl Counter {
            pub fn consume(self) -> i32 { self.count }
        }
    };
}

#[test]
fn impl_self_by_value_non_clone() {
    assert_snap! {
        pub struct Counter {
            pub count: i32,
        }
        impl Counter {
            pub fn consume(self) -> i32 { self.count }
        }
    };
}

#[test]
fn impl_multiple_blocks() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn dot(&self, other: &Self) -> i32 { self.x * other.x + self.y * other.y }
        }
        impl Point {
            pub fn negate(&self) -> Self { Self { x: -self.x, y: -self.y } }
        }
    };
}

#[test]
fn impl_of_collision() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn of(x: i32) -> Self {
                Self { x, y: x }
            }
        }
    };
}

#[test]
fn impl_of_no_collision_when_no_struct_expr() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Mat2x2 {
            _00: i32,
            _01: i32,
            _10: i32,
            _11: i32,
        }
        impl Mat2x2 {
            pub fn of(diag: i32) -> Self {
                Self { _00: diag, _01: 0, _10: 0, _11: diag }
            }
        }
    };
}

#[test]
fn impl_method_shadows_field() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn x(&self) -> i32 { self.x }
        }
    };
}

#[test]
fn impl_trait_method_ignored() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
        }
        pub trait Visible {
            fn render(&self) -> i32;
        }
        impl Visible for Point {
            fn render(&self) -> i32 { self.x }
        }
    };
}

#[test]
fn impl_method_on_struct_with_private_field() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
        impl Mixed {
            pub fn sum(&self) -> i32 { self.x + self.y }
        }
    };
}

#[test]
fn impl_method_mut_self_and_mut_param() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Counter {
            pub count: i32,
        }
        impl Counter {
            pub fn merge(&mut self, other: &mut Self) {
                self.count += other.count;
                other.count = 0;
            }
        }
    };
}

#[test]
fn impl_static_returns_self_taking_ref_self() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        impl Point {
            pub fn make_relative_to(p: &Self) -> Self {
                Self { x: p.x + 1, y: p.y + 1 }
            }
        }
    };
}

#[test]
fn enum_data_class_typed_variant_field() {
    assert_snap! {
        #[derive(Clone)]
        pub struct Mesh { pub x: f64 }

        #[derive(Clone)]
        pub enum Hitbox {
            Active { mesh: Mesh },
            Disabled,
        }
    };
}

#[test]
fn enum_data_class_field_getter_only() {
    // `Mesh` is not `Clone`, so the class-typed variant field's setter is
    // suppressed and only the getter is declared.
    assert_snap! {
        pub struct Mesh { pub x: f64 }

        pub enum Hitbox {
            Active { mesh: Mesh },
            Disabled,
        }
    };
}

#[test]
fn enum_data_zero_field_struct_variant() {
    // `Empty {}` is struct-shaped (NOT a unit), so it surfaces as
    // `{ type: 'empty' }` and its match arm is spelled `E::Empty { .. }`,
    // while the true unit `Unit` is spelled `E::Unit`.
    assert_snap! {
        #[derive(Clone)]
        pub enum E {
            Full { x: f64 },
            Empty {},
            Unit,
        }
    };
}
