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
fn option_param_and_return() {
    assert_ir_snap!(["f"] => {
        pub fn f(a: Option<i32>) -> Option<String> { None }
    });
}

#[test]
fn vec_param_and_return() {
    assert_ir_snap!(["f"] => {
        pub fn f(items: Vec<i32>) -> Vec<String> { vec![] }
    });
}

#[test]
fn vec_and_option_map_to_ir() {
    assert_ir_snap!(["f"] => {
        pub fn f(v: Vec<Option<i32>>) { let _ = v; }
    });
}

#[test]
fn vec_of_option() {
    assert_ir_snap!(["f"] => {
        pub fn f(items: Vec<Option<i32>>) -> Vec<Option<String>> { vec![] }
    });
}

#[test]
fn option_of_vec() {
    assert_ir_snap!(["f"] => {
        pub fn f(items: Option<Vec<i32>>) -> Option<Vec<String>> { None }
    });
}

#[test]
fn result_type() {
    assert_ir_snap!(["f"] => {
        pub fn f(x: i32) -> Result<String, i32> { Ok(String::new()) }
    });
}

#[test]
fn map_types() {
    assert_ir_snap!(["f"] => {
        pub fn f(
            a: std::collections::HashMap<String, i32>,
            b: std::collections::BTreeMap<i32, bool>,
            c: std::collections::HashMap<i64, String>,
        ) {}
    });
}

#[test]
fn array_and_slice() {
    assert_ir_snap!(["f"] => {
        pub fn f(arr: [i32; 4], slice: &[String]) -> [bool; 2] { todo!() }
    });
}

#[test]
fn tuple_types() {
    assert_ir_snap!(["f"] => {
        pub fn f(pair: (i32, String)) -> (bool, f64, String) { todo!() }
    });
}

#[test]
fn return_void() {
    assert_ir_snap!(["f"] => {
        pub fn f() {}
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
fn unit_enum() {
    assert_ir_snap!(["Direction"] => {
        pub enum Direction {
            North,
            South,
            East,
            West,
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
