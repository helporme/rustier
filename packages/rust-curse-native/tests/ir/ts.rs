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
