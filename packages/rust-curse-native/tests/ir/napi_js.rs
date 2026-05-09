#[test]
fn fn_primitives_root() {
    assert_napi_js_snap! {
        pub fn square(x: f64) -> f64 { x * x }
        pub fn square2(x: f64) -> f64 { x * x }
    };
}

#[test]
fn module_nested_fn() {
    assert_napi_js_snap! {
        pub mod math {
            pub fn square(x: f64) -> f64 { x * x }
            pub fn square2(x: f64) -> f64 { x * x }
        }
    };
}

#[test]
fn napi_struct_plain_root() {
    assert_napi_js_snap! {
        pub struct Point {
            pub x: f64,
            pub y: f64,
        }

        pub struct Point2 {
            pub x: f64,
            pub y: f64,
        }
    };
}

#[test]
fn module_cross_import() {
    assert_napi_js_snap! {
        pub mod types {
            pub struct Token {
                pub value: String,
            }
        }
        pub mod ops {
            pub fn parse(s: String) -> crate::types::Token { todo!() }
        }
    };
}

#[test]
fn re_export_renamed() {
    assert_napi_js_snap! {
        pub mod inner {
            pub struct Inner { pub x: i32 }
        }
        pub use inner::Inner as Outer;
    };
}

#[test]
fn re_export_renamed_multiple() {
    assert_napi_js_snap! {
        pub mod inner {
            pub struct Inner { pub x: i32 }
            pub struct Outer { pub y: i32 }
        }
        pub use inner::{Inner as Outer, Outer as Inner};
    };
}

#[test]
fn re_export_glob() {
    assert_napi_js_snap! {
        pub mod inner {
            pub struct S { pub x: i32 }
            pub fn f() -> i32 { 0 }
        }
        pub use inner::*;
    };
}

#[test]
fn deep_nested_modules() {
    assert_napi_js_snap! {
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
fn only_submodule_exports() {
    assert_napi_js_snap! {
        pub mod inner {
            pub fn f() {}
        }
    };
}
