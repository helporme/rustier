//! Tests for simple function generation with all primitive types as arguments
//! and return values.

use crate::utils::*;

#[test]
pub fn fn_with_all_primitives() {
    assert_snap! {
        @gen: ir, ts;

        pub fn foo(
            a: bool,
            b: char,
            c: str,
            d: i8,
            e: i16,
            f: i32,
            g: u8,
            h: u16,
            i: u32,
            j: f32,
            k: f64,
        ) { }
    }
}

#[test]
pub fn fn_returning_primitives() {
    assert_snap! {
        @gen: ir, ts;

        pub fn foo() -> bool {
            true
        }
    }
}

#[test]
pub fn fn_no_args_no_return() {
    assert_snap! {
        @gen: ir, ts;

        pub fn foo() { }
    }
}
