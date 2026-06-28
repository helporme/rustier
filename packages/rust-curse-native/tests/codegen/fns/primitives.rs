//! Tests for simple function generation with all primitive types as arguments
//! and return values.

// todo: convert them to unit tests and put near the code

use crate::utils::*;

#[test]
fn fn_with_all_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: bool,
            c: &str,
            d: i8,
            e: i16,
            f: i32,
            g: u8,
            h: u16,
            i: u32,
            k: f64,
        ) { }
    }
}
#[test]
fn fn_with_all_ref_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: &bool,
            c: &str,
            d: &i8,
            e: &i16,
            f: &i32,
            g: &u8,
            h: &u16,
            i: &u32,
            k: &f64,
        ) { }
    }
}

#[test]
fn fn_with_all_mut_ref_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: &mut bool,
            c: &mut str,
            d: &mut i8,
            e: &mut i16,
            f: &mut i32,
            g: &mut u8,
            h: &mut u16,
            i: &mut u32,
            k: &mut f64,
        ) { }
    }
}

#[test]
fn fn_returning_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() -> bool {
            true
        }
    }
}

#[test]
fn fn_no_args_no_return() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() { }
    }
}
