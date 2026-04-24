mod common;

#[test]
fn fn_primitives() {
    assert_napi_snap! {
        /// Every primitive type passes straight through to a napi-native type.
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
    assert_napi_snap! {
        /// 64-bit and pointer-sized integers pass through unchanged.
        pub fn f(a: i64, b: u64, c: isize, d: usize) {}
    };
}

#[test]
fn fn_borrow_modes_primitive() {
    assert_napi_snap! {
        /// Borrow mode for primitives passes through without wrapping.
        pub fn f(by_ref: &i32, by_mut_ref: &mut i32, by_value: i32) {}
    };
}

#[test]
fn fn_borrow_modes_struct() {
    assert_napi_snap! {
        /// `&T` / `&mut T` take handle refs, owned `T` takes the `__Copy` wrapper.
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
    assert_napi_snap! {
        /// `Option<T>` passes through unchanged.
        pub fn f(a: Option<i32>) -> Option<String> { None }
    };
}

#[test]
fn fn_vec() {
    assert_napi_snap! {
        /// `Vec<T>` passes through unchanged.
        pub fn f(items: Vec<i32>) -> Vec<String> { vec![] }
    };
}

#[test]
fn fn_result() {
    assert_napi_snap! {
        /// `Result<T, E>` at return position becomes `napi::Result<T>`.
        pub fn f(x: i32) -> Result<String, i32> { Ok(String::new()) }
    };
}

#[test]
fn fn_return_void() {
    assert_napi_snap! {
        /// Unit return type emits no return clause on the glue fn.
        pub fn f() {}
    };
}

#[test]
fn struct_plain() {
    assert_napi_snap! {
        /// All-public-field struct with no by-value use gets only the `__Ref` wrapper.
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
    assert_napi_snap! {
        /// A handle with a hidden internal state — no `__Copy` wrapper emitted.
        pub struct Handle {
            id: u32,
        }
    };
}

#[test]
fn struct_opaque_ctor() {
    assert_napi_snap! {
        /// Explicit ctor delegates to `Counter::new`.
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
fn ctor_new_returning_result() {
    assert_napi_snap! {
        /// `new() -> Result<Self, _>` lifts to `napi::Result<Self>` on the ctor.
        pub struct Loader {
            data: Vec<u8>,
        }
        impl Loader {
            pub fn new(capacity: usize) -> Result<Self, String> {
                Ok(Self { data: Vec::with_capacity(capacity) })
            }
        }
    };
}

#[test]
fn unit_struct() {
    assert_napi_snap! {
        pub struct Marker;
    };
}

#[test]
fn struct_mixed_pub_priv_fields() {
    assert_napi_snap! {
        pub struct Mixed {
            pub x: i32,
            y: i32,
        }
    };
}

#[test]
fn module_nested() {
    assert_napi_snap! {
        pub mod math {
            /// Returns the square of `x`.
            pub fn square(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn module_cross_import() {
    assert_napi_snap! {
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
fn module_deep_nested() {
    assert_napi_snap! {
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
fn fn_slice() {
    assert_napi_snap! {
        /// Slices appear as `Vec<T>` on the glue side.
        pub fn f(slice: &[String]) {}
    };
}

#[test]
fn struct_recursive_via_box() {
    assert_napi_snap! {
        /// Self-referencing struct must not self-import.
        pub struct Node {
            pub next: Option<Box<Node>>,
        }
    };
}
