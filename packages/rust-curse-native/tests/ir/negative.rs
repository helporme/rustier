//! Error-path tests: inputs that should be rejected by `build_ir`.

use rust_curse::ir::from_rustdoc::BuildError;

#[test]
fn i128_is_unsupported_primitive() {
    assert_ir_err!(
        { pub fn f(x: i128) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn u128_is_unsupported_primitive() {
    assert_ir_err!(
        { pub fn f(x: u128) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn generic_type_parameter_unsupported() {
    assert_ir_err!(
        { pub fn f<T>(x: T) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn dyn_trait_unsupported() {
    assert_ir_err!(
        {
            pub fn f(x: &dyn std::fmt::Debug) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn impl_trait_return_unsupported() {
    assert_ir_err!(
        {
            pub fn f() -> impl std::fmt::Debug { 1i32 }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn function_pointer_unsupported() {
    assert_ir_err!(
        {
            pub fn f(cb: fn(i32) -> i32) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn raw_pointer_unsupported() {
    assert_ir_err!(
        {
            pub fn f(p: *const i32) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn fixed_size_array_param_unsupported() {
    assert_ir_err!(
        {
            pub fn f(x: [i32; 4]) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn fixed_size_array_return_unsupported() {
    assert_ir_err!(
        {
            pub fn f() -> [bool; 2] { todo!() }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn zero_length_array_unsupported() {
    assert_ir_err!(
        {
            pub fn f(x: [i32; 0]) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn array_of_nominal_unsupported() {
    assert_ir_err!(
        {
            pub struct Point { pub x: i32 }
            pub fn f(arr: [Point; 4]) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn const_generic_array_length_unsupported() {
    assert_ir_err!(
        {
            pub fn f<const N: usize>(x: [i32; N]) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn union_type_unsupported() {
    assert_ir_err!(
        {
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
        {
            pub type Foo = i32;
            pub fn f(x: Foo) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn unit_enum_unsupported() {
    assert_ir_err!(
        {
            pub enum E { A, B, C }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn tuple_variant_enum_unsupported() {
    assert_ir_err!(
        {
            pub enum E { A(i32), B(String) }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn struct_variant_enum_unsupported() {
    assert_ir_err!(
        {
            pub enum E { A { x: i32 }, B { y: String } }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn enum_with_discriminants_unsupported() {
    assert_ir_err!(
        {
            pub enum E {
                A = 1,
                B = 2,
            }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn enum_in_fn_signature_unsupported() {
    assert_ir_err!(
        {
            pub enum E { A, B }
            pub fn f(e: E) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn tuple_type_unsupported() {
    assert_ir_err!(
        {
            pub fn f(t: (i32, String)) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn nested_tuple_unsupported() {
    assert_ir_err!(
        {
            pub fn f(t: ((i32, i32), String)) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn hash_map_unsupported() {
    assert_ir_err!(
        {
            pub fn f(m: std::collections::HashMap<String, i32>) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn btree_map_unsupported() {
    assert_ir_err!(
        {
            pub fn f(m: std::collections::BTreeMap<String, i32>) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn hash_set_unsupported() {
    assert_ir_err!(
        {
            pub fn f(s: std::collections::HashSet<i32>) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn btree_set_unsupported() {
    assert_ir_err!(
        {
            pub fn f(s: std::collections::BTreeSet<i32>) {}
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn vec_param_unsupported() {
    assert_ir_err!(
        { pub fn f(v: Vec<i32>) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn vec_return_unsupported() {
    assert_ir_err!(
        { pub fn f() -> Vec<i32> { vec![] } },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn vec_deque_unsupported() {
    assert_ir_err!(
        { pub fn f(v: std::collections::VecDeque<i32>) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn linked_list_unsupported() {
    assert_ir_err!(
        { pub fn f(v: std::collections::LinkedList<i32>) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn slice_param_unsupported() {
    assert_ir_err!(
        { pub fn f(v: &[i32]) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn mut_slice_param_unsupported() {
    assert_ir_err!(
        { pub fn f(v: &mut [i32]) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn slice_return_unsupported() {
    assert_ir_err!(
        { pub fn f() -> &'static [i32] { &[] } },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn option_param_unsupported() {
    assert_ir_err!(
        { pub fn f(x: Option<i32>) {} },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn option_return_unsupported() {
    assert_ir_err!(
        { pub fn f() -> Option<i32> { None } },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn result_return_unsupported() {
    assert_ir_err!(
        { pub fn f() -> Result<i32, String> { Ok(0) } },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn result_with_unit_return_unsupported() {
    assert_ir_err!(
        { pub fn f() -> Result<(), String> { Ok(()) } },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn ctor_new_returning_result_unsupported() {
    assert_ir_err!(
        {
            pub struct S { x: i32 }
            impl S {
                pub fn new() -> Result<Self, String> { Ok(Self { x: 0 }) }
            }
        },
        BuildError::UnsupportedType { .. }
    );
}

#[test]
fn use_from_external_crate_unsupported() {
    assert_ir_err!(
        {
            pub use std::collections::HashMap;
        },
        BuildError::UnsupportedImport { .. }
    );
}

#[test]
fn use_renamed_from_external_crate_unsupported() {
    assert_ir_err!(
        {
            pub use std::collections::HashMap as Map;
        },
        BuildError::UnsupportedImport { .. }
    );
}

#[test]
fn use_glob_from_external_crate_unsupported() {
    assert_ir_err!(
        {
            pub use std::collections::*;
        },
        BuildError::UnsupportedImport { .. }
    );
}

#[test]
fn use_external_function_unsupported() {
    assert_ir_err!(
        {
            pub use std::mem::swap;
        },
        BuildError::UnsupportedImport { .. }
    );
}

#[test]
fn use_external_in_nested_module_unsupported() {
    assert_ir_err!(
        {
            pub mod inner {
                pub use std::collections::HashMap;
            }
        },
        BuildError::UnsupportedImport { .. }
    );
}

