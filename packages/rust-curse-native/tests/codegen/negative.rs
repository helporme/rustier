//! Soft-rejection tests: inputs that produce warnings (not errors) and
//! skip the offending item from the IR.

const UNSUPPORTED_TYPE: &str = "rust-curse::unsupported_type";
const UNSUPPORTED_IMPORT: &str = "rust-curse::unsupported_import";

#[test]
fn i128_is_unsupported_primitive() {
    assert_warns!(
        {
            pub fn f(x: i128) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn u128_is_unsupported_primitive() {
    assert_warns!(
        {
            pub fn f(x: u128) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn generic_type_parameter_unsupported() {
    assert_warns!(
        {
            pub fn f<T>(x: T) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn dyn_trait_unsupported() {
    assert_warns!(
        {
            pub fn f(x: &dyn std::fmt::Debug) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn impl_trait_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> impl std::fmt::Debug {
                1i32
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn function_pointer_unsupported() {
    assert_warns!(
        {
            pub fn f(cb: fn(i32) -> i32) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn raw_pointer_unsupported() {
    assert_warns!(
        {
            pub fn f(p: *const i32) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn fixed_size_array_param_unsupported() {
    assert_warns!(
        {
            pub fn f(x: [i32; 4]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn fixed_size_array_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> [bool; 2] {
                todo!()
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn zero_length_array_unsupported() {
    assert_warns!(
        {
            pub fn f(x: [i32; 0]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn array_of_nominal_unsupported() {
    assert_warns!(
        {
            pub struct Point {
                pub x: i32,
            }
            pub fn f(arr: [Point; 4]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn const_generic_array_length_unsupported() {
    assert_warns!(
        {
            pub fn f<const N: usize>(x: [i32; N]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn union_type_unsupported() {
    assert_warns!(
        {
            pub union U {
                pub x: u32,
                pub y: u32,
            }
            pub fn f(u: U) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn type_alias_unsupported() {
    assert_warns!(
        {
            pub type Foo = i32;
            pub fn f(x: Foo) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn unit_enum_unsupported() {
    // All-unit (C-like) enums are deferred to a follow-up RFC.
    assert_ir_warns!(
        {
            pub enum E {
                A,
                B,
                C,
            }
        },
        "rust-curse::enum_all_unit_unsupported"
    );
}

#[test]
fn tuple_variant_enum_unsupported() {
    assert_ir_warns!(
        {
            pub enum E {
                A(i32),
                B(String),
            }
        },
        "rust-curse::enum_tuple_variant_unsupported"
    );
}

// A struct-variant enum is now a supported data enum (see the positive
// `enum_data_*` codegen snapshots), so there is no longer an
// "unsupported struct-variant enum" negative case.

#[test]
fn enum_with_discriminants_unsupported() {
    // Explicit discriminants imply a C-like all-unit enum -> still deferred.
    assert_ir_warns!(
        {
            pub enum E {
                A = 1,
                B = 2,
            }
        },
        "rust-curse::enum_all_unit_unsupported"
    );
}

#[test]
fn enum_in_fn_signature_skips_when_enum_unsupported() {
    // The enum is all-unit (unsupported), so a fn referencing it is dropped
    // with the enum-targeted cascade warning.
    assert_ir_warns!(
        {
            pub enum E {
                A,
                B,
            }
            pub fn f(e: E) {}
        },
        "rust-curse::skipped_enum_ref"
    );
}

#[test]
fn enum_shared_field_type_mismatch_unsupported() {
    assert_ir_warns!(
        {
            pub enum E {
                A { data: f64 },
                B { data: String },
            }
        },
        "rust-curse::enum_shared_field_type_mismatch"
    );
}

#[test]
fn tuple_type_unsupported() {
    assert_warns!(
        {
            pub fn f(t: (i32, String)) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn nested_tuple_unsupported() {
    assert_warns!(
        {
            pub fn f(t: ((i32, i32), String)) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn hash_map_unsupported() {
    assert_warns!(
        {
            pub fn f(m: std::collections::HashMap<String, i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn btree_map_unsupported() {
    assert_warns!(
        {
            pub fn f(m: std::collections::BTreeMap<String, i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn hash_set_unsupported() {
    assert_warns!(
        {
            pub fn f(s: std::collections::HashSet<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn btree_set_unsupported() {
    assert_warns!(
        {
            pub fn f(s: std::collections::BTreeSet<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn vec_param_unsupported() {
    assert_warns!(
        {
            pub fn f(v: Vec<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn vec_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> Vec<i32> {
                vec![]
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn vec_deque_unsupported() {
    assert_warns!(
        {
            pub fn f(v: std::collections::VecDeque<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn linked_list_unsupported() {
    assert_warns!(
        {
            pub fn f(v: std::collections::LinkedList<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn slice_param_unsupported() {
    assert_warns!(
        {
            pub fn f(v: &[i32]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn mut_slice_param_unsupported() {
    assert_warns!(
        {
            pub fn f(v: &mut [i32]) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn slice_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> &'static [i32] {
                &[]
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn option_param_unsupported() {
    assert_warns!(
        {
            pub fn f(x: Option<i32>) {}
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn option_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> Option<i32> {
                None
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn result_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> Result<i32, String> {
                Ok(0)
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn result_with_unit_return_unsupported() {
    assert_warns!(
        {
            pub fn f() -> Result<(), String> {
                Ok(())
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn ctor_new_returning_result_falls_back_to_implicit() {
    assert_warns!(
        {
            pub struct S {
                x: i32,
            }
            impl S {
                pub fn new() -> Result<Self, String> {
                    Ok(Self { x: 0 })
                }
            }
        },
        UNSUPPORTED_TYPE
    );
}

#[test]
fn use_from_external_crate_unsupported() {
    assert_warns!(
        {
            pub use std::collections::HashMap;
        },
        UNSUPPORTED_IMPORT
    );
}

#[test]
fn use_renamed_from_external_crate_unsupported() {
    assert_warns!(
        {
            pub use std::collections::HashMap as Map;
        },
        UNSUPPORTED_IMPORT
    );
}

#[test]
fn use_glob_from_external_crate_unsupported() {
    assert_warns!(
        {
            pub use std::collections::*;
        },
        UNSUPPORTED_IMPORT
    );
}

#[test]
fn use_external_function_unsupported() {
    assert_warns!(
        {
            pub use std::mem::swap;
        },
        UNSUPPORTED_IMPORT
    );
}

#[test]
fn use_external_in_nested_module_unsupported() {
    assert_warns!(
        {
            pub mod inner {
                pub use std::collections::HashMap;
            }
        },
        UNSUPPORTED_IMPORT
    );
}
