mod common;

// ── functions ────────────────────────────────────────────────────────────────

#[test]
fn fn_primitives() {
    assert_ts_snap! {
        /// Exercises every primitive type that maps to `number` or `string` or `boolean`.
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
    assert_ts_snap! {
        /// 64-bit and pointer-sized integers map to `bigint`.
        pub fn f(a: i64, b: u64, c: isize, d: usize) {}
    };
}

#[test]
fn fn_borrow_modes_primitive() {
    assert_ts_snap! {
        /// Borrow mode does not affect the TypeScript type for primitives.
        pub fn f(by_ref: &i32, by_mut_ref: &mut i32, by_value: i32) {}
    };
}

#[test]
fn fn_borrow_modes_struct() {
    assert_ts_snap! {
        /// By-value nominal types are wrapped in `Copy<>`, borrows are not.
        pub struct Point {
            /// X coordinate.
            pub x: f64,
            /// Y coordinate.
            pub y: f64,
        }
        pub fn translate(origin: &Point, target: &mut Point, offset: Point) {}
    };
}

#[test]
fn fn_option() {
    assert_ts_snap! {
        /// `Option<T>` maps to `T | undefined`.
        pub fn f(a: Option<i32>) -> Option<String> { None }
    };
}

#[test]
fn fn_vec() {
    assert_ts_snap! {
        /// `Vec<T>` maps to `T[]`.
        pub fn f(items: Vec<i32>) -> Vec<String> { vec![] }
    };
}

#[test]
fn fn_vec_of_option() {
    assert_ts_snap! {
        /// `Vec<Option<T>>` must parenthesise the union: `(T | undefined)[]`.
        pub fn f(items: Vec<Option<i32>>) -> Vec<Option<String>> { vec![] }
    };
}

#[test]
fn fn_option_of_vec() {
    assert_ts_snap! {
        /// `Option<Vec<T>>` does not need parens: `T[] | undefined`.
        pub fn f(items: Option<Vec<i32>>) -> Option<Vec<String>> { None }
    };
}

#[test]
fn fn_result() {
    assert_ts_snap! {
        /// `Result<T, E>` maps to the Ok type; the error branch is erased.
        pub fn f(x: i32) -> Result<String, i32> { Ok(String::new()) }
    };
}

#[test]
fn fn_map() {
    assert_ts_snap! {
        /// All maps become `Map<K, V>` regardless of key type.
        pub fn f(
            a: std::collections::HashMap<String, i32>,
            b: std::collections::BTreeMap<i32, bool>,
            c: std::collections::HashMap<i64, String>,
        ) {}
    };
}

#[test]
fn fn_array_and_slice() {
    assert_ts_snap! {
        /// Fixed-size arrays and slices both map to `T[]`.
        pub fn f(arr: [i32; 4], slice: &[String]) -> [bool; 2] { todo!() }
    };
}

#[test]
fn fn_tuple() {
    assert_ts_snap! {
        /// Tuples map to TypeScript tuple types.
        pub fn f(pair: (i32, String)) -> (bool, f64, String) { todo!() }
    };
}

#[test]
fn fn_return_void() {
    assert_ts_snap! {
        /// Unit return type maps to `void`.
        pub fn f() {}
    };
}

// ── structs ──────────────────────────────────────────────────────────────────

#[test]
fn struct_plain() {
    assert_ts_snap! {
        /// A point in 2D space. All fields are public, so it is copy-constructible.
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
    assert_ts_snap! {
        /// A handle with a hidden internal state and no explicit constructor.
        pub struct Handle {
            id: u32,
        }
    };
}

#[test]
fn struct_opaque_ctor() {
    assert_ts_snap! {
        /// A counter whose constructor is the canonical entry point.
        pub struct Counter {
            count: u32,
        }
        impl Counter {
            /// Creates a new counter starting at `start`.
            pub fn new(start: u32) -> Self { Self { count: start } }
        }
    };
}

#[test]
fn struct_tuple() {
    assert_ts_snap! {
        /// A pair of values stored as a tuple struct.
        pub struct Pair(pub i32, pub String);
    };
}

// ── enums ────────────────────────────────────────────────────────────────────

#[test]
fn enum_unit() {
    assert_ts_snap! {
        /// Cardinal compass directions — all unit variants become a string union.
        pub enum Direction {
            /// Toward the top.
            North,
            /// Toward the bottom.
            South,
            /// Toward the right.
            East,
            /// Toward the left.
            West,
        }
    };
}

#[test]
fn enum_complex() {
    assert_ts_snap! {
        /// A geometric shape with mixed variant kinds.
        pub enum Shape {
            /// A circle defined by its radius.
            Circle { radius: f64 },
            /// An axis-aligned rectangle.
            Rect { width: f64, height: f64 },
            /// A dimensionless point.
            Point,
        }
    };
}

// ── modules ──────────────────────────────────────────────────────────────────

#[test]
fn module_nested() {
    assert_ts_snap! {
        pub mod math {
            /// Returns the square of `x`.
            pub fn square(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn module_cross_import() {
    assert_ts_snap! {
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
fn empty_enum() {
    assert_ts_snap! {
        pub enum Void {}
    };
}

#[test]
fn unit_struct() {
    assert_ts_snap! {
        pub struct Marker;
    };
}

#[test]
fn empty_struct_plain() {
    assert_ts_snap! {
        pub struct Empty {}
    };
}

#[test]
fn fn_large_tuple() {
    assert_ts_snap! {
        pub fn f(t: (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32)) {}
    };
}

#[test]
fn fn_zero_length_array() {
    assert_ts_snap! {
        pub fn f(a: [i32; 0]) {}
    };
}

#[test]
fn fn_smart_pointer_inlining() {
    assert_ts_snap! {
        /// Smart pointers are erased, so every parameter maps to `number`.
        pub fn f(
            a: Box<i32>,
            b: std::sync::Arc<i32>,
            c: std::rc::Rc<i32>,
            d: std::cell::Cell<i32>,
            e: std::cell::RefCell<i32>,
        ) {}
    };
}

#[test]
fn fn_smart_pointer_nested() {
    assert_ts_snap! {
        pub fn f(a: Vec<Box<i32>>, b: Option<std::sync::Arc<String>>) {}
    };
}

#[test]
fn fn_hash_set_and_btree_set() {
    assert_ts_snap! {
        /// Both set flavours map to `T[]`.
        pub fn f(
            a: std::collections::HashSet<i32>,
            b: std::collections::BTreeSet<String>,
        ) {}
    };
}

#[test]
fn fn_vec_deque_and_linked_list() {
    assert_ts_snap! {
        pub fn f(
            a: std::collections::VecDeque<i32>,
            b: std::collections::LinkedList<String>,
        ) {}
    };
}

#[test]
fn fn_mut_slice() {
    assert_ts_snap! {
        /// Mutability of the borrow does not change the TypeScript type.
        pub fn f(items: &mut [i32]) {}
    };
}

#[test]
fn fn_deeply_nested_generics() {
    assert_ts_snap! {
        pub fn f(x: Vec<std::collections::HashMap<String, Vec<i32>>>) {}
    };
}

#[test]
fn fn_nested_tuple() {
    assert_ts_snap! {
        pub fn f(t: ((i32, i32), String)) {}
    };
}

#[test]
fn fn_tuple_with_unit_element() {
    assert_ts_snap! {
        /// A `()` element inside a tuple prints as `void`.
        pub fn f(t: (i32, (), String)) {}
    };
}

#[test]
fn fn_array_of_nominal() {
    assert_ts_snap! {
        pub struct Point {
            pub x: i32,
        }
        /// Nominal types inside arrays are not wrapped in `Copy<>`.
        pub fn f(arr: [Point; 4]) {}
    };
}

#[test]
fn fn_result_with_unit() {
    assert_ts_snap! {
        /// `Result<(), _>` returns `void`; `Result<T, ()>` returns `T`.
        pub fn f() -> Result<(), String> { Ok(()) }
        pub fn g(x: i32) -> Result<i32, ()> { Ok(x) }
    };
}

#[test]
fn struct_pub_crate_field_forces_ref() {
    assert_ts_snap! {
        /// A `pub(crate)` field downgrades the boundary to ref (opaque class).
        pub struct Gated {
            pub x: i32,
            pub(crate) y: i32,
        }
    };
}

#[test]
fn struct_mixed_pub_priv_fields() {
    assert_ts_snap! {
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    };
}

#[test]
fn struct_recursive_via_box() {
    assert_ts_snap! {
        /// Self-referencing class must not emit a self-import.
        pub struct Node {
            next: Option<Box<Node>>,
        }
    };
}

#[test]
fn struct_mutually_recursive() {
    assert_ts_snap! {
        /// Both classes live in the same module, so neither should be imported.
        pub struct A {
            b: Option<Box<B>>,
        }
        pub struct B {
            a: Option<Box<A>>,
        }
    };
}

#[test]
fn enum_recursive_via_box() {
    assert_ts_snap! {
        pub enum Tree {
            Node(Box<Tree>, Box<Tree>),
            Leaf(i32),
        }
    };
}

#[test]
fn enum_nominal_payload() {
    assert_ts_snap! {
        pub struct Point {
            pub x: i32,
            pub y: i32,
        }
        /// Mixes a tuple-with-nominal variant and a unit variant.
        pub enum Msg {
            Move(Point),
            Stop,
        }
    };
}

#[test]
fn module_deep_nested() {
    assert_ts_snap! {
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
fn module_duplicate_struct_names() {
    assert_ts_snap! {
        pub mod a {
            pub struct Thing { pub x: i32 }
        }
        pub mod b {
            pub struct Thing { pub y: i32 }
        }
    };
}

#[test]
fn module_re_export_is_dropped() {
    assert_ts_snap! {
        pub mod inner {
            pub struct Inner {
                pub x: i32,
            }
        }
        pub use inner::Inner;
    };
}

#[test]
fn ctor_new_returning_struct_name() {
    assert_ts_snap! {
        /// `new() -> Token` is recognised as an explicit constructor.
        pub struct Token {
            value: String,
        }
        impl Token {
            pub fn new(value: String) -> Token { Token { value } }
        }
    };
}

#[test]
fn ctor_new_returning_result_is_implicit() {
    assert_ts_snap! {
        /// `new() -> Result<Self, _>` is not treated as an explicit ctor.
        pub struct Loader {
            data: Vec<u8>,
        }
        impl Loader {
            pub fn new() -> Result<Self, String> { Ok(Self { data: Vec::new() }) }
        }
    };
}

#[test]
fn ctor_new_with_self_arg_is_implicit() {
    assert_ts_snap! {
        /// `new(&self)` is not treated as an explicit ctor.
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
    assert_ts_snap! {
        pub struct Counter {
            count: u32,
        }
        impl Counter {
            pub fn increment(&mut self) { self.count += 1; }
        }
        impl Counter {
            pub fn new() -> Self { Counter { count: 0 } }
        }
    };
}

#[test]
fn ctor_new_in_trait_impl_is_ignored() {
    assert_ts_snap! {
        pub trait Factory {
            fn new() -> Self;
        }
        pub struct Foo {
            x: i32,
        }
        impl Factory for Foo {
            fn new() -> Self { Foo { x: 0 } }
        }
    };
}
