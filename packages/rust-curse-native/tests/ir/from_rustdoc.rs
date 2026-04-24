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
#[ignore = "map support dropped for now"]
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
#[ignore = "tuple support dropped for now"]
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
#[ignore = "enum support dropped for now"]
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
#[ignore = "enum support dropped for now"]
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
fn vec_deque_and_linked_list() {
    assert_ir_snap!(["f"] => {
        pub fn f(
            a: std::collections::VecDeque<i32>,
            b: std::collections::LinkedList<String>,
        ) {}
    });
}

#[test]
#[ignore = "set support dropped for now"]
fn hash_set_and_btree_set() {
    assert_ir_snap!(["f"] => {
        pub fn f(
            a: std::collections::HashSet<i32>,
            b: std::collections::BTreeSet<String>,
        ) {}
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
fn smart_pointer_nested() {
    assert_ir_snap!(["f"] => {
        pub fn f(a: Vec<Box<i32>>, b: Option<std::sync::Arc<String>>) {}
    });
}

#[test]
fn mut_slice() {
    assert_ir_snap!(["f"] => {
        pub fn f(items: &mut [i32]) {}
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
fn recursive_struct_via_box() {
    assert_ir_snap!(["Node"] => {
        pub struct Node {
            next: Option<Box<Node>>,
        }
    });
}

#[test]
fn mutually_recursive_structs() {
    assert_ir_snap!(["A", "B"] => {
        pub struct A {
            b: Option<Box<B>>,
        }
        pub struct B {
            a: Option<Box<A>>,
        }
    });
}

#[test]
fn deeply_nested_generics() {
    assert_ir_snap!(["f"] => {
        pub fn f(x: Vec<Option<Vec<Option<i32>>>>) {}
    });
}

#[test]
fn result_with_unit() {
    assert_ir_snap!(["f", "g"] => {
        pub fn f() -> Result<(), String> { Ok(()) }
        pub fn g(x: i32) -> Result<i32, ()> { Ok(x) }
    });
}

#[test]
#[ignore = "tuple support dropped for now"]
fn tuple_with_unit_element() {
    assert_ir_snap!(["f"] => {
        pub fn f(t: (i32, (), String)) {}
    });
}

#[test]
#[ignore = "tuple support dropped for now"]
fn large_tuple() {
    assert_ir_snap!(["f"] => {
        pub fn f(t: (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32)) {}
    });
}

#[test]
#[ignore = "tuple support dropped for now"]
fn nested_tuple() {
    assert_ir_snap!(["f"] => {
        pub fn f(t: ((i32, i32), String)) {}
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
fn ctor_new_returning_result_is_explicit() {
    assert_ir_snap!(["Loader"] => {
        pub struct Loader {
            data: Vec<u8>,
        }
        impl Loader {
            pub fn new() -> Result<Self, String> { Ok(Self { data: Vec::new() }) }
        }
    });
}

#[test]
fn ctor_new_returning_result_struct_name_is_explicit() {
    assert_ir_snap!(["Token"] => {
        pub struct Token {
            value: String,
        }
        impl Token {
            pub fn new(value: String) -> Result<Token, String> { Ok(Token { value }) }
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
fn ctor_new_in_trait_impl_returning_result_is_explicit() {
    assert_ir_snap!(["Foo"] => {
        pub trait TryFactory {
            fn new() -> Result<Self, String> where Self: Sized;
        }
        pub struct Foo {
            x: i32,
        }
        impl TryFactory for Foo {
            fn new() -> Result<Self, String> { Ok(Foo { x: 0 }) }
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
#[ignore = "enum support dropped for now"]
fn empty_enum() {
    assert_ir_snap!(["Void"] => {
        pub enum Void {}
    });
}

#[test]
#[ignore = "enum support dropped for now"]
fn enum_nominal_payload() {
    assert_ir_snap!(["Msg"] => {
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        pub enum Msg {
            Move(Point),
            Stop,
        }
    });
}

#[test]
#[ignore = "enum support dropped for now"]
fn recursive_enum_via_box() {
    assert_ir_snap!(["Tree"] => {
        pub enum Tree {
            Node(Box<Tree>, Box<Tree>),
            Leaf(i32),
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
    let src = common::fmt_src(stringify!(
        pub fn a() {}
        pub fn b() {}
    ));
    let crate_ = common::gen_rustdoc(&src);
    let allowed = std::collections::HashSet::new();
    let ir = rust_curse::codegen::ir::from_rustdoc::build_ir(&allowed, &crate_).unwrap();
    insta::with_settings!({ description => src }, {
        insta::assert_debug_snapshot!(ir);
    });
}
