mod common;

use common::rustdoc_from_source::allowed_by_name;
use rust_curse::codegen::ir::from_rustdoc::build_ir;

#[test]
fn builds_root_only_with_one_fn() {
    let crate_ = rustdoc! {
        pub fn add(a: i32, b: i32) -> i32 { a + b }
    };
    let allowed = allowed_by_name(&crate_, &["add"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn interns_primitives_once() {
    let crate_ = rustdoc! {
        pub fn two_i32(a: i32, b: i32) {}
    };
    let allowed = allowed_by_name(&crate_, &["two_i32"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn plain_struct_all_pub_is_copy() {
    let crate_ = rustdoc! {
        pub struct Point {
            pub x: i32,
        }
    };
    let allowed = allowed_by_name(&crate_, &["Point"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn plain_struct_private_field_is_ref_implicit() {
    let crate_ = rustdoc! {
        pub struct Opaque {
            x: i32,
        }
    };
    let allowed = allowed_by_name(&crate_, &["Opaque"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn explicit_new_is_ctor_explicit() {
    let crate_ = rustdoc! {
        pub struct Opaque {
            x: i32,
        }
        impl Opaque {
            pub fn new(x: i32) -> Self { Self { x } }
        }
    };
    let allowed = allowed_by_name(&crate_, &["Opaque"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn enum_variants_shapes() {
    let crate_ = rustdoc! {
        pub enum E {
            A,
            B(i32),
            C { x: i32 },
        }
    };
    let allowed = allowed_by_name(&crate_, &["E"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn vec_and_option_map_to_ir() {
    let crate_ = rustdoc! {
        pub fn f(v: Vec<Option<i32>>) { let _ = v; }
    };
    let allowed = allowed_by_name(&crate_, &["f"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn nested_module_tree() {
    let crate_ = rustdoc! {
        pub mod child {
            pub fn leaf() {}
        }
        pub mod empty {}
    };
    let allowed = allowed_by_name(&crate_, &["leaf"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}

#[test]
fn borrow_mode_captured() {
    let crate_ = rustdoc! {
        pub fn takes(x: &mut i32) { *x += 1; }
    };
    let allowed = allowed_by_name(&crate_, &["takes"]);
    let ir = build_ir(&allowed, &crate_).unwrap();
    insta::assert_debug_snapshot!(ir);
}
