mod common;

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
fn enum_variants_shapes() {
    assert_ir_snap!(["E"] => {
        pub enum E {
            A,
            B(i32),
            C { x: i32 },
        }
    });
}

#[test]
fn vec_and_option_map_to_ir() {
    assert_ir_snap!(["f"] => {
        pub fn f(v: Vec<Option<i32>>) { let _ = v; }
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
fn borrow_mode_captured() {
    assert_ir_snap!(["takes"] => {
        pub fn takes(x: &mut i32) { *x += 1; }
    });
}
