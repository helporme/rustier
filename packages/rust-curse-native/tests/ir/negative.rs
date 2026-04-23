//! Error-path tests: inputs that should be rejected by `build_ir`.

mod common;

use rust_curse::codegen::ir::from_rustdoc::BuildError;

#[test]
fn i128_is_unsupported_primitive() {
    assert_ir_err!(
        ["f"] => { pub fn f(x: i128) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn u128_is_unsupported_primitive() {
    assert_ir_err!(
        ["f"] => { pub fn f(x: u128) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn generic_type_parameter_unsupported() {
    assert_ir_err!(
        ["f"] => { pub fn f<T>(x: T) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn dyn_trait_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub fn f(x: &dyn std::fmt::Debug) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn impl_trait_return_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub fn f() -> impl std::fmt::Debug { 1i32 }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn function_pointer_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub fn f(cb: fn(i32) -> i32) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn raw_pointer_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub fn f(p: *const i32) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn const_generic_array_length_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub fn f<const N: usize>(x: [i32; N]) {}
        },
        BuildError::BadArrayLen(_)
    );
}

#[test]
fn union_type_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub union U {
                pub x: u32,
                pub y: u32,
            }
            pub fn f(u: U) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn type_alias_unsupported() {
    assert_ir_err!(
        ["f"] => {
            pub type Foo = i32;
            pub fn f(x: Foo) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn enum_c_style_discriminants_unsupported() {
    assert_ir_err!(
        ["E"] => {
            pub enum E {
                A = 1,
                B = 2,
                C = 4,
            }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn enum_single_discriminant_unsupported() {
    assert_ir_err!(
        ["E"] => {
            pub enum E {
                Only = 7,
            }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn enum_mixed_plain_and_discriminant_unsupported() {
    assert_ir_err!(
        ["E"] => {
            pub enum E {
                Plain,
                Tagged = 3,
            }
        },
        BuildError::UnsupportedType { .. }
    );
}
