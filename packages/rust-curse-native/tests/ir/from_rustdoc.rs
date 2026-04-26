#[test]
fn builds_root_only_with_one_fn() {
    assert_ir_snap!(["add"] => {
        pub fn add(a: i32, b: i32) -> i32 { a + b }
    });
}

#[test]
fn interns_primitives_once() {
    assert_ir_snap!(["two_i32"] => {
        pub fn two_i32(a: i32, b: i32) {}
    });
}

#[test]
fn all_primitive_types() {
    assert_ir_snap!(["f"] => {
        pub fn f(
            a: i8, b: i16, c: i32,
            d: u8, e: u16, f: u32,
            g: f32, h: f64,
            i: bool,
            j: char, k: String, l: &str,
        ) {}
    });
}

#[test]
fn bigint_types() {
    assert_ir_snap!(["f"] => {
        pub fn f(a: i64, b: u64, c: isize, d: usize) {}
    });
}

#[test]
fn borrow_mode_captured() {
    assert_ir_snap!(["takes"] => {
        pub fn takes(x: &mut i32) { *x += 1; }
    });
}

#[test]
fn borrow_modes_primitive() {
    assert_ir_snap!(["f"] => {
        pub fn f(by_ref: &i32, by_mut_ref: &mut i32, by_value: i32) {}
    });
}

#[test]
fn borrow_modes_struct() {
    assert_ir_snap!(["translate"] => {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }
        pub fn translate(origin: &Point, target: &mut Point, offset: Point) {}
    });
}

#[test]
fn return_void() {
    assert_ir_snap!(["f"] => {
        pub fn f() {}
    });
}

#[test]
fn fn_returns_struct() {
    assert_ir_snap!(["Point", "make_origin"] => {
        /// A point in 2D space.
        pub struct Point {
            /// Horizontal position.
            pub x: f64,
            /// Vertical position.
            pub y: f64,
        }
        /// Returns the origin point `(0, 0)`.
        pub fn make_origin() -> Point { Point { x: 0.0, y: 0.0 } }
    });
}

#[test]
fn plain_struct_all_pub_is_copy() {
    assert_ir_snap!(["Point"] => {
        pub struct Point {
            pub x: i32,
        }
    });
}

#[test]
fn plain_struct_private_field_is_ref_implicit() {
    assert_ir_snap!(["Opaque"] => {
        pub struct Opaque {
            x: i32,
        }
    });
}

#[test]
fn explicit_new_is_ctor_explicit() {
    assert_ir_snap!(["Opaque"] => {
        pub struct Opaque {
            x: i32,
        }
        impl Opaque {
            pub fn new(x: i32) -> Self { Self { x } }
        }
    });
}

#[test]
fn tuple_struct() {
    assert_ir_snap!(["Pair"] => {
        pub struct Pair(pub i32, pub String);
    });
}

#[test]
fn nested_module_tree() {
    assert_ir_snap!(["leaf"] => {
        pub mod child {
            pub fn leaf() {}
        }
        pub mod empty {}
    });
}

#[test]
fn cross_module_types() {
    assert_ir_snap!(["Token", "parse"] => {
        pub mod types {
            pub struct Token {
                pub value: String,
            }
        }
        pub mod ops {
            pub fn parse(s: String) -> crate::types::Token { todo!() }
        }
    });
}

#[test]
fn unit_struct() {
    assert_ir_snap!(["Marker"] => {
        pub struct Marker;
    });
}

#[test]
fn empty_struct_plain() {
    assert_ir_snap!(["Empty"] => {
        pub struct Empty {}
    });
}

#[test]
fn smart_pointer_inlining() {
    assert_ir_snap!(["f"] => {
        pub fn f(
            a: Box<i32>,
            b: std::sync::Arc<i32>,
            c: std::rc::Rc<i32>,
            d: std::cell::Cell<i32>,
            e: std::cell::RefCell<i32>,
        ) {}
    });
}

#[test]
fn primitives_intern_across_fns() {
    assert_ir_snap!(["f", "g"] => {
        pub fn f(x: i32) {}
        pub fn g(y: i32) {}
    });
}

#[test]
fn struct_intern_across_uses() {
    assert_ir_snap!(["take", "produce", "Wrapper"] => {
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        pub struct Wrapper {
            pub p: Point,
        }
        pub fn take(p: Point) {}
        pub fn produce() -> Point { Point { x: 0, y: 0 } }
    });
}

#[test]
fn struct_with_struct_field() {
    assert_ir_snap!(["Point", "Circle"] => {
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
    });
}

#[test]
fn struct_mixed_pub_priv_fields() {
    assert_ir_snap!(["Mixed"] => {
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    });
}

#[test]
fn ctor_new_with_self_arg_is_implicit() {
    assert_ir_snap!(["Builder"] => {
        pub struct Builder {
            x: i32,
        }
        impl Builder {
            pub fn new(&self) -> Self { Self { x: self.x } }
        }
    });
}

#[test]
fn ctor_new_in_trait_impl_is_explicit() {
    assert_ir_snap!(["Foo"] => {
        pub trait Factory {
            fn new() -> Self;
        }
        pub struct Foo {
            x: i32,
        }
        impl Factory for Foo {
            fn new() -> Self { Foo { x: 0 } }
        }
    });
}

#[test]
fn ctor_multiple_impls_finds_new() {
    assert_ir_snap!(["Counter"] => {
        pub struct Counter {
            count: u32,
        }
        impl Counter {
            pub fn increment(&mut self) { self.count += 1; }
        }
        impl Counter {
            pub fn new() -> Self { Counter { count: 0 } }
        }
    });
}

#[test]
fn ctor_multiple_new_across_traits_picks_smallest_path() {
    assert_ir_snap!(["Foo"] => {
        pub mod zeta {
            pub trait Factory { fn new(from_zeta: i32) -> Self; }
        }
        pub mod alpha {
            pub trait Factory { fn new(from_alpha: u64) -> Self; }
        }
        pub struct Foo { x: i32 }
        impl zeta::Factory for Foo { fn new(from_zeta: i32) -> Self { Foo { x: from_zeta } } }
        impl alpha::Factory for Foo { fn new(from_alpha: u64) -> Self { Foo { x: from_alpha as i32 } } }
    });
}

#[test]
fn ctor_new_returning_struct_name() {
    assert_ir_snap!(["Token"] => {
        pub struct Token {
            value: String,
        }
        impl Token {
            pub fn new(value: String) -> Token { Token { value } }
        }
    });
}

#[test]
fn empty_module_is_pruned() {
    assert_ir_snap!(["stays"] => {
        pub mod kept {
            pub fn stays() {}
        }
        pub mod pruned {
            pub fn dropped() {}
        }
    });
}

#[test]
fn deep_nested_modules() {
    assert_ir_snap!(["deep"] => {
        pub mod a {
            pub mod b {
                pub mod c {
                    pub fn deep() {}
                }
            }
        }
    });
}

#[test]
fn re_export_of_struct() {
    assert_ir_snap!(["Inner"] => {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner;
    });
}

#[test]
fn re_export_renamed_struct() {
    assert_ir_snap!(["Inner"] => {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner as Renamed;
    });
}

#[test]
fn re_export_glob_surfaces_all_public_items() {
    assert_ir_snap_all!({
        pub mod inner {
            pub struct S { pub x: i32 }
            pub fn f() -> i32 { 0 }
        }
        pub use inner::*;
    });
}

#[test]
fn re_export_glob_expands_transitively() {
    assert_ir_snap_all!({
        pub mod deeper {
            pub struct X { pub x: i32 }
        }
        pub mod inner {
            pub use super::deeper::X;
        }
        pub use inner::*;
    });
}

#[test]
fn re_export_chain_resolves() {
    assert_ir_snap!(["Inner"] => {
        pub mod inner {
            pub struct Inner { pub x: i32 }
        }
        pub mod middle {
            pub use super::inner::Inner;
        }
        pub use middle::Inner;
    });
}

#[test]
fn re_export_of_primitive_is_skipped() {
    assert_ir_snap_all!({
        pub use i32 as MyI32;
    });
}

#[test]
fn pub_crate_field_forces_ref() {
    assert_ir_snap!(["Gated"] => {
        pub struct Gated {
            pub x: i32,
            pub(crate) y: i32,
        }
    });
}

#[test]
fn duplicate_struct_names_in_different_modules() {
    assert_ir_snap_all!({
        pub mod a {
            pub struct Thing { pub x: i32 }
        }
        pub mod b {
            pub struct Thing { pub y: i32 }
        }
    });
}

#[test]
fn items_not_in_allowed_are_dropped() {
    assert_ir_snap!(["keep"] => {
        pub fn keep() {}
        pub fn drop_me() {}
    });
}

#[test]
fn dependency_struct_is_reached_via_fn_signature() {
    assert_ir_snap!(["f"] => {
        pub struct Point {
            pub x: i32,
        }
        pub fn f(p: Point) {}
    });
}

#[test]
fn empty_allowed_set_produces_empty_root() {
    let src = crate::common::fmt_src(stringify!(
        pub fn a() {}
        pub fn b() {}
    ));
    let crate_ = crate::common::gen_rustdoc(&src);
    let allowed = std::collections::HashSet::new();
    let ir = rust_curse::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap();
    insta::with_settings!({ description => src }, {
        insta::assert_debug_snapshot!(ir);
    });
}
